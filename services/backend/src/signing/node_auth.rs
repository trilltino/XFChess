//! Node identity for callers that have no wallet: the lobby relay, the social graph, client
//! events and casual game writes.
//!
//! A node id is the base58 ed25519 public key of the key the client's network node was created
//! from. The client proves it holds that key with a self-issued token in `X-Node-Auth`:
//!
//! ```text
//! v1.<node_id>.<unix_seconds>.<base58 signature of "xfchess-node-auth-v1:<node_id>:<unix_seconds>">
//! ```
//!
//! A token is a bearer credential like a JWT: it is valid for `MAX_AGE_SECS` after it is issued,
//! so it is only safe over TLS. It is not bound to a request, because the Braid outbox sends
//! its headers from a fixed hook. A request that carries a token with a bad signature or the
//! wrong age is refused outright rather than treated as anonymous.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;

pub const HEADER: &str = "x-node-auth";
const PREFIX: &str = "xfchess-node-auth-v1";
/// How long after issue a token is accepted.
pub const MAX_AGE_SECS: i64 = 3600;
/// Clock skew tolerated for a token issued slightly in the future.
const MAX_FUTURE_SECS: i64 = 120;

/// The node id a request proved it holds the key for.
#[derive(Clone, Debug)]
pub struct AuthedNode(pub String);

pub fn signable(node_id: &str, issued_at: i64) -> String {
    format!("{PREFIX}:{node_id}:{issued_at}")
}

#[derive(Debug, PartialEq, Eq)]
pub enum NodeAuthError {
    Malformed,
    BadNodeId,
    BadSignature,
    Expired,
}

/// Returns the node id a token proves, or why it is refused.
pub fn verify_token(token: &str, now: i64) -> Result<String, NodeAuthError> {
    let mut parts = token.split('.');
    let (Some("v1"), Some(node_id), Some(issued), Some(signature), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Err(NodeAuthError::Malformed);
    };
    let issued_at: i64 = issued.parse().map_err(|_| NodeAuthError::Malformed)?;
    if issued_at > now.saturating_add(MAX_FUTURE_SECS)
        || now.saturating_sub(issued_at) > MAX_AGE_SECS
    {
        return Err(NodeAuthError::Expired);
    }
    let key = Pubkey::from_str(node_id).map_err(|_| NodeAuthError::BadNodeId)?;
    let signature = Signature::from_str(signature).map_err(|_| NodeAuthError::BadSignature)?;
    if signature.verify(key.as_ref(), signable(node_id, issued_at).as_bytes()) {
        Ok(node_id.to_string())
    } else {
        Err(NodeAuthError::BadSignature)
    }
}

/// The verified node id carried by `headers`, if there is a token. `Err` means a token was
/// present and refused.
pub fn node_from_headers(headers: &HeaderMap, now: i64) -> Result<Option<String>, NodeAuthError> {
    let Some(value) = headers.get(HEADER) else {
        return Ok(None);
    };
    let token = value.to_str().map_err(|_| NodeAuthError::Malformed)?;
    verify_token(token, now).map(Some)
}

/// Verifies `X-Node-Auth` when present and records the node on the request. A request without
/// the header continues anonymously; handlers that need a node use `RequireNode`.
pub async fn attach_node_identity(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match node_from_headers(request.headers(), chrono::Utc::now().timestamp()) {
        Ok(Some(node_id)) => {
            request.extensions_mut().insert(AuthedNode(node_id));
            Ok(next.run(request).await)
        }
        Ok(None) => Ok(next.run(request).await),
        Err(error) => {
            tracing::debug!("[node-auth] refused token: {error:?}");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// The node id the caller proved. Rejects with 401 when the request carried no valid token.
pub struct RequireNode(pub String);

impl<S> axum::extract::FromRequestParts<S> for RequireNode
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<AuthedNode>() {
            Some(AuthedNode(node)) => Ok(RequireNode(node.clone())),
            None => Err((
                StatusCode::UNAUTHORIZED,
                "This endpoint acts for a network node and requires an X-Node-Auth token \
                 signed by that node's key."
                    .to_string(),
            )),
        }
    }
}

impl RequireNode {
    /// 403 unless the request body or query names the node that authenticated.
    pub fn owns(&self, claimed: &str) -> Result<(), (StatusCode, String)> {
        if self.0 == claimed {
            Ok(())
        } else {
            Err((
                StatusCode::FORBIDDEN,
                "Authenticated node does not match the node this request acts for".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{Keypair, Signer};

    fn token_for(key: &Keypair, issued_at: i64) -> String {
        let node_id = key.pubkey().to_string();
        let signature = key.sign_message(signable(&node_id, issued_at).as_bytes());
        format!("v1.{node_id}.{issued_at}.{signature}")
    }

    #[test]
    fn a_fresh_token_proves_its_node() {
        let key = Keypair::new();
        let token = token_for(&key, 1_000);
        assert_eq!(verify_token(&token, 1_000), Ok(key.pubkey().to_string()));
        assert_eq!(
            verify_token(&token, 1_000 + MAX_AGE_SECS),
            Ok(key.pubkey().to_string())
        );
    }

    /// Issued by the game client's `vps::client` for the key `[7; 32]` at t=1000. If this stops
    /// verifying, the client and the backend disagree on the signed bytes.
    #[test]
    fn a_token_issued_by_the_client_verifies() {
        const CLIENT_TOKEN: &str = "v1.GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB.1000.4gHhj4H43dJeVWhjFZgTGo1p24TDwtiD8Box3AEzXRsmTQ3BUi3WV4pw8tYGrgLSkRywFhRCpZmJE8dthWQoRu7M";
        assert_eq!(
            verify_token(CLIENT_TOKEN, 1_000),
            Ok("GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB".to_string())
        );
    }

    #[test]
    fn expired_and_future_tokens_are_refused() {
        let token = token_for(&Keypair::new(), 1_000);
        assert_eq!(
            verify_token(&token, 1_000 + MAX_AGE_SECS + 1),
            Err(NodeAuthError::Expired)
        );
        assert_eq!(
            verify_token(&token, 1_000 - 121),
            Err(NodeAuthError::Expired)
        );
    }

    #[test]
    fn another_nodes_signature_is_refused() {
        let (owner, thief) = (Keypair::new(), Keypair::new());
        let forged = format!(
            "v1.{}.1000.{}",
            owner.pubkey(),
            thief.sign_message(signable(&owner.pubkey().to_string(), 1_000).as_bytes())
        );
        assert_eq!(
            verify_token(&forged, 1_000),
            Err(NodeAuthError::BadSignature)
        );
    }

    #[test]
    fn the_timestamp_is_covered_by_the_signature() {
        let key = Keypair::new();
        let node_id = key.pubkey().to_string();
        let signature = key.sign_message(signable(&node_id, 1_000).as_bytes());
        let replayed = format!("v1.{node_id}.2000.{signature}");
        assert_eq!(
            verify_token(&replayed, 2_000),
            Err(NodeAuthError::BadSignature)
        );
    }

    #[test]
    fn malformed_tokens_are_refused() {
        for token in [
            "",
            "v1",
            "v2.a.1.b",
            "v1.a.b.c",
            "v1.a.1.b.extra",
            "v1.notakey.1000.sig",
        ] {
            assert!(verify_token(token, 1_000).is_err(), "{token}");
        }
    }
}
