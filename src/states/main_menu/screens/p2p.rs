use super::*;

pub(in super::super) fn render_host_p2p_config_screen(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
) {
    // (The welcome card moved to the startup main menu — see render_welcome_panel.)
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new(if ctx.p2p_host.direct_mode {
                "Direct Connection — Host"
            } else {
                "Create Lobby"
            })
            .size(24.0)
            .color(egui::Color32::from_rgb(100, 200, 255))
            .strong(),
        );
        if ctx.p2p_host.direct_mode {
            ui.label(
                egui::RichText::new(
                    "Not listed anywhere — only whoever you give your ID to can join.",
                )
                .size(12.0)
                .color(egui::Color32::GRAY),
            );
        }
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label(
                egui::RichText::new("Room Name")
                    .size(15.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut ctx.p2p_host.lobby_name)
                    .hint_text("Leave blank for your username")
                    .desired_width(f32::INFINITY),
            );
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(
                egui::RichText::new("Time Control")
                    .size(15.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(8.0);

            // Preset mode buttons
            ui.horizontal_wrapped(|ui| {
                for (label, base_min, inc_sec) in [
                    ("Bullet 1+0", 1u32, 0u16),
                    ("Blitz 3+2", 3, 2),
                    ("Blitz 5+0", 5, 0),
                    ("Rapid 10+0", 10, 0),
                    ("Rapid 15+10", 15, 10),
                    ("30 min", 30, 0),
                ] {
                    let selected = ctx.p2p_host.base_time_minutes == base_min
                        && ctx.p2p_host.increment_seconds == inc_sec;
                    let response = crate::ui::styles::StyledButton::chip(
                        ui,
                        label,
                        selected,
                        egui::Vec2::new(0.0, 28.0),
                    );
                    if response.clicked() {
                        ctx.p2p_host.base_time_minutes = base_min;
                        ctx.p2p_host.increment_seconds = inc_sec;
                    }
                    ui.add_space(3.0);
                }
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Custom")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label("Minutes per side:");
                ui.add(egui::Slider::new(
                    &mut ctx.p2p_host.base_time_minutes,
                    1..=60,
                ));
            });
            ui.horizontal(|ui| {
                ui.label("Increment (seconds):");
                ui.add(egui::Slider::new(
                    &mut ctx.p2p_host.increment_seconds,
                    0..=60,
                ));
            });
        });

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            // Center the button pair: pad by half the leftover width (fixed
            // button sizes make the row width deterministic).
            ui.spacing_mut().item_spacing.x = 12.0;
            let (cancel_w, start_w) = (110.0, 190.0);
            ui.add_space(((ui.available_width() - (cancel_w + 12.0 + start_w)) / 2.0).max(0.0));

            if ui
                .add_sized(
                    [cancel_w, 40.0],
                    egui::Button::new(egui::RichText::new("Cancel").size(16.0)),
                )
                .clicked()
            {
                ctx.menu_state.set(crate::core::MenuState::Main);
            }

            let node_id_ready = ctx
                .network_state
                .as_ref()
                .map(|ns| ns.node_id.is_some())
                .unwrap_or(false);

            let start_btn = ui.add_enabled(
                node_id_ready,
                egui::Button::new(
                    egui::RichText::new("Start Hosting")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(40, 140, 80))
                .min_size(egui::vec2(start_w, 44.0)),
            );

            if start_btn.clicked() {
                let game_id = generate_room_code(ctx.p2p_host.base_time_minutes);
                ctx.p2p_host.game_id = Some(game_id.clone());
                ctx.p2p_host.last_heartbeat = Some(std::time::Instant::now());

                // Only direct hosts let the Iroh handshake start the game; public lobbies require GAME_START.
                if let Some(host_events) = &mut ctx.host_game_events {
                    host_events.write(crate::multiplayer::network::p2p::HostGameEvent {
                        drive_game_start: ctx.p2p_host.direct_mode,
                    });
                }

                // Announce to VPS (off render thread — blocking HTTP with 120s timeout)
                let display_name = if ctx.p2p_host.lobby_name.trim().is_empty() {
                    ctx.player_identity.display_name().to_string()
                } else {
                    ctx.p2p_host.lobby_name.trim().to_string()
                };
                let host_node_id = ctx
                    .network_state
                    .as_ref()
                    .and_then(|ns| {
                        ns.node_id
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                    })
                    .unwrap_or_default();
                // Direct hosts share node IDs out of band and never announce to the public lobby directory.
                if !ctx.p2p_host.direct_mode {
                    // Register with the host-side relay poller so we detect joiners via HTTP
                    if let Some(ref mut vps) = ctx.p2p_vps_state {
                        vps.hosting_game_id = Some(game_id.clone());
                        vps.hosting_node_id = Some(host_node_id.clone());
                        vps.host_poll_last = None; // trigger immediately
                        vps.hosting_stake_amount = 0.0;
                        vps.hosting_base_secs = ctx.p2p_host.base_time_minutes * 60;
                        vps.hosting_inc = ctx.p2p_host.increment_seconds;
                        vps.pending_joiner = None; // clear any stale joiner from previous session
                    }
                    {
                        let gid = game_id.clone();
                        let nid = host_node_id;
                        let dn = display_name;
                        let stake = 0.0_f64;
                        let base_secs = ctx.p2p_host.base_time_minutes * 60;
                        let inc = ctx.p2p_host.increment_seconds;
                        crate::core::net_task::spawn_detached("screens", move || {
                            match crate::multiplayer::vps_client::p2p_announce_game(
                                gid,
                                &nid,
                                &dn,
                                stake,
                                "P2P",
                                base_secs,
                                inc,
                                Some(dn.clone()),
                                None,
                                None,
                            ) {
                                Ok(()) => info!("[LOBBY] P2P game announced to VPS relay"),
                                Err(e) => warn!("[LOBBY] P2P announce failed: {}", e),
                            }
                        });
                    }
                }

                info!(
                    "[LOBBY] Hosting P2P game: {} ({} + {}) direct_mode={}",
                    game_id,
                    ctx.p2p_host.base_time_minutes,
                    ctx.p2p_host.increment_seconds,
                    ctx.p2p_host.direct_mode
                );

                // Transition to Waiting Screen
                ctx.menu_state.set(crate::core::MenuState::P2PWaiting);

                // Also update internal connection state
                if let Some(ref mut p2p_state) = ctx.p2p_state {
                    p2p_state.status = P2PConnectionStatus::Hosting;
                }
            }

            if !node_id_ready {
                ui.label(
                    egui::RichText::new("Wait for P2P initialization…")
                        .size(11.0)
                        .color(egui::Color32::RED),
                );
            }
        });
    });
}

