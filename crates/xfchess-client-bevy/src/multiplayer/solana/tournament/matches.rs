use super::*;

pub(super) fn handle_tournament_match_assigned(
    mut events: MessageReader<TournamentMatchAssignedEvent>,
    mut tournament: ResMut<TournamentClientState>,
    solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    network_state: Res<crate::multiplayer::types::OnlineNetworkState>,
    network_config: Res<crate::multiplayer::types::NetworkConfig>,
    mut online_session: ResMut<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    mut p2p_state: ResMut<crate::multiplayer::network::p2p::P2PConnectionState>,
    mut connect_events: MessageWriter<crate::multiplayer::network::p2p::ConnectToPeerEvent>,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut ai_config: ResMut<crate::game::ai::ChessAIResource>,
    mut next_game_state: ResMut<NextState<crate::core::GameState>>,
    mut game_started: MessageWriter<crate::game::events::GameStartedEvent>,
    mut solana_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
) {
    for ev in events.read() {
        let game_id = match ev.game_id {
            Some(id) => id,
            None => {
                warn!(
                    "[TOURNAMENT] Match assigned for tournament {} but game_id is None — skipping session setup",
                    ev.tournament_id
                );
                continue;
            }
        };

        if tournament.active_game_id == Some(game_id) {
            continue;
        }

        let wallet_str = solana_state
            .as_ref()
            .and_then(|s| s.wallet_pubkey)
            .map(|pk| pk.to_string())
            .unwrap_or_default();

        if wallet_str.is_empty() {
            warn!(
                "[TOURNAMENT] No wallet for tournament {} game {} session setup",
                ev.tournament_id, game_id
            );
            continue;
        }

        let tournament_id = ev.tournament_id;
        let is_white = ev.your_color == "white";
        let wallet = wallet_str.clone();
        let game_id_str = game_id.to_string();
        let local_node_id = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string());
        let opponent_node_id = ev.opponent_node_id.clone();

        if let Some(ref peer_node_id) = opponent_node_id {
            // Use is_white as a consistent host tie-breaker. Connection upgrades transport;
            // this system independently starts the tournament game.
            connect_events.write(crate::multiplayer::network::p2p::ConnectToPeerEvent {
                peer_node_id: peer_node_id.clone(),
                is_host: is_white,
                drive_game_start: false,
            });
        }

        if let Some(ref node_id) = local_node_id {
            announce_or_join_tournament_relay(
                game_id_str.clone(),
                node_id.clone(),
                wallet_str.clone(),
                is_white,
            );
        } else {
            warn!(
                "[TOURNAMENT] Local online node id not ready for relay setup on game {}",
                game_id
            );
        }

        crate::multiplayer::network::online_game_session::start_session(
            &mut online_session,
            network_config.vps_base_url.clone(),
            game_id_str,
            0.0,
            &network_state,
        );

        p2p_state.local_node_id = network_state.node_id;
        p2p_state.peer_node_id = opponent_node_id;
        p2p_state.game_id = Some(game_id);
        p2p_state.is_host = is_white;
        p2p_state.player_color = Some(if is_white {
            crate::rendering::pieces::PieceColor::White
        } else {
            crate::rendering::pieces::PieceColor::Black
        });
        p2p_state.status = crate::multiplayer::network::p2p::P2PConnectionStatus::InGame;

        *core_mode = crate::core::GameMode::OnlineMultiplayer;
        ai_config.mode = crate::game::ai::resource::AiMode::Multiplayer;
        solana_sync.game_id = Some(game_id);
        solana_sync.wager_amount = 0;
        // Tournament games run through ER delegation even with a zero wager.
        solana_sync.requires_delegation = true;
        competitive.game_id = Some(game_id);
        competitive.wager_lamports = 0;
        competitive.active = true;
        rollup_manager.assign_game(game_id);
        rollup_manager.is_creator = is_white;
        crate::multiplayer::network::game_id_store::set(game_id);

        game_started.write(crate::game::events::GameStartedEvent { game_id });
        next_game_state.set(crate::core::GameState::InGame);
        tournament.active_game_id = Some(game_id);
        tournament.waiting_for_next_match = false;

        crate::core::net_task::spawn_detached("tournament", move || {
            use crate::multiplayer::vps_client;
            if is_white {
                match vps_client::tournament_session_create_game(tournament_id, game_id, &wallet) {
                    Ok(session_pk) => info!(
                        "[TOURNAMENT] Session created for game {} → {}",
                        game_id, session_pk
                    ),
                    Err(e) => error!(
                        "[TOURNAMENT] session-create-game failed for game {}: {e}",
                        game_id
                    ),
                }
            } else {
                match vps_client::tournament_session_join_game(tournament_id, game_id, &wallet) {
                    Ok(session_pk) => info!(
                        "[TOURNAMENT] Session joined for game {} → {}",
                        game_id, session_pk
                    ),
                    Err(e) => error!(
                        "[TOURNAMENT] session-join-game failed for game {}: {e}",
                        game_id
                    ),
                }
            }
        });

        tournament.status_message = format!(
            "Match assigned — setting up game session for game {}…",
            game_id
        );
    }
}

pub(super) fn announce_or_join_tournament_relay(
    game_id: String,
    local_node_id: String,
    wallet: String,
    is_white: bool,
) {
    crate::core::net_task::spawn_detached("tournament", move || {
        use crate::multiplayer::vps_client;
        if is_white {
            match vps_client::p2p_announce_game(
                game_id.clone(),
                &local_node_id,
                &format!("Tournament {}", &wallet[..wallet.len().min(8)]),
                0.0,
                "tournament_match",
                0,
                0,
                Some(wallet),
                None,
                None,
            ) {
                Ok(()) => info!("[TOURNAMENT] Relay announced game {}", game_id),
                Err(e) => warn!(
                    "[TOURNAMENT] Relay announce failed for game {}: {}",
                    game_id, e
                ),
            }
        } else {
            for attempt in 1..=15 {
                match vps_client::p2p_join_game(game_id.clone(), &local_node_id) {
                    Ok(Some(host_id)) => {
                        info!(
                            "[TOURNAMENT] Relay joined game {} via host {}",
                            game_id, host_id
                        );
                        return;
                    }
                    Ok(None) => {
                        info!("[TOURNAMENT] Relay joined game {}", game_id);
                        return;
                    }
                    Err(e) => {
                        if attempt == 15 {
                            warn!(
                                "[TOURNAMENT] Relay join failed for game {} after retries: {}",
                                game_id, e
                            );
                        } else {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            }
        }
    });
}
