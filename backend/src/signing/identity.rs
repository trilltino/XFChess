//! Identity vault for encrypted KYC/PII data storage.
//!
//! This module provides AES-256-GCM encryption for sensitive user identity data
//! (full name, DOB, address, tax ID) with blind index support for searchable
//! encrypted fields without revealing plaintext.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use regex::Regex;

/// Country-specific validation rules for identity fields
#[derive(Debug, Clone)]
pub struct CountryValidationRules {
    /// Whether full name is required
    pub requires_full_name: bool,
    /// Whether date of birth is required
    pub requires_dob: bool,
    /// Whether address is required
    pub requires_address: bool,
    /// Tax ID format pattern (regex)
    pub tax_id_pattern: Option<Regex>,
    /// Tax ID field name
    pub tax_id_field_name: &'static str,
}

impl CountryValidationRules {
    /// Get validation rules for a specific country code
    pub fn for_country(country_code: &str) -> Self {
        match country_code {
            "GB" => CountryValidationRules {
                requires_full_name: true,
                requires_dob: true,
                requires_address: true,
                tax_id_pattern: Some(Regex::new(r"^[A-Z]{2}\d{6}[A-Z]$").unwrap()),
                tax_id_field_name: "National Insurance Number (NI)",
            },
            "BR" => CountryValidationRules {
                requires_full_name: true,
                requires_dob: true,
                requires_address: true,
                tax_id_pattern: Some(Regex::new(r"^\d{11}$").unwrap()),
                tax_id_field_name: "CPF (Cadastro de Pessoas Físicas)",
            },
            "CA" => CountryValidationRules {
                requires_full_name: true,
                requires_dob: true,
                requires_address: true,
                tax_id_pattern: Some(Regex::new(r"^\d{9}$").unwrap()),
                tax_id_field_name: "Social Insurance Number (SIN)",
            },
            "DE" => CountryValidationRules {
                requires_full_name: true,
                requires_dob: true,
                requires_address: true,
                tax_id_pattern: Some(Regex::new(r"^\d{11}$").unwrap()),
                tax_id_field_name: "Tax ID (Steueridentifikationsnummer)",
            },
            "US" => CountryValidationRules {
                requires_full_name: false,
                requires_dob: false,
                requires_address: false,
                tax_id_pattern: Some(Regex::new(r"^\d{9}$").unwrap()),
                tax_id_field_name: "Social Security Number (SSN) - Optional",
            },
            _ => CountryValidationRules {
                requires_full_name: false,
                requires_dob: false,
                requires_address: false,
                tax_id_pattern: None,
                tax_id_field_name: "Tax ID (Optional)",
            },
        }
    }

    /// Validate a field value according to country rules
    pub fn validate_field(&self, field_name: &str, value: &str) -> Result<(), String> {
        match field_name {
            "full_name" if self.requires_full_name && value.is_empty() => {
                Err("Full name is required for this country".to_string())
            }
            "dob" if self.requires_dob && value.is_empty() => {
                Err("Date of birth is required for this country".to_string())
            }
            "address" if self.requires_address && value.is_empty() => {
                Err("Address is required for this country".to_string())
            }
            "tax_id" => {
                if let Some(ref pattern) = self.tax_id_pattern {
                    if !value.is_empty() && !pattern.is_match(value) {
                        Err(format!("Invalid format for {}. Expected format: {}", 
                            self.tax_id_field_name, pattern.as_str()))
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}

/// Identity vault for encrypting and decrypting sensitive user data.
///
/// Uses AES-256-GCM for authenticated encryption and SHA-256 with salt
/// for generating blind indexes (searchable hashes).
#[derive(Clone, Default)]
pub struct IdentityVault {
    encryption_key: [u8; 32],
    index_salt: [u8; 32],
}

impl IdentityVault {
    /// Creates a new IdentityVault from hex-encoded key and salt.
    ///
    /// # Arguments
    /// * `key_hex` - 64-character hex string (32 bytes) for AES-256 key
    /// * `salt_hex` - 64-character hex string (32 bytes) for blind index salt
    ///
    /// # Returns
    /// A new IdentityVault instance, or an error if inputs are invalid
    pub fn new(key_hex: &str, salt_hex: &str) -> Result<Self, String> {
        let key_bytes = hex::decode(key_hex).map_err(|e| format!("Invalid key hex: {}", e))?;
        let salt_bytes = hex::decode(salt_hex).map_err(|e| format!("Invalid salt hex: {}", e))?;
        
        let mut encryption_key = [0u8; 32];
        let mut index_salt = [0u8; 32];
        
        if key_bytes.len() != 32 || salt_bytes.len() != 32 {
            return Err("Encryption key and salt must be exactly 32 bytes (64 hex chars)".into());
        }
        
        encryption_key.copy_from_slice(&key_bytes);
        index_salt.copy_from_slice(&salt_bytes);
        
        Ok(Self {
            encryption_key,
            index_salt,
        })
    }
    
    /// Generates a Blind Index (salted hash) for searching without decryption.
    ///
    /// The blind index allows searching by tax ID or other identifiers
    /// without storing or revealing the plaintext data.
    ///
    /// # Arguments
    /// * `raw_data` - The plaintext data to hash (e.g., tax ID)
    ///
    /// # Returns
    /// A hex-encoded SHA-256 hash of the salted data
    pub fn generate_blind_index(&self, raw_data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.index_salt);
        hasher.update(raw_data.trim().to_uppercase().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Encrypts sensitive data using AES-256-GCM.
    ///
    /// Returns a BLOB containing [12-byte Nonce][Ciphertext].
    /// The nonce is randomly generated for each encryption.
    ///
    /// # Arguments
    /// * `plaintext` - The plaintext string to encrypt
    ///
    /// # Returns
    /// A vector containing nonce + ciphertext, or an error on failure
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 12-bytes
        
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;
            
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    /// Decrypts a [Nonce][Ciphertext] BLOB.
    ///
    /// # Arguments
    /// * `blob` - The encrypted blob (nonce + ciphertext)
    ///
    /// # Returns
    /// The decrypted plaintext string, or an error on failure
    pub fn decrypt(&self, blob: &[u8]) -> Result<String, String> {
        if blob.len() < 12 {
            return Err("Invalid ciphertext blob length".into());
        }
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        let nonce_arr: [u8; 12] = nonce_bytes.try_into().unwrap();
        let nonce = Nonce::from(nonce_arr);
        
        let plaintext = cipher.decrypt(&nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;
            
        String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e))
    }

    /// Validates identity fields based on country-specific rules.
    ///
    /// # Arguments
    /// * `country_code` - ISO 3166-1 alpha-2 country code
    /// * `fields` - HashMap of field names to values (full_name, dob, address, tax_id)
    ///
    /// # Returns
    /// Ok(()) if all fields pass validation, or an error message if validation fails
    pub fn validate_fields(country_code: &str, fields: &std::collections::HashMap<&str, &str>) -> Result<(), String> {
        let rules = CountryValidationRules::for_country(country_code);
        
        for (field_name, value) in fields {
            rules.validate_field(field_name, value)?;
        }
        
        Ok(())
    }
}