pub(in super::super) fn render_p2p_waiting_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    // This popup serves both peers; p2p_host is populated only for the host.
    let is_joiner = ctx.p2p_host.game_id.is_none();
    if is_joiner {
        render_p2p_joiner_waiting_screen(ui, ctx);
        return;
    }

    // Refresh relay lobbies every 25s, below their 90s TTL. Direct Connection
    // hosts have no relay announcement to refresh.
    let should_heartbeat = !ctx.p2p_host.direct_mode
        && ctx
            .p2p_host
            .last_heartbeat
            .map(|t| t.elapsed().as_secs() >= 25)
            .unwrap_or(false); // announce just happened, first heartbeat at 25s
    if should_heartbeat {
        if let Some(game_id) = ctx.p2p_host.game_id.clone() {
            let node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_default();
            ctx.p2p_host.last_heartbeat = Some(std::time::Instant::now());
            crate::core::net_task::spawn_detached("screens", move || {
                if let Err(e) = crate::multiplayer::vps_client::p2p_heartbeat(game_id, &node_id) {
                    warn!("[LOBBY] Heartbeat failed: {}", e);
                }
            });
        }
    }

    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(
            egui::RichText::new("WAITING FOR OPPONENT")
                .size(24.0)
                .color(egui::Color32::GOLD)
                .strong(),
        );
        ui.add_space(20.0);

        if ctx.p2p_host.direct_mode {
            ui.label(
                egui::RichText::new("Send this ID to your friend so they can join you.")
                    .size(14.0)
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(8.0);
            let node_id = crate::multiplayer::network::identity::node_id_b58();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&node_id)
                        .size(12.0)
                        .color(egui::Color32::GRAY)
                        .monospace(),
                );
                if ui
                    .small_button("Copy")
                    .on_hover_text("Copy your node ID to clipboard")
                    .clicked()
                {
                    ui.output_mut(|o| {
                        o.commands
                            .push(egui::OutputCommand::CopyText(node_id.clone()))
                    });
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Your game is now visible in the lobby.")
                    .size(14.0)
                    .color(egui::Color32::WHITE),
            );
        }
        ui.add_space(30.0);

        // Show joiner identity once detected, along with the Start Game button
        // that actually begins the match — joining no longer auto-starts it.
        let pending_joiner = ctx
            .p2p_vps_state
            .as_ref()
            .and_then(|vps| vps.pending_joiner.clone());
        if let Some(ref joiner) = pending_joiner {
            ui.label(
                egui::RichText::new("Opponent found!")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(100, 220, 100))
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(&joiner.display_name)
                    .size(22.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            // This screen is always the casual relay flow, without a wallet-backed wager.
            ui.label(
                egui::RichText::new("(local)")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(160, 160, 175)),
            );
            if joiner.elo_str != "—" {
                ui.label(
                    egui::RichText::new(format!("ELO  {}", joiner.elo_str))
                        .size(14.0)
                        .color(egui::Color32::GOLD),
                );
            }

            ui.add_space(16.0);

            if ui
                .add_sized(
                    [180.0, 40.0],
                    egui::Button::new(
                        egui::RichText::new("Start Game")
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(40, 140, 80))
                    .corner_radius(6.0),
                )
                .clicked()
            {
                start_p2p_host_game(ctx);
            }
        }

        ui.add_space(40.0);

        // Friends invite panel
        {
            let game_id = ctx.p2p_host.game_id.clone().unwrap_or_default();
            let host_node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_default();
            let online_contacts: Vec<_> = ctx
                .friends
                .contacts
                .iter()
                .filter(|c| c.is_online)
                .cloned()
                .collect();
            if !online_contacts.is_empty() {
                ui.separator();
                ui.label(
                    egui::RichText::new("Invite a Friend")
                        .size(13.0)
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(4.0);
                for contact in online_contacts {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&contact.contact_display)
                                .size(12.0)
                                .color(egui::Color32::WHITE),
                        );
                        if ui.small_button("Invite").clicked()
                            && !game_id.is_empty()
                            && !host_node_id.is_empty()
                        {
                            let gid = game_id.clone();
                            let nid = host_node_id.clone();
                            let from_display = ctx.friends.our_display.clone();
                            let to_node = contact.contact_node_id.clone();
                            crate::core::net_task::spawn_detached("screens", move || {
                                if let Err(e) = crate::multiplayer::vps_client::push_lobby_invite(
                                    &gid,
                                    &nid,
                                    &from_display,
                                    &to_node,
                                ) {
                                    warn!(
                                        "[INVITE] Failed to send lobby invite to {}: {}",
                                        to_node, e
                                    );
                                } else {
                                    info!("[INVITE] Sent lobby invite to {}", to_node);
                                }
                            });
                        }
                    });
                }
                ui.add_space(12.0);
            }
        }

        if ui
            .button(
                egui::RichText::new("Cancel Hosting")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(255, 100, 100)),
            )
            .clicked()
        {
            if let Some(game_id) = ctx.p2p_host.game_id.clone() {
                let wagered = ctx.p2p_host.stake_amount > 0.0;
                let node_id = ctx
                    .network_state
                    .as_ref()
                    .and_then(|ns| {
                        ns.node_id
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                    })
                    .unwrap_or_default();
                // Non-blocking — don't freeze the render thread on cancel
                if wagered {
                    #[cfg(feature = "solana")]
                    {
                        if let Ok(game_id_u64) = game_id.parse::<u64>() {
                            if let Some(mut lobby) = ctx.solana_lobby.take() {
                                lobby.wager_sol = ctx.p2p_host.stake_amount as f32;
                                lobby.cached_node_id = Some(node_id.clone());
                                begin_solana_lobby_cancel(ctx, &mut lobby, game_id_u64);
                                ctx.solana_lobby = Some(lobby);
                                ctx.menu_state.set(crate::core::MenuState::SolanaLobby);
                            } else {
                                warn!(
                                    "[LOBBY] Cancel Hosting: Solana lobby state unavailable for wagered game {game_id}; keeping host state for retry"
                                );
                            }
                        } else {
                            warn!(
                                "[LOBBY] Cancel Hosting: wagered game id {game_id} is not numeric; keeping host state for owner review"
                            );
                        }
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        warn!(
                            "[LOBBY] Cancel Hosting: wagered game {game_id} cannot be cancelled without solana feature"
                        );
                    }
                } else {
                    let game_id = game_id;
                    crate::core::net_task::spawn_detached("screens", move || {
                        if let Err(e) =
                            crate::multiplayer::vps_client::p2p_leave_game(game_id.clone(), &node_id)
                        {
                            warn!("[LOBBY] Leave failed on cancel: {}", e);
                        } else {
                            info!("[LOBBY] Cancelled hosting for {}", game_id);
                        }
                    });
                }

                // Wagered lobby cancellation is handled above by the tracked
                // Solana cancel path. Free PvP relay cleanup remains local.
            }

            if ctx.p2p_host.stake_amount <= 0.0 {
                ctx.p2p_host.game_id = None;
                ctx.p2p_host.last_heartbeat = None;
                if let Some(ref mut p2p_state) = ctx.p2p_state {
                    p2p_state.status = P2PConnectionStatus::Disconnected;
                }
                // Stop host relay polling
                if let Some(ref mut vps) = ctx.p2p_vps_state {
                    vps.hosting_game_id = None;
                    vps.hosting_node_id = None;
                    vps.pending_joiner = None;
                }
                ctx.menu_state.set(crate::core::MenuState::BraidLobby);
            }
        }
    });
}

