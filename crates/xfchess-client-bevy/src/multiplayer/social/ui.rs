//! Reusable Friends view. Pagination keeps the whole dialog visible; related
//! host/join flows replace it rather than opening a second popup.
use super::*;
use crate::multiplayer::network::vps::LobbyInvite;
use crate::states::main_menu::WalletBridgePoller;
use crate::ui::system_params::MainMenuUIContext;
use bevy_egui::{egui, EguiContexts};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Contacts,
    Requests,
    Invites,
    Blocked,
}

#[derive(Resource, Default)]
pub struct FriendsUiState {
    pub open: bool,
    pub invite_after_host: Option<String>,
    tab: Tab,
    target: String,
    page: usize,
    confirm_remove: Option<(String, String)>,
    confirm_block: Option<(String, String)>,
}

impl FriendsUiState {
    pub(super) fn reset_profile(&mut self) {
        *self = Self {
            open: self.open,
            ..default()
        };
    }
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.invite_after_host = None;
        self.confirm_remove = None;
        self.confirm_block = None;
        self.target.clear();
    }
}

enum Navigation {
    Close,
    Host(String),
    Join(LobbyInvite),
}

fn copy_id(ui: &mut egui::Ui, label: &str, id: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label(short(id)).on_hover_text(id);
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(id.to_string());
        }
    });
}

fn short(id: &str) -> String {
    if id.len() <= 16 {
        id.to_string()
    } else {
        format!("{}...{}", &id[..6], &id[id.len() - 6..])
    }
}

/// A player is shown by name; an empty name, or one that is just their ID, falls back to a short ID.
fn name_or_short(name: &str, id: &str) -> String {
    if name.trim().is_empty() || name == id {
        short(id)
    } else {
        name.to_string()
    }
}

fn status(ui: &mut egui::Ui, text: &str, error: bool) {
    let trimmed: String = text.chars().take(140).collect();
    ui.colored_label(
        if error {
            egui::Color32::LIGHT_RED
        } else {
            egui::Color32::LIGHT_GREEN
        },
        trimmed,
    )
    .on_hover_text(text);
}

fn page_range(
    ui: &mut egui::Ui,
    page: &mut usize,
    count: usize,
    per_page: usize,
) -> std::ops::Range<usize> {
    let pages = count.div_ceil(per_page).max(1);
    *page = (*page).min(pages - 1);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(*page > 0, egui::Button::new("Previous"))
            .clicked()
        {
            *page -= 1;
        }
        ui.label(format!("{} / {pages}", *page + 1));
        if ui
            .add_enabled(*page + 1 < pages, egui::Button::new("Next"))
            .clicked()
        {
            *page += 1;
        }
    });
    let start = *page * per_page;
    start..(start + per_page).min(count)
}

