use super::*;

pub(super) fn poll_tournament_tasks(
    mut tournament: ResMut<TournamentClientState>,
    mut menu_state: ResMut<NextState<crate::core::states::MenuState>>,
) {
    if let Some(result) = tournament.leave_rx.as_ref().map(|rx| rx.try_recv()) {
        match result {
            Ok(Ok(signature)) => {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "tournament_leave_succeeded",
                    )
                    .tournament_id(
                        tournament
                            .active_tournament_id
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    )
                    .action("leave_tournament")
                    .signature(signature.clone())
                    .status("confirmed"),
                );
                tournament.money_flow =
                    crate::multiplayer::solana::money_flow::MoneyFlowState::confirmed(
                        "leave_tournament",
                        Some(signature),
                    );
                tournament.leave_rx = None;
                tournament.join_status = TournamentJoinStatus::Idle;
                tournament.active_tournament_id = None;
                tournament.registered_players.clear();
                tournament.status_message = "Left tournament.".to_string();
                tournament.stop_chat();
            }
            Ok(Err(e)) => {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new("tournament_leave_failed")
                        .tournament_id(
                            tournament
                                .active_tournament_id
                                .map(|id| id.to_string())
                                .unwrap_or_default(),
                        )
                        .action("leave_tournament")
                        .status("failed")
                        .reason(e.clone()),
                );
                tournament.money_flow =
                    crate::multiplayer::solana::money_flow::MoneyFlowState::failed(
                        "leave_tournament",
                        e.clone(),
                    );
                tournament.leave_rx = None;
                tournament.join_status = TournamentJoinStatus::Error(e.clone());
                tournament.status_message = format!("Leave failed: {e}");
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                tournament.leave_rx = None;
                tournament.money_flow =
                    crate::multiplayer::solana::money_flow::MoneyFlowState::admin_review(
                        "leave_tournament",
                        Option::<String>::None,
                        "leave task dropped before confirmation",
                    );
                tournament.join_status =
                    TournamentJoinStatus::Error("Leave task dropped unexpectedly".to_string());
                tournament.status_message = "Leave failed. Please retry.".to_string();
            }
        }
    }

    if let Some(ref mut rx) = tournament.tx_rx {
        match rx.try_recv() {
            Ok(Ok(slot)) => {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "tournament_registration_succeeded",
                    )
                    .game_id(
                        tournament
                            .active_tournament_id
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    ),
                );
                tournament.my_slot = Some(slot);
                tournament.join_status = TournamentJoinStatus::Registered(slot);
                tournament.status_message =
                    format!("Registered in slot {}. Waiting for bracket…", slot + 1);
                tournament.tx_rx = None;
                menu_state.set(crate::core::states::MenuState::Tournaments);
                info!("[TOURNAMENT] Registration confirmed — slot {}", slot);
            }
            Ok(Err(e)) => {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "tournament_registration_failed",
                    )
                    .game_id(
                        tournament
                            .active_tournament_id
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    )
                    .reason(e.clone()),
                );
                tournament.join_status = TournamentJoinStatus::Error(e.clone());
                tournament.status_message = format!("Registration failed: {}", e);
                tournament.tx_rx = None;
                warn!("[TOURNAMENT] Registration error: {}", e);
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.join_status =
                    TournamentJoinStatus::Error("Task dropped unexpectedly".to_string());
                tournament.status_message = "Registration task failed. Please retry.".to_string();
                tournament.tx_rx = None;
            }
        }
    }
}

pub(super) fn poll_tournament_list(
    mut tournament: ResMut<TournamentClientState>,
    menu_state: Option<Res<State<crate::core::MenuState>>>,
) {
    let is_tournaments_active = menu_state
        .map(|s| *s.get() == crate::core::MenuState::Tournaments)
        .unwrap_or(false);
    let poll_interval_secs: f64 = if is_tournaments_active { 10.0 } else { 30.0 };

    if let Some(ref rx) = tournament.list_rx {
        match rx.try_recv() {
            Ok(Ok(list)) => {
                if list.is_empty() {
                    crate::multiplayer::network::vps::emit_client_event(
                        crate::multiplayer::network::vps::ClientEvent::new("tournament_list_empty")
                            .reason(crate::multiplayer::network::vps::vps_base()),
                    );
                }
                tournament.available_tournaments = list;
                tournament.last_poll_error = None;
                tournament.list_rx = None;
            }
            Ok(Err(e)) => {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new("tournament_list_failed")
                        .reason(e.clone()),
                );
                tournament.last_poll_error = Some(e);
                tournament.list_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.last_poll_error =
                    Some("List poll task ended without responding".to_string());
                tournament.list_rx = None;
            }
        }
    }

    if tournament.list_rx.is_some() {
        return;
    }

    let should_poll = tournament
        .last_list_poll
        .map(|t| t.elapsed().as_secs_f64() >= poll_interval_secs)
        .unwrap_or(true);

    if !should_poll {
        return;
    }

    tournament.last_list_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.list_rx = Some(rx);

    crate::core::net_task::spawn_detached("tournament", move || {
        let result = crate::multiplayer::network::vps::list_tournaments();
        if let Err(ref e) = result {
            warn!("[TOURNAMENT] list_tournaments failed: {}", e);
        }
        let _ = tx.send(result);
    });
}