pub(super) fn render_p2p_joiner_waiting_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(
            egui::RichText::new("WAITING FOR HOST")
                .size(24.0)
                .color(egui::Color32::GOLD)
                .strong(),
        );
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("You've joined the game.")
                .size(14.0)
                .color(egui::Color32::WHITE),
        );
        ui.label(
            egui::RichText::new("Waiting for the host to start the match…")
                .size(13.0)
                .color(egui::Color32::GRAY),
        );

        // Explain delayed GAME_START delivery so lost messages do not resemble an idle host.
        let waited_secs = ctx
            .p2p_vps_state
            .as_ref()
            .and_then(|vps| vps.joining_since)
            .map(|since| since.elapsed().as_secs())
            .unwrap_or(0);
        if waited_secs >= 20 {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(format!(
                    "Still waiting after {}s — if the host already clicked Start Game, \
                     the signal may not have reached you. Try leaving and rejoining.",
                    waited_secs
                ))
                .size(12.0)
                .color(egui::Color32::from_rgb(230, 160, 60)),
            );
        }

        ui.add_space(40.0);

        if ui
            .button(
                egui::RichText::new("Leave")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(255, 100, 100)),
            )
            .clicked()
        {
            let left_game_id = ctx
                .p2p_vps_state
                .as_mut()
                .and_then(|vps| vps.joining_game_id.take());
            if let Some(ref mut vps) = ctx.p2p_vps_state {
                vps.joining_host_node_id = None;
                vps.joining_stake_amount = 0.0;
                vps.joining_since = None;
            }
            if let Some(game_id) = left_game_id {
                let node_id = ctx
                    .network_state
                    .as_ref()
                    .and_then(|ns| {
                        ns.node_id
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                    })
                    .unwrap_or_default();
                crate::core::net_task::spawn_detached("screens", move || {
                    if let Err(e) =
                        crate::multiplayer::vps_client::p2p_leave_game(game_id.clone(), &node_id)
                    {
                        warn!("[LOBBY] Leave failed: {}", e);
                    } else {
                        info!("[LOBBY] Left game {} before it started", game_id);
                    }
                });
            }
            if let Some(ref mut p2p_state) = ctx.p2p_state {
                p2p_state.status = P2PConnectionStatus::Disconnected;
            }
            ctx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
    });
}

