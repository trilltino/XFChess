use super::*;

pub(super) fn render_welcome_panel(
    ctx: &egui::Context,
    _commands: &mut Commands,
    _sounds: Option<&MenuSounds>,
) {
    let welcome_closed_id = egui::Id::new("startup_welcome_closed");
    if ctx.data(|d| d.get_temp::<bool>(welcome_closed_id).unwrap_or(false)) {
        return;
    }

    let panel_frame = egui::Frame {
        corner_radius: egui::CornerRadius::same(8),
        fill: egui::Color32::from_rgba_unmultiplied(8, 8, 12, 240),
        stroke: egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        ),
        inner_margin: egui::Margin::symmetric(18, 16),
        ..egui::Frame::NONE
    };
    // Pinned to the far right (vertically centred) and locked in place — the
    // welcome card is anchored and not draggable.
    egui::Window::new("xfchess_welcome_panel")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .movable(false)
        .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-20.0, 0.0))
        .fixed_size([300.0, 380.0])
        .frame(panel_frame)
        .show(ctx, |ui| {
            // Header: title + close button.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("XFChess Alpha")
                        .size(17.0)
                        .color(egui::Color32::from_rgb(100, 200, 255))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.add(
                        egui::Button::new(
                            // Plain "X" — the menu fonts have no "✕" glyph.
                            egui::RichText::new("X")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(180, 180, 180)),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    );
                    if close.clicked() {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(welcome_closed_id, true));
                    }
                    if close.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
            });

            ui.add_space(8.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // Scrollable body so longer copy (and images) never overflow the card.
            egui::ScrollArea::vertical()
                .max_height(270.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // A few short welcome paragraphs. `para` is a small local helper
                    // for consistent body text + spacing.
                    let para = |ui: &mut egui::Ui, text: &str| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(210, 215, 225)),
                        );
                        ui.add_space(8.0);
                    };

                    // First line: bold "XFChess Alpha" inline (egui RichText can't
                    // mix weights in one label, so compose it across segments).
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let seg = |t: &str| {
                            egui::RichText::new(t)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(210, 215, 225))
                        };
                        ui.label(seg("Welcome to "));
                        ui.label(seg("XFChess Alpha").strong());
                        ui.label(seg(" — thanks for being here early."));
                    });
                    ui.add_space(8.0);
                    para(
                        ui,
                        "Play 3D chess against the engine, friends, or players online.",
                    );
                    para(
                        ui,
                        "Compete in matches and tournaments built for fast, fair \
                         competition.",
                    );
                    para(
                        ui,
                        "Expect changes, report bugs, and help shape what XFChess becomes.",
                    );

                    ui.add_space(2.0);

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Pick a mode on the left to begin.")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(100, 200, 255))
                            .strong(),
                    );
                });
        });
}

pub(super) fn render_main_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Startup welcome card, pinned to the far right of the screen.
    render_welcome_panel(ui.ctx(), &mut cx.commands, cx.menu_sounds.as_deref());

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    // Section heading
    ui.label(
        egui::RichText::new("MAIN MENU")
            .size(12.0)
            .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
            .family(egui::FontFamily::Proportional)
            .strong(),
    );
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    // Live online-player count (refreshed every ~15s by the social subsystem).
    let online = cx.online_players.count;
    ui.horizontal(|ui| {
        // Paint the status dot directly — the menu fonts (Cinzel/OpenSans) have no
        // "●" glyph, so a text bullet would render as a missing-glyph box.
        let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 11.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot.center(), 4.0, egui::Color32::from_rgb(120, 220, 140));
        ui.label(
            egui::RichText::new(format!("{online} online"))
                .size(11.0)
                .color(egui::Color32::from_rgb(200, 220, 210))
                .family(egui::FontFamily::Proportional),
        );
    });
    ui.add_space(6.0);

    if item_tip(
        ui,
        "Play Against a Computer",
        "Play offline against the engine — pick your side, difficulty, and time control.",
        W,
    ) {
        cx.competitive_menu.try_open_modal(MainMenuModal::AiSetup);
    }
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Play Online",
        "Host or join a live game against a friend or a matched opponent.",
        W,
    ) {
        *cx.menu_panel = MenuPanel::PlayOnline;
    }
    ui.add_space(SP);

    if item_tip(
        ui,
        "Replay PGN",
        "Choose a .pgn file to replay and analyze a saved game.",
        W,
    ) {
        *cx.core_mode = GameMode::PgnReplay;
        cx.next_state.set(GameState::InGame);
    }
    ui.add_space(SP);

    // TempleOS tribute mode — dev builds only (`--features templeos`).
    #[cfg(feature = "templeos")]
    {
        if item_tip(
            ui,
            "TempleOS",
            "Single-player game in the retro TempleOS-style isometric view.",
            W,
        ) {
            *cx.view_mode = crate::game::view_mode::ViewMode::TempleOS;
            *cx.core_mode = GameMode::SinglePlayer;
            cx.next_state.set(GameState::InGame);
        }
        ui.add_space(SP);
    }

    if item_tip(
        ui,
        "XFChess.com",
        "Open the XFChess website in your browser.",
        W,
    ) {
        // Point at the locally deployed site when XFCHESS_WEB_URL is set (dev
        // stack exports http://localhost:5173); otherwise the public site.
        let url =
            std::env::var("XFCHESS_WEB_URL").unwrap_or_else(|_| "https://xfchess.com".to_string());
        if let Err(e) = webbrowser::open(&url) {
            tracing::warn!("[Menu] Failed to open {}: {}", url, e);
        }
    }
    ui.add_space(SP);

    // Android updates through the dApp Store, not these desktop installers.
    #[cfg(not(target_os = "android"))]
    render_update_entry(ui, cx, W);
    ui.add_space(SP);
}

