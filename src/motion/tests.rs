//! The motion timeline against the real move pipeline: whatever is drawn, the
//! accepted move, FEN, turn, clocks and published events are the same.

use super::*;
use crate::engine::board_state::ChessEngine;
use crate::game::components::{FadingCapture, HasMoved};
use crate::game::events::{MoveMadeEvent, NetworkMoveEvent, NetworkMoveResult, RemoteMoveApplied};
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameTimer, MoveHistory, PendingTurnAdvance, Selection,
};
use crate::game::systems::network_move::handle_network_moves;
use crate::game::systems::visual::flush_pending_turn;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::ecs::message::MessageCursor;
use bevy::ecs::system::Command;
use bevy::prelude::*;
use std::time::Duration;

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

type TestResult = Result<(), String>;

struct Game {
    app: App,
    cursor: MessageCursor<MoveMadeEvent>,
    published: usize,
    ack_cursor: MessageCursor<RemoteMoveApplied>,
    acknowledged: usize,
}

/// Everything that must not depend on how the move was drawn.
#[derive(Debug, PartialEq)]
struct Authority {
    fen: String,
    history_len: usize,
    published: usize,
    acknowledged: usize,
    turn: (PieceColor, u32),
    clocks: (f32, f32),
    pieces: Vec<(u8, u8, PieceType, PieceColor)>,
    legal_moves: Vec<((u8, u8), (u8, u8))>,
}

impl Game {
    fn from_fen(fen: &str) -> Result<Self, String> {
        let mut engine = ChessEngine::default();
        engine.set_from_fen(fen)?;
        engine.rebuild_legal_move_cache();
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<BoardEpoch>();
        app.init_resource::<crate::game::systems::board_snapshot::PendingBoardSnapshot>();
        app.init_resource::<crate::game::resources::PendingPromotion>();
        app.init_resource::<crate::game::resources::premove::Premove>();
        app.insert_resource(engine);
        app.init_resource::<Selection>();
        app.init_resource::<PendingTurnAdvance>();
        app.init_resource::<MoveHistory>();
        app.init_resource::<CapturedPieces>();
        app.init_resource::<CurrentTurn>();
        app.init_resource::<crate::game::resources::CurrentGamePhase>();
        app.init_resource::<crate::game::resources::GameOverState>();
        app.init_resource::<crate::game::view_mode::ViewMode>();
        app.insert_resource(GameTimer {
            white_time_left: 300.0,
            black_time_left: 300.0,
            increment: 2.0,
            is_running: true,
        });
        app.add_message::<NetworkMoveEvent>();
        app.add_message::<MoveMadeEvent>();
        app.add_message::<RemoteMoveApplied>();
        app.add_message::<NetworkMoveResult>();
        app.add_systems(
            Update,
            (
                crate::game::systems::board_snapshot::apply_board_snapshot,
                handle_network_moves,
                flush_pending_turn,
                crate::game::systems::game_logic::update_game_phase,
                advance_piece_motions,
                present_piece_motion_3d,
                crate::game::systems::visual::animate_capture_fade,
            )
                .chain(),
        );
        let placement = fen.split_whitespace().next().ok_or("empty fen")?;
        for (row, line) in placement.split('/').enumerate() {
            let rank = 7 - row as u8;
            let mut file = 0u8;
            for ch in line.chars() {
                if let Some(skip) = ch.to_digit(10) {
                    file += skip as u8;
                    continue;
                }
                let color = if ch.is_ascii_uppercase() {
                    PieceColor::White
                } else {
                    PieceColor::Black
                };
                let kind = match ch.to_ascii_lowercase() {
                    'p' => PieceType::Pawn,
                    'n' => PieceType::Knight,
                    'b' => PieceType::Bishop,
                    'r' => PieceType::Rook,
                    'q' => PieceType::Queen,
                    'k' => PieceType::King,
                    other => return Err(format!("bad fen piece {other}")),
                };
                app.world_mut().spawn((
                    Piece::new(color, kind, file, rank),
                    HasMoved::default(),
                    Transform::from_translation(board_to_world(square_to_board((file, rank)))),
                ));
                file += 1;
            }
        }
        let side = fen.split_whitespace().nth(1).unwrap_or("w");
        if side == "b" {
            app.world_mut().resource_mut::<CurrentTurn>().color = PieceColor::Black;
        }
        Ok(Self {
            app,
            cursor: MessageCursor::default(),
            published: 0,
            ack_cursor: MessageCursor::default(),
            acknowledged: 0,
        })
    }