pub(super) fn start_p2p_host_game(ctx: &mut MainMenuUIContext) {
    let Some(game_id) = ctx.p2p_host.game_id.clone() else {
        return;
    };
    let stake = ctx
        .p2p_vps_state
        .as_ref()
        .map(|vps| vps.hosting_stake_amount)
        .unwrap_or(0.0);

    let node_id = ctx
        .network_state
        .as_ref()
        .and_then(|ns| {
            ns.node_id
                .map(|id| bs58::encode(id.as_bytes()).into_string())
        })
        .unwrap_or_default();
    let secret_key_bytes = ctx
        .network_state
        .as_ref()
        .and_then(|ns| ns.secret_key_bytes);
    let game_start_msg = format!("GAME_START:1|{}", ctx.player_identity.display_name());
    {
        let gid = game_id.clone();
        let nid = node_id;
        crate::core::net_task::spawn_detached("screens", move || {
            // Retry GAME_START delivery because the host enters locally without
            // waiting for this send; a dropped request would strand the joiner.
            let mut delivered = false;
            for attempt in 1..=5u32 {
                let send_result = match secret_key_bytes {
                    Some(sk) => crate::multiplayer::vps_client::p2p_send_message(
                        gid.clone(),
                        &nid,
                        &game_start_msg,
                        &sk,
                    ),
                    None => Err("no signing key available yet".to_string()),
                };
                match send_result {
                    Ok(()) => {
                        info!("[LOBBY] Sent GAME_START for {} (attempt {})", gid, attempt);
                        delivered = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "[LOBBY] GAME_START send failed (attempt {}/5) for {}: {}",
                            attempt, gid, e
                        );
                        std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
                    }
                }
            }
            if !delivered {
                error!(
                    "[LOBBY] GAME_START never delivered for {} after 5 attempts — joiner is likely stuck on the waiting screen",
                    gid
                );
            }
            // Mark the relay listing InProgress as best-effort bookkeeping; failure
            // must not block play.
            if let Err(e) = crate::multiplayer::vps_client::p2p_accept_join(gid.clone(), &nid) {
                warn!("[LOBBY] p2p_accept_join failed for {}: {}", gid, e);
            }
        });
    }

    if stake > 0.0 {
        #[cfg(feature = "solana")]
        {
            info!("[LOBBY] Wagered game — routing host to Solana Lobby to create on-chain game.");
            ctx.menu_state.set(crate::core::MenuState::SolanaLobby);
            if let Some(ref mut lobby) = ctx.solana_lobby {
                lobby.game_id_input = game_id.clone();
                lobby.wager_sol = stake as f32;
                lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                lobby.allow_create = true;
            }
        }
        #[cfg(not(feature = "solana"))]
        {
            warn!(
                "[LOBBY] Wagered game requested but solana feature not enabled — starting as free game."
            );
            enter_p2p_host_game(ctx, &game_id);
        }
    } else {
        enter_p2p_host_game(ctx, &game_id);
    }

    if let Some(ref mut vps) = ctx.p2p_vps_state {
        vps.pending_joiner = None;
        vps.hosting_stake_amount = 0.0;
    }
}

