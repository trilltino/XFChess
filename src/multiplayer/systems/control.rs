use super::*;

pub fn record_casual_game_on_end(
    game_over: Res<GameOverState>,
    game_mode: Res<crate::core::state::GameMode>,
    players: Option<Res<crate::game::resources::player::Players>>,
    player_identity: Option<Res<crate::states::main_menu::PlayerIdentity>>,
    mut recorded: Local<bool>,
) {
    if !game_over.is_game_over() {
        *recorded = false;
        return;
    }
    if *recorded {
        return;
    }
    if *game_mode != crate::core::state::GameMode::SinglePlayer {
        return;
    }
    *recorded = true;

    let Some(identity) = player_identity else {
        return;
    };
    if identity.is_guest {
        return;
    }
    let Some(token) = identity.jwt_token.clone() else {
        return;
    };

    let Some(players) = players else { return };
    let human_color = if players.player_1.is_human {
        players.player_1.color
    } else if players.player_2.is_human {
        players.player_2.color
    } else {
        return; // no human side (shouldn't happen in SinglePlayer)
    };

    let result = match game_over.winner() {
        Some(winner) if winner == human_color => "win",
        Some(_) => "loss",
        None => "draw",
    };

    let base_url = crate::multiplayer::network::vps::vps_base();
    let result = result.to_string();
    crate::core::net_task::spawn_detached("systems", move || {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "opponent_type": "bot",
            "result": result,
        });
        if let Err(e) = client
            .post(format!("{base_url}/api/games/casual"))
            .bearer_auth(token)
            .json(&body)
            .send()
        {
            warn!("[casual-game] Failed to record bot game result: {}", e);
        }
    });
}

pub fn handle_resync_response(
    mut network_events: MessageReader<NetworkEvent>,
    mut engine: ResMut<crate::engine::board_state::ChessEngine>,
    mut selection: ResMut<crate::game::resources::Selection>,
    session: Res<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::ResyncResponse {
            game_id,
            committed_fen,
            ..
        }) = event
        {
            if !session.active
                || *game_id
                    != crate::multiplayer::network::online_game_session::numeric_game_id(
                        &session.game_id,
                    )
                || session.wager_amount > 0.0
                || {
                    #[cfg(feature = "solana")]
                    {
                        game_sync
                            .as_ref()
                            .is_some_and(|sync| sync.game_id == Some(*game_id))
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        false
                    }
                }
            {
                continue;
            }
            let (Some(incoming_ply), Some(current_ply)) =
                (fen_ply(committed_fen), fen_ply(engine.current_fen()))
            else {
                warn!("[NET] Ignoring malformed resync FEN for game {game_id}");
                continue;
            };
            if incoming_ply < current_ply
                || (incoming_ply == current_ply && committed_fen != engine.current_fen())
            {
                warn!("[NET] Ignoring stale or conflicting resync for game {game_id}");
                continue;
            }
            warn!(
                "[NET] Applying ResyncResponse — overwriting local engine with FEN: {}",
                committed_fen
            );
            let _ = engine.set_from_fen(committed_fen);
            *selection = crate::game::resources::Selection::default();
        }
    }
}

pub(super) fn is_current_game_message(
    msg: &crate::multiplayer::network::protocol::NetworkMessage,
    session: Option<&crate::multiplayer::network::online_game_session::OnlineGameSession>,
) -> bool {
    session.is_some_and(|session| {
        session.active
            && msg.game_id()
                == crate::multiplayer::network::online_game_session::numeric_game_id(
                    &session.game_id,
                )
    })
}

