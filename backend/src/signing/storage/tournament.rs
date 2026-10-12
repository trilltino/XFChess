//! Tournament records and their store.
//!
//! A record holds what every tournament has (players, fees, placings, prizes) plus
//! its format's own state in [`FormatState`]: a bracket for single elimination
//! ([`crate::signing::elimination`]) or rounds and standings for Swiss
//! ([`crate::signing::swiss`]). Format rules live in those modules; this one
//! dispatches to them.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub use crate::signing::elimination::{EliminationState, TournamentMatch};
pub use crate::signing::swiss::state::SwissStorageData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentFormat {
    SingleElimination,
    Swiss { rounds: u8 },
}

/// The format's own state. A record is one format or the other, never both.
///
/// Stored records and the public tournament API have always carried this as three
/// keys, `format`, `matches` (the bracket) and `swiss_data`; the record flattens it
/// back into exactly that shape, so older rows still load.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatState {
    Elimination(EliminationState),
    Swiss(SwissStorageData),
}

#[derive(Serialize)]
struct FormatWireRef<'a> {
    format: TournamentFormat,
    matches: &'a [TournamentMatch],
    swiss_data: Option<&'a SwissStorageData>,
}

#[derive(Deserialize)]
struct FormatWire {
    format: TournamentFormat,
    // Older rows padded an undrawn bracket with nulls.
    #[serde(default)]
    matches: Vec<Option<TournamentMatch>>,
    // Older rows left this empty until a Swiss event's first round was set up.
    #[serde(default)]
    swiss_data: Option<SwissStorageData>,
}

impl Serialize for FormatState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (matches, swiss_data): (&[TournamentMatch], _) = match self {
            Self::Elimination(bracket) => (&bracket.matches, None),
            Self::Swiss(sd) => (&[], Some(sd)),
        };
        FormatWireRef {
            format: self.format(),
            matches,
            swiss_data,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FormatState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FormatWire::deserialize(deserializer)?;
        Ok(match wire.format {
            TournamentFormat::SingleElimination => Self::Elimination(EliminationState {
                matches: wire.matches.into_iter().flatten().collect(),
            }),
            TournamentFormat::Swiss { rounds } => Self::Swiss(
                wire.swiss_data
                    .unwrap_or_else(|| SwissStorageData::new(rounds)),
            ),
        })
    }
}

impl FormatState {
    /// A format's state before the event starts.
    pub fn for_format(format: &TournamentFormat) -> Self {
        match format {
            TournamentFormat::SingleElimination => Self::Elimination(EliminationState::default()),
            TournamentFormat::Swiss { rounds } => Self::Swiss(SwissStorageData::new(*rounds)),
        }
    }

    pub fn format(&self) -> TournamentFormat {
        match self {
            Self::Elimination(_) => TournamentFormat::SingleElimination,
            Self::Swiss(sd) => TournamentFormat::Swiss {
                rounds: sd.total_rounds,
            },
        }
    }

    /// 0-based round in play, for either format.
    pub fn current_round(&self) -> Option<u8> {
        match self {
            Self::Elimination(bracket) => bracket.current_round(),
            Self::Swiss(sd) => sd.current_round_index(),
        }
    }

    pub fn total_rounds(&self) -> u8 {
        match self {
            Self::Elimination(bracket) => bracket.total_rounds(),
            Self::Swiss(sd) => sd.total_rounds,
        }
    }

    /// Undecided games in the 0-based `round`.
    pub fn open_matches_in(&self, round: u8) -> usize {
        match self {
            Self::Elimination(bracket) => bracket.open_matches_in(round),
            Self::Swiss(sd) => sd.open_boards_in(round),
        }
    }

    pub fn assignment_for(
        &self,
        tournament_id: u64,
        player: &str,
        node_ids: &HashMap<String, String>,
    ) -> Option<MatchAssignment> {
        match self {
            Self::Elimination(bracket) => bracket.assignment_for(player, node_ids),
            Self::Swiss(sd) => sd.assignment_for(tournament_id, player, node_ids),
        }
    }

    /// Games that can be played now: both players known, no result yet.
    pub fn open_games(&self, tournament_id: u64) -> Vec<AssignedGame> {
        match self {
            Self::Elimination(bracket) => bracket.open_games(tournament_id),
            Self::Swiss(sd) => sd.open_games(tournament_id),
        }
    }

    /// Every game the event has scheduled, including drawn bracket games that were
    /// replayed.
    pub fn game_ids(&self, tournament_id: u64) -> Vec<u64> {
        match self {
            Self::Elimination(bracket) => bracket.game_ids(),
            Self::Swiss(sd) => sd.game_ids(tournament_id),
        }
    }

    /// The 0-based round `game_id` belongs to, if it is one of this event's games.
    pub fn round_of_game(&self, tournament_id: u64, game_id: u64) -> Option<u8> {
        match self {
            Self::Elimination(bracket) => bracket.match_for_game(game_id).map(|m| m.round),
            Self::Swiss(sd) => sd.round_of_game(tournament_id, game_id),
        }
    }

    /// The bracket view's `matches` and 0-based `current_round` (255 once a bracket's
    /// final is decided).
    pub fn bracket_view(&self, tournament_id: u64) -> (serde_json::Value, serde_json::Value) {
        match self {
            Self::Elimination(bracket) => {
                let finished = bracket
                    .matches
                    .last()
                    .is_some_and(TournamentMatch::is_decided);
                let current_round = if finished {
                    255
                } else {
                    bracket.current_round().unwrap_or(0)
                };
                (
                    serde_json::to_value(&bracket.matches).unwrap_or_default(),
                    serde_json::json!(current_round),
                )
            }
            Self::Swiss(sd) => (
                sd.bracket_matches(tournament_id),
                serde_json::json!(sd.current_round.saturating_sub(1)),
            ),
        }
    }