    fn start() -> Result<Self, String> {
        Self::from_fen(START)
    }

    /// Delivers a move as the network would, then runs one frame of `dt`.
    fn receive(&mut self, uci: &str, dt: f32) -> TestResult {
        let (from, to, promotion) =
            ChessEngine::parse_uci_move(uci).ok_or_else(|| format!("bad uci {uci}"))?;
        self.app.world_mut().write_message(NetworkMoveEvent {
            from,
            to,
            promotion,
            expected_fen: None,
            dedup_version: None,
        });
        self.tick(dt);
        Ok(())
    }

    fn tick(&mut self, dt: f32) {
        self.app
            .world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(dt));
        self.app.update();
        let events = self.app.world().resource::<Messages<MoveMadeEvent>>();
        self.published += self.cursor.read(events).count();
        self.acknowledged += self
            .ack_cursor
            .read(self.app.world().resource::<Messages<RemoteMoveApplied>>())
            .count();
    }

    /// Runs frames until every motion has finished.
    fn settle(&mut self, dt: f32) {
        for _ in 0..200 {
            self.tick(dt);
        }
    }

    fn entity_at(&mut self, square: (u8, u8)) -> Option<Entity> {
        let mut query = self.app.world_mut().query::<(Entity, &Piece)>();
        query
            .iter(self.app.world())
            .find(|(_, p)| (p.x, p.y) == square)
            .map(|(e, _)| e)
    }

    fn piece(&mut self, square: (u8, u8)) -> Option<Piece> {
        let entity = self.entity_at(square)?;
        self.app.world().get::<Piece>(entity).copied()
    }

    fn motion(&mut self, square: (u8, u8)) -> Option<PieceMotion> {
        let entity = self.entity_at(square)?;
        self.app.world().get::<PieceMotion>(entity).cloned()
    }

    fn translation(&mut self, square: (u8, u8)) -> Option<Vec3> {
        let entity = self.entity_at(square)?;
        self.app
            .world()
            .get::<Transform>(entity)
            .map(|t| t.translation)
    }

    fn active_motions(&mut self) -> usize {
        let mut query = self.app.world_mut().query::<&PieceMotion>();
        query.iter(self.app.world()).count()
    }

    fn authority(&mut self) -> Authority {
        let world = self.app.world();
        let fen = world.resource::<ChessEngine>().current_fen().to_string();
        let history_len = world.resource::<MoveHistory>().moves.len();
        let turn = world.resource::<CurrentTurn>();
        let timer = world.resource::<GameTimer>();
        let turn = (turn.color, turn.move_number);
        let clocks = (timer.white_time_left, timer.black_time_left);
        let mut query = self.app.world_mut().query::<&Piece>();
        let mut pieces: Vec<_> = query
            .iter(self.app.world())
            .filter(|p| p.x < 8 && p.y < 8)
            .map(|p| (p.x, p.y, p.piece_type, p.color))
            .collect();
        pieces.sort_by_key(|p| (p.0, p.1));
        let engine = self.app.world().resource::<ChessEngine>();
        let mut legal_moves: Vec<_> = pieces
            .iter()
            .flat_map(|p| {
                engine
                    .get_legal_moves_for_square((p.0, p.1), p.3)
                    .into_iter()
                    .map(move |to| ((p.0, p.1), to))
            })
            .collect();
        legal_moves.sort_unstable();
        Authority {
            fen,
            history_len,
            published: self.published,
            acknowledged: self.acknowledged,
            turn,
            clocks,
            pieces,
            legal_moves,
        }
    }
}

fn world(file: u8, rank: u8) -> Vec3 {
    board_to_world(square_to_board((file, rank)))
}

