use serde::{Deserialize, Serialize};

/// Re-export from braid_core for convenience
pub use braid_core::{Update, Version};

/// Braid HTTP request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraidRequest {
    pub version: Version,
    pub parents: Vec<Version>,
    pub body: Vec<u8>,
}

impl BraidRequest {
    pub fn new() -> Self {
        Self {
            version: Version::new("root"),
            parents: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn subscribe(self) -> Self {
        self
    }

    pub fn with_method(self, _method: &str) -> Self {
        self
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn with_content_type(self, _content_type: &str) -> Self {
        self
    }

    pub fn with_version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    pub fn with_parent(mut self, parent: Version) -> Self {
        self.parents.push(parent);
        self
    }

    pub fn with_merge_type(self, _merge_type: &str) -> Self {
        self
    }
}
