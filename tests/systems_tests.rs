use bevy::prelude::*;
use xfchess_app::engine::board_state::ChessEngine;
use xfchess_app::game::components::{GamePhase, MoveRecord};
use xfchess_app::game::resources::*;
use xfchess_app::game::systems::game_init::reset_game_resources;
use xfchess_app::rendering::{PieceColor, PieceType};

#[test]
fn test_reset_game_resources_system() -> anyhow::Result<()> {
    let mut app = App::new();

    app.insert_resource(CurrentTurn {
        color: PieceColor::Black,
        move_number: 42,
    });
    app.insert_resource(CurrentGamePhase(GamePhase::Check));

    let piece_entity = app.world_mut().spawn_empty().id();
    let selection = Selection {
        selected_entity: Some(piece_entity),
        selected_position: Some((4, 4)),
        ..Default::default()
    };
    app.insert_resource(selection);

    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        promotion: None,
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1),
        to: (4, 3),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });
    app.insert_resource(history);

    app.insert_resource(GameTimer::default()); // Dirtying timer requires pub fields or helper
    app.insert_resource(CapturedPieces::default());
    app.insert_resource(GameOverState::WhiteWon);
    app.insert_resource(TurnStateContext::default());
    app.insert_resource(ChessEngine::default());
    app.insert_resource(ActiveTimeControl::default()); // reset_game_resources reads this to seed GameTimer
    app.init_resource::<xfchess_app::motion::BoardEpoch>();
    app.init_resource::<premove::Premove>();
    app.init_resource::<PendingPromotion>();
    let mut pending_turn = PendingTurnAdvance::default();
    pending_turn.record_accepted(PieceColor::Black);
    app.insert_resource(pending_turn);

    // Register types used in spawn
    app.register_type::<PointLight>();
    app.register_type::<Transform>();
    app.register_type::<GlobalTransform>();

    // 2. Add the system
    // We use Update schedule and run it once
    app.add_systems(Update, reset_game_resources);

    app.update();

    let turn = app
        .world()
        .get_resource::<CurrentTurn>()
        .ok_or_else(|| anyhow::anyhow!("expected game resource is missing"))?;
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 1);

    let phase = app
        .world()
        .get_resource::<CurrentGamePhase>()
        .ok_or_else(|| anyhow::anyhow!("expected game resource is missing"))?;
    assert_eq!(phase.0, GamePhase::Playing);

    let selection = app
        .world()
        .get_resource::<Selection>()
        .ok_or_else(|| anyhow::anyhow!("expected game resource is missing"))?;
    assert!(selection.selected_entity.is_none());

    let game_over = app
        .world()
        .get_resource::<GameOverState>()
        .ok_or_else(|| anyhow::anyhow!("expected game resource is missing"))?;
    assert_eq!(*game_over, GameOverState::Playing);
    assert!(!app.world().resource::<PendingTurnAdvance>().is_pending());
    assert_eq!(
        app.world().resource::<xfchess_app::motion::BoardEpoch>().0,
        1
    );
    assert!(!app.world().resource::<premove::Premove>().is_active());
    Ok(())
}
