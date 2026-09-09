use crate::error::BraidChessError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChessStream {
    Moves,
    Clock,
    Engine,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessResource {
    pub game_id: String,
    pub stream: ChessStream,
}

impl ChessResource {
    pub fn moves(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Moves,
        }
    }

    pub fn clock(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Clock,
        }
    }

    pub fn engine(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Engine,
        }
    }

    pub fn chat(game_id: impl Into<String>) -> Self {
        Self {
            game_id: game_id.into(),
            stream: ChessStream::Chat,
        }
    }

    pub fn to_http_path(&self) -> String {
        format!("/game/{}/{}", self.game_id, self.stream.as_str())
    }

    pub fn to_url(&self, base: &str) -> String {
        format!("{}{}", base.trim_end_matches('/'), self.to_http_path())
    }

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
        assert_eq!(
            res.to_url("http://localhost:3000"),
            "http://localhost:3000/game/ABCD42/moves"
        );
        // Trailing slash on base is tolerated.
        assert_eq!(
            res.to_url("http://localhost:3000/"),
            "http://localhost:3000/game/ABCD42/moves"
        );
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
