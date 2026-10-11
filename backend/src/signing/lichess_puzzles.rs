//! Puzzles served live from a player's Lichess account.
//!
//! A player who links Lichess with the `puzzle:read puzzle:write` scopes can train on
//! puzzles they have never seen on Lichess, and have each result count toward their
//! Lichess puzzle rating. Verification stays on this server: a Lichess puzzle is
//! converted into the same `fen` + UCI `line` shape as the imported puzzle bank, the
//! client only ever receives the setup move and one reply at a time, and the access
//! token never leaves the backend.
//!
//! Lichess API rules this module follows: one request at a time per player, a 60 s
//! pause after HTTP 429, and no bulk enumeration (the CC0 puzzle dump imported by
//! `import_puzzles` is the bulk source).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;
use shakmaty::fen::Fen;
use shakmaty::san::SanPlus;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};

use crate::signing::identity::IdentityVault;

/// OAuth scopes a Lichess link asks for: ratings (existing ELO link) plus puzzles.
pub const LICHESS_SCOPES: &str = "preference:read puzzle:read puzzle:write";

const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A Lichess puzzle in the puzzle bank's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LichessPuzzle {
    pub id: String,
    /// Position before the opponent's setup move.
    pub fen: String,
    /// Setup move followed by the solution, in UCI.
    pub line: Vec<String>,
    pub rating: i64,
    pub themes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LichessError {
    #[error("Lichess is rate limiting this server; try again in a minute")]
    RateLimited,
    #[error("a Lichess request for this player is already in flight")]
    Busy,
    #[error("the Lichess link has no puzzle permission; relink Lichess")]
    Unauthorized,
    #[error("Lichess request failed: {0}")]
    Http(String),
    #[error("unexpected Lichess puzzle: {0}")]
    Malformed(String),
}

