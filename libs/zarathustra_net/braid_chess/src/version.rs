use sha2::{Digest, Sha256};

pub fn version_hash(state: &str, move_number: u32) -> String {
    hex::encode(Sha256::digest(format!("{state}:{move_number}").as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_are_deterministic_and_distinguish_repeated_positions() {
        assert_eq!(version_hash("fen", 1), version_hash("fen", 1));
        assert_eq!(version_hash("fen", 1).len(), 64);
        assert_ne!(version_hash("fen", 1), version_hash("fen", 2));
    }
}
