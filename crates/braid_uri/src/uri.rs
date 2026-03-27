//! Typed Braid-HTTP resource URIs for XFChess game streams.
//!
//! A [`ChessUri`] represents a Braid resource endpoint for a specific game.
//! The URI maps to an HTTP path that the backend exposes as a Braid resource,
//! allowing clients to subscribe (GET) and push updates (PUT).
//!
//! # URI Structure
//!
//! ```text
//! /game/{game_id}/moves    — move stream
//! /game/{game_id}/clock    — clock updates
//! /game/{game_id}/engine   — engine analysis hints
//! ```

use crate::error::BraidUriError;
use serde::{Deserialize, Serialize};

/// The sub-resource within a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChessPath {
    /// Move stream – the authoritative game move log.
    Moves,
    /// Clock state updates.
    Clock,
    /// Engine analysis hint stream (Stockfish depth/score lines).
    Engine,
}

impl ChessPath {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChessPath::Moves => "moves",
            ChessPath::Clock => "clock",
            ChessPath::Engine => "engine",
        }
    }
}

/// A typed Braid-HTTP resource URI for a chess game stream.
///
/// # Examples
///
/// ```rust
/// use braid_uri::ChessUri;
///
/// let uri = ChessUri::moves("ABCD42".to_string());
/// assert_eq!(uri.to_http_path(), "/game/ABCD42/moves");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessUri {
    /// Unique game room code (e.g. "ABCD42").
    pub game_id: String,
    /// Which sub-resource this URI refers to.
    pub path: ChessPath,
}

impl ChessUri {
    /// Construct a moves stream URI.
    pub fn moves(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            path: ChessPath::Moves,
        }
    }

    /// Construct a clock stream URI.
    pub fn clock(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            path: ChessPath::Clock,
        }
    }

    /// Construct an engine hints URI.
    pub fn engine(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            path: ChessPath::Engine,
        }
    }

    /// Convert to the HTTP path that the Axum backend exposes.
    pub fn to_http_path(&self) -> String {
        format!("/game/{}/{}", self.game_id, self.path.as_str())
    }

    /// Parse from an HTTP path (e.g. `/game/ABCD42/moves`).
    pub fn from_http_path(path: &str) -> Result<Self, BraidUriError> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.splitn(4, '/').collect();
        // Expects ["game", "{id}", "{resource}"]
        match parts.as_slice() {
            ["game", game_id, resource] => {
                let chess_path = match *resource {
                    "moves" => ChessPath::Moves,
                    "clock" => ChessPath::Clock,
                    "engine" => ChessPath::Engine,
                    other => return Err(BraidUriError::UnknownResource(other.to_string())),
                };
                Ok(ChessUri {
                    game_id: game_id.to_string(),
                    path: chess_path,
                })
            }
            _ => Err(BraidUriError::InvalidPath(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_moves() {
        let uri = ChessUri::moves("ABCD42");
        assert_eq!(uri.to_http_path(), "/game/ABCD42/moves");
        let parsed = ChessUri::from_http_path("/game/ABCD42/moves").unwrap();
        assert_eq!(uri, parsed);
    }

    #[test]
    fn roundtrip_clock() {
        let uri = ChessUri::clock("XYZ99");
        let parsed = ChessUri::from_http_path(&uri.to_http_path()).unwrap();
        assert_eq!(uri, parsed);
    }

    #[test]
    fn unknown_resource_error() {
        let result = ChessUri::from_http_path("/game/ABCD42/chat");
        assert!(matches!(result, Err(BraidUriError::UnknownResource(_))));
    }

    #[test]
    fn invalid_path_error() {
        assert!(ChessUri::from_http_path("/notgame/ABCD42/moves").is_err());
    }
}
