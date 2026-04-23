use serde::{Deserialize, Serialize};
use std::fmt;

/// Braid protocol version identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Version {
    String(String),
    U64(u64),
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Version::String(s) => write!(f, "{}", s),
            Version::U64(n) => write!(f, "{}", n),
        }
    }
}

impl Version {
    pub fn new(s: impl Into<String>) -> Self {
        Version::String(s.into())
    }

    /// Returns an iterator over the version (for compatibility with existing code)
    pub fn iter(&self) -> impl Iterator<Item = &Self> {
        std::iter::once(self)
    }
}

/// Braid protocol update (a versioned change to a resource)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub version: Version,
    pub parents: Vec<Version>,
    pub body: Option<Vec<u8>>,
    pub status: u16,
}

impl Update {
    pub fn snapshot(version: Version, body: bytes::Bytes) -> Self {
        Self {
            version,
            parents: Vec::new(),
            body: Some(body.to_vec()),
            status: 200,
        }
    }

    pub fn body_str(&self) -> Option<&str> {
        self.body.as_ref().and_then(|b| std::str::from_utf8(b).ok())
    }

    pub fn patches(&self) -> Option<&Vec<u8>> {
        self.body.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_new_from_string() {
        let v = Version::new("root");
        assert_eq!(v, Version::String("root".to_string()));
    }

    #[test]
    fn version_display_string() {
        let v = Version::String("abc".to_string());
        assert_eq!(format!("{}", v), "abc");
    }

    #[test]
    fn version_display_u64() {
        let v = Version::U64(42);
        assert_eq!(format!("{}", v), "42");
    }

    #[test]
    fn version_iter_returns_once() {
        let v = Version::U64(1);
        let collected: Vec<_> = v.iter().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0], &v);
    }

    #[test]
    fn update_snapshot_roundtrip() {
        let ver = Version::new("v1");
        let body = bytes::Bytes::from_static(b"hello");
        let up = Update::snapshot(ver.clone(), body);

        assert_eq!(up.version, ver);
        assert_eq!(up.parents.len(), 0);
        assert_eq!(up.body, Some(b"hello".to_vec()));
        assert_eq!(up.status, 200);
    }

    #[test]
    fn update_body_str_valid_utf8() {
        let up = Update {
            version: Version::new("v1"),
            parents: vec![],
            body: Some(b"chess".to_vec()),
            status: 200,
        };
        assert_eq!(up.body_str(), Some("chess"));
    }

    #[test]
    fn update_body_str_invalid_utf8() {
        let up = Update {
            version: Version::new("v1"),
            parents: vec![],
            body: Some(vec![0x80, 0x81]),
            status: 200,
        };
        assert_eq!(up.body_str(), None);
    }

    #[test]
    fn update_patches_returns_body() {
        let up = Update {
            version: Version::new("v1"),
            parents: vec![],
            body: Some(vec![1, 2, 3]),
            status: 200,
        };
        assert_eq!(up.patches(), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn update_patches_none_when_no_body() {
        let up = Update {
            version: Version::new("v1"),
            parents: vec![Version::new("v0")],
            body: None,
            status: 204,
        };
        assert_eq!(up.patches(), None);
    }
}