#[test]
fn an_accepted_move_is_authoritative_before_any_animation_frame() -> TestResult {
    let mut game = Game::start()?;
    game.receive("e2e4", 0.0)?;

    assert!(game.piece((4, 1)).is_none());
    assert_eq!(
        game.piece((4, 3)).map(|p| p.piece_type),
        Some(PieceType::Pawn)
    );
    let authority = game.authority();
    assert!(authority.fen.contains(" b "));
    assert!(authority.fen.contains(" e3 "));
    assert_eq!(authority.history_len, 1);
    assert_eq!(authority.published, 1);
    assert_eq!(authority.turn.0, PieceColor::Black);

    // The piece is already on e4 but is still drawn on e2, so there is no
    // one-frame flash at the destination.
    let motion = game.motion((4, 3)).ok_or("no motion")?;
    assert_eq!(motion.tx.from, Vec2::new(4.0, 1.0));
    assert_eq!(motion.tx.to, Vec2::new(4.0, 3.0));
    assert_eq!(game.translation((4, 3)), Some(world(4, 1)));
    Ok(())
}

#[test]
fn creation_frame_does_not_spend_the_delta_from_before_acceptance() -> TestResult {
    let mut game = Game::start()?;
    game.receive("g1f3", 0.8)?;
    assert_eq!(game.motion((5, 2)).ok_or("no motion")?.tx.elapsed, 0.0);
    assert_eq!(game.translation((5, 2)), Some(world(6, 0)));
    game.tick(0.4);
    assert_eq!(game.active_motions(), 0);
    assert_eq!(game.translation((5, 2)), Some(world(5, 2)));
    Ok(())
}

