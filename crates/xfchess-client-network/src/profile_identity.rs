//! Private application identity for a local profile. This is never a wallet key.
use ed25519_dalek::{Signer, SigningKey};
use std::{io::Write, path::Path};

#[derive(Clone)]
pub struct ProfileIdentity {
    pub id: String,
    pub display: String,
    signing: SigningKey,
}

impl ProfileIdentity {
    pub fn load(base: &Path, profile: &str, display: &str) -> Result<Self, String> {
        uuid::Uuid::parse_str(profile).map_err(|_| "Invalid local profile ID")?;
        let dir = base.join("social");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{profile}.key"));
        if !path.exists() {
            let key = iroh::SecretKey::generate();
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => file.write_all(&key.to_bytes()).map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(format!("Cannot save the profile identity: {e}")),
            }
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "Profile identity file is damaged; restore it from backup")?;
        let signing = SigningKey::from_bytes(&bytes);
        Ok(Self {
            id: bs58::encode(signing.verifying_key().as_bytes()).into_string(),
            display: display.to_string(),
            signing,
        })
    }

    pub fn auth_token(&self) -> String {
        let now = chrono::Utc::now().timestamp();
        let message = format!("xfchess-node-auth-v1:{}:{now}", self.id);
        let signature =
            bs58::encode(self.signing.sign(message.as_bytes()).to_bytes()).into_string();
        format!("v1.{}.{now}.{signature}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identities_survive_rename_and_do_not_cross_profiles() {
        let base =
            std::env::temp_dir().join(format!("xfchess-social-test-{}", uuid::Uuid::new_v4()));
        let first = uuid::Uuid::new_v4().to_string();
        let a = ProfileIdentity::load(&base, &first, "Alice").unwrap();
        let renamed = ProfileIdentity::load(&base, &first, "Renamed").unwrap();
        let b = ProfileIdentity::load(&base, &uuid::Uuid::new_v4().to_string(), "Bob").unwrap();
        assert_eq!(a.id, renamed.id);
        assert_ne!(a.id, b.id);
        assert!(a.auth_token().starts_with(&format!("v1.{}.", a.id)));
        // Never silently replace a damaged key (which would orphan the graph).
        std::fs::write(base.join("social").join(format!("{first}.key")), []).unwrap();
        assert!(ProfileIdentity::load(&base, &first, "Alice").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