pub(super) fn poll_expanded_tournament_games(mut tournament: ResMut<TournamentClientState>) {
    if let Some(ref rx) = tournament.detail_games_rx {
        match rx.try_recv() {
            Ok(Ok(games)) => {
                tournament.detail_games = games;
                tournament.detail_games_rx = None;
            }
            Ok(Err(e)) => {
                warn!("[TOURNAMENT] tournament games fetch failed: {e}");
                tournament.detail_games_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => tournament.detail_games_rx = None,
        }
    }

    let Some(expanded) = tournament.expanded_tournament_id else {
        // Collapsed: drop the list so a stale tournament's games can't render
        // under a different card later.
        if tournament.detail_games_for.is_some() {
            tournament.detail_games.clear();
            tournament.detail_games_for = None;
        }
        return;
    };

    // Switching cards must not show the previous tournament's games.
    if tournament.detail_games_for != Some(expanded) {
        tournament.detail_games.clear();
        tournament.detail_games_for = Some(expanded);
        tournament.last_detail_poll = None;
    }

    if tournament.detail_games_rx.is_some() {
        return;
    }
    let should_poll = tournament
        .last_detail_poll
        .map(|t| t.elapsed().as_secs() >= 3)
        .unwrap_or(true);
    if !should_poll {
        return;
    }

    tournament.last_detail_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.detail_games_rx = Some(rx);

    crate::core::net_task::spawn_detached("tournament", move || {
        let _ = tx.send(crate::multiplayer::network::vps::fetch_tournament_games(
            expanded,
        ));
    });
}

pub(super) fn poll_tournament_games_list(
    mut tournament: ResMut<TournamentClientState>,
    menu_state: Option<Res<State<crate::core::MenuState>>>,
) {
    if let Some(ref rx) = tournament.games_rx {
        match rx.try_recv() {
            Ok(games) => {
                tournament.tournament_games = games;
                tournament.games_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.games_rx = None;
            }
        }
    }

    // Only fetch while the browser is actually open — the fetch walks one
    // bracket request per advertised tournament.
    let is_tournaments_active = menu_state
        .map(|s| *s.get() == crate::core::MenuState::Tournaments)
        .unwrap_or(false);
    if !is_tournaments_active || tournament.games_rx.is_some() {
        return;
    }
    let should_poll = tournament
        .last_games_poll
        .map(|t| t.elapsed().as_secs() >= 10)
        .unwrap_or(true);
    if !should_poll {
        return;
    }

    tournament.last_games_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.games_rx = Some(rx);

    crate::core::net_task::spawn_detached("tournament", move || {
        match crate::multiplayer::network::vps::list_tournament_games() {
            Ok(games) => {
                let _ = tx.send(games);
            }
            Err(e) => warn!("[TOURNAMENT] list_tournament_games failed: {}", e),
        }
    });
}

pub(super) fn poll_bracket_fired(mut tournament: ResMut<TournamentClientState>) {
    let Some(ref rx) = tournament.bracket_fired_rx else {
        return;
    };
    if let Ok(event) = rx.try_recv() {
        info!(
            "[TOURNAMENT] BracketFired received: tournament {} — {} players",
            event.tournament_id, event.player_count
        );
        tournament.bracket_ready = true;
        tournament.status_message = format!(
            "Bracket fired! {} players entered. Fetching your match…",
            event.player_count,
        );
        tournament.bracket_fired_rx = None;
    }
}

pub(super) const MY_STATUS_POLL_SECS: f32 = 3.0;

pub(super) fn poll_my_tournament_status(
    time: Res<Time>,
    mut tournament: ResMut<TournamentClientState>,
    solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    mut assigned: MessageWriter<TournamentMatchAssignedEvent>,
) {
    let Some(tournament_id) = tournament.active_tournament_id else {
        return;
    };
    let Some(wallet) = solana_state.as_ref().and_then(|s| s.wallet_pubkey) else {
        return;
    };

    if let Some(rx) = tournament.status_rx.as_ref() {
        match rx.try_recv() {
            Ok(Ok(status)) => {
                tournament.status_rx = None;
                apply_my_status(&mut tournament, tournament_id, status, &mut assigned);
            }
            Ok(Err(e)) => {
                tournament.status_rx = None;
                tournament.last_poll_error = Some(e);
            }
            Err(crossbeam_channel::TryRecvError::Empty) => return,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                tournament.status_rx = None;
            }
        }
    }

    tournament.poll_timer += time.delta_secs();
    if tournament.poll_timer < MY_STATUS_POLL_SECS {
        return;
    }
    tournament.poll_timer = 0.0;

    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.status_rx = Some(rx);
    let wallet_str = wallet.to_string();
    crate::core::net_task::spawn_detached("tournament", move || {
        let result =
            crate::multiplayer::network::vps::my_tournament_status(tournament_id, &wallet_str);
        let _ = tx.send(result);
    });
}