#[cfg(not(target_os = "android"))]
fn render_update_entry(ui: &mut egui::Ui, cx: &mut MainMenuUIContext, width: f32) {
    let update_label = match &cx.update_check.status {
        UpdateStatus::Available(release) => format!("Update Available: {}", release.version),
        UpdateStatus::Checking => "Checking for Updates".to_string(),
        UpdateStatus::UpToDate => "Check for Updates".to_string(),
        UpdateStatus::Failed(_) => "Update Check Failed".to_string(),
        UpdateStatus::Idle => "Check for Updates".to_string(),
    };
    let update_tip = match &cx.update_check.status {
        UpdateStatus::Available(_) => {
            "Download the latest XFChess release for your operating system."
        }
        UpdateStatus::Checking => "XFChess is asking GitHub for the latest release.",
        UpdateStatus::UpToDate => {
            "You are on the latest published release. Open release downloads anyway."
        }
        UpdateStatus::Failed(_) => "Retry the release check or open the download links.",
        UpdateStatus::Idle => "Check GitHub releases and show platform downloads.",
    };
    if item_expandable_tip(ui, &update_label, update_tip, width) {
        if !cx.update_check.is_checking() {
            cx.update_check.start();
        }
        *cx.menu_panel = MenuPanel::Updates;
    }
}

pub(super) fn render_play_online_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    // Back arrow + "Play Online" label styled the same as the Main Menu header
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Main;
        }
        ui.label(
            egui::RichText::new("Play Online")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    if item_expandable_tip(
        ui,
        "Multiplayer",
        "Free casual and rated games — lobbies, spectating, direct connect, and tournaments. No wallet needed.",
        W,
    ) {
        *cx.menu_panel = MenuPanel::Multiplayer;
    }
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Solana Multiplayer",
        "Wager SOL on the outcome — moves and payouts settle on-chain. Requires a connected wallet.",
        W,
    ) {
        *cx.menu_panel = MenuPanel::SolanaConnect;
    }
}

pub(super) fn render_multiplayer_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::PlayOnline;
        }
        ui.label(
            egui::RichText::new("Multiplayer")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    if item(ui, "Create Lobby", W) {
        cx.p2p_host.direct_mode = false;
        cx.menu_state.set(MenuState::HostConfig);
    }
    ui.add_space(SP);

    if item(ui, "Join Lobby", W) {
        // Trigger immediate poll so the lobby list is fresh on arrival
        if let Some(ref mut vps) = cx.p2p_vps_state {
            vps.last_poll = None;
        }
        cx.menu_state.set(MenuState::BraidLobby);
    }
    ui.add_space(SP);

    if item(ui, "Spectator", W) {
        cx.competitive_menu.try_open_modal(MainMenuModal::Spectator);
    }
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Direct Connection",
        "This works by using an ID. You copy it and send it to your friend — no login, no lobby list.",
        W,
    ) {
        *cx.menu_panel = MenuPanel::DirectConnection;
    }
    ui.add_space(SP + 4.0);

    if item_expandable(ui, "Tournaments", W) {
        *cx.menu_panel = MenuPanel::Tournaments;
    }
}

