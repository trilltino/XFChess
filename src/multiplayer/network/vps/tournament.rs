//! Tournament discovery and registration endpoints on the VPS.
//!
//! Lists all advertised tournaments and joins them (optionally gated by a
//! private-tournament password). Returns the slot the player was placed in.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::client::{client, vps_base};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub registered: usize,
    pub status: String,
    pub is_private: bool,
    /// true for Swiss/knockout, false for posted PvP games.
    pub is_tournament: bool,
    #[serde(default)]
    pub usdc_mint: Option<String>,
    #[serde(default)]
    pub max_players: usize,
    #[serde(default)]
    pub min_elo: u32,
    #[serde(default)]
    pub max_elo: u32,
    #[serde(default)]
    pub round_deadline_at: Option<i64>,
    /// "swiss" or "single_elimination" (see backend `TournamentFormat`).
    /// Defaults to empty (treated as Swiss) so an older backend that hasn't
    /// deployed this field yet doesn't break the whole tournament list.
    #[serde(default)]
    pub format: String,
}

/// Which tab a tournament game belongs in, as decided by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Live,
    Upcoming,
    Finished,
}

impl GameState {
    fn parse(s: &str) -> Self {
        match s {
            "live" => Self::Live,
            "finished" => Self::Finished,
            _ => Self::Upcoming,
        }
    }
}

/// One on-chain game created by the backend tournament orchestrator — a bracket
/// match whose `game_id` has been set (i.e. the Solana game account exists).
///
/// Every viewer-facing judgement here (`state`, `watchable`, the result) is
/// computed by the backend from the bracket record joined against the moves and
/// games tables, so the game client and any web viewer agree without either one
/// re-deriving it. The client used to infer "watchable" from the bracket's own
/// `Active` status, which flips when the orchestrator creates the game
/// *account* — long before anyone has moved.
#[derive(Debug, Clone)]
pub struct TournamentGameListing {
    pub tournament_id: u64,
    pub tournament_name: String,
    pub round: u8,
    pub match_index: u16,
    pub white: Option<String>,
    pub black: Option<String>,
    /// Usernames resolved from the players' on-backend profiles, when they have one.
    pub white_name: Option<String>,
    pub black_name: Option<String>,
    pub game_id: u64,
    /// Match status as reported by the backend: "Pending" / "Active" / "Completed".
    pub status: String,
    /// Which tab this game belongs in.
    pub state: GameState,
    /// Moves recorded so far. Zero means an empty board.
    pub move_count: i64,
    /// Unix seconds of the last move; 0 when there are none.
    pub last_move_at: i64,
    /// `1-0` / `0-1` / `1/2-1/2`, once finished.
    pub result: Option<String>,
    /// 0 = live feed available. Non-zero forces the delayed HTTP path.
    pub broadcast_delay_secs: i64,
    /// Whether the Watch button should be enabled.
    pub watchable: bool,
    /// Why not, shown instead of an unexplained greyed-out button.
    pub not_watchable_reason: Option<String>,
}

impl TournamentGameListing {
    /// "last move 12s ago", or `None` when nothing has been played.
    pub fn last_move_label(&self) -> Option<String> {
        if self.last_move_at == 0 {
            return None;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let age = (now - self.last_move_at).max(0);
        Some(match age {
            0..=59 => format!("last move {age}s ago"),
            60..=3599 => format!("last move {}m ago", age / 60),
            _ => format!("last move {}h ago", age / 3600),
        })
    }
}

impl TournamentGameListing {
    /// Display label for white: username, else truncated pubkey, else "TBD".
    pub fn white_label(&self) -> String {
        player_label(&self.white_name, &self.white)
    }

    /// Display label for black: username, else truncated pubkey, else "TBD".
    pub fn black_label(&self) -> String {
        player_label(&self.black_name, &self.black)
    }
}

fn player_label(name: &Option<String>, pubkey: &Option<String>) -> String {
    if let Some(n) = name {
        return n.clone();
    }
    match pubkey.as_deref() {
        Some(p) if p.len() > 8 => format!("{}…{}", &p[..4], &p[p.len() - 4..]),
        Some(p) => p.to_string(),
        None => "TBD".to_string(),
    }
}

/// Per-process cache of wallet → username lookups. Failed/absent profiles are
/// cached as `None` so a profileless player doesn't get re-queried every poll
/// (a name created mid-session shows up after restart, which is acceptable).
static USERNAME_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn resolve_username(pubkey: &str) -> Option<String> {
    let cache = USERNAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(pubkey).cloned()) {
        return hit;
    }
    let resolved = super::identity::fetch_player_profile(pubkey)
        .ok()
        .map(|p| p.username)
        .filter(|u| !u.is_empty());
    if let Ok(mut c) = cache.lock() {
        c.insert(pubkey.to_string(), resolved.clone());
    }
    resolved
}

