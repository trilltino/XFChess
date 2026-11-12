//! Formal FIDE draw claims (Articles 9.2, 9.3, 9.4 and 9.5).
//!
//! A claim is a deliberate action by the side to move, made before it commits a
//! move. A correct claim ends the game at once without the opponent's consent. An
//! incorrect but formal claim costs two minutes of the claimant's clock (given to
//! the opponent) and binds the claimant to any move they declared. A claim that is
//! not formal (game already over, history not provable, illegal intended move) is
//! rejected and never penalised.
//!
//! In a wagered game the program adjudicates; [`ChainDrawGate`] hands the request
//! to the chain layer, and nothing here ends the game. Everywhere else the claim is
//! adjudicated against the engine's verified move record.

use crate::engine::board_state::ChessEngine;
use crate::game::resources::{GameOverState, GameTimer};
use crate::game::systems::network_move::ChainDrawGate;
use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;
use xfchess_rules::ClaimOutcome;

/// FIDE 9.5.3: the opponent receives two extra minutes.
pub const INCORRECT_CLAIM_PENALTY_SECS: f32 = 120.0;

/// The player asked for a draw by repetition or by the fifty-move rule.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct DrawClaimRequest {
    /// True for threefold repetition, false for the fifty-move rule.
    pub repetition: bool,
    /// The legal move the claimant irrevocably declares, if the claim relies on
    /// the position that move would create. It is never applied by the claim.
    pub intended: Option<String>,
}

/// A claim this client filed in a game against a remote opponent, to be published.
/// The opponent adjudicates the same claim against the same verified history.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct DrawClaimFiled {
    /// "white" or "black": the side that was to move.
    pub player: String,
    /// The accepted-move count of the position the claim was made in.
    pub ply: u32,
    pub repetition: bool,
    pub intended: Option<String>,
}

/// The opponent's claim, delivered over the move log.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct RemoteDrawClaim {
    pub player: String,
    pub ply: u32,
    pub repetition: bool,
    pub intended: Option<String>,
}

/// Remote claims whose position this client has not reached yet (their move may
/// still be in flight). A claim for a position already passed is stale.
#[derive(Resource, Default, Debug)]
pub struct PendingRemoteClaims(pub Vec<RemoteDrawClaim>);

/// Where the player's claim stands, for the HUD.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ClaimStatus {
    #[default]
    Idle,
    /// Handed to the wallet or rollup; the result is not known yet.
    Submitting,
    /// Not a claim (and not penalised); the text says why.
    Rejected(String),
    /// Incorrect: the opponent was awarded two minutes.
    Penalized { declared_move: Option<String> },
}

/// What the claim controls show. `selected` is a move picked for review only; it
/// becomes irrevocable when the player presses the declare button.
#[derive(Resource, Default, Debug)]
pub struct ClaimUi {
    pub status: ClaimStatus,
    pub selected: Option<String>,
}

/// Adjudicate `request` against the engine's verified record and apply the result.
/// Returns the status to show.
pub fn apply_claim(
    engine: &mut ChessEngine,
    timer: &mut GameTimer,
    game_over: &mut GameOverState,
    request: &DrawClaimRequest,
) -> ClaimStatus {
    if game_over.is_game_over() {
        return ClaimStatus::Rejected("The game is already over.".to_string());
    }
    if !engine.history_proven() {
        return ClaimStatus::Rejected(
            "This game's earlier positions cannot be proven, so no claim can be checked."
                .to_string(),
        );
    }
    let outcome = engine
        .match_record()
        .adjudicate_claim(request.repetition, request.intended.as_deref());
    match outcome {
        ClaimOutcome::Confirmed(_) => {
            *game_over = GameOverState::from_claim(request.repetition);
            timer.is_running = false;
            ClaimStatus::Idle
        }
        ClaimOutcome::Incorrect { declared_move } => {
            // The claimant is the side to move; the other side gets the minutes.
            match engine.side_to_move() {
                PieceColor::White => timer.black_time_left += INCORRECT_CLAIM_PENALTY_SECS,
                PieceColor::Black => timer.white_time_left += INCORRECT_CLAIM_PENALTY_SECS,
            }
            engine.require_move(declared_move.clone());
            ClaimStatus::Penalized { declared_move }
        }
        ClaimOutcome::Rejected(reason) => ClaimStatus::Rejected(reason),
    }
}

/// Local, AI and offline claims. Wagered games are the chain layer's.
pub fn adjudicate_local_claims(
    mut requests: MessageReader<DrawClaimRequest>,
    mut filed: MessageWriter<DrawClaimFiled>,
    gate: Res<ChainDrawGate>,
    mut engine: ResMut<ChessEngine>,
    mut timer: ResMut<GameTimer>,
    mut game_over: ResMut<GameOverState>,
    mut ui: ResMut<ClaimUi>,
) {
    for request in requests.read() {
        if gate.active {
            continue;
        }
        let claimant = match engine.side_to_move() {
            PieceColor::White => "white",
            PieceColor::Black => "black",
        };
        let ply = engine.match_record().ply();
        ui.status = apply_claim(&mut engine, &mut timer, &mut game_over, request);
        // Only a formal claim is published; a rejected one never reaches the opponent.
        if !matches!(ui.status, ClaimStatus::Rejected(_)) {
            filed.write(DrawClaimFiled {
                player: claimant.to_string(),
                ply,
                repetition: request.repetition,
                intended: request.intended.clone(),
            });
        }
        ui.selected = None;
    }
}