    /// The bracket view's entry for the match or board `game_id` was played in.
    pub fn match_view(&self, tournament_id: u64, game_id: u64) -> Option<serde_json::Value> {
        match self {
            Self::Elimination(bracket) => {
                serde_json::to_value(bracket.match_for_game(game_id)?).ok()
            }
            Self::Swiss(sd) => match sd.bracket_matches(tournament_id) {
                serde_json::Value::Array(boards) => boards
                    .into_iter()
                    .find(|board| board["game_id"] == serde_json::json!(game_id)),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Cancelled,
}

impl TournamentStatus {
    /// The value of the `tournaments.status` column (the serde name).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Registration => "Registration",
            Self::Active => "Active",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchStatus {
    Pending,
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResultSource {
    OnChain,
    Oracle,
    Forfeit,
    DrawAgreed,
    /// Every game of a bracket match was drawn; the higher seed went through.
    SeedTiebreak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRecord {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub platform_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub status: TournamentStatus,
    #[serde(flatten)]
    pub state: FormatState,
    pub players: Vec<String>,
    pub player_elos: Vec<u32>,
    pub node_ids: HashMap<String, String>,
    pub winner: Option<String>,
    pub second_place: Option<String>,
    pub third_place: Option<String>,
    pub fourth_place: Option<String>,
    pub fifth_place: Option<String>,
    pub sixth_place: Option<String>,
    pub seventh_place: Option<String>,
    pub eighth_place: Option<String>,
    pub ninth_place: Option<String>,
    pub tenth_place: Option<String>,
    pub prize_shares: [u16; 10],
    pub elo_min: Option<u32>,
    pub elo_max: Option<u32>,
    pub min_players: Option<u16>,
    pub created_at: i64,
    pub scheduled_at: Option<i64>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    #[serde(default)]
    pub kyc_required: bool,
    pub password_hash: Option<String>,
    #[serde(default)]
    pub prizes_distributed: bool,
    #[serde(default)]
    pub prize_release_approved: bool,
    #[serde(default)]
    pub broadcast_delay_secs: u32,
    /// Game ids whose on-chain Game this backend has created (`create_tournament_game`).
    /// A settled game is closed on-chain, so existence alone cannot say "done".
    #[serde(default)]
    pub provisioned_game_ids: Vec<u64>,
}

impl TournamentRecord {
    pub fn new(tournament_id: u64, name: &str, entry_fee_lamports: u64) -> Self {
        Self {
            tournament_id,
            name: name.to_string(),
            entry_fee_lamports,
            ..Self::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_config(
        tournament_id: u64,
        name: String,
        entry_fee_lamports: u64,
        platform_fee_lamports: u64,
        max_players: u16,
        prize_shares: [u16; 10],
        format: TournamentFormat,
        elo_min: Option<u32>,
        elo_max: Option<u32>,
        min_players: Option<u16>,
        scheduled_at: Option<i64>,
        kyc_required: bool,
    ) -> Self {
        Self {
            tournament_id,
            name,
            entry_fee_lamports,
            platform_fee_lamports,
            max_players,
            state: FormatState::for_format(&format),
            players: Vec::with_capacity(max_players as usize),
            player_elos: Vec::with_capacity(max_players as usize),
            prize_shares,
            elo_min,
            elo_max,
            min_players,
            scheduled_at,
            kyc_required,
            ..Self::default()
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= self.max_players as usize
    }

    pub fn format(&self) -> TournamentFormat {
        self.state.format()
    }

    pub fn is_swiss(&self) -> bool {
        matches!(self.state, FormatState::Swiss(_))
    }

    /// Results still to mirror on-chain: decided bracket matches not yet synced,
    /// or a Swiss event whose on-chain completion has not landed. A bracket's
    /// final is mirrored after the event completes, so this outlives `Active`.
    pub fn chain_sync_pending(&self) -> bool {
        matches!(
            self.status,
            TournamentStatus::Active | TournamentStatus::Completed
        ) && match &self.state {
            FormatState::Elimination(bracket) => bracket.unsynced().next().is_some(),
            FormatState::Swiss(sd) => !sd.chain_completed,
        }
    }

    /// Completed with a prize pool and a winner, and not paid out yet.
    pub fn awaiting_prizes(&self) -> bool {
        self.status == TournamentStatus::Completed
            && !self.prizes_distributed
            && self.prize_pool > 0
            && self.winner.is_some()
    }

    pub fn swiss(&self) -> Option<&SwissStorageData> {
        match &self.state {
            FormatState::Swiss(sd) => Some(sd),
            FormatState::Elimination(_) => None,
        }
    }

    pub fn swiss_mut(&mut self) -> Option<&mut SwissStorageData> {
        match &mut self.state {
            FormatState::Swiss(sd) => Some(sd),
            FormatState::Elimination(_) => None,
        }
    }

    pub fn elimination(&self) -> Option<&EliminationState> {
        match &self.state {
            FormatState::Elimination(bracket) => Some(bracket),
            FormatState::Swiss(_) => None,
        }
    }

    /// 0-based round in play. `None` before a Swiss event pairs round 1 and once a
    /// bracket is decided.
    pub fn current_round(&self) -> Option<u8> {
        self.state.current_round()
    }

    /// Undecided games in the 0-based `round`.
    pub fn matches_remaining_in_round(&self, round: u8) -> usize {
        self.state.open_matches_in(round)
    }

    pub fn total_rounds(&self) -> u8 {
        self.state.total_rounds()
    }

    /// Players best first: highest rating, earlier registration on a tie. Brackets
    /// are drawn and draws are broken in this order.
    pub fn seeding(&self) -> Vec<(String, u32)> {
        let mut seeded: Vec<(String, u32)> = self
            .players
            .iter()
            .cloned()
            .zip(self.player_elos.iter().copied())
            .collect();
        seeded.sort_by_key(|&(_, elo)| std::cmp::Reverse(elo));
        seeded
    }

    /// Seeds the players and sets the event going: a bracket is drawn, a Swiss event
    /// waits for its first round to be paired. A drawn bracket is never redrawn, so a
    /// repeated start cannot wipe results.
    pub fn start(&mut self) -> Result<(), String> {
        if matches!(
            self.status,
            TournamentStatus::Completed | TournamentStatus::Cancelled
        ) {
            return Err(format!("Tournament is {:?}", self.status));
        }
        if self.elimination().is_some_and(EliminationState::is_drawn) {
            return Err("Bracket already drawn".to_string());
        }
        if self.is_swiss() && self.players.len() < usize::from(self.min_players.unwrap_or(2)) {
            return Err("Not enough players for Swiss tournament".to_string());
        }
        let (players, elos) = self.seeding().into_iter().unzip();
        self.players = players;
        self.player_elos = elos;
        if let FormatState::Elimination(bracket) = &mut self.state {
            *bracket = EliminationState::draw(self.tournament_id, &self.players);
        }
        self.status = TournamentStatus::Active;
        self.started_at = Some(chrono::Utc::now().timestamp());
        Ok(())
    }

    fn placing_for(&self, player: &str) -> Option<u8> {
        let places = [
            &self.winner,
            &self.second_place,
            &self.third_place,
            &self.fourth_place,
            &self.fifth_place,
            &self.sixth_place,
            &self.seventh_place,
            &self.eighth_place,
            &self.ninth_place,
            &self.tenth_place,
        ];
        places
            .iter()
            .position(|p| p.as_deref() == Some(player))
            .map(|i| (i + 1) as u8)
    }

    pub fn player_status(&self, player: &str) -> PlayerTournamentStatus {
        let registered = self.players.iter().any(|p| p == player);
        let current_round = self.current_round();
        let mut status = PlayerTournamentStatus {
            state: PlayerState::NotRegistered,
            registered,
            tournament_status: self.status.clone(),
            round: current_round,
            total_rounds: self.total_rounds(),
            r#match: None,
            blocked_by: None,
            last_result: self
                .elimination()
                .and_then(|bracket| bracket.last_result_for(player)),
            placing: None,
            prize_lamports: None,
        };

        if !registered {
            return status;
        }

        match self.status {
            TournamentStatus::Registration => {
                status.state = PlayerState::Registered;
                return status;
            }
            TournamentStatus::Cancelled => {
                status.state = PlayerState::Cancelled;
                return status;
            }
            TournamentStatus::Completed => {
                let placing = self.placing_for(player);
                status.placing = placing;
                status.prize_lamports = placing.map(|p| self.calculate_prize(p));
                status.state = if placing == Some(1) {
                    PlayerState::Champion
                } else {
                    PlayerState::Eliminated
                };
                return status;
            }
            TournamentStatus::Active => {}
        }

        let assignment = self.match_for_player(player);
        match &self.state {
            // Swiss players are never knocked out, only withdrawn.
            FormatState::Swiss(sd) if sd.is_withdrawn(player) => {
                status.state = PlayerState::Eliminated;
                return status;
            }
            FormatState::Elimination(bracket)
                if assignment.is_none() && bracket.is_knocked_out(player) =>
            {
                status.state = PlayerState::Eliminated;
                status.placing = self.placing_for(player);
                status.prize_lamports = status.placing.map(|p| self.calculate_prize(p));
                return status;
            }
            _ => {}
        }

        if let Some(seat) = assignment.as_ref().filter(|a| !a.is_bye) {
            status.state = if seat.game_id.is_some() {
                PlayerState::MatchReady
            } else {
                // Both players known but the game is not scheduled yet.
                PlayerState::AwaitingGameId
            };
            status.r#match = assignment;
            return status;
        }

        // Between games: a bracket winner waiting for the next opponent, or a Swiss
        // player (or bye) waiting for the round to finish.
        status.state = PlayerState::AwaitingOpponent;
        status.r#match = assignment;
        if let Some(round) = current_round {
            status.blocked_by = Some(BlockedBy {
                round,
                matches_remaining: self.matches_remaining_in_round(round),
            });
        }
        status
    }

    pub fn match_for_player(&self, player: &str) -> Option<MatchAssignment> {
        self.state
            .assignment_for(self.tournament_id, player, &self.node_ids)
    }

    /// Lamports `place` (1-based, up to tenth) wins from the prize pool.
    pub fn calculate_prize(&self, place: u8) -> u64 {
        let Some(&share_bps) = usize::from(place)
            .checked_sub(1)
            .and_then(|i| self.prize_shares.get(i))
        else {
            return 0;
        };
        let prize = u128::from(self.prize_pool) * u128::from(share_bps) / 10_000;
        u64::try_from(prize).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedBy {
    pub round: u8,
    pub matches_remaining: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastMatchResult {
    pub round: u8,
    pub won: bool,
    pub opponent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerTournamentStatus {
    pub state: PlayerState,
    pub registered: bool,
    pub tournament_status: TournamentStatus,
    pub round: Option<u8>,
    pub total_rounds: u8,
    pub r#match: Option<MatchAssignment>,
    pub blocked_by: Option<BlockedBy>,
    pub last_result: Option<LastMatchResult>,
    pub placing: Option<u8>,
    pub prize_lamports: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAssignment {
    pub match_index: u16,
    pub round: Option<u8>,
    pub board: Option<u16>,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
    pub status: MatchStatus,
    pub is_bye: bool,
}

/// A game a tournament has scheduled: both players known, no result yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedGame {
    pub tournament_id: u64,
    pub game_id: u64,
    pub white: String,
    pub black: String,
}

/// How a tournament game ended, as the settlement worker reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOutcome {
    WhiteWin,
    BlackWin,
    Draw,
}

#[derive(Clone)]
pub struct TournamentStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationTransaction {
    pub tournament_id: u64,
    pub player: String,
    pub elo: u32,
    pub signature: String,
    pub confirmed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentTransaction {
    pub tournament_id: u64,
    pub signature: String,
    pub operation: String,
    pub status: String,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
}

/// The record is the JSON in `data`. `status`, `chain_sync_pending` and
/// `prizes_pending` are derived from it on every write ([`IndexColumns`]), so the
/// background loops select only the events they act on instead of parsing every
/// tournament ever held.
const TOURNAMENTS_SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS tournaments (
        id                 INTEGER PRIMARY KEY,
        data               TEXT    NOT NULL,
        updated_at         INTEGER NOT NULL DEFAULT 0,
        status             TEXT,
        chain_sync_pending INTEGER NOT NULL DEFAULT 0,
        prizes_pending     INTEGER NOT NULL DEFAULT 0
    )",
    // Rows from before these columns existed get them NULL / 0 and are filled
    // in by `backfill_index_columns`.
    "ALTER TABLE tournaments ADD COLUMN status TEXT",
    "ALTER TABLE tournaments ADD COLUMN chain_sync_pending INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE tournaments ADD COLUMN prizes_pending INTEGER NOT NULL DEFAULT 0",
    "CREATE INDEX IF NOT EXISTS idx_tournaments_status ON tournaments (status)",
    "CREATE INDEX IF NOT EXISTS idx_tournaments_chain_sync ON tournaments (id) WHERE chain_sync_pending = 1",
    "CREATE INDEX IF NOT EXISTS idx_tournaments_prizes ON tournaments (id) WHERE prizes_pending = 1",
];

/// The columns stored beside a record's JSON for filtering.
struct IndexColumns {
    status: &'static str,
    chain_sync_pending: bool,
    prizes_pending: bool,
}

impl IndexColumns {
    fn of(t: &TournamentRecord) -> Self {
        Self {
            status: t.status.as_str(),
            chain_sync_pending: t.chain_sync_pending(),
            prizes_pending: t.awaiting_prizes(),
        }
    }
}

/// Parses a stored record, logging one that no longer parses instead of letting
/// it vanish from every query.
fn decode(id: i64, data: &str) -> Option<TournamentRecord> {
    serde_json::from_str(data)
        .inspect_err(|e| tracing::error!("[tournament-store] tournament {id} is unreadable: {e}"))
        .ok()
}

/// Fills the index columns of rows written before they existed. Runs at every
/// start; after the first it finds nothing to do.
async fn backfill_index_columns(pool: &SqlitePool) {
    let rows: Vec<(i64, String)> =
        match sqlx::query_as("SELECT id, data FROM tournaments WHERE status IS NULL")
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!("[tournament-store] backfill query failed: {e}");
                return;
            }
        };
    let mut filled = 0usize;
    for (id, data) in rows {
        let Some(record) = decode(id, &data) else {
            continue;
        };
        let columns = IndexColumns::of(&record);
        let updated = sqlx::query(
            "UPDATE tournaments SET status = ?, chain_sync_pending = ?, prizes_pending = ?
             WHERE id = ? AND status IS NULL",
        )
        .bind(columns.status)
        .bind(columns.chain_sync_pending)
        .bind(columns.prizes_pending)
        .bind(id)
        .execute(pool)
        .await;
        match updated {
            Ok(_) => filled += 1,
            Err(e) => tracing::error!("[tournament-store] backfill of tournament {id} failed: {e}"),
        }
    }
    if filled > 0 {
        tracing::info!("[tournament-store] indexed {filled} existing tournament(s)");
    }
}

impl TournamentStore {
    pub async fn new(pool: SqlitePool) -> Self {
        crate::signing::tournament_operations::init(&pool)
            .await
            .expect("tournament operations storage must be available");
        for statement in TOURNAMENTS_SCHEMA {
            if let Err(e) = sqlx::query(*statement).execute(&pool).await {
                if !e.to_string().contains("duplicate column") {
                    tracing::error!("[tournament-store] schema statement failed: {e}");
                }
            }
        }
        backfill_index_columns(&pool).await;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tournament_registration_transactions (
                tournament_id INTEGER NOT NULL,
                player TEXT NOT NULL,
                elo INTEGER NOT NULL,
                signature TEXT NOT NULL,
                confirmed_at INTEGER NOT NULL,
                PRIMARY KEY (tournament_id, player)
            );",
        )
        .execute(&pool)
        .await
        .ok();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tournament_transactions (
                tournament_id INTEGER NOT NULL,
                signature TEXT PRIMARY KEY,
                operation TEXT NOT NULL,
                status TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                next_retry_at INTEGER,
                created_at INTEGER NOT NULL
            );",
        )
        .execute(&pool)
        .await
        .ok();
        tracing::info!("[tournament-store] SQLite table ready");
        Self { pool }
    }

    pub async fn record_registration_transaction(&self, tx: RegistrationTransaction) -> bool {
        sqlx::query(
            "INSERT OR REPLACE INTO tournament_registration_transactions
             (tournament_id, player, elo, signature, confirmed_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(tx.tournament_id as i64)
        .bind(tx.player)
        .bind(tx.elo as i64)
        .bind(tx.signature)
        .bind(tx.confirmed_at)
        .execute(&self.pool)
        .await
        .is_ok()
    }

    pub async fn record_transaction(&self, tx: TournamentTransaction) -> bool {
        sqlx::query(
            "INSERT OR REPLACE INTO tournament_transactions
             (tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(tx.tournament_id as i64)
        .bind(tx.signature)
        .bind(tx.operation)
        .bind(tx.status)
        .bind(tx.retry_count as i64)
        .bind(tx.last_error)
        .bind(tx.next_retry_at)
        .bind(tx.created_at)
        .execute(&self.pool)
        .await
        .is_ok()
    }

    pub async fn transactions(&self, id: u64) -> Vec<TournamentTransaction> {
        sqlx::query_as::<_, (i64, String, String, String, i64, Option<String>, Option<i64>, i64)>(
            "SELECT tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at
             FROM tournament_transactions WHERE tournament_id = ? ORDER BY created_at DESC",
        )
        .bind(id as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at)| TournamentTransaction {
            tournament_id: tournament_id as u64,
            signature,
            operation,
            status,
            retry_count: retry_count as u32,
            last_error,
            next_retry_at,
            created_at,
        })
        .collect()
    }

    pub async fn registration_transactions(&self, id: u64) -> Vec<RegistrationTransaction> {
        sqlx::query_as::<_, (i64, String, i64, String, i64)>(
            "SELECT tournament_id, player, elo, signature, confirmed_at
             FROM tournament_registration_transactions WHERE tournament_id = ?
             ORDER BY confirmed_at ASC",
        )
        .bind(id as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(
            |(tournament_id, player, elo, signature, confirmed_at)| RegistrationTransaction {
                tournament_id: tournament_id as u64,
                player,
                elo: elo as u32,
                signature,
                confirmed_at,
            },
        )
        .collect()
    }

    pub async fn create(&self, record: TournamentRecord) {
        let _ = self.create_checked(record).await;
    }

    pub async fn create_checked(&self, record: TournamentRecord) -> Result<(), sqlx::Error> {
        let data = serde_json::to_string(&record).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
        let columns = IndexColumns::of(&record);
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO tournaments (id, data, updated_at, status, chain_sync_pending, prizes_pending)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(record.tournament_id as i64)
        .bind(&data)
        .bind(now)
        .bind(columns.status)
        .bind(columns.chain_sync_pending)
        .bind(columns.prizes_pending)
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    pub async fn get(&self, id: u64) -> Option<TournamentRecord> {
        let id = id as i64;
        let data: String = sqlx::query_scalar("SELECT data FROM tournaments WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .ok()??;
        decode(id, &data)
    }

    /// Every tournament ever held. For on-demand lookups only: a polling loop
    /// selects what it acts on with [`Self::with_status`],
    /// [`Self::chain_sync_pending`] or [`Self::awaiting_prizes`].
    pub async fn list(&self) -> Vec<TournamentRecord> {
        self.select(sqlx::query_as("SELECT id, data FROM tournaments"))
            .await
    }

    pub async fn with_status(&self, status: TournamentStatus) -> Vec<TournamentRecord> {
        self.select(
            sqlx::query_as("SELECT id, data FROM tournaments WHERE status = ?")
                .bind(status.as_str()),
        )
        .await
    }

    /// Events with results still to mirror on-chain
    /// ([`TournamentRecord::chain_sync_pending`]).
    pub async fn chain_sync_pending(&self) -> Vec<TournamentRecord> {
        self.select(sqlx::query_as(
            "SELECT id, data FROM tournaments WHERE chain_sync_pending = 1",
        ))
        .await
    }

    /// Completed events whose prizes are not paid out yet
    /// ([`TournamentRecord::awaiting_prizes`]).
    pub async fn awaiting_prizes(&self) -> Vec<TournamentRecord> {
        self.select(sqlx::query_as(
            "SELECT id, data FROM tournaments WHERE prizes_pending = 1",
        ))
        .await
    }

    /// The tournament `game_id` was scheduled by. A bracket or Swiss game id
    /// carries its tournament's id above the low 16 bits; a game id an admin
    /// assigned by hand is looked for among running events.
    pub async fn find_by_game(&self, game_id: u64) -> Option<TournamentRecord> {
        let owns = |t: &TournamentRecord| t.state.round_of_game(t.tournament_id, game_id).is_some();
        if let Some(t) = self.get(game_id >> 16).await.filter(owns) {
            return Some(t);
        }
        self.with_status(TournamentStatus::Active)
            .await
            .into_iter()
            .find(owns)
    }

    async fn select<'q>(
        &self,
        query: sqlx::query::QueryAs<'q, sqlx::Sqlite, (i64, String), sqlx::sqlite::SqliteArguments>,
    ) -> Vec<TournamentRecord> {
        match query.fetch_all(&self.pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|(id, data)| decode(id, &data))
                .collect(),
            Err(e) => {
                tracing::error!("[tournament-store] tournament query failed: {e}");
                Vec::new()
            }
        }
    }

    pub async fn visible(&self) -> Result<Vec<TournamentRecord>, sqlx::Error> {
        let data: Vec<String> = sqlx::query_scalar("SELECT data FROM tournaments WHERE id NOT IN (SELECT tournament_id FROM archived_tournaments) ORDER BY id DESC")
            .fetch_all(&self.pool).await?;
        data.into_iter()
            .map(|data| serde_json::from_str(&data).map_err(|e| sqlx::Error::Decode(Box::new(e))))
            .collect()
    }

    pub async fn delete(&self, id: u64) -> bool {
        sqlx::query("INSERT OR IGNORE INTO archived_tournaments (tournament_id, archived_at) VALUES (?, unixepoch())")
            .bind(id as i64)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }

    pub async fn update<F: FnOnce(&mut TournamentRecord)>(&self, id: u64, f: F) -> bool {
        self.update_checked(id, f).await.is_ok()
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn update_checked<F: FnOnce(&mut TournamentRecord)>(
        &self,
        id: u64,
        f: F,
    ) -> Result<(), sqlx::Error> {
        // Acquire the SQLite write lock before reading the JSON record. Otherwise
        // simultaneous registrations/results can silently replace each other.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let data: Option<String> = sqlx::query_scalar("SELECT data FROM tournaments WHERE id = ?")
            .bind(id as i64)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(data) = data {
            let mut record: TournamentRecord =
                serde_json::from_str(&data).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
            f(&mut record);
            let data =
                serde_json::to_string(&record).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
            let columns = IndexColumns::of(&record);
            let now = chrono::Utc::now().timestamp();
            sqlx::query(
                "UPDATE tournaments
                 SET data = ?, updated_at = ?, status = ?, chain_sync_pending = ?, prizes_pending = ?
                 WHERE id = ?",
            )
            .bind(data)
            .bind(now)
            .bind(columns.status)
            .bind(columns.chain_sync_pending)
            .bind(columns.prizes_pending)
            .bind(id as i64)
            .execute(&mut *tx)
            .await?;
            tx.commit().await
        } else {
            Err(sqlx::Error::RowNotFound)
        }
    }

    pub async fn register_node_id(&self, id: u64, player: String, node_id: String) -> bool {
        self.update(id, |t| {
            t.node_ids.insert(player, node_id);
        })
        .await
    }

    pub async fn leave_tournament(&self, id: u64, player: &str) -> bool {
        self.update(id, |t| {
            if let Some(pos) = t.players.iter().position(|p| p == player) {
                t.players.remove(pos);
                t.player_elos.remove(pos);
                if t.prize_pool >= t.entry_fee_lamports {
                    t.prize_pool -= t.entry_fee_lamports;
                }
            }
        })
        .await
    }

    pub fn deterministic_game_id(tournament_id: u64, match_index: u16) -> Option<u64> {
        (tournament_id < (1u64 << 48)).then_some((tournament_id << 16) | u64::from(match_index))
    }

    /// A bracket match's game id. `attempt` counts the match's drawn games, so each
    /// replay gets a fresh id (`attempt << 8 | match_index`; a bracket has at most 255
    /// matches). A tournament is one format, so this never meets a Swiss id.
    pub fn bracket_game_id(tournament_id: u64, match_index: u16, attempt: u8) -> Option<u64> {
        if match_index > 0xff {
            return None;
        }
        Self::deterministic_game_id(tournament_id, (u16::from(attempt) << 8) | match_index)
    }

    /// Swiss boards reuse the deterministic scheme with `match_index = round << 8 |
    /// board` (1-based round and board; at most 255 boards per round). A tournament
    /// is either Swiss or a bracket, so the two layouts never share an id space.
    pub fn swiss_game_id(tournament_id: u64, round: u8, board: u16) -> Option<u64> {
        if round == 0 || board == 0 || board > 0xff {
            return None;
        }
        Self::deterministic_game_id(tournament_id, (u16::from(round) << 8) | board)
    }

    /// Inverse of [`Self::swiss_game_id`]: the `(round, board)` a game id belongs to,
    /// if it is one of `tournament_id`'s Swiss games.
    pub fn swiss_board_for_game_id(tournament_id: u64, game_id: u64) -> Option<(u8, u16)> {
        if game_id >> 16 != tournament_id {
            return None;
        }
        let match_index = (game_id & 0xffff) as u16;
        let (round, board) = ((match_index >> 8) as u8, match_index & 0xff);
        (round > 0 && board > 0).then_some((round, board))
    }

    pub async fn update_status(&self, id: u64, status: TournamentStatus) -> bool {
        self.update(id, |t| {
            t.status = status;
        })
        .await
    }

    /// Seeds the players and starts the event ([`TournamentRecord::start`]).
    pub async fn start_tournament(&self, id: u64) -> Result<(), String> {
        let mut outcome = Err("Tournament not found".to_string());
        self.update(id, |t| outcome = t.start()).await;
        outcome
    }
}

impl Default for TournamentRecord {
    fn default() -> Self {
        Self {
            tournament_id: 0,
            name: String::new(),
            entry_fee_lamports: 0,
            platform_fee_lamports: 4_000_000, // 50p
            prize_pool: 0,
            max_players: 8,
            status: TournamentStatus::Registration,
            state: FormatState::for_format(&TournamentFormat::SingleElimination),
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            winner: None,
            second_place: None,
            third_place: None,
            fourth_place: None,
            fifth_place: None,
            sixth_place: None,
            seventh_place: None,
            eighth_place: None,
            ninth_place: None,
            tenth_place: None,
            password_hash: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            min_players: None,
            prize_shares: [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            elo_min: None,
            elo_max: None,
            created_at: chrono::Utc::now().timestamp(),
            kyc_required: false,
            prizes_distributed: false,
            prize_release_approved: false,
            broadcast_delay_secs: 0,
            provisioned_game_ids: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with_players(tournament_id: u64, n: usize) -> TournamentRecord {
        let mut t = TournamentRecord::new(tournament_id, "test", 0);
        t.max_players = n as u16;
        for i in 0..n {
            t.players.push(format!("P{i}"));
            t.player_elos.push(2000 - i as u32); // P0 highest seed
        }
        t
    }

    fn started_bracket(tournament_id: u64, n: usize) -> TournamentRecord {
        let mut t = record_with_players(tournament_id, n);
        t.start().unwrap();
        t
    }

    fn swiss_record(tournament_id: u64) -> TournamentRecord {
        let mut t = record_with_players(tournament_id, 5);
        let mut sd = SwissStorageData::new(3);
        sd.current_round = 1;
        sd.rounds = vec![swiss_pairing::SwissRound {
            round: 1,
            pairings: vec![
                swiss_pairing::Pairing {
                    white: "P0".into(),
                    black: "P2".into(),
                    board: 1,
                },
                swiss_pairing::Pairing {
                    white: "P3".into(),
                    black: "P1".into(),
                    board: 2,
                },
            ],
            byes: vec!["P4".into()],
            float_downs: vec![],
            float_ups: vec![],
        }];
        t.state = FormatState::Swiss(sd);
        t.status = TournamentStatus::Active;
        t
    }

    /// `t` as an older row stored it: an undrawn bracket padded with nulls and no
    /// Swiss data before round 1.
    fn legacy_json(t: &TournamentRecord, padding: usize) -> String {
        let mut v = serde_json::to_value(t).unwrap();
        v["matches"] = serde_json::Value::Array(vec![serde_json::Value::Null; padding]);
        v["swiss_data"] = serde_json::Value::Null;
        v.to_string()
    }

    #[test]
    fn older_bracket_rows_load_and_save_in_the_same_shape() {
        let t = record_with_players(1, 8);
        let loaded: TournamentRecord = serde_json::from_str(&legacy_json(&t, 7)).unwrap();
        assert_eq!(loaded.format(), TournamentFormat::SingleElimination);
        assert_eq!(loaded.elimination(), Some(&EliminationState::default()));

        let saved = serde_json::to_value(started_bracket(1, 8)).unwrap();
        assert_eq!(saved["format"], serde_json::json!("SingleElimination"));
        assert_eq!(saved["matches"].as_array().unwrap().len(), 7);
        assert!(saved["swiss_data"].is_null());
        let again: TournamentRecord = serde_json::from_value(saved).unwrap();
        assert_eq!(again.state, started_bracket(1, 8).state);
    }

    #[test]
    fn older_swiss_rows_without_swiss_data_keep_their_round_count() {
        let mut t = record_with_players(2, 6);
        t.state = FormatState::for_format(&TournamentFormat::Swiss { rounds: 5 });
        let loaded: TournamentRecord = serde_json::from_str(&legacy_json(&t, 5)).unwrap();
        assert_eq!(loaded.format(), TournamentFormat::Swiss { rounds: 5 });
        assert_eq!(loaded.swiss().map(|sd| sd.total_rounds), Some(5));

        let saved = serde_json::to_value(swiss_record(2)).unwrap();
        assert_eq!(
            saved["format"],
            serde_json::json!({ "Swiss": { "rounds": 3 } })
        );
        assert_eq!(saved["matches"], serde_json::json!([]));
        assert_eq!(saved["swiss_data"]["current_round"], serde_json::json!(1));
    }

    #[test]
    fn starting_a_bracket_seeds_by_rating_and_schedules_round_one() {
        let mut t = record_with_players(3, 4);
        t.players.reverse();
        t.player_elos.reverse();
        t.start().unwrap();
        assert_eq!(t.players, vec!["P0", "P1", "P2", "P3"]);
        assert_eq!(t.status, TournamentStatus::Active);
        let ready: Vec<_> = t
            .state
            .open_games(3)
            .into_iter()
            .map(|g| (g.white, g.black))
            .collect();
        assert_eq!(
            ready,
            vec![("P0".into(), "P3".into()), ("P1".into(), "P2".into())]
        );
    }

    #[test]
    fn a_swiss_event_needs_its_minimum_to_start_and_pairs_later() {
        let mut t = record_with_players(4, 3);
        t.state = FormatState::for_format(&TournamentFormat::Swiss { rounds: 3 });
        t.min_players = Some(4);
        assert!(t.start().is_err());
        t.min_players = Some(3);
        t.start().unwrap();
        assert_eq!(
            t.current_round(),
            None,
            "round 1 is paired by the Swiss service"
        );
        assert!(t.state.open_games(4).is_empty());
    }

    #[test]
    fn game_ids_are_unique_per_match_replay_and_board() {
        assert_eq!(
            TournamentStore::deterministic_game_id(42, 7),
            Some((42 << 16) | 7)
        );
        assert_eq!(TournamentStore::deterministic_game_id(1 << 48, 0), None);
        assert_eq!(
            TournamentStore::bracket_game_id(42, 7, 0),
            Some((42 << 16) | 7)
        );
        assert_ne!(
            TournamentStore::bracket_game_id(42, 7, 0),
            TournamentStore::bracket_game_id(42, 7, 1)
        );
        assert_eq!(TournamentStore::bracket_game_id(42, 256, 0), None);

        let id = TournamentStore::swiss_game_id(7, 3, 12).unwrap();
        assert_eq!(
            TournamentStore::swiss_board_for_game_id(7, id),
            Some((3, 12))
        );
        assert_eq!(TournamentStore::swiss_board_for_game_id(8, id), None);
        assert_eq!(TournamentStore::swiss_game_id(7, 0, 1), None);
        assert_eq!(TournamentStore::swiss_game_id(7, 1, 0), None);
        assert_eq!(TournamentStore::swiss_game_id(7, 1, 256), None);
    }

    #[test]
    fn every_format_lists_its_games_and_their_rounds() {
        let swiss = swiss_record(9);
        let board_two = TournamentStore::swiss_game_id(9, 1, 2).unwrap();
        assert_eq!(swiss.state.game_ids(9).len(), 2);
        assert_eq!(swiss.state.round_of_game(9, board_two), Some(0));
        assert_eq!(swiss.state.round_of_game(9, 12345), None);

        let bracket = started_bracket(10, 4);
        let first = bracket.state.open_games(10)[0].game_id;
        assert_eq!(
            bracket.state.game_ids(10).len(),
            2,
            "the final has no game yet"
        );
        assert_eq!(bracket.state.round_of_game(10, first), Some(0));
    }

    #[test]
    fn every_paid_place_gets_its_share() {
        let mut t = record_with_players(5, 4);
        t.prize_pool = 1_000_000;
        t.prize_shares = [4000, 2000, 1000, 1000, 500, 500, 300, 300, 200, 200];
        assert_eq!(t.calculate_prize(1), 400_000);
        assert_eq!(t.calculate_prize(5), 50_000);
        assert_eq!(t.calculate_prize(10), 20_000);
        assert_eq!(t.calculate_prize(0), 0);
        assert_eq!(t.calculate_prize(11), 0);
    }

    #[test]
    fn bracket_players_see_their_match_then_the_next_one() {
        let mut t = started_bracket(1, 4);
        let ready = t.player_status("P0");
        assert_eq!(ready.state, PlayerState::MatchReady);
        assert!(ready.r#match.unwrap().game_id.is_some());

        let FormatState::Elimination(bracket) = &mut t.state else {
            unreachable!()
        };
        bracket
            .decide(1, 0, "P0", "P3", ResultSource::OnChain)
            .unwrap();

        let advanced = t.player_status("P0");
        assert_eq!(advanced.state, PlayerState::AwaitingOpponent);
        let blocked = advanced.blocked_by.expect("waits on the other semifinal");
        assert_eq!((blocked.round, blocked.matches_remaining), (0, 1));
        let last = advanced.last_result.expect("recalls the win");
        assert!(last.won && last.opponent == "P3");

        assert_eq!(t.player_status("P3").state, PlayerState::Eliminated);
        let stranger = t.player_status("NOT_A_PLAYER");
        assert_eq!(stranger.state, PlayerState::NotRegistered);
        assert!(stranger.blocked_by.is_none());
    }

    #[test]
    fn registration_and_terminal_states() {
        let mut t = record_with_players(3, 4);
        assert_eq!(t.player_status("P0").state, PlayerState::Registered);

        t.start().unwrap();
        t.status = TournamentStatus::Completed;
        t.winner = Some("P1".into());
        t.second_place = Some("P0".into());
        t.prize_pool = 1_000_000;
        t.prize_shares = [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0];

        let champ = t.player_status("P1");
        assert_eq!(champ.state, PlayerState::Champion);
        assert_eq!(
            (champ.placing, champ.prize_lamports),
            (Some(1), Some(600_000))
        );
        let runner_up = t.player_status("P0");
        assert_eq!(runner_up.state, PlayerState::Eliminated);
        assert_eq!(
            (runner_up.placing, runner_up.prize_lamports),
            (Some(2), Some(300_000))
        );

        t.status = TournamentStatus::Cancelled;
        assert_eq!(t.player_status("P0").state, PlayerState::Cancelled);
    }

    #[test]
    fn sixteen_player_bracket_reports_round_progress() {
        let mut t = started_bracket(4, 16);
        assert_eq!(t.total_rounds(), 4);
        let FormatState::Elimination(bracket) = &mut t.state else {
            unreachable!()
        };
        for i in 0..8u16 {
            let (white, black) = {
                let m = &bracket.matches[i as usize];
                (
                    m.player_white.clone().unwrap(),
                    m.player_black.clone().unwrap(),
                )
            };
            bracket
                .decide(4, i, &white, &black, ResultSource::OnChain)
                .unwrap();
        }
        assert_eq!(t.current_round(), Some(1));
        assert_eq!(t.matches_remaining_in_round(1), 4);
        let s = t.player_status("P0");
        assert_eq!(s.state, PlayerState::MatchReady);
        assert_eq!((s.round, s.total_rounds), (Some(1), 4));
    }

    #[test]
    fn swiss_players_get_their_board_game_until_it_is_decided() {
        let mut t = swiss_record(9);

        let white = t.player_status("P0");
        assert_eq!(white.state, PlayerState::MatchReady);
        assert_eq!(
            white.round,
            Some(0),
            "round is reported 0-based like brackets"
        );
        let m = white.r#match.unwrap();
        assert_eq!(m.round, Some(0), "and so is the board's round");
        assert_eq!(m.your_color, "white");
        assert_eq!(m.game_id, TournamentStore::swiss_game_id(9, 1, 1));
        assert_eq!(t.player_status("P1").r#match.unwrap().your_color, "black");

        // The bye waits for the round instead of being sent into a game.
        let bye = t.player_status("P4");
        assert_eq!(bye.state, PlayerState::AwaitingOpponent);
        assert_eq!(bye.blocked_by.unwrap().matches_remaining, 2);

        // Once board 1 is decided its players wait for board 2.
        t.swiss_mut()
            .unwrap()
            .results
            .push((1, 1, swiss_pairing::MatchResult::Draw));
        let done = t.player_status("P0");
        assert_eq!(done.state, PlayerState::AwaitingOpponent);
        assert_eq!(done.blocked_by.unwrap().matches_remaining, 1);
        assert_eq!(t.player_status("P3").state, PlayerState::MatchReady);
    }

    #[test]
    fn withdrawn_swiss_player_is_out() {
        let mut t = swiss_record(9);
        t.swiss_mut().unwrap().withdrawn_players.push("P2".into());
        assert_eq!(t.player_status("P2").state, PlayerState::Eliminated);
    }

    /// One connection: each `sqlite::memory:` connection is its own database.
    async fn memory_pool() -> SqlitePool {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap()
    }

    fn ids(records: Vec<TournamentRecord>) -> Vec<u64> {
        let mut ids: Vec<u64> = records.iter().map(|t| t.tournament_id).collect();
        ids.sort_unstable();
        ids
    }

    #[tokio::test]
    async fn each_loop_sees_only_the_events_it_acts_on() {
        let store = TournamentStore::new(memory_pool().await).await;
        store.create(record_with_players(21, 4)).await;
        store.create(started_bracket(22, 4)).await;
        let mut unpaid = record_with_players(23, 4);
        unpaid.status = TournamentStatus::Completed;
        unpaid.prize_pool = 1_000;
        unpaid.winner = Some("P0".into());
        store.create(unpaid.clone()).await;
        let mut paid = unpaid.clone();
        paid.tournament_id = 24;
        paid.prizes_distributed = true;
        store.create(paid).await;
        let mut cancelled = record_with_players(25, 4);
        cancelled.status = TournamentStatus::Cancelled;
        store.create(cancelled).await;

        assert_eq!(
            ids(store.with_status(TournamentStatus::Registration).await),
            [21]
        );
        assert_eq!(ids(store.with_status(TournamentStatus::Active).await), [22]);
        assert_eq!(ids(store.awaiting_prizes().await), [23]);
        assert!(store.chain_sync_pending().await.is_empty());
        assert_eq!(store.list().await.len(), 5);

        // Every write re-derives the columns from the record.
        store.start_tournament(21).await.unwrap();
        assert_eq!(
            ids(store.with_status(TournamentStatus::Active).await),
            [21, 22]
        );
        store
            .update(22, |t| {
                let FormatState::Elimination(bracket) = &mut t.state else {
                    unreachable!()
                };
                bracket
                    .decide(22, 0, "P0", "P3", ResultSource::OnChain)
                    .unwrap();
            })
            .await;
        assert_eq!(ids(store.chain_sync_pending().await), [22]);
        store
            .update(22, |t| {
                if let FormatState::Elimination(bracket) = &mut t.state {
                    bracket.mark_synced(0);
                }
            })
            .await;
        assert!(store.chain_sync_pending().await.is_empty());
        store.update(23, |t| t.prizes_distributed = true).await;
        assert!(store.awaiting_prizes().await.is_empty());
    }

    #[tokio::test]
    async fn rows_from_before_the_index_columns_are_backfilled() {
        let pool = memory_pool().await;
        sqlx::raw_sql(
            "CREATE TABLE tournaments (id INTEGER PRIMARY KEY, data TEXT NOT NULL, \
             updated_at INTEGER NOT NULL DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut unpaid = started_bracket(41, 2);
        unpaid.status = TournamentStatus::Completed;
        unpaid.prize_pool = 1_000;
        unpaid.winner = Some("P0".into());
        for (id, data) in [
            (41, serde_json::to_string(&unpaid).unwrap()),
            (42, "{not a record".to_string()),
        ] {
            sqlx::query("INSERT INTO tournaments (id, data) VALUES (?, ?)")
                .bind(id)
                .bind(data)
                .execute(&pool)
                .await
                .unwrap();
        }

        let store = TournamentStore::new(pool.clone()).await;
        assert_eq!(
            ids(store.with_status(TournamentStatus::Completed).await),
            [41]
        );
        assert_eq!(ids(store.awaiting_prizes().await), [41]);
        assert_eq!(
            ids(store.list().await),
            [41],
            "the unreadable row is skipped"
        );
        let unindexed: Option<String> =
            sqlx::query_scalar("SELECT status FROM tournaments WHERE id = 42")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(unindexed, None, "left for a later start to retry");

        // A restart finds nothing left to fill.
        let again = TournamentStore::new(pool).await;
        assert_eq!(ids(again.awaiting_prizes().await), [41]);
    }

    #[tokio::test]
    async fn a_game_leads_to_its_tournament() {
        let store = TournamentStore::new(memory_pool().await).await;
        let bracket = started_bracket(31, 4);
        let bracket_game = bracket.state.open_games(31)[0].game_id;
        store.create(bracket).await;
        store.create(swiss_record(32)).await;
        let swiss_game = TournamentStore::swiss_game_id(32, 1, 2).unwrap();

        let found = |game_id| {
            let store = store.clone();
            async move { store.find_by_game(game_id).await.map(|t| t.tournament_id) }
        };
        assert_eq!(found(bracket_game).await, Some(31));
        assert_eq!(found(swiss_game).await, Some(32));
        assert_eq!(found((31 << 16) | 200).await, None, "not one of 31's games");

        // A game id an admin assigned by hand is found among running events.
        store
            .update(31, |t| {
                if let FormatState::Elimination(bracket) = &mut t.state {
                    bracket.set_game_id(1, 999_999).unwrap();
                }
            })
            .await;
        assert_eq!(found(999_999).await, Some(31));
    }

    #[tokio::test]
    async fn the_store_starts_an_event_once() {
        let store = TournamentStore::new(memory_pool().await).await;
        store.create(record_with_players(11, 2)).await;
        store.start_tournament(11).await.unwrap();
        let t = store.get(11).await.unwrap();
        assert_eq!(t.status, TournamentStatus::Active);
        assert_eq!(t.elimination().unwrap().matches.len(), 1);
        assert!(store.start_tournament(99).await.is_err());
    }
}