pub(super) fn render_direct_connection_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Multiplayer;
        }
        ui.label(
            egui::RichText::new("Direct Connection")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(10.0);

    ui.label(
        egui::RichText::new(
            "Play a friend directly — no account, no lobby list. One of you hosts and shares an ID; the other pastes it in to connect.",
        )
        .size(12.0)
        .color(egui::Color32::from_rgb(200, 200, 215)),
    );
    ui.add_space(16.0);

    ui.label(
        egui::RichText::new("HOST")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 220, 140))
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Start a game and get an ID to send to your friend.")
            .size(11.0)
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    ui.add_space(6.0);
    if item(ui, "Host a Game", W) {
        cx.p2p_host.direct_mode = true;
        cx.menu_state.set(MenuState::HostConfig);
    }

    ui.add_space(20.0);
    ui.label(
        egui::RichText::new("JOIN")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 180, 255))
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Paste the ID your friend sent you.")
            .size(11.0)
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    ui.add_space(6.0);

    if let Some(p2p_ui) = cx.p2p_ui.as_mut() {
        ui.add(
            egui::TextEdit::singleline(&mut p2p_ui.peer_input)
                .hint_text("Friend's node ID")
                .desired_width(W),
        );
        ui.add_space(8.0);

        if let Some(err) = &p2p_ui.error_message {
            ui.label(
                egui::RichText::new(err)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(240, 120, 120)),
            );
            ui.add_space(6.0);
        }

        if item(ui, "Connect", W) {
            match p2p_ui.validate_node_id() {
                Ok(()) => {
                    p2p_ui.clear_error();
                    let peer_node_id = p2p_ui.peer_input.trim().to_string();
                    if let Some(connect_events) = cx.connect_events.as_mut() {
                        connect_events.write(
                            crate::multiplayer::network::p2p::ConnectToPeerEvent {
                                peer_node_id,
                                is_host: false,
                                // Direct Connection has no VPS relay / backend — this
                                // handshake is the only way the match ever starts.
                                drive_game_start: true,
                            },
                        );
                    }
                }
                Err(e) => p2p_ui.set_error(e),
            }
        }
    }

    if let Some(p2p_state) = cx.p2p_state.as_ref() {
        let status_text = match &p2p_state.status {
            crate::multiplayer::network::p2p::P2PConnectionStatus::Connecting => {
                Some(("Connecting…", egui::Color32::GOLD))
            }
            crate::multiplayer::network::p2p::P2PConnectionStatus::Connected => {
                Some(("Connected!", egui::Color32::from_rgb(120, 220, 140)))
            }
            crate::multiplayer::network::p2p::P2PConnectionStatus::Error(msg) => {
                Some((msg.as_str(), egui::Color32::from_rgb(240, 120, 120)))
            }
            _ => None,
        };
        if let Some((text, color)) = status_text {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(text).size(12.0).color(color));
        }
    }
}

pub(super) fn render_puzzles_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;

    // Back arrow + "Puzzles" header (matches the other sub-panels).
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Main;
        }
        ui.label(
            egui::RichText::new("Puzzles")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    let wallet = cx.player_identity.pubkey_str.as_deref();
    let action = crate::puzzle::show_hub(ui, &mut cx.puzzle_hub, &cx.puzzle_session, wallet, W);
    match action {
        Some(crate::puzzle::HubAction::Start(mut request)) => {
            // Guests play the free modes under their local node id.
            if request.wallet.is_empty() {
                request.wallet = crate::multiplayer::network::identity::node_id_b58();
            }
            cx.start_puzzle.write(crate::puzzle::StartPuzzle(request));
        }
        Some(crate::puzzle::HubAction::LinkLichess(wallet)) => {
            crate::multiplayer::network::vps::start_lichess_link(wallet);
        }
        None => {}
    }
}

pub(super) fn render_tournaments_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Multiplayer;
        }
        ui.label(
            egui::RichText::new("Tournaments")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    if item(ui, "Join Tournament", W) {
        cx.menu_state.set(MenuState::Tournaments);
    }
    ui.add_space(SP);

    if item(ui, "Spectate Tournament", W) {
        cx.competitive_menu.try_open_modal(MainMenuModal::Spectator);
    }
}

