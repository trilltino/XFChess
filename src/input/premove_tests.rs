//! Premoves queued from either board go through the normal validated move path,
//! once, and only for a seat that may play.

use super::board::{
    apply_board_input, fire_premove, queue_board_move, queue_premove_click, try_select_piece,
    InputSystemParams, PendingBoardInput,
};
use crate::core::state::GameMode;
use crate::engine::board_state::ChessEngine;
use crate::game::components::HasMoved;
use crate::game::events::MoveMadeEvent;
use crate::game::resources::player::{Player, Players};
use crate::game::resources::premove::Premove;
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameOverState, MoveHistory, PendingPromotion, PendingTurnAdvance,
    Selection,
};
use crate::motion::BoardEpoch;
use crate::multiplayer::types::OnlineStartBarrier;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::ecs::message::MessageCursor;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

type TestResult = Result<(), String>;

const BLACK_TO_MOVE: &str = "4k3/8/8/8/8/8/4P3/4K3 b - - 0 1";
const WHITE_TO_MOVE: &str = "3k4/8/8/8/8/8/4P3/4K3 w - - 0 1";

fn queue_system(
    In(square): In<(u8, u8)>,
    mut params: InputSystemParams,
    mut premove: ResMut<Premove>,
    epoch: Res<BoardEpoch>,
) {
    queue_premove_click(&mut params, &mut premove, &epoch, square);
}

struct Board {
    app: App,
    cursor: MessageCursor<MoveMadeEvent>,
}

impl Board {
    /// The local player is White against an AI; it is Black's turn.
    fn new(mode: GameMode, both_human: bool) -> Result<Self, String> {
        let mut engine = ChessEngine::default();
        engine.set_from_fen(BLACK_TO_MOVE)?;
        engine.rebuild_legal_move_cache();
        let mut app = App::new();
        app.insert_resource(engine);
        app.init_resource::<Selection>();
        app.init_resource::<PendingTurnAdvance>();
        app.init_resource::<MoveHistory>();
        app.init_resource::<CapturedPieces>();
        app.init_resource::<GameOverState>();
        app.init_resource::<PendingPromotion>();
        app.init_resource::<OnlineStartBarrier>();
        app.init_resource::<Premove>();
        app.init_resource::<BoardEpoch>();
        app.init_resource::<PendingBoardInput>();
        app.insert_resource(mode);
        app.insert_resource(CurrentTurn {
            color: PieceColor::Black,
            ..Default::default()
        });
        app.insert_resource(Players {
            player_1: Player::new(1, "Me".into(), PieceColor::White, true),
            player_2: Player::new(2, "Them".into(), PieceColor::Black, both_human),
        });
        app.add_message::<MoveMadeEvent>();
        app.add_systems(Update, (fire_premove, apply_board_input).chain());
        for (color, kind, x, y) in [
            (PieceColor::White, PieceType::Pawn, 4, 1),
            (PieceColor::White, PieceType::King, 4, 0),
            (PieceColor::Black, PieceType::King, 4, 7),
        ] {
            app.world_mut().spawn((
                Piece::new(color, kind, x, y),
                HasMoved::default(),
                Transform::default(),
            ));
        }
        Ok(Self {
            app,
            cursor: MessageCursor::default(),
        })
    }

    fn click(&mut self, square: (u8, u8)) -> TestResult {
        self.app
            .world_mut()
            .run_system_once_with(queue_system, square)
            .map_err(|e| e.to_string())
    }

    fn premove(&self) -> Premove {
        self.app.world().resource::<Premove>().clone()
    }

    fn published(&mut self) -> usize {
        let events = self.app.world().resource::<Messages<MoveMadeEvent>>();
        self.cursor.read(events).count()
    }

    fn fen(&self) -> String {
        self.app
            .world()
            .resource::<ChessEngine>()
            .current_fen()
            .to_string()
    }

    /// The opponent has moved: White to play again.
    fn opponent_replied(&mut self) -> TestResult {
        let mut engine = self.app.world_mut().resource_mut::<ChessEngine>();
        engine.set_from_fen(WHITE_TO_MOVE)?;
        engine.rebuild_legal_move_cache();
        self.app.world_mut().resource_mut::<CurrentTurn>().color = PieceColor::White;
        let mut pieces = self.app.world_mut().query::<&mut Piece>();
        for mut piece in pieces.iter_mut(self.app.world_mut()) {
            if piece.color == PieceColor::Black {
                piece.x = 3;
            }
        }
        Ok(())
    }

    fn white_pawn_square(&mut self) -> Option<(u8, u8)> {
        let mut pieces = self.app.world_mut().query::<&Piece>();
        pieces
            .iter(self.app.world())
            .find(|p| p.color == PieceColor::White && p.piece_type == PieceType::Pawn)
            .map(|p| (p.x, p.y))
    }
}

