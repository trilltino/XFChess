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