#[test]
fn every_piece_kind_moves_through_the_same_timeline() -> TestResult {
    let cases = [
        (START, "e2e3"),
        (START, "e2e4"),
        (START, "g1f3"),
        ("4k3/8/8/8/8/8/4B3/4K3 w - - 0 1", "e2a6"),
        ("k7/8/8/8/8/8/4R3/4K3 w - - 0 1", "e2e7"),
        ("k7/8/8/8/8/8/4Q3/4K3 w - - 0 1", "e2h5"),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", "e1d2"),
        ("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "e4d5"),
    ];
    for (fen, uci) in cases {
        let mut game = Game::from_fen(fen)?;
        let (from, to, _) = ChessEngine::parse_uci_move(uci).ok_or("bad uci")?;
        game.receive(uci, 0.0)?;
        let motion = game.motion(to).ok_or_else(|| format!("{uci}: no motion"))?;
        assert_eq!(motion.tx.from, square_to_board(from), "{uci}");
        assert_eq!(motion.tx.to, square_to_board(to), "{uci}");
        game.settle(0.02);
        assert_eq!(game.active_motions(), 0, "{uci}");
        assert_eq!(
            game.translation(to),
            Some(board_to_world(square_to_board(to)))
        );
        assert_eq!(game.authority().published, 1, "{uci}");
    }
    Ok(())
}

#[test]
fn a_capture_fades_the_victim_and_moves_the_attacker() -> TestResult {
    let mut game = Game::from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1")?;
    let victim = game.entity_at((3, 4)).ok_or("no victim")?;
    game.receive("e4d5", 0.0)?;
    assert!(game.app.world().get::<FadingCapture>(victim).is_some());
    assert_eq!(game.piece((3, 4)).map(|p| p.color), Some(PieceColor::White));
    assert!(game.motion((3, 4)).is_some());
    Ok(())
}

#[test]
fn kingside_castling_moves_king_and_rook_together_as_one_published_move() -> TestResult {
    let mut game = Game::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1")?;
    game.receive("e1g1", 0.0)?;

    assert_eq!(game.authority().published, 1);
    assert_eq!(game.authority().history_len, 1);
    let king = game.motion((6, 0)).ok_or("no king motion")?;
    let rook = game.motion((5, 0)).ok_or("no rook motion")?;
    assert_eq!(king.tx.from, Vec2::new(4.0, 0.0));
    assert_eq!(rook.tx.from, Vec2::new(7.0, 0.0));
    assert_eq!(rook.tx.to, Vec2::new(5.0, 0.0));
    assert_eq!(king.tx.duration, rook.tx.duration);
    assert_eq!(king.epoch, rook.epoch);

    game.tick(0.2);
    assert_eq!(game.active_motions(), 2);
    game.settle(0.02);
    assert_eq!(game.active_motions(), 0);
    Ok(())
}

#[test]
fn queenside_castling_moves_the_rook_too() -> TestResult {
    let mut game = Game::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1")?;
    game.receive("e8c8", 0.0)?;
    let king = game.motion((2, 7)).ok_or("no king motion")?;
    let rook = game.motion((3, 7)).ok_or("no rook motion")?;
    assert_eq!(king.tx.to, Vec2::new(2.0, 7.0));
    assert_eq!(rook.tx.from, Vec2::new(0.0, 7.0));
    assert_eq!(rook.tx.to, Vec2::new(3.0, 7.0));
    assert_eq!(game.authority().published, 1);
    Ok(())
}

#[test]
fn en_passant_fades_the_pawn_beside_not_on_the_destination() -> TestResult {
    let mut game = Game::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1")?;
    let victim = game.entity_at((3, 4)).ok_or("no victim")?;
    game.receive("e5d6", 0.0)?;

    assert!(game.app.world().get::<FadingCapture>(victim).is_some());
    let victim_piece = game.app.world().get::<Piece>(victim).copied();
    assert_eq!(victim_piece.map(|p| p.x), Some(u8::MAX));
    let mover = game.motion((3, 5)).ok_or("no pawn motion")?;
    assert_eq!(mover.tx.from, Vec2::new(4.0, 4.0));
    assert!(game.authority().fen.starts_with("4k3/8/3P4/8/"));
    Ok(())
}

#[test]
fn promotion_applies_at_once_but_is_drawn_as_a_pawn_until_the_midpoint() -> TestResult {
    let mut game = Game::from_fen("8/P6k/8/8/8/8/8/4K3 w - - 0 1")?;
    game.receive("a7a8q", 0.0)?;

    assert_eq!(
        game.piece((0, 7)).map(|p| p.piece_type),
        Some(PieceType::Queen)
    );
    let motion = game.motion((0, 7)).ok_or("no motion")?;
    assert_eq!(motion.drawn_type(PieceType::Queen), PieceType::Pawn);
    game.tick(0.3);
    let motion = game.motion((0, 7)).ok_or("no motion")?;
    assert_eq!(motion.drawn_type(PieceType::Queen), PieceType::Queen);
    assert_eq!(game.authority().published, 1);
    Ok(())
}

#[test]
fn promotion_with_capture_moves_and_fades_correctly() -> TestResult {
    let mut game = Game::from_fen("1n5k/P7/8/8/8/8/8/4K3 w - - 0 1")?;
    let victim = game.entity_at((1, 7)).ok_or("no knight")?;
    game.receive("a7b8q", 0.0)?;
    assert!(game.app.world().get::<FadingCapture>(victim).is_some());
    let motion = game.motion((1, 7)).ok_or("no motion")?;
    assert_eq!(motion.promoted_from, Some(PieceType::Pawn));
    assert_eq!(motion.tx.from, Vec2::new(0.0, 6.0));
    Ok(())
}

#[test]
fn a_mating_move_is_final_while_its_animation_still_plays() -> TestResult {
    let mut game = Game::start()?;
    for uci in ["f2f3", "e7e5", "g2g4"] {
        game.receive(uci, 0.5)?;
    }
    game.receive("d8h4", 0.0)?;
    let engine = game.app.world().resource::<ChessEngine>();
    assert!(engine.is_check());
    assert!(!engine.has_legal_moves());
    assert_eq!(
        *game
            .app
            .world()
            .resource::<crate::game::resources::GameOverState>(),
        crate::game::resources::GameOverState::BlackWon
    );
    assert!(game.active_motions() >= 1);
    Ok(())
}

#[test]
fn several_moves_in_one_frame_keep_every_motion() -> TestResult {
    let mut game = Game::start()?;
    for uci in ["e2e4", "e7e5"] {
        let (from, to, promotion) = ChessEngine::parse_uci_move(uci).ok_or("bad uci")?;
        game.app.world_mut().write_message(NetworkMoveEvent {
            from,
            to,
            promotion,
            expected_fen: None,
            dedup_version: None,
        });
    }
    game.tick(0.0);
    assert!(game.motion((4, 3)).is_some());
    assert!(game.motion((4, 4)).is_some());
    assert_eq!(game.authority().published, 2);
    let authority = game.authority();
    assert_eq!(authority.acknowledged, 2);
    assert_eq!(authority.turn, (PieceColor::White, 2));
    assert_eq!(authority.clocks, (302.0, 302.0));
    Ok(())
}

#[test]
fn moving_the_same_piece_again_retargets_from_where_it_is_drawn() -> TestResult {
    let mut game = Game::start()?;
    game.receive("g1f3", 0.0)?;
    game.receive("a7a6", 0.1)?;
    let drawn = game.motion((5, 2)).ok_or("no motion")?.tx.position();
    game.receive("f3g5", 0.0)?;
    let motion = game.motion((6, 4)).ok_or("no motion")?;
    assert_eq!(motion.tx.to, Vec2::new(6.0, 4.0));
    assert!((motion.tx.from - drawn).length() < 1e-4);
    // The knight's motion was retargeted, not stacked: one per moving piece.
    assert_eq!(game.active_motions(), 2);
    Ok(())
}

fn authority_after(
    plies: &[&str],
    mut between: impl FnMut(&mut Game),
) -> Result<Authority, String> {
    let mut game = Game::start()?;
    for uci in plies {
        game.receive(uci, 0.0)?;
        between(&mut game);
    }
    game.settle(0.05);
    Ok(game.authority())
}

#[test]
fn drawing_never_changes_fen_turn_clocks_history_or_publications() -> TestResult {
    let plies = [
        "e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6", "e1g1", "f6e4",
    ];
    // Never advanced: the animation is simply not played.
    let frozen = authority_after(&plies, |_| {})?;
    assert_eq!(frozen.published, plies.len());
    assert_eq!(frozen.history_len, plies.len());
    assert_eq!(frozen.acknowledged, plies.len());

    let at_60fps = authority_after(&plies, |g| {
        for _ in 0..30 {
            g.tick(1.0 / 60.0);
        }
    })?;
    let at_30fps = authority_after(&plies, |g| {
        for _ in 0..15 {
            g.tick(1.0 / 30.0);
        }
    })?;
    let stalled = authority_after(&plies, |g| g.tick(30.0))?;
    let interrupted = authority_after(&plies, |g| {
        g.tick(0.1);
        // A resync or view change arrives mid-flight.
        retire_board_visuals(g.app.world_mut());
    })?;
    let retargeted = authority_after(&plies, |g| {
        g.tick(0.13);
        g.tick(0.0);
    })?;
    for other in [at_60fps, at_30fps, stalled, interrupted, retargeted] {
        assert_eq!(other, frozen);
    }
    Ok(())
}

#[test]
fn disabled_animation_and_view_switches_preserve_authority_and_acknowledgements() -> TestResult {
    use crate::game::view_mode::ViewMode;
    let mut visible = Game::start()?;
    let mut disabled = Game::start()?;
    disabled
        .app
        .insert_resource(super::gameplay::MotionSettings {
            enabled: false,
            duration: 0.4,
        });
    visible.app.insert_resource(ViewMode::Standard3D);
    for uci in ["e2e4", "e7e5", "g1f3", "b8c6"] {
        visible.receive(uci, 0.0)?;
        disabled.receive(uci, 0.0)?;
        visible.tick(0.13);
        *visible.app.world_mut().resource_mut::<ViewMode>() = ViewMode::Standard2D;
        visible.tick(0.0);
        *visible.app.world_mut().resource_mut::<ViewMode>() = ViewMode::Standard3D;
        assert_eq!(visible.authority(), disabled.authority());
        assert_eq!(disabled.active_motions(), 0);
    }
    Ok(())
}

#[test]
fn fen_mismatch_retires_motion_and_never_acknowledges_visual_completion() -> TestResult {
    let mut game = Game::start()?;
    game.receive("e2e4", 0.0)?;
    game.tick(0.15);
    game.app.world_mut().write_message(NetworkMoveEvent {
        from: (4, 6),
        to: (4, 4),
        promotion: None,
        expected_fen: Some("disagreed".into()),
        dedup_version: Some("bad".into()),
    });
    game.tick(0.0);
    assert_eq!(game.active_motions(), 0);
    assert_eq!(game.acknowledged, 1);
    game.settle(0.1);
    assert_eq!(game.acknowledged, 1);
    assert_eq!(game.published, 2);
    Ok(())
}

#[test]
fn snapshot_rebuilds_ecs_during_capture_and_clears_old_intents() -> TestResult {
    use crate::game::resources::premove::Premove;
    use crate::game::systems::board_snapshot::PendingBoardSnapshot;
    let mut game = Game::from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1")?;
    let victim = game.entity_at((3, 4)).ok_or("missing victim")?;
    game.receive("e4d5", 0.0)?;
    game.tick(0.1);
    game.app
        .world_mut()
        .resource_mut::<Premove>()
        .click((4, 0), true, 0);
    let replacement = "4k3/8/8/8/8/8/3P4/4K3 b - - 0 2";
    game.app
        .world_mut()
        .resource_mut::<PendingBoardSnapshot>()
        .0 = Some(replacement.into());
    game.tick(0.0);
    assert!(game.app.world().get_entity(victim).is_err());
    assert!(game.piece((3, 4)).is_none());
    assert_eq!(
        game.piece((3, 1)).map(|p| p.piece_type),
        Some(PieceType::Pawn)
    );
    assert_eq!(game.authority().fen, replacement);
    assert_eq!(game.authority().turn, (PieceColor::Black, 2));
    assert_eq!(game.active_motions(), 0);
    assert!(!game.app.world().resource::<Premove>().is_active());
    game.settle(0.05);
    assert_eq!(game.translation((3, 1)), Some(world(3, 1)));
    assert_eq!(game.published, 1);
    assert_eq!(game.acknowledged, 1);
    Ok(())
}

#[test]
fn casual_resync_handler_replaces_a_board_halfway_through_a_move() -> TestResult {
    use crate::multiplayer::network::online_game_session::OnlineGameSession;
    use crate::multiplayer::network::protocol::NetworkMessage;
    use crate::multiplayer::types::NetworkEvent;
    let mut game = Game::start()?;
    game.app.insert_resource(OnlineGameSession {
        active: true,
        game_id: "42".into(),
        ..Default::default()
    });
    game.app.add_message::<NetworkEvent>();
    game.app.add_systems(
        Update,
        crate::multiplayer::systems::handle_resync_response
            .before(crate::game::systems::board_snapshot::apply_board_snapshot),
    );
    game.receive("e2e4", 0.0)?;
    game.tick(0.2);
    let mut position = xfchess_rules::Game::from_fen(START)?;
    for uci in ["e2e4", "e7e5", "g1f3"] {
        position.play_uci(uci)?;
    }
    let fen = position.fen();
    game.app
        .world_mut()
        .write_message(NetworkEvent::MessageReceived(
            NetworkMessage::ResyncResponse {
                game_id: 42,
                committed_fen: fen.clone(),
                committed_turn: 3,
            },
        ));
    game.tick(0.0);
    assert_eq!(game.authority().fen, fen);
    assert_eq!(game.active_motions(), 0);
    assert!(game.piece((5, 2)).is_some());
    assert!(game.piece((6, 0)).is_none());
    assert_eq!(game.published, 1);
    Ok(())
}

#[test]
fn same_ply_recovery_requires_the_scoped_rules_checkpoint() -> TestResult {
    use crate::multiplayer::{
        network::{online_game_session::OnlineGameSession, protocol::NetworkMessage},
        types::NetworkEvent,
    };
    let mut game = Game::start()?;
    game.app.insert_resource(OnlineGameSession {
        active: true,
        game_id: "42".into(),
        ..Default::default()
    });
    game.app.add_message::<NetworkEvent>();
    game.app.add_systems(
        Update,
        crate::multiplayer::systems::handle_resync_response
            .before(crate::game::systems::board_snapshot::apply_board_snapshot),
    );
    game.receive("e2e4", 0.0)?;
    let verified = game.authority().fen;
    game.tick(0.15);
    let mut conflicting = xfchess_rules::Game::default();
    conflicting.play_uci("d2d4")?;
    let conflict = conflicting.fen();
    // A peer cannot select another legal branch at the same ply.
    game.app
        .world_mut()
        .write_message(NetworkEvent::MessageReceived(
            NetworkMessage::ResyncResponse {
                game_id: 42,
                committed_fen: conflict.clone(),
                committed_turn: 1,
            },
        ));
    game.tick(0.0);
    assert_eq!(game.authority().fen, verified);
    // The independently recorded checkpoint can repair corrupted local state.
    game.app
        .world_mut()
        .resource_mut::<ChessEngine>()
        .set_from_fen(&conflict)?;
    game.app
        .world_mut()
        .write_message(NetworkEvent::MessageReceived(
            NetworkMessage::ResyncResponse {
                game_id: 42,
                committed_fen: verified.clone(),
                committed_turn: 1,
            },
        ));
    game.tick(0.0);
    assert_eq!(game.authority().fen, verified);
    assert_eq!(game.active_motions(), 0);
    assert!(game.piece((4, 3)).is_some());
    assert!(game.piece((3, 3)).is_none());
    assert_eq!(game.published, 1);
    Ok(())
}

#[test]
fn spectator_steps_animate_and_jumps_replace_without_local_moves() -> TestResult {
    use crate::game::systems::spectate_sync::{sync_spectator_board, SpectatorSyncStatus};
    use crate::multiplayer::network::protocol::NetworkMessage;
    use crate::multiplayer::types::NetworkEvent;
    let mut game = Game::start()?;
    game.app.add_message::<NetworkEvent>();
    game.app.init_resource::<SpectatorSyncStatus>();
    let mut session = crate::multiplayer::spectator::SpectatorSession::default();
    session.begin_session("42".into(), None);
    session.delayed = false;
    session.delay_checked = true;
    game.app.insert_resource(session);
    game.app.add_systems(
        Update,
        sync_spectator_board.before(crate::game::systems::board_snapshot::apply_board_snapshot),
    );
    let broadcast = |fen, last_move| {
        NetworkEvent::MessageReceived(NetworkMessage::GameStateBroadcast {
            game_id: 42,
            fen,
            last_move,
            move_number: 1,
            is_check: false,
        })
    };
    game.app
        .world_mut()
        .write_message(broadcast(START.into(), None));
    game.tick(0.0);
    let mut position = xfchess_rules::Game::from_fen(START)?;
    position.play_uci("e2e4")?;
    game.app
        .world_mut()
        .write_message(broadcast(position.fen(), Some("e2e4".into())));
    game.tick(0.0);
    assert!(game.motion((4, 3)).is_some());
    let mut cursor = MessageCursor::<MoveMadeEvent>::default();
    assert!(cursor
        .read(game.app.world().resource::<Messages<MoveMadeEvent>>())
        .all(|event| event.remote));
    for uci in ["e7e5", "g1f3", "b8c6"] {
        position.play_uci(uci)?;
    }
    game.app
        .world_mut()
        .write_message(broadcast(position.fen(), Some("b8c6".into())));
    game.tick(0.0);
    assert_eq!(game.authority().fen, position.fen());
    assert_eq!(game.active_motions(), 0);
    assert_eq!(game.published, 1);
    Ok(())
}

#[test]
fn resync_mid_move_settles_on_the_authoritative_square_without_a_ghost() -> TestResult {
    let mut game = Game::from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1")?;
    game.receive("e4d5", 0.0)?;
    game.tick(0.1);
    assert_eq!(game.active_motions(), 1);

    retire_board_visuals(game.app.world_mut());
    game.tick(0.0);
    game.tick(0.0);

    assert_eq!(game.active_motions(), 0);
    assert_eq!(game.translation((3, 4)), Some(world(3, 4)));
    Ok(())
}

#[test]
fn a_motion_from_an_old_board_is_dropped_and_a_new_one_uses_the_new_epoch() -> TestResult {
    let mut game = Game::start()?;
    game.receive("e2e4", 0.0)?;
    let entity = game.entity_at((4, 3)).ok_or("no piece")?;
    game.app.world_mut().resource_mut::<BoardEpoch>().bump();
    game.tick(0.0);
    game.tick(0.0);
    assert!(game.app.world().get::<PieceMotion>(entity).is_none());

    StartPieceMotion::gameplay(entity, (4, 1), (4, 3)).apply(game.app.world_mut());
    let epoch = game.app.world().resource::<BoardEpoch>().0;
    let fresh = game
        .app
        .world()
        .get::<PieceMotion>(entity)
        .ok_or("no fresh motion")?;
    assert_eq!(fresh.epoch, epoch);
    Ok(())
}