#[test]
fn queueing_changes_nothing_on_the_board_and_sends_nothing() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    let fen = board.fen();
    board.click((4, 1))?;
    board.click((4, 3))?;
    assert!(board.premove().is_set());
    board.app.update();

    assert_eq!(board.fen(), fen);
    assert_eq!(board.published(), 0);
    assert_eq!(
        board.app.world().resource::<CurrentTurn>().color,
        PieceColor::Black
    );
    assert!(board.app.world().resource::<MoveHistory>().moves.is_empty());
    assert_eq!(board.white_pawn_square(), Some((4, 1)));
    // Still waiting for the opponent, so it is kept.
    assert!(board.premove().is_set());
    Ok(())
}

#[test]
fn a_legal_premove_plays_once_when_the_turn_arrives() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 3))?;
    board.opponent_replied()?;

    board.app.update();
    assert_eq!(board.white_pawn_square(), Some((4, 3)));
    assert_eq!(board.published(), 1);
    assert!(!board.premove().is_active());

    // Nothing is left to play on later frames.
    board.app.update();
    board.app.update();
    assert_eq!(board.published(), 0);
    Ok(())
}

#[cfg(feature = "solana")]
#[test]
fn a_new_seat_lease_cannot_submit_an_old_premove() -> TestResult {
    let mut game = Board::new(GameMode::SinglePlayer, false)?;
    let mut seat = crate::multiplayer::seat::SeatState::default();
    seat.game_id = 42;
    seat.epoch = 7;
    game.app.insert_resource(seat);
    game.click((4, 1))?;
    game.click((4, 3))?;
    assert!(game.premove().is_set());
    game.app
        .world_mut()
        .resource_mut::<crate::multiplayer::seat::SeatState>()
        .epoch = 8;
    game.opponent_replied()?;
    game.app.update();
    assert!(!game.premove().is_active());
    assert_eq!(game.white_pawn_square(), Some((4, 1)));
    assert_eq!(game.published(), 0);
    Ok(())
}

#[test]
fn a_premove_that_is_no_longer_legal_is_discarded_without_effect() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 4))?;
    board.opponent_replied()?;
    let fen = board.fen();

    board.app.update();
    assert_eq!(board.fen(), fen);
    assert_eq!(board.published(), 0);
    assert_eq!(board.white_pawn_square(), Some((4, 1)));
    assert!(!board.premove().is_active());
    Ok(())
}

#[test]
fn a_premove_from_an_earlier_board_is_dropped_unplayed() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 3))?;
    board.app.world_mut().resource_mut::<BoardEpoch>().bump();
    board.opponent_replied()?;

    board.app.update();
    assert_eq!(board.white_pawn_square(), Some((4, 1)));
    assert_eq!(board.published(), 0);
    assert!(!board.premove().is_active());
    Ok(())
}

#[test]
fn clicking_cancels_and_replaces() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 1))?;
    assert!(!board.premove().is_active());

    board.click((4, 1))?;
    board.click((4, 3))?;
    board.click((4, 0))?;
    assert_eq!(board.premove().from(), Some((4, 0)));
    assert_eq!(board.premove().to(), None);
    Ok(())
}

#[test]
fn only_the_local_players_own_pieces_can_start_a_premove() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 7))?;
    assert!(!board.premove().is_active());
    board.click((3, 3))?;
    assert!(!board.premove().is_active());
    Ok(())
}

#[test]
fn seats_without_a_defined_premove_never_queue_one() -> TestResult {
    for (mode, both_human) in [
        (GameMode::SinglePlayer, true),
        (GameMode::MultiplayerLocal, true),
        (GameMode::Spectator, false),
        (GameMode::PgnReplay, false),
        (GameMode::Puzzle, false),
    ] {
        let mut board = Board::new(mode, both_human)?;
        board.click((4, 1))?;
        board.click((4, 3))?;
        assert!(!board.premove().is_active(), "{mode:?}");
    }
    Ok(())
}

#[test]
fn an_online_seat_that_has_not_started_cannot_queue() -> TestResult {
    let mut board = Board::new(GameMode::OnlineMultiplayer, false)?;
    board.click((4, 1))?;
    assert!(!board.premove().is_active());
    Ok(())
}