pub fn handle_game_control_messages(
    mut network_events: MessageReader<NetworkEvent>,
    mut draw_offer: MessageWriter<crate::game::events::DrawOfferEvent>,
    mut draw_response: MessageWriter<crate::game::events::DrawResponseEvent>,
    mut rematch_offer: MessageWriter<crate::game::events::RematchOfferEvent>,
    mut rematch_response: MessageWriter<crate::game::events::RematchResponseEvent>,
    mut flag_timeout: MessageWriter<crate::game::events::FlagTimeoutEvent>,
    network_state: Res<OnlineNetworkState>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    for event in network_events.read() {
        let NetworkEvent::MessageReceived(msg) = event else {
            continue;
        };
        if !is_current_game_message(msg, session.as_deref()) {
            continue;
        }

        match msg {
            NetworkMessage::DrawOffer { player, .. } => {
                draw_offer.write(crate::game::events::DrawOfferEvent {
                    player: player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::DrawResponse {
                player, accepted, ..
            } => {
                draw_response.write(crate::game::events::DrawResponseEvent {
                    player: player.clone(),
                    accepted: *accepted,
                    remote: true,
                });
            }
            NetworkMessage::FlagTimeout { flagged_player, .. } => {
                flag_timeout.write(crate::game::events::FlagTimeoutEvent {
                    flagged_player: flagged_player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::RematchOffer { player, .. } => {
                rematch_offer.write(crate::game::events::RematchOfferEvent {
                    player: player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::RematchResponse {
                player, accepted, ..
            } => {
                rematch_response.write(crate::game::events::RematchResponseEvent {
                    player: player.clone(),
                    accepted: *accepted,
                    remote: true,
                });
            }
            NetworkMessage::Ping {
                game_id,
                timestamp_ms,
            } => {
                // Ping/Pong uses gossip only; a live Braid subscription already signals liveness.
                let pong = NetworkMessage::Pong {
                    game_id: *game_id,
                    timestamp_ms: *timestamp_ms,
                };
                if let Some(tx) = &network_state.message_sender {
                    let _ = tx.send(pong);
                }
            }
            _ => {}
        }
    }
}

pub fn send_local_draw_events(
    mut local_draw_offers: MessageReader<crate::game::events::DrawOfferEvent>,
    mut local_draw_responses: MessageReader<crate::game::events::DrawResponseEvent>,
    mut local_rematch_offers: MessageReader<crate::game::events::RematchOfferEvent>,
    mut local_rematch_responses: MessageReader<crate::game::events::RematchResponseEvent>,
    mut local_flag_timeouts: MessageReader<crate::game::events::FlagTimeoutEvent>,
    mut local_resigns: MessageReader<crate::game::events::ResignEvent>,
    network_state: Res<OnlineNetworkState>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    let game_id = session.as_ref().map_or(0, |s| {
        crate::multiplayer::network::online_game_session::numeric_game_id(&s.game_id)
    });
    let Some(tx) = &network_state.message_sender else {
        return;
    };

    for ev in local_draw_offers.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::DrawOffer {
            game_id,
            player: ev.player.clone(),
        });
    }
    for ev in local_draw_responses.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::DrawResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_rematch_offers.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::RematchOffer {
            game_id,
            player: ev.player.clone(),
        });
    }
    for ev in local_rematch_responses.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::RematchResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_flag_timeouts.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::FlagTimeout {
            game_id,
            flagged_player: ev.flagged_player.clone(),
        });
    }
    for ev in local_resigns.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::Resign {
            game_id,
            winner: ev.winner.clone(),
            nonce: 0, // resign doesn't need strict nonce ordering — same as input.rs's exit-resign path
        });
    }
}

pub fn handle_resync_request(
    mut network_events: MessageReader<NetworkEvent>,
    engine: Res<crate::engine::board_state::ChessEngine>,
    network_state: Res<OnlineNetworkState>,
    session: Res<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::ResyncRequest { game_id }) = event {
            if !session.active
                || *game_id
                    != crate::multiplayer::network::online_game_session::numeric_game_id(
                        &session.game_id,
                    )
                || session.wager_amount > 0.0
                || {
                    #[cfg(feature = "solana")]
                    {
                        game_sync
                            .as_ref()
                            .is_some_and(|sync| sync.game_id == Some(*game_id))
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        false
                    }
                }
            {
                continue;
            }
            if let Some(tx) = &network_state.message_sender {
                let response = NetworkMessage::ResyncResponse {
                    game_id: *game_id,
                    committed_fen: engine.current_fen().to_string(),
                    committed_turn: fen_ply(engine.current_fen())
                        .unwrap_or(0)
                        .min(u16::MAX as u64) as u16,
                };
                if let Err(e) = tx.send(response) {
                    warn!("[NET] Failed to send ResyncResponse: {e}");
                } else {
                    info!("[NET] Sent ResyncResponse for game {game_id}");
                }
            }
        }
    }
}

