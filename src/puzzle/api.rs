//! The backend's puzzle API (`backend/src/signing/routes/puzzle.rs`).
//!
//! Every function here blocks on HTTP, so call them only from a
//! [`Pending`](crate::core::net_task::Pending) worker, never from a system.

use serde::{de::DeserializeOwned, Deserialize};

use crate::multiplayer::network::vps::{client, vps_base};

/// Where the next puzzle comes from and how it counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PuzzleKind {
    /// Rated training from the imported Lichess puzzle bank.
    #[default]
    Rated,
    /// The server's puzzle of the day (the same for everyone).
    Daily,
    /// Served by the player's linked Lichess account; results sync back to it.
    Lichess,
    /// Puzzles with an active SOL bounty (needs a wallet).
    Earn,
}

impl PuzzleKind {
    pub const ALL: [PuzzleKind; 4] = [Self::Rated, Self::Daily, Self::Lichess, Self::Earn];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rated => "Rated",
            Self::Daily => "Daily",
            Self::Lichess => "Lichess",
            Self::Earn => "Earn",
        }
    }

    /// Whether the theme and difficulty pickers apply.
    pub fn is_tunable(self) -> bool {
        matches!(self, Self::Rated | Self::Lichess)
    }

    /// Whether this kind needs a real wallet (a guest identifier is not enough).
    pub fn needs_wallet(self) -> bool {
        matches!(self, Self::Lichess | Self::Earn)
    }
}

/// Puzzle difficulty relative to the player's puzzle rating (Lichess' scale).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Difficulty {
    Easiest,
    Easier,
    #[default]
    Normal,
    Harder,
    Hardest,
}

impl Difficulty {
    pub const ALL: [Difficulty; 5] = [
        Self::Easiest,
        Self::Easier,
        Self::Normal,
        Self::Harder,
        Self::Hardest,
    ];

    /// The name the backend (and Lichess) uses.
    pub fn key(self) -> &'static str {
        match self {
            Self::Easiest => "easiest",
            Self::Easier => "easier",
            Self::Normal => "normal",
            Self::Harder => "harder",
            Self::Hardest => "hardest",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Easiest => "Easiest",
            Self::Easier => "Easier",
            Self::Normal => "Normal",
            Self::Harder => "Harder",
            Self::Hardest => "Hardest",
        }
    }
}

/// Everything needed to ask for the next puzzle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PuzzleRequest {
    pub kind: PuzzleKind,
    /// Wallet pubkey, or the local node id for guests.
    pub wallet: String,
    /// Lichess theme key; `None` means any.
    pub theme: Option<String>,
    pub difficulty: Difficulty,
}

impl PuzzleRequest {
    fn url(&self, base: &str) -> String {
        // Wallets, node ids, theme keys and difficulty names are all URL-safe.
        let mut url = match self.kind {
            PuzzleKind::Daily => return format!("{base}/puzzle/daily?wallet={}", self.wallet),
            PuzzleKind::Earn => format!("{base}/puzzle/next?mode=earn&wallet={}", self.wallet),
            PuzzleKind::Rated => format!("{base}/puzzle/next?mode=solve&wallet={}", self.wallet),
            PuzzleKind::Lichess => {
                format!("{base}/puzzle/next?source=lichess&wallet={}", self.wallet)
            }
        };
        if self.kind.is_tunable() {
            if let Some(theme) = &self.theme {
                url.push_str(&format!("&theme={theme}"));
            }
            url.push_str(&format!("&difficulty={}", self.difficulty.key()));
        }
        url
    }
}

/// A puzzle as served: the position before the opponent's setup move.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PuzzleData {
    #[serde(default)]
    pub nonce: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub fen: String,
    #[serde(default)]
    pub setup_move: String,
    /// The side the player solves for, `white` or `black`.
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub rating: i64,
    /// How many moves the player has to find.
    #[serde(default)]
    pub solution_len: i64,
    #[serde(default)]
    pub themes: Vec<String>,
    /// `local` or `lichess`.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub reward_lamports: Option<i64>,
    /// No puzzle matched (bank empty, or every match already attempted).
    #[serde(default)]
    pub exhausted: bool,
    /// Today's daily puzzle was already played.
    #[serde(default)]
    pub already_attempted: bool,
}

