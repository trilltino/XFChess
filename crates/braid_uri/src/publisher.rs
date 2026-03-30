//! Full Braid-HTTP publisher for chess game move streams.
//!
//! [`ChessPublisher`] uses `braid-http`'s `BraidClient` to PUT chess events
//! to Braid resource endpoints with proper `Version` and `Parents` headers.
//!
//! # Example
//!
//! ```rust,no_run
//! use braid_uri::{ChessPublisher, MovePayload};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut pub_ = ChessPublisher::new("http://localhost:3000", "ABCD42").unwrap();
//!     let payload = MovePayload::from_uci("e2e4", "fen...", 1, "alice");
//!     pub_.publish_move(&payload).await.unwrap();
//! }
//! ```

use crate::error::BraidUriError;
use crate::message::{ChessMessage, EngineHint, MovePayload};
use crate::patch::version_hash;
use crate::uri::ChessUri;
use braid_http::types::{BraidRequest, Version};
use braid_http::BraidClient;
use tracing::{debug, info};

/// Publishes chess game events to Braid-HTTP resource endpoints.
pub struct ChessPublisher {
    client: BraidClient,
    base_url: String,
    game_id: String,
    /// Current head version (used as `Parents` for next PUT).
    current_version: Version,
}

impl ChessPublisher {
    /// Create a publisher for the given game.
    ///
    /// `base_url` e.g. `"http://localhost:3000"`.
    pub fn new(
        base_url: impl Into<String>,
        game_id: impl Into<String>,
    ) -> Result<Self, BraidUriError> {
        let client = BraidClient::new().map_err(|e| BraidUriError::Http(e.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            game_id: game_id.into(),
            current_version: Version::new("root"),
        })
    }

    /// Override the current version (e.g. after reconnecting and re-syncing).
    pub fn set_version(&mut self, v: impl Into<String>) {
        self.current_version = Version::new(v);
    }

    /// Current head version.
    pub fn version(&self) -> &Version {
        &self.current_version
    }

    // ─── Public API ──────────────────────────────────────────────────────────

    /// PUT a chess move onto the moves sub-resource.
    pub async fn publish_move(&mut self, payload: &MovePayload) -> Result<(), BraidUriError> {
        let new_version = Version::new(version_hash(&payload.fen_after, payload.move_number));
        let msg = ChessMessage::Move(payload.clone());
        self.put(ChessUri::moves(&self.game_id), msg, new_version)
            .await
    }

    /// PUT a resign event onto the moves sub-resource.
    pub async fn publish_resign(&mut self, player: &str) -> Result<(), BraidUriError> {
        let new_version = Version::new(version_hash(player, 0));
        let msg = ChessMessage::Resign {
            player: player.to_string(),
        };
        self.put(ChessUri::moves(&self.game_id), msg, new_version)
            .await
    }

    /// PUT a draw offer onto the moves sub-resource.
    pub async fn publish_offer_draw(&mut self, player: &str) -> Result<(), BraidUriError> {
        let new_version = Version::new(version_hash(&format!("draw:{}", player), 0));
        let msg = ChessMessage::OfferDraw {
            player: player.to_string(),
        };
        self.put(ChessUri::moves(&self.game_id), msg, new_version)
            .await
    }

    /// PUT a Stockfish engine hint onto the engine sub-resource.
    pub async fn publish_engine_hint(&mut self, hint: EngineHint) -> Result<(), BraidUriError> {
        let new_version = Version::new(version_hash(&hint.best_move, hint.depth as u32));
        let msg = ChessMessage::EngineAnalysis(hint);
        self.put(ChessUri::engine(&self.game_id), msg, new_version)
            .await
    }

    // ─── Internal ────────────────────────────────────────────────────────────

    async fn put(
        &mut self,
        uri: ChessUri,
        msg: ChessMessage,
        new_version: Version,
    ) -> Result<(), BraidUriError> {
        let url = format!("{}{}", self.base_url, uri.to_http_path());
        let body_json = serde_json::to_string(&msg)?;

        debug!(
            "[BRAID PUB] PUT {} v={} parents=[{}]",
            url, new_version, self.current_version
        );

        // Use BraidClient.put() which correctly sets Content-Type and a version UUID
        // We override the version and parents via the BraidRequest builder
        let request = BraidRequest::new()
            .with_method("PUT")
            .with_body(body_json)
            .with_content_type("application/json")
            .with_version(new_version.clone())
            .with_parent(self.current_version.clone())
            .with_merge_type("replace");

        let response = self
            .client
            .fetch(&url, request)
            .await
            .map_err(|e| BraidUriError::Http(e.to_string()))?;

        if response.status < 400 {
            info!(
                "[BRAID PUB] {} published (status {})",
                uri.path.as_str(),
                response.status
            );
            self.current_version = new_version;
            Ok(())
        } else {
            Err(BraidUriError::HttpStatus(response.status))
        }
    }
}
