use crate::error::BraidChessError;
use crate::message::{ChatPayload, ChessMessage, ClockState, EngineHint, MovePayload};
use crate::patch::version_hash;
use crate::resource::ChessResource;
use braid_http::types::{BraidRequest, Version};
use braid_http::BraidClient;
use tracing::{debug, info};

pub struct ChessPublisher {
    client: BraidClient,
    base_url: String,
    game_id: String,
    current_version: Version,
}

impl ChessPublisher {
    pub fn new(
        base_url: impl Into<String>,
        game_id: impl Into<String>,
    ) -> Result<Self, BraidChessError> {
        let client = BraidClient::new().map_err(|e| BraidChessError::Http(e.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            game_id: game_id.into(),
            current_version: Version::new("root"),
        })
    }

    pub fn set_version(&mut self, v: impl Into<String>) {
        self.current_version = Version::new(v);
    }

    pub fn version(&self) -> &Version {
        &self.current_version
    }

    // ─── Public API ──────────────────────────────────────────────────────────

    pub async fn publish_move(&mut self, payload: &MovePayload) -> Result<(), BraidChessError> {
        let new_version = Version::new(version_hash(&payload.fen_after, payload.move_number));
        let msg = ChessMessage::Move(payload.clone());
        self.put(ChessResource::moves(&self.game_id), msg, new_version)
            .await
    }

    pub async fn publish_resign(&mut self, player: &str) -> Result<(), BraidChessError> {
        let new_version = Version::new(version_hash(player, 0));
        let msg = ChessMessage::Resign {
            player: player.to_string(),
        };
        self.put(ChessResource::moves(&self.game_id), msg, new_version)
            .await
    }

    pub async fn publish_offer_draw(&mut self, player: &str) -> Result<(), BraidChessError> {
        let new_version = Version::new(version_hash(&format!("draw:{}", player), 0));
        let msg = ChessMessage::OfferDraw {
            player: player.to_string(),
        };
        self.put(ChessResource::moves(&self.game_id), msg, new_version)
            .await
    }

    pub async fn publish_chat(
        &mut self,
        player: &str,
        text: &str,
        timestamp_ms: u64,
    ) -> Result<(), BraidChessError> {
        let seed = format!("chat:{}:{}", player, timestamp_ms);
        let new_version = Version::new(version_hash(&seed, 0));
        let msg = ChessMessage::Chat(ChatPayload {
            player: player.to_string(),
            text: text.to_string(),
            timestamp_ms,
        });
        self.put(ChessResource::chat(&self.game_id), msg, new_version)
            .await
    }

    pub async fn publish_clock(&mut self, state: &ClockState) -> Result<(), BraidChessError> {
        let new_version = Version::new(version_hash(
            &format!("clock:{}:{}", state.white_ms, state.black_ms),
            0,
        ));
        let msg = ChessMessage::Clock(state.clone());
        self.put(ChessResource::clock(&self.game_id), msg, new_version)
            .await
    }

    pub async fn publish_engine_hint(&mut self, hint: EngineHint) -> Result<(), BraidChessError> {
        let new_version = Version::new(version_hash(&hint.best_move, hint.depth as u32));
        let msg = ChessMessage::EngineAnalysis(hint);
        self.put(ChessResource::engine(&self.game_id), msg, new_version)
            .await
    }

    // ─── Internal ────────────────────────────────────────────────────────────

    async fn put(
        &mut self,
        resource: ChessResource,
        msg: ChessMessage,
        new_version: Version,
    ) -> Result<(), BraidChessError> {
        let url = resource.to_url(&self.base_url);
        let body_json = serde_json::to_string(&msg)?;

        debug!(
            "[BRAID PUB] PUT {} v={} parents=[{}]",
            url, new_version, self.current_version
        );

        // Use BraidClient.put() which correctly sets Content-Type and a version UUID
        // We override the version and parents via the BraidRequest builder
        let request = BraidRequest::new()
            .with_method("PUT")
            .with_body(body_json.into_bytes())
            .with_content_type("application/json")
            .with_version(new_version.clone())
            .with_parent(self.current_version.clone())
            .with_merge_type("replace");

        let response = self
            .client
            .fetch(&url, request)
            .await
            .map_err(|e| BraidChessError::Http(e.to_string()))?;

        if response.status < 400 {
            info!(
                "[BRAID PUB] {} published (status {})",
                resource.stream.as_str(),
                response.status
            );
            self.current_version = new_version;
            Ok(())
        } else {
            Err(BraidChessError::HttpStatus(response.status))
        }
    }
}