fn view(
    ctx: &egui::Context,
    ui_state: &mut FriendsUiState,
    friends: &mut FriendsState,
    can_join: bool,
) -> Option<Navigation> {
    let mut navigation = None;
    let width = 480.0f32.min((ctx.content_rect().width() - 80.0).max(240.0));
    let per_page = (((ctx.content_rect().height() - 320.0) / 66.0).floor() as usize).clamp(1, 4);
    let response = crate::ui::modal::show(
        ctx,
        "friends_dialog",
        width,
        crate::ui::styles::StyledPanel::popup(),
        |ui| {
            ui.horizontal(|ui| {
                ui.heading("Friends");
                // The label never changes, so the row keeps its width while a sync runs. The
                // spinner has a reserved slot and fades, so it does not pop in and shift the row.
                if ui.small_button("Refresh").clicked() {
                    friends.refresh();
                }
                let (slot, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                let fade = ui.ctx().animate_bool_with_time(
                    egui::Id::new("friends_sync_spinner"),
                    friends.loading,
                    0.2,
                );
                if fade > 0.0 {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                        ui.set_opacity(fade);
                        ui.add(egui::Spinner::new().size(14.0));
                    });
                }
            });
            if let Some(profile) = friends.profile.as_ref() {
                ui.label(format!("Local profile: {}", profile.display));
                copy_id(ui, "Your friend ID:", &profile.id);
            } else {
                ui.label("Select a local profile to use profile friends.");
            }
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (
                        Tab::Contacts,
                        format!("Friends ({})", friends.contacts.len()),
                    ),
                    (
                        Tab::Requests,
                        format!("Requests ({})", friends.pending_requests.len()),
                    ),
                    (
                        Tab::Invites,
                        format!("Invitations ({})", friends.pending_invites.len()),
                    ),
                    (Tab::Blocked, format!("Blocked ({})", friends.blocked.len())),
                ] {
                    if ui.selectable_label(ui_state.tab == tab, label).clicked() {
                        ui_state.tab = tab;
                        ui_state.page = 0;
                        ui_state.target.clear();
                        ui_state.confirm_remove = None;
                        ui_state.confirm_block = None;
                    }
                }
            });
            ui.separator();
            // One reserved line for errors, notices and progress, so a message appearing or
            // clearing never changes the dialog's height (the modal re-centres on every change).
            ui.scope(|ui| {
                ui.set_min_height(ui.text_style_height(&egui::TextStyle::Body));
                if let Some(error) = &friends.error {
                    status(ui, error, true);
                } else if let Some(notice) = &friends.notice {
                    status(ui, notice, false);
                } else if friends.busy {
                    ui.label("Working...");
                }
            });
            if let Some((id, name)) = ui_state.confirm_block.clone() {
                ui.label(format!(
                    "Block {name}? They are removed from your friends and can no longer send you requests or invitations."
                ));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!friends.busy, egui::Button::new("Block"))
                        .clicked()
                    {
                        friends.enqueue(FriendAction::Block(id));
                        ui_state.confirm_block = None;
                    }
                    if ui.button("Cancel").clicked() {
                        ui_state.confirm_block = None;
                    }
                });
            } else if let Some((id, name)) = ui_state.confirm_remove.clone() {
                ui.label(format!("Remove {name} from this profile's friends?"));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!friends.busy, egui::Button::new("Remove friend"))
                        .clicked()
                    {
                        friends.enqueue(FriendAction::Remove(id));
                        ui_state.confirm_remove = None;
                    }
                    if ui.button("Keep friend").clicked() {
                        ui_state.confirm_remove = None;
                    }
                });
            } else {
                // Keep the list area at least two rows tall, so switching tabs or a refresh that
                // changes the count by one does not resize and re-centre the whole dialog.
                let list_min = per_page.min(2) as f32 * 66.0 + 30.0;
                ui.scope(|ui| {
                ui.set_min_height(list_min);
                match ui_state.tab {
                    Tab::Contacts => {
                        ui.add_enabled_ui(friends.profile.is_some() && !friends.busy, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ui_state.target)
                                        .hint_text("Username or friend ID")
                                        .desired_width((width - 100.0).max(80.0)),
                                );
                                let entry = ui_state.target.trim().to_string();
                                // An ID is a 32-byte key (43+ characters); usernames are at most 30
                                // characters, so the two cannot be confused.
                                let is_id = bs58::decode(&entry)
                                    .into_vec()
                                    .is_ok_and(|bytes| bytes.len() == 32);
                                let is_username = !is_id
                                    && !entry.is_empty()
                                    && entry.chars().count() <= 30
                                    && !entry.chars().any(char::is_control);
                                let own = friends.profile.as_ref().is_some_and(|p| p.id == entry);
                                if ui
                                    .add_enabled((is_id && !own) || is_username, egui::Button::new("Add"))
                                    .clicked()
                                {
                                    friends.enqueue(if is_id {
                                        FriendAction::Send(entry)
                                    } else {
                                        FriendAction::SendName(entry)
                                    });
                                    ui_state.target.clear();
                                }
                            });
                        });
                        // Wait for the first sync, not for each one: the background sync every 10
                        // seconds would otherwise flash this line in and out.
                        if friends.contacts.is_empty() && friends.last_sync.is_some() {
                            ui.label(
                                "Add a friend by username. Guests have no username: share your friend ID or paste theirs.",
                            );
                        }
                        let range =
                            page_range(ui, &mut ui_state.page, friends.contacts.len(), per_page);
                        for contact in friends.contacts[range].to_vec() {
                            ui.push_id(&contact.contact_node_id, |ui| {
                                ui.horizontal(|ui| {
                                    let color = if contact.is_online {
                                        egui::Color32::LIGHT_GREEN
                                    } else {
                                        egui::Color32::GRAY
                                    };
                                    ui.colored_label(
                                        color,
                                        if contact.is_online {
                                            "Online"
                                        } else {
                                            "Offline"
                                        },
                                    );
                                    ui.add(
                                        egui::Label::new(name_or_short(
                                            &contact.contact_display,
                                            &contact.contact_node_id,
                                        ))
                                        .truncate(),
                                    )
                                    .on_hover_text(&contact.contact_node_id);
                                });
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            can_join && contact.is_online && !friends.busy,
                                            egui::Button::new("Play"),
                                        )
                                        .clicked()
                                    {
                                        navigation =
                                            Some(Navigation::Host(contact.contact_node_id.clone()));
                                    }
                                    if ui.small_button("Copy ID").clicked() {
                                        ui.ctx().copy_text(contact.contact_node_id.clone());
                                    }
                                    if ui
                                        .add_enabled(!friends.busy, egui::Button::new("Remove"))
                                        .clicked()
                                    {
                                        ui_state.confirm_remove = Some((
                                            contact.contact_node_id.clone(),
                                            contact.contact_display.clone(),
                                        ));
                                    }
                                    if ui
                                        .add_enabled(!friends.busy, egui::Button::new("Block"))
                                        .clicked()
                                    {
                                        ui_state.confirm_block = Some((
                                            contact.contact_node_id.clone(),
                                            contact.contact_display.clone(),
                                        ));
                                    }
                                });
                                ui.separator();
                            });
                        }
                    }
                    Tab::Requests => {
                        let incoming = friends.pending_requests.len();
                        let total = incoming + friends.outgoing_requests.len();
                        if total == 0 {
                            ui.label("No pending friend requests.");
                        }
                        let range = page_range(ui, &mut ui_state.page, total, per_page);
                        for index in range {
                            let (request, received) = if index < incoming {
                                (friends.pending_requests[index].clone(), true)
                            } else {
                                (friends.outgoing_requests[index - incoming].clone(), false)
                            };
                            let id = request.id.clone();
                            let other_id = if received {
                                request.from_node_id.clone()
                            } else {
                                request
                                    .to_node_id
                                    .clone()
                                    .or_else(|| request.to_pubkey.clone())
                                    .unwrap_or_default()
                            };
                            let label = if received {
                                format!(
                                    "From {}",
                                    name_or_short(&request.from_display, &request.from_node_id)
                                )
                            } else {
                                format!(
                                    "Sent to {}",
                                    name_or_short(
                                        request.to_display.as_deref().unwrap_or_default(),
                                        &other_id
                                    )
                                )
                            };
                            ui.push_id(&id, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(label).truncate())
                                        .on_hover_text(&other_id);
                                    if !other_id.is_empty() && ui.small_button("Copy ID").clicked() {
                                        ui.ctx().copy_text(other_id.clone());
                                    }
                                });
                                ui.add_enabled_ui(!friends.busy, |ui| {
                                    ui.horizontal(|ui| {
                                        if received {
                                            if ui.button("Accept").clicked() {
                                                friends.enqueue(FriendAction::Respond(
                                                    id.clone(),
                                                    true,
                                                ));
                                            }
                                            if ui.button("Decline").clicked() {
                                                friends.enqueue(FriendAction::Respond(
                                                    id.clone(),
                                                    false,
                                                ));
                                            }
                                            if ui.button("Block").clicked() {
                                                ui_state.confirm_block = Some((
                                                    request.from_node_id.clone(),
                                                    request.from_display.clone(),
                                                ));
                                            }
                                        } else if ui.button("Cancel request").clicked() {
                                            friends.enqueue(FriendAction::Cancel(id.clone()));
                                        }
                                    })
                                });
                                ui.separator();
                            });
                        }
                    }
                    Tab::Invites => {
                        if !can_join {
                            ui.label("Return to the main menu to join another game.");
                        }
                        if friends.pending_invites.is_empty() {
                            ui.label("No game invitations.");
                        }
                        let range = page_range(
                            ui,
                            &mut ui_state.page,
                            friends.pending_invites.len(),
                            per_page,
                        );
                        for invite in friends.pending_invites[range].to_vec() {
                            ui.push_id(&invite.id, |ui| {
                                ui.add(
                                    egui::Label::new(format!(
                                        "{} invited you to play",
                                        invite.from_display
                                    ))
                                    .truncate(),
                                );
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            can_join && !friends.busy,
                                            egui::Button::new(if invite.direct_node_id.is_some() {
                                                "Join"
                                            } else {
                                                "Open lobby"
                                            }),
                                        )
                                        .clicked()
                                    {
                                        navigation = Some(Navigation::Join(invite.clone()));
                                    }
                                    if ui
                                        .add_enabled(!friends.busy, egui::Button::new("Dismiss"))
                                        .clicked()
                                    {
                                        friends.enqueue(FriendAction::DismissInvite(
                                            invite.id.clone(),
                                        ));
                                    }
                                });
                                ui.separator();
                            });
                        }
                    }
                    Tab::Blocked => {
                        if friends.blocked.is_empty() {
                            ui.label("You have not blocked anyone.");
                        }
                        let range =
                            page_range(ui, &mut ui_state.page, friends.blocked.len(), per_page);
                        for player in friends.blocked[range].to_vec() {
                            ui.push_id(&player.blocked_node_id, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Label::new(name_or_short(
                                            &player.blocked_display,
                                            &player.blocked_node_id,
                                        ))
                                        .truncate(),
                                    )
                                    .on_hover_text(&player.blocked_node_id);
                                    if ui.small_button("Copy ID").clicked() {
                                        ui.ctx().copy_text(player.blocked_node_id.clone());
                                    }
                                    if ui
                                        .add_enabled(!friends.busy, egui::Button::new("Unblock"))
                                        .clicked()
                                    {
                                        friends.enqueue(FriendAction::Unblock(
                                            player.blocked_node_id.clone(),
                                        ));
                                    }
                                });
                                ui.separator();
                            });
                        }
                    }
                }
                });
            }
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                navigation = Some(Navigation::Close);
            }
        },
    );
    if response.should_close() {
        navigation = Some(Navigation::Close);
    }
    navigation
}

