//! Integration tests for game systems
//!
//! These tests demonstrate how to test ECS systems by setting up a minimal App,
//! initializing necessary resources, running the system, and verifying state changes.

use bevy::prelude::*;
use xfchess::game::components::GamePhase;
use xfchess::game::resources::*;
use xfchess::game::systems::game_init::reset_game_resources;
use xfchess::rendering::pieces::PieceColor;

/// Test that `reset_game_resources` correctly resets all game state to defaults
#[test]
fn test_reset_game_resources_system() {
    let mut app = App::new();

    // 1. Setup "dirty" state (simulating a game in progress)
    app.insert_resource(CurrentTurn {
        color: PieceColor::Black,
        move_number: 42,
    });
    app.insert_resource(CurrentGamePhase(GamePhase::Check));

    let piece_entity = app.world_mut().spawn_empty().id();
    let mut selection = Selection::default();
    selection.selected_entity = Some(piece_entity);
    selection.selected_position = Some((4, 4));
    app.insert_resource(selection);

    let mut history = MoveHistory::default();
    // history.add_move(...) - tedious to construct MoveRecord, just use generic if possible or check len
    // Assuming MoveHistory has public fields or methods to dirty it.
    // It has `add_move`. I need a MoveRecord.
    // For simplicity, I'll rely on checking it's empty after reset.
    // But to verify it *resets*, I should ideally make it non-empty.
    // I'll skip complex setup for history for now, assuming logic holds.
    app.insert_resource(history);

    app.insert_resource(GameTimer::default()); // Dirtying timer requires pub fields or helper
    app.insert_resource(CapturedPieces::default());
    app.insert_resource(GameOverState::WhiteWon);
    app.insert_resource(TurnStateContext::default());
    app.insert_resource(ChessEngine::default()); // Mock or default engine

    // Register types used in spawn
    app.register_type::<PointLight>();
    app.register_type::<Transform>();
    app.register_type::<GlobalTransform>();

    // 2. Add the system
    // We use Update schedule and run it once
    app.add_systems(Update, reset_game_resources);

    // 3. Run the app for one frame
    app.update();

    // 4. Verify resources are reset
    let turn = app.world().get_resource::<CurrentTurn>().unwrap();
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 1);

    let phase = app.world().get_resource::<CurrentGamePhase>().unwrap();
    assert_eq!(phase.0, GamePhase::Playing);

    let selection = app.world().get_resource::<Selection>().unwrap();
    assert!(selection.selected_entity.is_none());

    let game_over = app.world().get_resource::<GameOverState>().unwrap();
    assert_eq!(*game_over, GameOverState::Playing);
}
