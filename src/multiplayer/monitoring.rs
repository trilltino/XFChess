use bevy::prelude::*;

use crate::game::resources::history::game_over::GameOverState;
use crate::game::resources::CurrentTurn;
use crate::multiplayer::network::online_game_session::OnlineGameSession;
use crate::multiplayer::types::{CausalChainState, OnlineNetworkState};

const SNAPSHOT_INTERVAL_SECS: f32 = 5.0;

pub struct GameHealthMonitorPlugin;

impl Plugin for GameHealthMonitorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, log_game_health_snapshot_core);
    }
}

fn log_game_health_snapshot_core(
    session: Option<Res<OnlineGameSession>>,
    network_state: Option<Res<OnlineNetworkState>>,
    causal: Option<Res<CausalChainState>>,
    current_turn: Option<Res<CurrentTurn>>,
    game_over: Option<Res<GameOverState>>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    let Some(session) = session else { return };
    if !session.is_configured() {
        *timer = 0.0; // so the next game's first snapshot logs right away
        return;
    }
    if game_over.map(|g| g.is_game_over()).unwrap_or(false) {
        return;
    }

    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = SNAPSHOT_INTERVAL_SECS;

    let game_id: u64 =
        crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id);

    let (gossip_up, peer_count, signing_key_present) = network_state
        .as_ref()
        .map(|n| {
            (
                n.connected,
                n.discovered_peers.len(),
                n.session_signing_key.is_some(),
            )
        })
        .unwrap_or((false, 0, false));

    let roster_size = causal
        .as_ref()
        .and_then(|c| c.roster.get(&game_id))
        .map(|r| r.len())
        .unwrap_or(0);

    let turn_str = current_turn
        .as_ref()
        .map(|t| format!("{:?} move {}", t.color, t.move_number))
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        "[HEALTH] game {} | turn: {} | gossip: {} ({} peer(s)) | signing_key: {} | roster: {} entries",
        game_id,
        turn_str,
        if gossip_up { "up" } else { "DOWN" },
        peer_count,
        if signing_key_present {
            "present"
        } else {
            "MISSING — outgoing messages are unsigned"
        },
        roster_size,
    );
}

#[cfg(feature = "solana")]
pub struct RollupHealthMonitorPlugin;

#[cfg(feature = "solana")]
impl Plugin for RollupHealthMonitorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, log_game_health_snapshot_rollup);
    }
}

#[cfg(feature = "solana")]
fn log_game_health_snapshot_rollup(
    rollup_manager: Option<Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>>,
    magicblock: Option<Res<crate::multiplayer::rollup::magicblock::MagicBlockResolver>>,
    game_over: Option<Res<GameOverState>>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    let Some(rollup_manager) = rollup_manager else {
        return;
    };
    if rollup_manager.game_id == 0 {
        *timer = 0.0;
        return;
    }
    if game_over.map(|g| g.is_game_over()).unwrap_or(false) {
        return;
    }

    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = SNAPSHOT_INTERVAL_SECS;

    let delegated = magicblock.map(|m| m.is_delegated()).unwrap_or(false);

    info!(
        "[HEALTH-ER] game {} | role: {} | signing path: {} | delegated: {}",
        rollup_manager.game_id,
        if rollup_manager.is_creator {
            "host"
        } else {
            "joiner"
        },
        if rollup_manager.used_global_session {
            "global session (zero popups)"
        } else {
            "per-game VPS session key"
        },
        if delegated { "yes" } else { "NOT YET" },
    );
}