pub(super) fn time_control_category(base_time_minutes: u32) -> (&'static str, &'static str) {
    match base_time_minutes {
        0..=2 => ("BU", "Bullet"),
        3..=9 => ("BZ", "Blitz"),
        10..=29 => ("RA", "Rapid"),
        _ => ("CL", "Classical"),
    }
}

pub(super) fn generate_room_code(base_time_minutes: u32) -> String {
    let (abbrev, _) = time_control_category(base_time_minutes);
    const CHARS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let suffix: String = (0..4)
        .map(|_| CHARS[rand::random::<u32>() as usize % CHARS.len()] as char)
        .collect();
    format!("{abbrev}{base_time_minutes}-{suffix}")
}

pub(super) fn enter_p2p_host_game(ctx: &mut MainMenuUIContext, game_id: &str) {
    // Clear prior on-chain context before entering a casual game.
    #[cfg(feature = "solana")]
    crate::multiplayer::solana::addon::clear_on_chain_game_state(
        ctx.solana_sync.as_deref_mut(),
        ctx.competitive.as_deref_mut(),
    );

    ctx.ai_config.mode = crate::game::ai::resource::AiMode::Multiplayer;
    *ctx.core_mode = crate::core::GameMode::OnlineMultiplayer;
    if let Some(ref mut p2p) = ctx.p2p_state {
        p2p.is_host = true;
        p2p.player_color = Some(crate::rendering::pieces::PieceColor::White);
        p2p.status = P2PConnectionStatus::InGame;
    }
    let gid = crate::multiplayer::network::p2p_vps::parse_game_id_u64(game_id);
    ctx.game_started_events
        .write(crate::game::events::GameStartedEvent { game_id: gid });
    ctx.next_state.set(crate::core::GameState::InGame);
    ctx.menu_state.set(crate::core::MenuState::Main);

    ctx.p2p_host.game_id = None;
    ctx.p2p_host.last_heartbeat = None;
}
