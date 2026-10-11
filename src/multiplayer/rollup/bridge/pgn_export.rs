use super::*;

pub(super) fn handle_game_end_pgn_export(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    rollup_manager: Res<EphemeralRollupManager>,
    profile: Res<crate::multiplayer::solana::addon::SolanaProfile>,
    competitive: Res<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut bridge: ResMut<RollupNetworkBridge>,
) {
    for event in game_ended_events.read() {
        if bridge.pgn_rx.is_some() {
            continue;
        }

        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            event.game_id
        };

        // Prefer wallet usernames and ELO; fall back to generic labels when profiles are unavailable.
        let my_name = if profile.username.is_empty() {
            "You".to_string()
        } else {
            profile.username.clone()
        };
        let opponent_name = if competitive.opponent_username.is_empty() {
            "Opponent".to_string()
        } else {
            competitive.opponent_username.clone()
        };
        let my_elo = (competitive.elo_rating > 0).then_some(competitive.elo_rating);
        let opponent_elo = (competitive.opponent_elo > 0).then_some(competitive.opponent_elo);

        let (white_name, black_name, white_elo, black_elo) = if rollup_manager.is_creator {
            (my_name, opponent_name, my_elo, opponent_elo)
        } else {
            (opponent_name, my_name, opponent_elo, my_elo)
        };

        let result_str = match event.winner.as_deref() {
            Some("white") => "1-0",
            Some("black") => "0-1",
            _ => "1/2-1/2",
        }
        .to_string();

        let (tx, rx) = oneshot::channel();
        bridge.pgn_rx = Some(rx);

        crate::core::net_task::spawn_detached("bridge", move || {
            use crate::game::replay_braid::braid_move_log_to_parsed_pgn_rated;
            use crate::multiplayer::vps_client;

            let moves = match vps_client::fetch_move_log(game_id) {
                Ok(m) => m,
                Err(e) => {
                    warn!(
                        "[PGN-EXPORT] fetch_move_log failed for game {}: {}",
                        game_id, e
                    );
                    let _ = tx.send(None);
                    return;
                }
            };

            let pgn = braid_move_log_to_parsed_pgn_rated(
                &moves,
                &white_name,
                &black_name,
                white_elo,
                black_elo,
                &result_str,
            );
            if pgn.is_none() {
                warn!(
                    "[PGN-EXPORT] Failed to build PGN for game {} ({} moves)",
                    game_id,
                    moves.len()
                );
            }
            let _ = tx.send(pgn);
        });
    }
}

pub(super) fn apply_pgn_export_result(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut commands: Commands,
    mut cached_pgn: Option<ResMut<crate::ui::menus::game_over_popup::CachedGamePgn>>,
) {
    let rx = match bridge.pgn_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(Some(pgn)) => {
            bridge.pgn_rx = None;
            info!(
                "[PGN-EXPORT] Inserting ParsedPgnGameResource ({} moves)",
                pgn.moves.len()
            );

            // Update CachedGamePgn with the authoritative VPS-fetched PGN so that
            // the Review / Analyze / Save PGN buttons use the full Braid move log.
            if let Some(ref mut cached) = cached_pgn {
                let pgn_str = crate::ui::menus::game_over_popup::pgn_to_string(&pgn);
                cached.pgn_string = pgn_str;
                cached.pgn = Some(pgn.clone());
                cached.braid_pgn_ready = true;
                info!("[PGN-EXPORT] CachedGamePgn updated from Braid log");
            }

            commands.insert_resource(ParsedPgnGameResource {
                inner: pgn,
                show_eval_graph: false,
                puzzle_mode: false,
                puzzle_revealed: false,
            });
        }
        Ok(None) => {
            bridge.pgn_rx = None;
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.pgn_rx = None;
        }
    }
}
