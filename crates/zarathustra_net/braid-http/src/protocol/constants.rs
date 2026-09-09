// =============================================================================
// Top-Level Status Code Constants
// =============================================================================

pub const STATUS_SUBSCRIPTION: u16 = 209;

pub const STATUS_MERGE_CONFLICT: u16 = 293;

pub const STATUS_GONE: u16 = 410;

pub const STATUS_RANGE_NOT_SATISFIABLE: u16 = 416;

// =============================================================================
// Status Code Module
// =============================================================================

pub mod status {
    pub const OK: u16 = 200;

    pub const PARTIAL_CONTENT: u16 = 206;

    pub const SUBSCRIPTION: u16 = 209;

    pub const MERGE_CONFLICT: u16 = 293;

    pub const GONE: u16 = 410;

    pub const RANGE_NOT_SATISFIABLE: u16 = 416;
}

// =============================================================================
// Header Names Module
// =============================================================================

pub mod headers {
    use http::HeaderName;

    pub const VERSION: HeaderName = HeaderName::from_static("version");

    pub const PARENTS: HeaderName = HeaderName::from_static("parents");

    pub const CURRENT_VERSION: HeaderName = HeaderName::from_static("current-version");

    pub const SUBSCRIBE: HeaderName = HeaderName::from_static("subscribe");

    pub const HEARTBEATS: HeaderName = HeaderName::from_static("heartbeats");

    pub const PEER: HeaderName = HeaderName::from_static("peer");

    pub const MERGE_TYPE: HeaderName = HeaderName::from_static("merge-type");

    pub const CONTENT_RANGE: HeaderName = http::header::CONTENT_RANGE;

    pub const PATCHES: HeaderName = HeaderName::from_static("patches");

    pub const MULTIPLEX_VERSION: HeaderName = HeaderName::from_static("multiplex-version");

    pub const MULTIPLEX_THROUGH: HeaderName = HeaderName::from_static("multiplex-through");

    pub const RETRY_AFTER: HeaderName = http::header::RETRY_AFTER;

    pub const CONTENT_LENGTH: HeaderName = http::header::CONTENT_LENGTH;

    pub const CONTENT_TYPE: HeaderName = http::header::CONTENT_TYPE;
}

// =============================================================================
// Merge Types Module
// =============================================================================

pub mod merge_types {

    pub const DIAMOND: &str = "diamond";

    pub const SIMPLETON: &str = "simpleton";
}

// =============================================================================
// Media Types Module
// =============================================================================

pub mod media_types {
    pub const BRAID_PATCH: &str = "application/braid-patch";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(STATUS_SUBSCRIPTION, 209);
        assert_eq!(STATUS_MERGE_CONFLICT, 293);
        assert_eq!(STATUS_GONE, 410);
        assert_eq!(STATUS_RANGE_NOT_SATISFIABLE, 416);
    }

    #[test]
    fn test_status_module() {
        assert_eq!(status::OK, 200);
        assert_eq!(status::PARTIAL_CONTENT, 206);
        assert_eq!(status::SUBSCRIPTION, 209);
        assert_eq!(status::MERGE_CONFLICT, 293);
    }

    #[test]
    fn test_header_names() {
        assert_eq!(headers::VERSION.as_str(), "version");
        assert_eq!(headers::PARENTS.as_str(), "parents");
        assert_eq!(headers::SUBSCRIBE.as_str(), "subscribe");
        assert_eq!(headers::MERGE_TYPE.as_str(), "merge-type");
    }

    #[test]
    fn test_merge_types() {
        assert_eq!(merge_types::DIAMOND, "diamond");
    }
}