/// Adjudicate the opponent's claim against our own verified history. Both sides
/// hold the same history, so both reach the same result, penalty and binding move.
pub fn apply_remote_claims(
    mut incoming: MessageReader<RemoteDrawClaim>,
    mut pending: ResMut<PendingRemoteClaims>,
    mut engine: ResMut<ChessEngine>,
    mut timer: ResMut<GameTimer>,
    mut game_over: ResMut<GameOverState>,
) {
    pending.0.extend(incoming.read().cloned());
    if pending.0.is_empty() || game_over.is_game_over() {
        pending.0.clear();
        return;
    }
    let ply = engine.match_record().ply();
    let side = match engine.side_to_move() {
        PieceColor::White => "white",
        PieceColor::Black => "black",
    };
    // Keep claims for positions still ahead; drop stale ones.
    let (ready, later): (Vec<_>, Vec<_>) = std::mem::take(&mut pending.0)
        .into_iter()
        .filter(|claim| claim.ply >= ply)
        .partition(|claim| claim.ply == ply);
    pending.0 = later;
    for claim in ready {
        // Only the side that is to move may claim.
        if claim.player != side {
            continue;
        }
        let request = DrawClaimRequest {
            repetition: claim.repetition,
            intended: claim.intended,
        };
        apply_claim(&mut engine, &mut timer, &mut game_over, &request);
        if game_over.is_game_over() {
            break;
        }
    }
}

/// The AI claims a draw it is entitled to only when its last search does not
/// favour it (score of zero or worse), so it never forces a draw out of a
/// better position and never claims merely because a position repeated once.
/// Documented strategy; the claim itself is adjudicated like any other.
pub fn ai_claims_when_entitled(
    engine: Res<ChessEngine>,
    ai_config: Res<crate::game::ai::ChessAIResource>,
    stats: Res<crate::game::ai::AIStatistics>,
    turn: Res<crate::game::resources::CurrentTurn>,
    game_over: Res<GameOverState>,
    mut requests: MessageWriter<DrawClaimRequest>,
    mut claimed_at: Local<Option<u32>>,
) {
    let crate::game::ai::resource::AiMode::VsAI { ai_color } = ai_config.mode else {
        return;
    };
    if game_over.is_game_over() || turn.color != ai_color || stats.last_score > 0 {
        return;
    }
    let ply = engine.match_record().ply();
    if *claimed_at == Some(ply) {
        return;
    }
    let claims = engine.claim_availability();
    if claims.repetition || claims.fifty_moves {
        *claimed_at = Some(ply);
        requests.write(DrawClaimRequest {
            repetition: claims.repetition,
            intended: None,
        });
    }
}

/// A new game starts with no claim pending.
pub fn reset_claim_ui(mut ui: ResMut<ClaimUi>, mut pending: ResMut<PendingRemoteClaims>) {
    *ui = ClaimUi::default();
    pending.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_after(moves: &[&str]) -> ChessEngine {
        let mut engine = ChessEngine::default();
        for uci in moves {
            engine.record_accepted_move(uci);
        }
        engine
    }

    const EIGHT: [&str; 8] = [
        "g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8",
    ];

    fn timer() -> GameTimer {
        GameTimer {
            white_time_left: 300.0,
            black_time_left: 300.0,
            increment: 0.0,
            is_running: true,
        }
    }

    #[test]
    fn a_correct_threefold_claim_ends_the_game_without_consent() {
        let mut engine = engine_after(&EIGHT);
        let (mut timer, mut over) = (timer(), GameOverState::Playing);
        let status = apply_claim(
            &mut engine,
            &mut timer,
            &mut over,
            &DrawClaimRequest {
                repetition: true,
                intended: None,
            },
        );
        assert_eq!(status, ClaimStatus::Idle);
        assert_eq!(over, GameOverState::ThreefoldRepetition);
        assert!(!timer.is_running);
    }

    #[test]
    fn an_incorrect_claim_costs_two_minutes_and_binds_the_declared_move() {
        let mut engine = ChessEngine::default();
        let (mut timer, mut over) = (timer(), GameOverState::Playing);
        let status = apply_claim(
            &mut engine,
            &mut timer,
            &mut over,
            &DrawClaimRequest {
                repetition: true,
                intended: Some("g1f3".to_string()),
            },
        );
        assert_eq!(
            status,
            ClaimStatus::Penalized {
                declared_move: Some("g1f3".to_string())
            }
        );
        assert_eq!(over, GameOverState::Playing);
        // White claimed, so black is awarded the two minutes.
        assert_eq!(timer.black_time_left, 420.0);
        assert_eq!(timer.white_time_left, 300.0);
        assert!(engine.is_move_legal_by_uci("g1f3"));
        assert!(!engine.is_move_legal_by_uci("e2e4"));
    }

    #[test]
    fn a_non_formal_claim_is_rejected_without_penalty() {
        let mut engine = ChessEngine::default();
        let (mut timer, mut over) = (timer(), GameOverState::Playing);
        let status = apply_claim(
            &mut engine,
            &mut timer,
            &mut over,
            &DrawClaimRequest {
                repetition: false,
                intended: Some("e2e5".to_string()),
            },
        );
        assert!(matches!(status, ClaimStatus::Rejected(_)));
        assert_eq!((timer.white_time_left, timer.black_time_left), (300.0, 300.0));
        assert!(engine.mandatory_move().is_none());
    }
}