/// The server's verdict on one player move.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MoveOutcome {
    #[serde(default)]
    pub correct: bool,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub win: bool,
    /// The opponent's answer, when the line continues.
    #[serde(default)]
    pub reply: Option<String>,
    #[serde(default)]
    pub rating: i64,
    #[serde(default)]
    pub rating_diff: i64,
    #[serde(default)]
    pub payout_sig: Option<String>,
    #[serde(default)]
    pub paid_lamports: i64,
}

/// A theme the server can filter by.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Theme {
    pub key: String,
    pub label: String,
}

#[derive(Deserialize)]
struct Hint {
    from: String,
}

pub fn fetch_puzzle(req: &PuzzleRequest) -> Result<PuzzleData, String> {
    get(&req.url(&vps_base()))
}

pub fn fetch_themes() -> Result<Vec<Theme>, String> {
    get(&format!("{}/puzzle/themes", vps_base()))
}

pub fn submit_move(nonce: &str, uci: &str) -> Result<MoveOutcome, String> {
    post(
        "/puzzle/move",
        serde_json::json!({ "nonce": nonce, "uci": uci }),
    )
}

/// The square the next solution move starts from. Makes the round unrated.
pub fn request_hint(nonce: &str) -> Result<String, String> {
    post::<Hint>("/puzzle/hint", serde_json::json!({ "nonce": nonce })).map(|h| h.from)
}

fn get<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    read(client()?.get(url).send())
}

fn post<T: DeserializeOwned>(path: &str, body: serde_json::Value) -> Result<T, String> {
    read(
        client()?
            .post(format!("{}{path}", vps_base()))
            .json(&body)
            .send(),
    )
}

fn read<T: DeserializeOwned>(
    response: reqwest::Result<reqwest::blocking::Response>,
) -> Result<T, String> {
    let response = response.map_err(|e| format!("network error: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(describe_failure(status.as_u16(), body.trim()));
    }
    response
        .json()
        .map_err(|e| format!("unexpected response: {e}"))
}

/// A short, player-facing reason for a failed request. The backend's own message
/// wins when it sent one.
fn describe_failure(status: u16, body: &str) -> String {
    if status == 401 {
        return "sign in with your wallet first".into();
    }
    if !body.is_empty() {
        return body.chars().take(120).collect();
    }
    match status {
        404 => "puzzle not found".into(),
        409 => "puzzle already finished".into(),
        410 => "puzzle expired — load the next one".into(),
        412 => "link your Lichess account first".into(),
        _ => format!("server error ({status})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: PuzzleKind) -> PuzzleRequest {
        PuzzleRequest {
            kind,
            wallet: "W".into(),
            theme: Some("fork".into()),
            difficulty: Difficulty::Harder,
        }
    }

    #[test]
    fn builds_urls_per_kind() {
        assert_eq!(
            request(PuzzleKind::Rated).url("B"),
            "B/puzzle/next?mode=solve&wallet=W&theme=fork&difficulty=harder"
        );
        assert_eq!(
            request(PuzzleKind::Lichess).url("B"),
            "B/puzzle/next?source=lichess&wallet=W&theme=fork&difficulty=harder"
        );
        assert_eq!(
            request(PuzzleKind::Earn).url("B"),
            "B/puzzle/next?mode=earn&wallet=W"
        );
        assert_eq!(
            request(PuzzleKind::Daily).url("B"),
            "B/puzzle/daily?wallet=W"
        );
    }

    #[test]
    fn any_theme_is_omitted() {
        let mut req = request(PuzzleKind::Rated);
        req.theme = None;
        assert_eq!(
            req.url("B"),
            "B/puzzle/next?mode=solve&wallet=W&difficulty=harder"
        );
    }

    #[test]
    fn decodes_served_puzzle() -> Result<(), Box<dyn std::error::Error>> {
        let p: PuzzleData = serde_json::from_str(
            r#"{"nonce":"n","id":"abc","fen":"8/8/8/8/8/8/8/8 w - - 0 1","setup_move":"e2e4",
                "color":"black","rating":1500,"solution_len":2,"themes":["fork","short"],
                "source":"lichess"}"#,
        )?;
        assert_eq!(p.themes, ["fork", "short"]);
        assert_eq!(p.source, "lichess");
        assert!(!p.exhausted);

        Ok(())
    }
}