#[cfg(feature = "solana")]
#[test]
fn online_premoves_require_start_seat_and_delegation_at_queue_and_execution() -> TestResult {
    use crate::multiplayer::{
        network::online_game_session::OnlineGameSession,
        rollup::magicblock::{DelegationStatus, MagicBlockResolver},
        seat::SeatState,
        solana::addon::SolanaGameSync,
    };
    for (started, displaced, resolver, allowed) in [
        (false, false, 2, false),
        (true, true, 2, false),
        (true, false, 0, false),
        (true, false, 1, false),
        (true, false, 2, true),
    ] {
        let mut board = Board::new(GameMode::OnlineMultiplayer, false)?;
        board.app.insert_resource(OnlineGameSession {
            active: true,
            game_id: "42".into(),
            ..Default::default()
        });
        board.app.insert_resource(OnlineStartBarrier {
            game_id: 42,
            local_ready: started,
            remote_ready: started,
            start_confirmed: started,
            ready_sent: started,
            start_sent: started,
        });
        board.app.insert_resource(SolanaGameSync {
            game_id: Some(42),
            requires_delegation: true,
            ..Default::default()
        });
        let mut seat = SeatState::default();
        seat.game_id = 42;
        seat.epoch = 7;
        seat.superseded = displaced;
        board.app.insert_resource(seat);
        if resolver > 0 {
            let mut state = MagicBlockResolver::default();
            if resolver == 2 {
                state.delegation_status = DelegationStatus::Delegated;
                state.delegated_game_pda = Some(solana_sdk::pubkey::Pubkey::new_unique());
            }
            board.app.insert_resource(state);
        }
        board.click((4, 1))?;
        board.click((4, 3))?;
        assert_eq!(board.premove().is_set(), allowed);
        board.opponent_replied()?;
        board.app.update();
        assert_eq!(board.published(), usize::from(allowed));
        assert!(!board.premove().is_active());
    }
    // A permission that disappears while queued is checked again at execution.
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 3))?;
    let mut seat = SeatState::default();
    seat.superseded = true;
    board.app.insert_resource(seat);
    board.opponent_replied()?;
    board.app.update();
    assert_eq!(board.published(), 0);
    assert!(!board.premove().is_active());
    Ok(())
}

#[test]
fn game_over_clears_a_queued_premove() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.click((4, 1))?;
    board.click((4, 3))?;
    *board.app.world_mut().resource_mut::<GameOverState>() = GameOverState::BlackWon;
    board.app.update();
    assert!(!board.premove().is_active());
    assert_eq!(board.published(), 0);
    Ok(())
}

fn enqueue_move(mut params: InputSystemParams) {
    let source = params
        .pieces
        .p1()
        .iter()
        .find(|(_, p, _, _)| p.piece_type == PieceType::Pawn && p.color == PieceColor::White)
        .map(|(entity, piece, _, _)| (entity, *piece));
    if let Some((entity, piece)) = source {
        try_select_piece(&mut params, entity, piece, true);
        queue_board_move(&mut params, (4, 3), None, "test_egui");
    }
}

#[test]
fn egui_intent_waits_for_execution_and_is_published_once() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.opponent_replied()?;
    let fen = board.fen();
    board
        .app
        .world_mut()
        .run_system_once(enqueue_move)
        .map_err(|error| error.to_string())?;
    assert_eq!(board.fen(), fen);
    assert_eq!(board.published(), 0);
    board.app.update();
    assert_eq!(board.white_pawn_square(), Some((4, 3)));
    assert_eq!(board.published(), 1);
    board.app.update();
    assert_eq!(board.published(), 0);
    Ok(())
}

#[test]
fn an_input_intent_cannot_cross_a_reset() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    board.opponent_replied()?;
    board
        .app
        .world_mut()
        .run_system_once(enqueue_move)
        .map_err(|error| error.to_string())?;
    board.app.world_mut().resource_mut::<BoardEpoch>().bump();
    board.app.update();
    assert_eq!(board.white_pawn_square(), Some((4, 1)));
    assert_eq!(board.published(), 0);
    Ok(())
}

#[test]
fn premove_underpromotion_is_validated_and_applied_once() -> TestResult {
    let mut board = Board::new(GameMode::SinglePlayer, false)?;
    {
        let mut pieces = board.app.world_mut().query::<&mut Piece>();
        for mut piece in pieces.iter_mut(board.app.world_mut()) {
            if piece.piece_type == PieceType::Pawn {
                piece.x = 0;
                piece.y = 6;
            }
            if piece.color == PieceColor::Black {
                piece.x = 7;
                piece.y = 6;
            }
        }
    }
    board.click((0, 6))?;
    board.click((0, 7))?;
    board
        .app
        .world_mut()
        .resource_mut::<Premove>()
        .choose_promotion(PieceType::Knight);
    {
        let mut engine = board.app.world_mut().resource_mut::<ChessEngine>();
        engine.set_from_fen("8/P6k/8/8/8/8/8/4K3 w - - 0 1")?;
        engine.rebuild_legal_move_cache();
    }
    board.app.world_mut().resource_mut::<CurrentTurn>().color = PieceColor::White;
    board.app.update();
    let mut pieces = board.app.world_mut().query::<&Piece>();
    assert!(pieces
        .iter(board.app.world())
        .any(|p| p.x == 0 && p.y == 7 && p.piece_type == PieceType::Knight));
    assert_eq!(board.published(), 1);
    assert!(!board.premove().is_active());
    Ok(())
}