/// Converts Lichess's `GET /api/puzzle/next` response (`{game: {pgn}, puzzle: {id,
/// solution, rating, themes}}`) into the bank's shape. The game PGN ends with the
/// opponent's setup move: replaying all but that move gives the starting FEN, and
/// the setup move becomes `line[0]`. The solution must then be legal move by move.
pub fn puzzle_from_api(body: &Value) -> Result<LichessPuzzle, LichessError> {
    let malformed = |what: &str| LichessError::Malformed(what.to_string());
    let puzzle = body
        .get("puzzle")
        .ok_or_else(|| malformed("missing puzzle"))?;
    let id = puzzle
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("missing puzzle.id"))?
        .to_string();
    let pgn = body
        .pointer("/game/pgn")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("missing game.pgn"))?;
    let solution: Vec<String> = puzzle
        .get("solution")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("missing puzzle.solution"))?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    if solution.is_empty() {
        return Err(malformed("empty solution"));
    }

    let sans: Vec<&str> = pgn
        .split_whitespace()
        .filter(|tok| !is_pgn_noise(tok))
        .collect();
    let (setup_san, history) = sans.split_last().ok_or_else(|| malformed("empty pgn"))?;

    let mut pos = Chess::default();
    for san in history {
        pos = play_san(pos, san)?;
    }
    let fen = Fen::from_position(&pos, EnPassantMode::Legal).to_string();

    let setup = parse_san(setup_san)?
        .san
        .to_move(&pos)
        .map_err(|_| malformed(&format!("illegal setup move {setup_san}")))?;
    let mut line = vec![UciMove::from_move(setup, CastlingMode::Standard).to_string()];
    pos = pos
        .play(setup)
        .map_err(|_| malformed("setup move did not play"))?;

    for uci in &solution {
        let m = UciMove::from_ascii(uci.as_bytes())
            .map_err(|_| malformed(&format!("bad uci {uci}")))?
            .to_move(&pos)
            .map_err(|_| malformed(&format!("illegal solution move {uci}")))?;
        pos = pos
            .play(m)
            .map_err(|_| malformed("solution move did not play"))?;
        line.push(uci.clone());
    }

    Ok(LichessPuzzle {
        id,
        fen,
        line,
        rating: puzzle.get("rating").and_then(Value::as_i64).unwrap_or(1500),
        themes: puzzle
            .get("themes")
            .and_then(Value::as_array)
            .map(|t| {
                t.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Move numbers (`12.`, `12...`) and results are not moves.
fn is_pgn_noise(tok: &str) -> bool {
    tok.ends_with('.') || matches!(tok, "1-0" | "0-1" | "1/2-1/2" | "*")
}

fn parse_san(san: &str) -> Result<SanPlus, LichessError> {
    san.parse::<SanPlus>()
        .map_err(|_| LichessError::Malformed(format!("bad san {san}")))
}

fn play_san(pos: Chess, san: &str) -> Result<Chess, LichessError> {
    let m = parse_san(san)?
        .san
        .to_move(&pos)
        .map_err(|_| LichessError::Malformed(format!("illegal pgn move {san}")))?;
    pos.play(m)
        .map_err(|_| LichessError::Malformed(format!("pgn move {san} did not play")))
}

/// HTTP access to Lichess's puzzle API with the rate-limit discipline above.
pub struct LichessClient {
    http: reqwest::Client,
    base: String,
    paused_until: Mutex<Option<Instant>>,
    in_flight: Mutex<HashSet<String>>,
}

/// Holds a player's in-flight slot; releasing it on drop keeps the slot correct on
/// every early return.
struct InFlight<'a> {
    client: &'a LichessClient,
    wallet: String,
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = self.client.in_flight.lock() {
            set.remove(&self.wallet);
        }
    }
}

impl LichessClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client builds with a timeout only"),
            base: base.into().trim_end_matches('/').to_string(),
            paused_until: Mutex::new(None),
            in_flight: Mutex::new(HashSet::new()),
        }
    }

    /// Process-wide client; `LICHESS_API_URL` overrides the host (tests, staging).
    pub fn global() -> &'static LichessClient {
        static CLIENT: OnceLock<LichessClient> = OnceLock::new();
        CLIENT.get_or_init(|| {
            LichessClient::new(
                std::env::var("LICHESS_API_URL").unwrap_or_else(|_| "https://lichess.org".into()),
            )
        })
    }

    fn begin(&self, wallet: &str) -> Result<InFlight<'_>, LichessError> {
        if let Ok(paused) = self.paused_until.lock() {
            if paused.is_some_and(|until| Instant::now() < until) {
                return Err(LichessError::RateLimited);
            }
        }
        let mut set = self.in_flight.lock().map_err(|_| LichessError::Busy)?;
        if !set.insert(wallet.to_string()) {
            return Err(LichessError::Busy);
        }
        Ok(InFlight {
            client: self,
            wallet: wallet.to_string(),
        })
    }

    fn check(&self, status: reqwest::StatusCode) -> Result<(), LichessError> {
        match status.as_u16() {
            200..=299 => Ok(()),
            429 => {
                if let Ok(mut paused) = self.paused_until.lock() {
                    *paused = Some(Instant::now() + RATE_LIMIT_PAUSE);
                }
                Err(LichessError::RateLimited)
            }
            401 | 403 => Err(LichessError::Unauthorized),
            code => Err(LichessError::Http(format!("HTTP {code}"))),
        }
    }

    /// A puzzle this player has not seen on Lichess.
    pub async fn next_puzzle(
        &self,
        wallet: &str,
        token: &str,
        angle: Option<&str>,
        difficulty: Option<&str>,
    ) -> Result<LichessPuzzle, LichessError> {
        let _slot = self.begin(wallet)?;
        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(angle) = angle {
            query.push(("angle", angle));
        }
        if let Some(difficulty) = difficulty {
            query.push(("difficulty", difficulty));
        }
        let resp = self
            .http
            .get(format!("{}/api/puzzle/next", self.base))
            .bearer_auth(token)
            .query(&query)
            .send()
            .await
            .map_err(|e| LichessError::Http(e.to_string()))?;
        self.check(resp.status())?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| LichessError::Http(e.to_string()))?;
        puzzle_from_api(&body)
    }

    /// Records a result on the player's Lichess account; `rated` results also move
    /// their Lichess puzzle rating.
    pub async fn record_result(
        &self,
        wallet: &str,
        token: &str,
        puzzle_id: &str,
        win: bool,
        rated: bool,
    ) -> Result<(), LichessError> {
        let _slot = self.begin(wallet)?;
        let resp = self
            .http
            .post(format!("{}/api/puzzle/batch/mix", self.base))
            .bearer_auth(token)
            .json(&serde_json::json!({
                "solutions": [{ "id": puzzle_id, "win": win, "rated": rated }]
            }))
            .send()
            .await
            .map_err(|e| LichessError::Http(e.to_string()))?;
        self.check(resp.status())
    }
}

