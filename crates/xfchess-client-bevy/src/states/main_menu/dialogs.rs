//! One dialog owns the menu at a time. Related screens replace one another;
//! dismissing a flow returns to Main and runs its own connection cleanup.

use super::*;
use crate::core::MenuState;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MenuUiRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuDialog {
    LocalProfile,
    Compliance,
    Wallet,
    Exit,
    Flow(MenuState),
    Panel(MainMenuModal),
}

#[derive(Resource, Default)]
pub struct MenuDialogs {
    pub active: Option<MenuDialog>,
    pub close_requested: bool,
}

impl MenuDialogs {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }
}

fn select_dialog(
    local_profile: bool,
    compliance: bool,
    wallet: bool,
    exit: bool,
    screen: MenuState,
    panel: Option<MainMenuModal>,
) -> Option<MenuDialog> {
    if local_profile {
        Some(MenuDialog::LocalProfile)
    } else if compliance {
        Some(MenuDialog::Compliance)
    } else if wallet {
        Some(MenuDialog::Wallet)
    } else if exit {
        Some(MenuDialog::Exit)
    } else if !matches!(screen, MenuState::Main | MenuState::ModeSelect) {
        Some(MenuDialog::Flow(screen))
    } else {
        panel.map(MenuDialog::Panel)
    }
}

pub(super) fn resolve(ctx: &MainMenuUIContext) -> Option<MenuDialog> {
    #[cfg(feature = "solana")]
    let wallet_connected = ctx
        .solana_state
        .as_ref()
        .is_some_and(|s| s.wallet_pubkey.is_some());
    #[cfg(not(feature = "solana"))]
    let wallet_connected = false;
    select_dialog(
        crate::ui::menus::profile_onboarding::needs_local_profile(
            &ctx.player_identity,
            wallet_connected,
        ),
        ctx.compliance.show,
        ctx.wallet_bridge.show_connect_overlay,
        ctx.exit_confirm.visible,
        ctx.current_menu_state
            .as_ref()
            .map_or(MenuState::Main, |state| *state.get()),
        ctx.competitive_menu.active_modal(),
    )
}

pub(crate) fn close_to_main(ctx: &mut MainMenuUIContext) {
    ctx.friends_ui.close();
    ctx.menu_state.set(MenuState::Main);
    *ctx.menu_panel = MenuPanel::Main;
    ctx.competitive_menu.set_active_modal(None);
    ctx.exit_confirm.visible = false;
    ctx.focus_mode.active = false;
    ctx.dialogs.active = None;
    ctx.dialogs.close_requested = false;
}

pub(super) fn dismiss(ctx: &mut MainMenuUIContext) {
    ctx.dialogs.close_requested = false;
    match ctx.dialogs.active {
        // Authentication and verification must be completed explicitly.
        Some(MenuDialog::LocalProfile | MenuDialog::Compliance | MenuDialog::Wallet) => {}
        Some(MenuDialog::Flow(MenuState::P2PWaiting)) => {
            if ctx.p2p_host.game_id.is_some() {
                screens::cancel_hosting(ctx);
            } else {
                screens::leave_joiner(ctx);
            }
        }
        #[cfg(feature = "solana")]
        Some(MenuDialog::Flow(MenuState::SolanaLobby)) => screens::dismiss_solana_lobby(ctx),
        Some(MenuDialog::Panel(MainMenuModal::Controls))
            if *ctx.menu_panel == MenuPanel::Settings =>
        {
            ctx.competitive_menu.set_active_modal(None);
            ctx.dialogs.active = None;
        }
        _ => close_to_main(ctx),
    }
}

pub(super) fn render_selected(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    match cx.dialogs.active {
        Some(MenuDialog::Panel(modal)) => {
            match modal {
                MainMenuModal::Friends => {
                    crate::multiplayer::social::ui::render_menu(ctx, cx);
                    return;
                }
                MainMenuModal::AiSetup => render_ai_setup_modal(
                    ctx,
                    &mut cx.competitive_menu,
                    &mut cx.ai_config,
                    &mut cx.core_mode,
                    &mut cx.next_state,
                    &mut cx.active_time_control,
                ),
                MainMenuModal::Controls => render_controls_popup(ctx, &mut cx.competitive_menu),
                MainMenuModal::PgnInput => render_pgn_input_modal(
                    ctx,
                    &mut cx.competitive_menu,
                    &mut cx.core_mode,
                    &mut cx.next_state,
                    &mut cx.commands,
                ),
                MainMenuModal::Spectator => {
                    let games = cx
                        .p2p_vps_state
                        .as_ref()
                        .map(|v| v.cached_games.clone())
                        .unwrap_or_default();
                    #[cfg(feature = "solana")]
                    let rate = cx
                        .sol_usd_wager_rate
                        .as_ref()
                        .and_then(|r| r.snapshot())
                        .map(|s| s.usd_per_sol);
                    #[cfg(not(feature = "solana"))]
                    let rate = None;
                    render_spectator_popup(
                        ctx,
                        &mut cx.competitive_menu,
                        &games,
                        &mut cx.spectate_events,
                        rate,
                    );
                }
            }
            if cx.competitive_menu.active_modal().is_none() {
                if modal == MainMenuModal::Controls && *cx.menu_panel == MenuPanel::Settings {
                    cx.dialogs.active = None;
                } else {
                    close_to_main(cx);
                }
            }
        }
        Some(MenuDialog::Flow(screen)) => {
            let (id, width) = match screen {
                MenuState::BraidLobby => ("p2p_lobby_popup", 420.0),
                MenuState::Tournaments => ("tournaments_popup", 480.0),
                MenuState::HostConfig => ("host_config_popup", 380.0),
                MenuState::P2PWaiting => ("p2p_waiting_popup", 360.0),
                #[cfg(feature = "solana")]
                MenuState::SolanaLobby => ("solana_lobby_popup", 460.0),
                // Lobby selection, login and wallet UI have their own systems,
                // gated by this same owner to prevent overlapping dialogs.
                _ => return,
            };
            crate::ui::modal::show(
                ctx,
                id,
                width,
                crate::ui::styles::StyledPanel::popup(),
                |ui| match screen {
                    MenuState::BraidLobby => render_braid_lobby_screen(ui, cx),
                    MenuState::Tournaments => render_tournament_browser_screen(ui, cx),
                    MenuState::HostConfig => render_host_p2p_config_screen(ui, cx),
                    MenuState::P2PWaiting => render_p2p_waiting_screen(ui, cx),
                    #[cfg(feature = "solana")]
                    MenuState::SolanaLobby => ui_solana_lobby(ui, cx),
                    _ => {}
                },
            );
            // Existing screen buttons signal a completed close with Main.
            if matches!(
                *cx.menu_state,
                NextState::Pending(MenuState::Main | MenuState::ModeSelect)
            ) {
                close_to_main(cx);
            }
        }
        _ => {}
    }
}