/// Subset of the backend bracket match JSON we care about.
#[derive(Deserialize)]
struct BracketMatch {
    match_index: u16,
    round: u8,
    player_white: Option<String>,
    player_black: Option<String>,
    game_id: Option<u64>,
    status: String,
}

/// Fetch a finished game's PGN from `GET /games/{id}/pgn`.
///
/// The backend returns a pre-assembled PGN when it has one and assembles from
/// stored SAN otherwise, so this works for any game that has been recorded —
/// including one that finished moments ago.
pub fn fetch_game_pgn(game_id: u64) -> Result<String, String> {
    let resp = client()?
        .get(format!("{}/games/{}/pgn", vps_base(), game_id))
        .send()
        .map_err(|e| format!("PGN request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("PGN unavailable: HTTP {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| format!("PGN response was not JSON: {e}"))?;

    body.get("pgn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "this game has no recorded moves to replay".to_string())
}

/// Fetch one tournament's games from `GET /api/tournament/{id}/games`.
///
/// One request per tournament the viewer is actually looking at, replacing
/// [`list_tournament_games`]'s fan-out of one `/bracket` call per *advertised*
/// tournament every poll. Usernames, watchability, move counts and results all
/// arrive resolved.
pub fn fetch_tournament_games(tournament_id: u64) -> Result<Vec<TournamentGameListing>, String> {
    let resp = client()?
        .get(format!(
            "{}/api/tournament/{}/games",
            vps_base(),
            tournament_id
        ))
        .send()
        .map_err(|e| format!("tournament games request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("tournament games: HTTP {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| format!("tournament games: bad JSON: {e}"))?;

    let tournament_name = body
        .get("tournament_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let games = body
        .get("games")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(games
        .into_iter()
        .map(|g| {
            let str_at = |k: &str| {
                g.get(k)
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
            };
            TournamentGameListing {
                tournament_id,
                tournament_name: tournament_name.clone(),
                round: g.get("round").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                match_index: g.get("board").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
                white: str_at("white"),
                black: str_at("black"),
                white_name: str_at("white_name"),
                black_name: str_at("black_name"),
                game_id: g.get("game_id").and_then(|v| v.as_u64()).unwrap_or(0),
                status: str_at("state").unwrap_or_else(|| "upcoming".into()),
                state: GameState::parse(g.get("state").and_then(|v| v.as_str()).unwrap_or("")),
                move_count: g.get("move_count").and_then(|v| v.as_i64()).unwrap_or(0),
                last_move_at: g.get("last_move_at").and_then(|v| v.as_i64()).unwrap_or(0),
                result: str_at("result"),
                broadcast_delay_secs: g
                    .get("broadcast_delay_secs")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                watchable: g
                    .get("watchable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                not_watchable_reason: str_at("not_watchable_reason"),
            }
        })
        .collect())
}

/// Fetch every Solana game created by backend tournaments: walks the advertised
/// tournaments, pulls each bracket, and keeps only matches with an on-chain
/// `game_id`. Skips posted-PvP entries (`is_tournament == false`) and
/// tournaments whose bracket can't be fetched.
///
/// Kept for the cross-tournament "all live games" view. For a single
/// tournament prefer [`fetch_tournament_games`] — this one issues a request per
/// advertised tournament.
pub fn list_tournament_games() -> Result<Vec<TournamentGameListing>, String> {
    let tournaments = list_tournaments()?;
    let mut out = Vec::new();
    for t in tournaments.into_iter().filter(|t| t.is_tournament) {
        let resp = match client()?
            .get(format!(
                "{}/api/tournament/{}/bracket",
                vps_base(),
                t.tournament_id
            ))
            .send()
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue, // e.g. not started yet — no bracket, no games
        };
        let Ok(bracket) = resp.json::<serde_json::Value>() else {
            continue;
        };
        let matches: Vec<Option<BracketMatch>> = bracket
            .get("matches")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        for m in matches.into_iter().flatten() {
            let Some(game_id) = m.game_id else { continue };
            let white_name = m.player_white.as_deref().and_then(resolve_username);
            let black_name = m.player_black.as_deref().and_then(resolve_username);
            out.push(TournamentGameListing {
                tournament_id: t.tournament_id,
                tournament_name: t.name.clone(),
                round: m.round,
                match_index: m.match_index,
                white: m.player_white,
                black: m.player_black,
                white_name,
                black_name,
                game_id,
                // This path only knows the bracket record. Everything the
                // per-tournament endpoint computes is unavailable here, so it
                // reports the conservative answer: not watchable. Callers that
                // need a Watch button must use `fetch_tournament_games`.
                state: match m.status.as_str() {
                    "Active" => GameState::Live,
                    "Completed" => GameState::Finished,
                    _ => GameState::Upcoming,
                },
                move_count: 0,
                last_move_at: 0,
                result: None,
                broadcast_delay_secs: 0,
                watchable: false,
                not_watchable_reason: Some("open the tournament for live status".into()),
                status: m.status,
            });
        }
    }
    Ok(out)
}

/// Fetch the list of advertised tournaments from the VPS.
///
/// Deliberately `/api/tournaments`, not the bare `/tournaments` path: the
/// web frontend has its own unrelated marketing page at that exact path, and
/// in production nginx has no way to distinguish "browser wants the page"
/// from "game client wants JSON" on an identical URL — it always resolves
/// to the frontend's SPA catch-all. See infrastructure/router.rs (backend).
pub fn list_tournaments() -> Result<Vec<TournamentSummary>, String> {
    let resp = client()?
        .get(format!("{}/api/tournaments", vps_base()))
        .send()
        .map_err(|e| format!("vps list_tournaments: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps list_tournaments: HTTP {}", resp.status()));
    }
    resp.json::<Vec<TournamentSummary>>()
        .map_err(|e| format!("vps list_tournaments parse: {e}"))
}

/// Create a VPS session for the white player of a tournament match.
/// Returns the session pubkey that white must include in the `create_game` instruction.
pub fn tournament_session_create_game(
    tournament_id: u64,
    game_id: u64,
    wallet_pubkey: &str,
) -> Result<String, String> {
    let resp = client()?
        .post(format!(
            "{}/api/tournament/{}/session-create-game",
            vps_base(),
            tournament_id
        ))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_create_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "tournament_session_create_game: HTTP {status} — {body}"
        ));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("tournament_session_create_game parse: {e}"))?;
    data.get("session_pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing session_pubkey in response".to_string())
}

/// Retrieve/create the VPS session for the black player of a tournament match.
/// Returns the same session pubkey that white already created.
pub fn tournament_session_join_game(
    tournament_id: u64,
    game_id: u64,
    wallet_pubkey: &str,
) -> Result<String, String> {
    let resp = client()?
        .post(format!(
            "{}/api/tournament/{}/session-join-game",
            vps_base(),
            tournament_id
        ))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_join_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "tournament_session_join_game: HTTP {status} — {body}"
        ));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("tournament_session_join_game parse: {e}"))?;
    data.get("session_pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing session_pubkey in response".to_string())
}

