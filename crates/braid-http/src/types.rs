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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braid_request_default() {
        let req = BraidRequest::new();
        assert_eq!(req.version, Version::new("root"));
        assert!(req.parents.is_empty());
        assert!(req.body.is_empty());
    }

    #[test]
    fn braid_request_with_body() {
        let req = BraidRequest::new().with_body(vec![1, 2, 3]);
        assert_eq!(req.body, vec![1, 2, 3]);
    }

    #[test]
    fn braid_request_with_version() {
        let v = Version::U64(7);
        let req = BraidRequest::new().with_version(v.clone());
        assert_eq!(req.version, v);
    }

    #[test]
    fn braid_request_with_parent() {
        let parent = Version::new("p1");
        let req = BraidRequest::new().with_parent(parent.clone());
        assert_eq!(req.parents.len(), 1);
        assert_eq!(req.parents[0], parent);
    }

    #[test]
    fn braid_request_chain_builders() {
        let req = BraidRequest::new()
            .with_body(vec![0xAA])
            .with_version(Version::new("v2"))
            .with_parent(Version::new("v1"))
            .subscribe()
            .with_method("GET")
            .with_content_type("application/octet-stream")
            .with_merge_type("sync");

        assert_eq!(req.body, vec![0xAA]);
        assert_eq!(req.version, Version::new("v2"));
        assert_eq!(req.parents.len(), 1);
    }
}
