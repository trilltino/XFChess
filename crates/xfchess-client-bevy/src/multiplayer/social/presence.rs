use super::*;
use crate::multiplayer::network::vps::{get_node_presence, update_presence, SocialPresence};
use crossbeam_channel::{bounded, Receiver};
pub(super) fn tick_presence_sync(
    friends: Res<FriendsState>,
    game: Res<State<crate::core::GameState>>,
    session: Res<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    hosting: Res<crate::states::main_menu::P2PHostState>,
    mut online: ResMut<OnlinePlayersState>,
) {
    // Drain any in-flight result first.
    if let Some(rx) = online.fetch_rx.as_ref() {
        match rx.try_recv() {
            Ok(count) => {
                online.count = count;
                online.last_sync = Some(Instant::now());
                online.fetch_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                online.fetch_rx = None;
                online.last_sync = Some(Instant::now());
            }
            _ => {}
        }
    }

    if online.fetch_rx.is_some() {
        return;
    }
    let elapsed = online
        .last_sync
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(u64::MAX);
    if elapsed < 2 {
        return;
    }

    // Need a stable identity before we can announce presence.
    let Some(node_id) = friends.our_node_id.clone() else {
        return;
    };
    let pubkey = friends.our_pubkey.clone();
    let display = friends.our_display.clone();
    let profile_client = friends.client().ok();
    let status = if *game.get() == crate::core::GameState::MainMenu {
        "online"
    } else {
        "in_game"
    };
    let game_id = if *game.get() == crate::core::GameState::MainMenu {
        hosting.game_id.clone()
    } else {
        session.is_configured().then(|| session.game_id.clone())
    };

    let (tx, rx) = bounded(1);
    online.fetch_rx = Some(rx);

    crate::core::net_task::spawn_detached("social", move || {
        // Heartbeat — best-effort; ignore failures.
        if let Some(profile_client) = profile_client {
            let _ = profile_client.heartbeat(status, game_id.as_deref(), pubkey.as_deref());
        }
        let count = update_presence(&SocialPresence {
            node_id,
            pubkey,
            display_name: display,
            status: status.to_string(),
            game_id,
            updated_at: chrono::Utc::now().to_rfc3339(),
            transport_node_id: None,
        })
        .unwrap_or(0);
        let _ = tx.send(count);
    });
}

pub(super) fn check_opponent_presence(
    time: Res<Time>,
    mut liveness: ResMut<OpponentLivenessState>,
    game_mode: Res<crate::core::state::GameMode>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    use crate::core::state::GameMode;

    let is_online = matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    );
    if !is_online {
        *liveness = OpponentLivenessState::default();
        return;
    }
    let Some(opponent_node_id) = p2p_conn.as_ref().and_then(|c| c.peer_node_id.clone()) else {
        *liveness = OpponentLivenessState::default();
        return;
    };
    if liveness.opponent_node_id.as_ref() != Some(&opponent_node_id) {
        *liveness = OpponentLivenessState {
            opponent_node_id: Some(opponent_node_id.clone()),
            ..default()
        };
    }

    if let Some(rx) = &liveness.fetch_rx {
        if let Ok(seen) = rx.try_recv() {
            if seen.is_some() {
                liveness.opponent_last_seen = seen;
            }
            liveness.fetch_rx = None;
        }
    }

    liveness.since_last_poll += time.delta_secs();
    if liveness.fetch_rx.is_none() && liveness.since_last_poll >= 2.0 {
        liveness.since_last_poll = 0.0;
        let (tx, rx) = bounded(1);
        liveness.fetch_rx = Some(rx);
        crate::core::net_task::spawn_detached("social", move || {
            let seen = get_node_presence(&opponent_node_id)
                .ok()
                .flatten()
                .and_then(|p| {
                    chrono::DateTime::parse_from_rfc3339(&p.updated_at)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                });
            let _ = tx.send(seen);
        });
    }
}

pub(super) fn fetch_backend_region_once(
    mut region: ResMut<BackendRegion>,
    mut state: Local<RegionFetchState>,
) {
    match &*state {
        RegionFetchState::NotStarted => {
            let (tx, rx) = bounded::<(String, String, u32)>(1);
            *state = RegionFetchState::Pending(rx);
            crate::core::net_task::spawn_detached("social", move || {
                let start = std::time::Instant::now();
                let (tag, label) = crate::multiplayer::vps_client::fetch_region()
                    .unwrap_or_else(|_| ("unknown".to_string(), "Unknown Region".to_string()));
                let latency_ms = start.elapsed().as_millis() as u32;
                let _ = tx.send((tag, label, latency_ms));
            });
        }
        RegionFetchState::Pending(rx) => {
            if let Ok((tag, label, latency_ms)) = rx.try_recv() {
                region.tag = tag;
                region.label = label;
                region.latency_ms = Some(latency_ms);
                *state = RegionFetchState::Done;
            }
        }
        RegionFetchState::Done => {}
    }
}

#[derive(Default)]
pub(super) enum RegionFetchState {
    #[default]
    NotStarted,
    Pending(Receiver<(String, String, u32)>),
    Done,
}