/// Join a tournament. Returns the slot (registration position) on success.
/// Where a player stands in a tournament. Mirrors the backend's
/// `PlayerState` (see `backend/src/signing/storage/tournament.rs`).
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq)]
pub enum PlayerState {
    NotRegistered,
    Registered,
    MatchReady,
    AwaitingGameId,
    AwaitingOpponent,
    Eliminated,
    Champion,
    Cancelled,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BlockedBy {
    pub round: u8,
    pub matches_remaining: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LastMatchResult {
    pub round: u8,
    pub won: bool,
    pub opponent: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MyMatch {
    pub match_index: u16,
    pub round: Option<u8>,
    pub board: Option<u16>,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
    pub is_bye: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MyTournamentStatus {
    pub state: PlayerState,
    pub registered: bool,
    pub round: Option<u8>,
    pub total_rounds: u8,
    #[serde(rename = "match")]
    pub my_match: Option<MyMatch>,
    pub blocked_by: Option<BlockedBy>,
    pub last_result: Option<LastMatchResult>,
    pub placing: Option<u8>,
    pub prize_lamports: Option<u64>,
}

/// Fetches the player's full tournament position in one call.
///
/// Supersedes `/my-match`, which returned a bare `{"found": false}` both for a
/// player who isn't in the tournament and for one who just won and is waiting
/// on their next opponent — indistinguishable, so the client rendered nothing
/// for either and a player who advanced was silently dropped to the menu.
pub fn my_tournament_status(
    tournament_id: u64,
    player_pubkey: &str,
) -> Result<MyTournamentStatus, String> {
    // Built inline rather than via reqwest's `query()`, which needs a feature
    // this build doesn't enable. A base58 pubkey is URL-safe by construction
    // (no reserved characters), so no escaping is required.
    let resp = client()?
        .get(format!(
            "{}/api/tournament/{}/my-status?player={}",
            vps_base(),
            tournament_id,
            player_pubkey
        ))
        .send()
        .map_err(|e| format!("vps my_tournament_status: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps my_tournament_status: HTTP {}", resp.status()));
    }
    resp.json::<MyTournamentStatus>()
        .map_err(|e| format!("vps my_tournament_status parse: {e}"))
}

/// Confirms an already-submitted, on-chain `register_player` transaction with
/// the backend, which adds the player to the roster and returns their slot.
///
/// This replaced `join_tournament`, which POSTed to `/join` and expected a
/// `slot` straight back. That endpoint used to insert into the store on the
/// strength of a self-declared pubkey alone — anyone who knew a tournament ID
/// could `curl` arbitrary wallets into any bracket at any claimed ELO without
/// paying an entry fee. It was rewritten to return an *unsigned* transaction
/// instead of registering anyone, so the old client call started failing with
/// "Missing slot in join response": the response simply no longer carries one.
///
/// The backend now re-reads `signature` from chain and verifies the player
/// actually signed a `register_player` for this tournament, at this ELO,
/// before touching the roster. `slot` comes back from that confirmation.
///
/// The caller must have submitted the on-chain registration first —
/// `solana::tournament::register_tournament` returns the signature to pass in.
pub fn confirm_join(
    tournament_id: u64,
    player_pubkey: &str,
    elo: u32,
    signature: &str,
    password: Option<&str>,
) -> Result<u32, String> {
    let mut body = serde_json::json!({
        "player": player_pubkey,
        "elo": elo,
        "signature": signature,
    });
    if let Some(pw) = password {
        body["password"] = serde_json::Value::String(pw.to_string());
    }
    let resp = client()?
        .post(format!(
            "{}/api/tournament/{}/confirm-join",
            vps_base(),
            tournament_id
        ))
        .json(&body)
        .send()
        .map_err(|e| format!("vps confirm_join: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        // 202 = tx not visible to the RPC yet. The registration is real and
        // will land; it just needs another moment before the backend can see
        // it, so this is worth retrying rather than surfacing as a failure.
        return Err(format!("vps confirm_join: HTTP {status} - {body}"));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("vps confirm_join parse: {e}"))?;
    data.get("slot")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| "Missing slot in confirm-join response".to_string())
}

/// `confirm_join` with a short retry loop for the 202 case above: the tx is
/// confirmed locally but the backend's RPC hasn't caught up yet. Without this
/// a player who genuinely paid and registered on-chain would see their join
/// fail purely on propagation timing.
pub fn confirm_join_with_retry(
    tournament_id: u64,
    player_pubkey: &str,
    elo: u32,
    signature: &str,
    password: Option<&str>,
) -> Result<u32, String> {
    let mut last_err = String::new();
    for attempt in 0..6 {
        match confirm_join(tournament_id, player_pubkey, elo, signature, password) {
            Ok(slot) => return Ok(slot),
            Err(e) => {
                last_err = e;
                // Only propagation-ish failures are worth retrying; a 403/409
                // (ELO out of range, already registered, tournament full) is
                // a definitive answer and retrying just delays the error.
                if !(last_err.contains("202") || last_err.contains("502")) {
                    return Err(last_err);
                }
                std::thread::sleep(std::time::Duration::from_millis(1500 * (attempt + 1)));
            }
        }
    }
    Err(format!("confirm_join gave up after retries: {last_err}"))
}