pub fn render_menu(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    match view(ctx, &mut cx.friends_ui, &mut cx.friends, true) {
        Some(Navigation::Close) => crate::states::main_menu::dialogs::close_to_main(cx),
        Some(Navigation::Host(target)) => {
            cx.friends_ui.invite_after_host = Some(target);
            cx.p2p_host.direct_mode = true;
            cx.competitive_menu.set_active_modal(None);
            cx.menu_state.set(crate::core::MenuState::HostConfig);
        }
        Some(Navigation::Join(invite)) => {
            if let Some(peer_node_id) = invite.direct_node_id {
                if let Some(events) = cx.connect_events.as_mut() {
                    events.write(crate::multiplayer::network::p2p::ConnectToPeerEvent {
                        peer_node_id,
                        is_host: false,
                        drive_game_start: true,
                    });
                }
                crate::states::main_menu::dialogs::close_to_main(cx);
                *cx.menu_panel = crate::states::main_menu::MenuPanel::DirectConnection;
            } else {
                cx.competitive_menu.join_game_id = invite.game_id;
                cx.competitive_menu.set_active_modal(None);
                cx.menu_state.set(crate::core::MenuState::BraidLobby);
            }
            cx.friends.enqueue(FriendAction::DismissInvite(invite.id));
        }
        None => {}
    }
}

pub(super) fn friends_ui(
    mut contexts: EguiContexts,
    game: Res<State<crate::core::GameState>>,
    mut ui: ResMut<FriendsUiState>,
    mut friends: ResMut<FriendsState>,
    wallet: Res<WalletBridgePoller>,
    compliance: Res<crate::ui::compliance_modal::ComplianceState>,
    exit: Res<crate::input::board::InGameExitConfirmation>,
) {
    if *game.get() != crate::core::GameState::InGame || !ui.open {
        return;
    }
    if wallet.show_connect_overlay || compliance.show || exit.visible {
        ui.open = false;
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if let Some(Navigation::Close) = view(ctx, &mut ui, &mut friends, false) {
        ui.close();
    }
}
