use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct IdentityVault {
    encryption_key: [u8; 32],
    index_salt: [u8; 32],
}

impl IdentityVault {
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

    pub fn generate_blind_index(&self, raw_data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.index_salt);
        hasher.update(raw_data.trim().to_uppercase().as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 12-bytes

        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, blob: &[u8]) -> Result<String, String> {
        let plaintext = self.decrypt_bytes(blob)?;
        String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e))
    }

    pub fn encrypt_bytes(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 12-bytes

        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt_bytes(&self, blob: &[u8]) -> Result<Vec<u8>, String> {
        let Some((nonce, ciphertext)) = blob.split_first_chunk::<12>() else {
            return Err("Invalid ciphertext blob length".into());
        };
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        cipher
            .decrypt(&Nonce::from(*nonce), ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))
    }
}