pub(super) fn apply_my_status(
    tournament: &mut TournamentClientState,
    tournament_id: u64,
    status: crate::multiplayer::network::vps::MyTournamentStatus,
    assigned: &mut MessageWriter<TournamentMatchAssignedEvent>,
) {
    use crate::multiplayer::network::vps::PlayerState;

    tournament.last_poll_error = None;
    tournament.my_state = Some(status.state);
    tournament.placing = status.placing;
    tournament.prize_lamports = status.prize_lamports;

    if let Some(ref last) = status.last_result {
        tournament.last_match_result = Some(if last.won {
            format!("Round {} — win", last.round + 1)
        } else {
            format!("Round {} — loss", last.round + 1)
        });
    }

    match status.state {
        PlayerState::MatchReady => {
            let Some(m) = status.my_match else { return };
            let Some(game_id) = m.game_id else { return };
            // Already in this game — nothing to do.
            if tournament.active_game_id == Some(game_id) {
                return;
            }
            tournament.waiting_for_next_match = false;
            tournament.status_message = format!(
                "Round {} — your match is ready",
                status.round.map(|r| r + 1).unwrap_or(1)
            );
            info!(
                "[TOURNAMENT] Match ready in tournament {} (game {}, vs {})",
                tournament_id, game_id, m.opponent_pubkey
            );
            assigned.write(TournamentMatchAssignedEvent {
                tournament_id,
                match_index: m.match_index as u8,
                game_id: Some(game_id),
                opponent_pubkey: m.opponent_pubkey,
                opponent_node_id: m.opponent_node_id,
                your_color: m.your_color,
            });
        }
        PlayerState::AwaitingOpponent | PlayerState::AwaitingGameId => {
            // Won (or seated) but nothing to enter yet. Clear the stale game
            // id so the next MatchReady isn't suppressed by the guard above.
            tournament.active_game_id = None;
            tournament.waiting_for_next_match = true;
            tournament.status_message = match status.blocked_by {
                Some(ref b) if b.matches_remaining > 0 => format!(
                    "Waiting for round {} to finish — {} match{} remaining",
                    b.round + 1,
                    b.matches_remaining,
                    if b.matches_remaining == 1 { "" } else { "es" }
                ),
                _ => "Waiting for your next opponent…".to_string(),
            };
        }
        PlayerState::Eliminated | PlayerState::Champion | PlayerState::Cancelled => {
            tournament.waiting_for_next_match = false;
            tournament.active_game_id = None;
            tournament.status_message = match status.state {
                PlayerState::Champion => "You won the tournament".to_string(),
                PlayerState::Cancelled => "Tournament cancelled".to_string(),
                _ => match status.placing {
                    Some(p) => format!("Eliminated — finished #{p}"),
                    None => "Eliminated".to_string(),
                },
            };
        }
        PlayerState::Registered => {
            tournament.waiting_for_next_match = false;
            tournament.status_message = "Registered — waiting for the tournament to start".into();
        }
        PlayerState::NotRegistered => {
            tournament.waiting_for_next_match = false;
        }
    }
}
