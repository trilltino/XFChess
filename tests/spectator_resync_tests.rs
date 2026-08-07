//! docs/PRE_MAINNET_E2E_PLAN.md §1.7: `apply_braid_resync_to_spectator`
//! forwards every `RollupEvent::ResyncedMove` it reads straight to
//! `NetworkMoveEvent` with no dedup of its own (real replay safety is
//! `handle_network_moves`' incidental "source square already empty"
//! legality rejection downstream — out of scope for a spectator-layer test,
//! since it needs a full board/piece-query fixture). What *is* this layer's
//! job, and was previously broken: advancing `SpectatorSession::applied_move_count`
//! so the VPS poll (`tick_spectator_poll`) doesn't also re-fetch and
//! re-queue moves already applied via gossip resync. Before this fix the
//! counter was read (`let _ = session.applied_move_count;`) but never
//! written, so it stayed flat forever on the gossip-resync path.
#![cfg(feature = "solana")]

use bevy::prelude::*;
use xfchess::core::states::GameMode;
use xfchess::game::events::NetworkMoveEvent;
use xfchess::multiplayer::rollup::manager::RollupEvent;
use xfchess::multiplayer::spectator::{apply_braid_resync_to_spectator, SpectatorSession};

fn resynced_move(n: u32) -> RollupEvent {
    RollupEvent::ResyncedMove {
        game_id: 1,
        move_uci: format!("e{n}e{n}"),
        next_fen: format!("fen_{n}"),
        move_number: n,
    }
}

fn app_with_session(delayed: bool) -> App {
    let mut app = App::new();
    app.insert_resource(GameMode::Spectator);
    app.insert_resource(SpectatorSession {
        game_id: Some("1".to_string()),
        delayed,
        delay_checked: true,
        ..Default::default()
    });
    app.add_message::<RollupEvent>();
    app.add_message::<NetworkMoveEvent>();
    app.add_systems(Update, apply_braid_resync_to_spectator);
    app
}

#[test]
fn resync_advances_applied_move_count_per_move() {
    let mut app = app_with_session(false);
    app.world_mut().write_message(resynced_move(1));
    app.update();

    let session = app.world().resource::<SpectatorSession>();
    assert_eq!(
        session.applied_move_count, 1,
        "applied_move_count must advance so the VPS poll doesn't re-fetch this move"
    );
    let events = app.world().resource::<Messages<NetworkMoveEvent>>();
    assert_eq!(
        events.len(),
        1,
        "exactly one NetworkMoveEvent per resync move"
    );
}

#[test]
fn resync_advances_once_per_message_across_multiple_moves() {
    let mut app = app_with_session(false);
    app.world_mut().write_message(resynced_move(1));
    app.world_mut().write_message(resynced_move(2));
    app.world_mut().write_message(resynced_move(3));
    app.update();

    let session = app.world().resource::<SpectatorSession>();
    assert_eq!(session.applied_move_count, 3);
    let events = app.world().resource::<Messages<NetworkMoveEvent>>();
    assert_eq!(
        events.len(),
        3,
        "this layer forwards one NetworkMoveEvent per resync message with no dedup of its \
         own — replay safety for an already-applied move is enforced downstream by \
         handle_network_moves' legality check, not here"
    );
}

#[test]
fn resync_is_a_noop_while_broadcast_is_delayed() {
    let mut app = app_with_session(true);
    app.world_mut().write_message(resynced_move(1));
    app.update();

    let session = app.world().resource::<SpectatorSession>();
    assert_eq!(
        session.applied_move_count, 0,
        "a delayed broadcast must never apply live gossip resync moves"
    );
    let events = app.world().resource::<Messages<NetworkMoveEvent>>();
    assert_eq!(events.len(), 0);
}