pub fn tick_heartbeat(
    time: Res<Time>,
    mut heartbeat: ResMut<HeartbeatState>,
    network_state: Res<OnlineNetworkState>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
    game_mode: Res<crate::core::state::GameMode>,
) {
    use crate::core::state::GameMode;
    use crate::multiplayer::network::protocol::NetworkMessage;

    if *game_mode != GameMode::OnlineMultiplayer {
        return;
    }

    let dt = time.delta_secs();
    heartbeat.since_last_ping += dt;
    heartbeat.since_last_pong += dt;

    // Send a ping on interval (±0.5 s jitter to spread VPS load across clients).
    if heartbeat.since_last_ping >= heartbeat.ping_interval {
        // Jitter: next interval = 4.5 .. 5.5 s based on system-time sub-millis
        let jitter = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis()
            % 1000) as f32
            / 1000.0
            - 0.5;
        heartbeat.ping_interval = 5.0 + jitter;
        heartbeat.since_last_ping = 0.0;
        if let Some(sess) = session.as_ref() {
            let game_id =
                crate::multiplayer::network::online_game_session::numeric_game_id(&sess.game_id);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let ping = NetworkMessage::Ping {
                game_id,
                timestamp_ms: ts,
            };
            if let Some(tx) = &network_state.message_sender {
                let _ = tx.send(ping);
            }
        }
    }
}

pub fn handle_pong(
    mut network_events: MessageReader<NetworkEvent>,
    mut heartbeat: ResMut<HeartbeatState>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(
            crate::multiplayer::network::protocol::NetworkMessage::Pong { .. },
        ) = event
        {
            heartbeat.since_last_pong = 0.0;
        }
    }
}

pub fn reset_multiplayer_session_state(
    mut p2p_conn: ResMut<crate::multiplayer::network::p2p::P2PConnectionState>,
    mut heartbeat: ResMut<HeartbeatState>,
    mut braid_transport: ResMut<crate::multiplayer::network::braid_transport::BraidTransportState>,
    mut liveness: ResMut<crate::multiplayer::social::OpponentLivenessState>,
    mut pending: ResMut<crate::multiplayer::types::PendingMoveBuffer>,
    #[cfg(feature = "solana")] mut rollup_manager: ResMut<
        crate::multiplayer::rollup::manager::EphemeralRollupManager,
    >,
    #[cfg(feature = "solana")] mut rollup_bridge: ResMut<
        crate::multiplayer::rollup::bridge::RollupNetworkBridge,
    >,
) {
    *p2p_conn = crate::multiplayer::network::p2p::P2PConnectionState::default();
    *heartbeat = HeartbeatState::default();
    braid_transport.reset();
    pending.sequencers.clear();
    pending.oldest_buffered_since.clear();
    // Reset opponent liveness between matches to avoid a false disconnect at kickoff.
    *liveness = crate::multiplayer::social::OpponentLivenessState::default();
    #[cfg(feature = "solana")]
    {
        *rollup_manager = crate::multiplayer::rollup::manager::EphemeralRollupManager::default();
        // Reset stale rollup state when leaving a game, but retain in-flight finalization
        // so dismissing the game-over prompt cannot strand the wager.
        rollup_bridge.reset_preserving_finalization();
    }
    info!("[NET] Reset P2P connection, heartbeat, Braid transport, opponent-liveness, and rollup bridge state on match exit");
}

#[cfg(test)]
mod game_control_scope_tests {
    use super::*;
    use crate::multiplayer::network::online_game_session::OnlineGameSession;

    #[test]
    fn late_control_message_cannot_affect_the_next_game() {
        let current = OnlineGameSession {
            game_id: "22".to_string(),
            active: true,
            ..Default::default()
        };
        let old_timeout = NetworkMessage::FlagTimeout {
            game_id: 21,
            flagged_player: "white".to_string(),
        };
        let current_timeout = NetworkMessage::FlagTimeout {
            game_id: 22,
            flagged_player: "white".to_string(),
        };
        assert!(!is_current_game_message(&old_timeout, Some(&current)));
        assert!(is_current_game_message(&current_timeout, Some(&current)));
        assert!(!is_current_game_message(&current_timeout, None));
    }
}
