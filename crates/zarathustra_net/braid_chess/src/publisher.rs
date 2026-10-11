use crate::{
    version_hash, BraidChessError, ChatPayload, ChessMessage, ChessResource, ChessStream,
    ClockState, EngineHint, MovePayload,
};
use braid_http::{BraidClient, BraidRequest, Version};
use std::collections::HashMap;

pub struct ChessPublisher {
    client: BraidClient,
    base_url: String,
    game_id: String,
    versions: HashMap<ChessStream, Version>,
}

impl ChessPublisher {
    pub fn new(
        base_url: impl Into<String>,
        game_id: impl Into<String>,
    ) -> Result<Self, BraidChessError> {
        Ok(Self {
            client: BraidClient::new().map_err(|e| BraidChessError::Http(e.to_string()))?,
            base_url: base_url.into(),
            game_id: game_id.into(),
            versions: HashMap::new(),
        })
    }

    pub async fn publish_move(&mut self, payload: &MovePayload) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::moves(&self.game_id),
            ChessMessage::Move(payload.clone()),
        )
        .await
    }
    pub async fn publish_resign(&mut self, player: &str) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::moves(&self.game_id),
            ChessMessage::Resign {
                player: player.into(),
            },
        )
        .await
    }
    pub async fn publish_offer_draw(&mut self, player: &str) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::moves(&self.game_id),
            ChessMessage::OfferDraw {
                player: player.into(),
            },
        )
        .await
    }
    pub async fn publish_chat(
        &mut self,
        player: &str,
        text: &str,
        timestamp_ms: u64,
    ) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::chat(&self.game_id),
            ChessMessage::Chat(ChatPayload {
                player: player.into(),
                text: text.into(),
                timestamp_ms,
            }),
        )
        .await
    }
    pub async fn publish_clock(&mut self, state: &ClockState) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::clock(&self.game_id),
            ChessMessage::Clock(state.clone()),
        )
        .await
    }
    pub async fn publish_engine_hint(&mut self, hint: EngineHint) -> Result<(), BraidChessError> {
        self.put(
            ChessResource::engine(&self.game_id),
            ChessMessage::EngineAnalysis(hint),
        )
        .await
    }

    async fn put(
        &mut self,
        resource: ChessResource,
        message: ChessMessage,
    ) -> Result<(), BraidChessError> {
        let url = resource.to_url(&self.base_url);
        let body = serde_json::to_vec(&message)?;
        let parent = self
            .versions
            .get(&resource.stream)
            .cloned()
            .unwrap_or_else(|| Version::new("root"));
        let version = Version::new(match &message {
            ChessMessage::Move(payload) => version_hash(&payload.fen_after, payload.move_number),
            // Include the resource and parent so repeated events remain distinct.
            _ => version_hash(
                &format!(
                    "{}:{parent}:{}",
                    resource.to_http_path(),
                    std::str::from_utf8(&body).expect("JSON is UTF-8")
                ),
                0,
            ),
        });
        let request = BraidRequest::new()
            .with_method("PUT")
            .with_body(body)
            .with_content_type("application/json")
            .with_version(version.clone())
            .with_parent(parent)
            .with_merge_type("replace");
        let response = self
            .client
            .fetch(&url, request)
            .await
            .map_err(|e| BraidChessError::Http(e.to_string()))?;
        if !response.is_success() {
            return Err(BraidChessError::HttpStatus(response.status));
        }
        self.versions.insert(resource.stream, version);
        Ok(())
    }
}