/// Stores a player's Lichess token, encrypted with the identity vault.
pub async fn save_token(
    pool: &sqlx::SqlitePool,
    vault: &IdentityVault,
    wallet: &str,
    token: &str,
    scope: &str,
) -> Result<(), String> {
    let encrypted = vault.encrypt_bytes(token.as_bytes())?;
    sqlx::query(
        "INSERT INTO lichess_tokens (pubkey, token_enc, scope, created_at) VALUES (?, ?, ?, ?)
         ON CONFLICT(pubkey) DO UPDATE SET token_enc = excluded.token_enc,
           scope = excluded.scope, created_at = excluded.created_at",
    )
    .bind(wallet)
    .bind(&encrypted[..])
    .bind(scope)
    .bind(chrono::Utc::now().timestamp())
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// The player's Lichess token, if they linked with puzzle permission.
pub async fn load_puzzle_token(
    pool: &sqlx::SqlitePool,
    vault: &IdentityVault,
    wallet: &str,
) -> Option<String> {
    let (blob, scope): (Vec<u8>, String) =
        sqlx::query_as("SELECT token_enc, scope FROM lichess_tokens WHERE pubkey = ?")
            .bind(wallet)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()?;
    if !scope.split_whitespace().any(|s| s == "puzzle:read") {
        return None;
    }
    let bytes = vault.decrypt_bytes(&blob).ok()?;
    String::from_utf8(bytes).ok()
}

/// Puzzle themes offered in the client's picker, as `(lichess key, label)`. A curated
/// subset of Lichess's theme list; `mix` means any theme.
pub const THEMES: &[(&str, &str)] = &[
    ("mix", "Healthy mix"),
    ("mateIn1", "Mate in 1"),
    ("mateIn2", "Mate in 2"),
    ("mateIn3", "Mate in 3"),
    ("fork", "Fork"),
    ("pin", "Pin"),
    ("skewer", "Skewer"),
    ("discoveredAttack", "Discovered attack"),
    ("doubleCheck", "Double check"),
    ("sacrifice", "Sacrifice"),
    ("deflection", "Deflection"),
    ("attraction", "Attraction"),
    ("hangingPiece", "Hanging piece"),
    ("trappedPiece", "Trapped piece"),
    ("backRankMate", "Back-rank mate"),
    ("smotheredMate", "Smothered mate"),
    ("promotion", "Promotion"),
    ("endgame", "Endgame"),
    ("rookEndgame", "Rook endgame"),
    ("pawnEndgame", "Pawn endgame"),
    ("opening", "Opening"),
    ("middlegame", "Middlegame"),
    ("short", "Short (2 moves)"),
    ("long", "Long (3+ moves)"),
];

/// Lichess's difficulty names and the rating offset each applies to the player's
/// puzzle rating (the same scale Lichess uses).
pub fn difficulty_offset(difficulty: &str) -> Option<i64> {
    match difficulty {
        "easiest" => Some(-600),
        "easier" => Some(-300),
        "normal" => Some(0),
        "harder" => Some(300),
        "hardest" => Some(600),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_an_api_puzzle_into_fen_and_line() {
        // 1.e4 e5 2.Qh5 Nc6 3.Bc4 Nf6?? — the PGN ends on the setup move Nf6,
        // and the solution is Qxf7#.
        let body = json!({
            "game": { "pgn": "e4 e5 Qh5 Nc6 Bc4 Nf6" },
            "puzzle": { "id": "abc12", "rating": 900, "solution": ["h5f7"], "themes": ["mateIn1", "short"] }
        });
        let p = puzzle_from_api(&body).unwrap();
        assert_eq!(p.id, "abc12");
        assert_eq!(p.line, vec!["g8f6".to_string(), "h5f7".to_string()]);
        assert_eq!(
            p.fen,
            "r1bqkbnr/pppp1ppp/2n5/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 3 3"
        );
        assert_eq!(p.rating, 900);
        assert_eq!(p.themes, vec!["mateIn1".to_string(), "short".to_string()]);
    }

    #[test]
    fn tolerates_move_numbers_and_check_marks() {
        let body = json!({
            "game": { "pgn": "1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6" },
            "puzzle": { "id": "x", "solution": ["h5f7"] }
        });
        assert_eq!(puzzle_from_api(&body).unwrap().line[0], "g8f6");
    }

    #[test]
    fn rejects_a_solution_that_is_not_legal() {
        let body = json!({
            "game": { "pgn": "e4 e5 Qh5 Nc6 Bc4 Nf6" },
            "puzzle": { "id": "x", "solution": ["h5h8"] }
        });
        assert!(matches!(
            puzzle_from_api(&body),
            Err(LichessError::Malformed(_))
        ));
    }

    #[test]
    fn difficulty_names_match_lichess() {
        assert_eq!(difficulty_offset("hardest"), Some(600));
        assert_eq!(difficulty_offset("normal"), Some(0));
        assert_eq!(difficulty_offset("impossible"), None);
    }

    #[tokio::test]
    async fn rate_limit_pauses_and_slots_release() {
        let client = LichessClient::new("http://127.0.0.1:9");
        {
            let _slot = client.begin("w").unwrap();
            assert!(matches!(client.begin("w"), Err(LichessError::Busy)));
        }
        assert!(client.begin("w").is_ok(), "slot released on drop");
        assert!(client
            .check(reqwest::StatusCode::TOO_MANY_REQUESTS)
            .is_err());
        assert!(matches!(client.begin("w"), Err(LichessError::RateLimited)));
    }
}
