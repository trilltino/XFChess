//! Typed, base-agnostic chess resource references for XFChess game streams.
//!
//! A [`ChessResource`] identifies a Braid resource for a specific game's
//! sub-stream, *independent of which origin serves it*. It models the path
//! portion only — the origin (scheme + host) is supplied by the transport
//! ([`crate::ChessPublisher`] / [`crate::ChessSubscriber`] hold the base URL),
//! so the same `ChessResource` works against localhost, the VPS, or an Iroh
//! tunnel. Use [`ChessResource::to_url`] to combine it with a base into a full
//! URL.
//!
//! # Resource structure
//!
//! ```text
//! /game/{game_id}/moves    — move stream
//! /game/{game_id}/clock    — clock updates
//! /game/{game_id}/engine   — engine analysis hints
//! /game/{game_id}/chat     — peer chat
//! ```

use crate::error::BraidChessError;
use serde::{Deserialize, Serialize};

/// The sub-stream within a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChessStream {
    /// Move stream – the authoritative game move log.
    Moves,
    /// Clock state updates.
    Clock,
    /// Engine analysis hint stream (Stockfish depth/score lines).
    Engine,
    /// In-game peer chat messages.
    Chat,
}

impl ChessStream {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChessStream::Moves => "moves",
            ChessStream::Clock => "clock",
            ChessStream::Engine => "engine",
            ChessStream::Chat => "chat",
        }
    }
}

/// A typed, base-agnostic reference to a chess game's Braid sub-stream.
///
/// Models the path only; combine with an origin via [`ChessResource::to_url`].
///
/// # Examples
///
/// ```rust
/// use braid_chess::ChessResource;
///
/// let res = ChessResource::moves("ABCD42".to_string());
/// assert_eq!(res.to_http_path(), "/game/ABCD42/moves");
/// assert_eq!(res.to_url("http://localhost:3000"), "http://localhost:3000/game/ABCD42/moves");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessResource {
    /// Unique game room code (e.g. "ABCD42").
    pub game_id: String,
    /// Which sub-stream this resource refers to.
    pub stream: ChessStream,
}

impl ChessResource {
    /// Construct a moves stream resource.
    pub fn moves(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Moves,
        }
    }

    /// Construct a clock stream resource.
    pub fn clock(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Clock,
        }
    }

    /// Construct an engine hints resource.
    pub fn engine(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Engine,
        }
    }

    /// Construct a chat stream resource.
    pub fn chat(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Chat,
        }
    }

    /// The relative HTTP path the backend exposes (no scheme/host).
    pub fn to_http_path(&self) -> String {
        format!("/game/{}/{}", self.game_id, self.stream.as_str())
    }

    /// Combine with an origin into a full URL.
    ///
    /// `base` is the scheme + host (e.g. `"http://127.0.0.1:8090"`); a trailing
    /// slash on `base` is tolerated.
    pub fn to_url(&self, base: &str) -> String {
        format!("{}{}", base.trim_end_matches('/'), self.to_http_path())
    }

    /// Parse from an HTTP path (e.g. `/game/ABCD42/moves`).
    pub fn from_http_path(path: &str) -> Result<Self, BraidChessError> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.splitn(4, '/').collect();
        // Expects ["game", "{id}", "{resource}"]
        match parts.as_slice() {
            ["game", game_id, resource] => {
                let stream = match *resource {
                    "moves" => ChessStream::Moves,
                    "clock" => ChessStream::Clock,
                    "engine" => ChessStream::Engine,
                    "chat" => ChessStream::Chat,
                    other => return Err(BraidChessError::UnknownResource(other.to_string())),
                };
                Ok(ChessResource {
                    game_id: game_id.to_string(),
                    stream,
                })
            }
            _ => Err(BraidChessError::InvalidPath(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_moves() {
        let res = ChessResource::moves("ABCD42");
        assert_eq!(res.to_http_path(), "/game/ABCD42/moves");
        let parsed = ChessResource::from_http_path("/game/ABCD42/moves").unwrap();
        assert_eq!(res, parsed);
    }

    #[test]
    fn to_url_combines_origin() {
        let res = ChessResource::moves("ABCD42");
        assert_eq!(res.to_url("http://localhost:3000"), "http://localhost:3000/game/ABCD42/moves");
        // Trailing slash on base is tolerated.
        assert_eq!(res.to_url("http://localhost:3000/"), "http://localhost:3000/game/ABCD42/moves");
    }

    #[test]
    fn roundtrip_clock() {
        let res = ChessResource::clock("XYZ99");
        let parsed = ChessResource::from_http_path(&res.to_http_path()).unwrap();
        assert_eq!(res, parsed);
    }

    #[test]
    fn roundtrip_chat() {
        let res = ChessResource::chat("ABCD42");
        let parsed = ChessResource::from_http_path("/game/ABCD42/chat").unwrap();
        assert_eq!(res, parsed);
    }

    #[test]
    fn invalid_path_error() {
        assert!(ChessResource::from_http_path("/notgame/ABCD42/moves").is_err());
    }
}