pub(super) fn render_settings_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Settings")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(14.0);

    const W: f32 = 280.0;

    section(ui, "Controls");

    if item(ui, "Keyboard Shortcuts", W) {
        cx.competitive_menu.try_open_modal(MainMenuModal::Controls);
    }

    ui.add_space(14.0);
    render_preferences(ui, cx);

    #[cfg(not(target_os = "android"))]
    {
        ui.add_space(14.0);
        render_support_export(ui, cx, W);
    }

    if cx.player_identity.username.is_some() {
        ui.add_space(14.0);
        section(ui, "Account");

        if ui
            .add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Logout")
                        .size(10.8)
                        .color(egui::Color32::WHITE)
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgb(100, 40, 40))
                .corner_radius(6.0),
            )
            .clicked()
        {
            #[cfg(feature = "solana")]
            complete_logout(cx.tokio_runtime.0.handle());
            cx.wallet_bridge.enabled = false;
            cx.wallet_bridge.known_pubkey = None;
            cx.wallet_bridge.timer = 0.0;
            cx.wallet_bridge.status_rx = None;
            cx.wallet_bridge.balance_rx = None;
            {
                let mut d = cx.wallet_bridge.data.lock();
                d.sol_balance = 0.0;
                d.usd_balance = None;
                d.balance_loaded = false;
            }
            #[cfg(feature = "solana")]
            if let Some(wallet) = cx.wallet.as_mut() {
                wallet.pubkey = None;
                wallet.keypair = None;
                wallet.ranked_active = false;
                wallet.tournament_match_id = None;
            }
            #[cfg(feature = "solana")]
            if let Some(sync) = cx.solana_sync.as_mut() {
                sync.game_id = None;
                sync.moves_submitted = 0;
                sync.wager_amount = 0;
            }
            // Clear wallet-derived integration state on logout so reconnect loads the
            // new wallet's session, profile, and name.
            #[cfg(feature = "solana")]
            if let Some(solana_state) = cx.solana_state.as_mut() {
                **solana_state =
                    crate::multiplayer::solana::integration::state::SolanaIntegrationState::default(
                    );
            }
            // Clear lobby identity caches as well; they populate only while unset.
            #[cfg(feature = "solana")]
            if let Some(lobby) = cx.solana_lobby.as_mut() {
                **lobby = crate::multiplayer::solana::lobby::SolanaLobbyState::default();
            }
            *cx.player_identity = crate::states::main_menu::PlayerIdentity::default();
            *cx.menu_panel = MenuPanel::Main;
            // Just disconnect the wallet — stay on the main menu, no login screen.
        }
    }
}

/// Graphics, gameplay and sound preferences. Widgets edit the settings without
/// marking them changed; only a real edit does, so the file is saved once per
/// change instead of every frame the panel is open.
fn render_preferences(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    use crate::core::GraphicsQuality;

    let caption = egui::Color32::from_rgba_unmultiplied(200, 200, 215, 170);
    let settings = cx.settings.bypass_change_detection();
    let mut changed = false;

    section(ui, "Graphics");
    ui.horizontal_wrapped(|ui| {
        for (quality, label) in [
            (GraphicsQuality::Low, "Low"),
            (GraphicsQuality::Medium, "Medium"),
            (GraphicsQuality::High, "High"),
            (GraphicsQuality::Ultra, "Ultra"),
        ] {
            changed |= ui
                .radio_value(&mut settings.graphics_quality, quality, label)
                .changed();
        }
    });
    ui.label(
        egui::RichText::new(settings.graphics_quality.description())
            .small()
            .color(caption),
    );

    ui.add_space(14.0);
    section(ui, "Gameplay");
    changed |= ui
        .checkbox(&mut settings.show_hints, "Show move hints")
        .changed();
    changed |= ui
        .checkbox(&mut settings.highlight_last_move, "Highlight last move")
        .changed();
    changed |= ui
        .checkbox(
            &mut settings.use_vps_relay,
            "Relay P2P games through the server",
        )
        .on_hover_text("More reliable behind strict NATs; adds a little latency")
        .changed();

    ui.add_space(14.0);
    section(ui, "Sound");
    changed |= ui
        .add(egui::Slider::new(&mut settings.master_volume, 0.0..=1.0).text("Volume"))
        .changed();

    if changed {
        cx.settings.set_changed();
    }
}

/// Exports recent logs to a text file for bug reports (desktop only). The export
/// itself runs in `core::diagnostics::support_bundle::run_requested_export`.
#[cfg(not(target_os = "android"))]
fn render_support_export(ui: &mut egui::Ui, cx: &mut MainMenuUIContext, width: f32) {
    section(ui, "Help & Support");
    if item_tip(
        ui,
        "Export Logs",
        "Save the latest game logs to a text file you can send to the developers.",
        width,
    ) {
        cx.support_bundle.export_requested = true;
    }
    let support = &cx.support_bundle;
    if let Some(status) = &support.status {
        ui.label(egui::RichText::new(status).small());
    }
    if let Some(error) = &support.error {
        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), error);
    }
}
