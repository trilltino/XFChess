use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use xfchess_app::core::{GameMode, GameState, InGameplay};
use xfchess_app::engine::board_state::ChessEngine;
use xfchess_app::game::ai::systems::AIEngineStatus;
use xfchess_app::game::replay::{ParsedPgnGameResource, PgnReplayState};
use xfchess_app::game::resources::{CurrentTurn, PendingGameOver};
use xfchess_app::game::systems::camera::BoardCamera;
use xfchess_app::game::view_mode::ViewMode;
use xfchess_app::game::GamePlugin;
use xfchess_app::rendering::pieces::{Piece, PieceColor, PieceType, PiecesSpawned};
use xfchess_app::rendering::{
    board::{Board, BoardPlugin},
    utils::BoardUtils,
};
use xfchess_app::states::main_menu::PlayerIdentity;
use xfchess_app::PersistentEguiCamera;

fn startup_app(mode: GameMode, view: ViewMode) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, AssetPlugin::default()))
        .init_asset::<bevy::audio::AudioSource>()
        .init_state::<GameState>()
        .add_computed_state::<InGameplay>()
        .insert_resource(mode)
        .insert_resource(view)
        .init_resource::<PersistentEguiCamera>()
        .init_resource::<PlayerIdentity>()
        .init_resource::<PiecesSpawned>()
        .init_resource::<bevy::light::GlobalAmbientLight>()
        .add_plugins((GamePlugin, BoardUtils, BoardPlugin));
    // Exercise state transitions without running rendering, UI or networking.
    app.world_mut().run_schedule(StateTransition);
    app
}

fn enter_game(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.world_mut().run_schedule(StateTransition);
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::InGame
    );
}

#[test]
fn game_entry_completes_for_every_play_mode_and_view() -> Result<(), String> {
    for mode in [
        GameMode::SinglePlayer,
        GameMode::MultiplayerLocal,
        GameMode::MultiplayerCompetitive,
        GameMode::OnlineMultiplayer,
        GameMode::Spectator,
        GameMode::Puzzle,
    ] {
        for view in [ViewMode::Standard3D, ViewMode::Standard2D] {
            let mut app = startup_app(mode, view);
            let stale_piece = app
                .world_mut()
                .spawn(Piece::new(PieceColor::White, PieceType::Pawn, 4, 1))
                .id();
            app.world_mut().resource_mut::<CurrentTurn>().color = PieceColor::Black;
            app.world_mut().resource_mut::<PendingGameOver>().active = true;
            app.world_mut().resource_mut::<AIEngineStatus>().error = Some("old search".into());

            enter_game(&mut app);

            assert!(app.world().get_entity(stale_piece).is_err(), "{mode:?}");
            assert_eq!(
                app.world().resource::<CurrentTurn>().color,
                PieceColor::White
            );
            assert_eq!(
                app.world().resource::<ChessEngine>().current_fen(),
                xfchess_rules::STARTING_FEN
            );
            assert!(!app.world().resource::<PendingGameOver>().active);
            let status = app.world().resource::<AIEngineStatus>();
            assert!(status.error.is_none());
            assert_eq!(
                status.game_id,
                if mode == GameMode::SinglePlayer { 2 } else { 1 }
            );
            let world = app.world_mut();
            let mut cameras = world.query_filtered::<&Camera, With<BoardCamera>>();
            let camera = cameras
                .single(world)
                .map_err(|e| format!("{mode:?}/{view:?}: board camera missing: {e}"))?;
            assert_eq!(camera.is_active, view == ViewMode::Standard3D);
            assert_eq!(world.query::<&PointLight>().iter(world).count(), 2);
            assert_eq!(
                world
                    .query_filtered::<Entity, With<Board>>()
                    .iter(world)
                    .count(),
                64
            );
        }
    }
    Ok(())
}

#[test]
fn replay_entry_loads_pgn_and_creates_scene_in_both_views() -> Result<(), String> {
    for view in [ViewMode::Standard3D, ViewMode::Standard2D] {
        let mut app = startup_app(GameMode::PgnReplay, view);
        app.insert_resource(ParsedPgnGameResource {
            inner: xfchess_rules::parse_pgn("1. e4 e5 *").map_err(|e| e.to_string())?,
            show_eval_graph: false,
            puzzle_mode: false,
            puzzle_revealed: false,
        });

        enter_game(&mut app);

        let replay = app.world().resource::<PgnReplayState>();
        assert_eq!(replay.fen_snapshots.len(), 3);
        assert!(!replay.board_ready);
        assert_eq!(app.world().resource::<AIEngineStatus>().game_id, 1);
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<BoardCamera>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(world.query::<&PointLight>().iter(world).count(), 2);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Board>>()
                .iter(world)
                .count(),
            64
        );
    }
    Ok(())
}
