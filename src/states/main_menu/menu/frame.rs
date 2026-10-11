use super::*;

pub fn menu_escape_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<MenuPanel>,
    mut exit_confirm: ResMut<MenuExitConfirm>,
    mut focus_mode: ResMut<MenuFocusMode>,
    mut competitive: ResMut<CompetitiveMenuState>,
) {
    if competitive.active_modal().is_some() {
        if keyboard.just_pressed(KeyCode::Escape) {
            competitive.set_active_modal(None);
        }
        return;
    }
    if exit_confirm.visible {
        if keyboard.just_pressed(KeyCode::Escape) {
            exit_confirm.visible = false;
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        focus_mode.active = !focus_mode.active;
        return;
    }
    if focus_mode.active {
        // Any other shortcut first drops out of focus mode rather than
        // acting underneath the hidden UI.
        if keyboard.just_pressed(KeyCode::KeyG) || keyboard.just_pressed(KeyCode::Escape) {
            focus_mode.active = false;
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        *panel = MenuPanel::Settings;
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        if *panel == MenuPanel::Main {
            exit_confirm.visible = true;
        } else {
            *panel = MenuPanel::Main;
        }
    }
}

/// Outer size of the menu window, margins included; taller panels are clipped.
const MENU_WIDTH: f32 = 340.0;
const MENU_MAX_HEIGHT: f32 = 600.0;
const MENU_MARGIN: i8 = 28;

pub fn render_menu(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    render_title_logo(ctx, cx);
    render_hint_bar(ctx);
    render_board_caption(ctx, cx);

    // Panel navigation is rendered immediately. Fading the entire new panel
    // from opacity 0 made every click look like a dropped or delayed frame.
    // A newly found release opens its panel once, but never pulls the player out
    // of another panel.
    if *cx.menu_panel == MenuPanel::Main && cx.update_check.take_raise_request() {
        *cx.menu_panel = MenuPanel::Updates;
    }
    let current = *cx.menu_panel;

    if cx.exit_confirm.visible {
        render_exit_confirm_backdrop(ctx);
        egui::Window::new("##exit_confirm")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(300.0, 120.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .frame(crate::ui::styles::StyledPanel::popup())
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Are you sure you want to exit?")
                            .size(15.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        if ui
                            .add_sized(
                                [110.0, 34.0],
                                egui::Button::new(
                                    egui::RichText::new("Exit")
                                        .size(13.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(160, 50, 50))
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            // Use AppExit so normal shutdown can clean up relay lobbies.
                            cx.app_exit.write(AppExit::Success);
                        }
                        ui.add_space(12.0);
                        if ui
                            .add_sized(
                                [110.0, 34.0],
                                egui::Button::new(egui::RichText::new("Cancel").size(13.0))
                                    .fill(egui::Color32::from_rgba_unmultiplied(70, 70, 70, 220))
                                    .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            cx.exit_confirm.visible = false;
                        }
                    });
                });
            });
        return;
    }

    // The current panel's content is centred vertically on the left, but never
    // under the wallet HUD in the top-left corner (short windows push a centred
    // menu up into it). Its transparent margin may sit under the HUD; its content
    // may not.
    let height_id = egui::Id::new("xfc_menu_height");
    let height = ctx
        .data(|d| d.get_temp::<f32>(height_id))
        .unwrap_or(MENU_MAX_HEIGHT);
    let screen = ctx.content_rect();
    let margin = f32::from(MENU_MARGIN);
    let hud_bottom = wallet_hud_bottom(ctx).map(|bottom| bottom - screen.top());
    let top = menu_top(screen.height(), height, margin, hud_bottom);

    egui::Window::new("##xfc_new_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .fixed_size(egui::vec2(MENU_WIDTH, height))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(36.0, top))
        .frame(egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            inner_margin: egui::Margin::same(MENU_MARGIN),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            let panel = ui.scope(|ui| match current {
                MenuPanel::Main => render_main_panel(ui, cx),
                MenuPanel::PlayOnline => render_play_online_panel(ui, cx),
                MenuPanel::Multiplayer => render_multiplayer_panel(ui, cx),
                MenuPanel::Puzzles => render_puzzles_panel(ui, cx),
                MenuPanel::Tournaments => render_tournaments_panel(ui, cx),
                MenuPanel::SolanaConnect => render_solana_connect_panel(ui, cx),
                MenuPanel::DirectConnection => render_direct_connection_panel(ui, cx),
                MenuPanel::Settings => render_settings_panel(ui, cx),
                MenuPanel::Profile => render_profile_panel(ui, cx),
                MenuPanel::Updates => render_updates_panel(ui, cx),
            });
            let measured = (panel.response.rect.height() + 2.0 * margin)
                .ceil()
                .min(MENU_MAX_HEIGHT);
            if measured != height {
                ui.ctx().data_mut(|d| d.insert_temp(height_id, measured));
                // Lay out again at the new height rather than showing one frame
                // at the old position.
                ui.ctx().request_discard("menu panel height changed");
            }
        });
}

/// Top edge of a menu window `height` tall (margins included): centred on a
/// screen `screen_height` tall, but with its content below the wallet HUD's
/// bottom edge when there is one.
fn menu_top(screen_height: f32, height: f32, margin: f32, hud_bottom: Option<f32>) -> f32 {
    let centred = (screen_height - height) / 2.0;
    let below_hud = hud_bottom.map_or(0.0, |bottom| bottom + 8.0 - margin);
    centred.max(below_hud).max(0.0)
}

pub(super) fn render_exit_confirm_backdrop(ctx: &egui::Context) {
    let screen_rect = ctx.input(|i| i.content_rect());
    egui::Area::new("exit_confirm_backdrop".into())
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                screen_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::from_black_alpha(150),
            );
            ui.allocate_rect(screen_rect, egui::Sense::click_and_drag());
        });
}

pub(super) fn render_title_logo(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    super::super::ensure_brand_logo_texture(ctx, &mut cx.brand_logo);

    let Some((id, display_w, display_h)) = cx.brand_logo.texture.as_ref().map(|handle| {
        let [w, h] = handle.size();
        let display_h = 150.0_f32;
        (handle.id(), (w as f32 / h as f32) * display_h, display_h)
    }) else {
        return;
    };

    let clicked_id = egui::Id::new("title_logo_clicked");
    let fade_id = egui::Id::new("title_logo_fade");

    // Once clicked, animate opacity 1 → 0 over ~0.6s, then stop drawing.
    let clicked = ctx.data(|d| d.get_temp::<bool>(clicked_id).unwrap_or(false));
    let mut alpha = ctx.data(|d| d.get_temp::<f32>(fade_id).unwrap_or(1.0));
    if clicked && alpha > 0.0 {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        alpha = (alpha - dt / 0.6).max(0.0);
        ctx.request_repaint(); // keep animating without further input
    }
    ctx.data_mut(|d| d.insert_temp(fade_id, alpha));
    if alpha <= 0.001 {
        return;
    }

    // Pinned top-center; anchoring keeps it centred across window resizes.
    egui::Area::new("title_logo".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 20.0))
        .show(ctx, |ui| {
            ui.set_opacity(alpha);
            let resp = ui
                .add(egui::Image::new(egui::load::SizedTexture::new(
                    id,
                    [display_w, display_h],
                )))
                .interact(egui::Sense::click());

            // A click starts the fade-out.
            if resp.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(clicked_id, true));
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
}

pub(super) fn caption_nav_arrow(ui: &mut egui::Ui, glyph: &str) -> egui::Response {
    let resp = ui.add(
        egui::Label::new(
            egui::RichText::new(glyph)
                .size(11.0)
                .color(egui::Color32::from_rgba_unmultiplied(216, 202, 168, 140)),
        )
        .sense(egui::Sense::click()),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub(in super::super) fn render_board_caption(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    let caption = super::super::famous_games::FAMOUS_GAMES[cx.board_animator.game_index].caption;
    egui::Area::new("board_caption".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 262.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if caption_nav_arrow(ui, "‹").clicked() {
                    super::super::board_animation::request_game_nav(&mut cx.board_animator, -1);
                }
                ui.label(
                    egui::RichText::new(caption)
                        .size(12.0)
                        .italics()
                        .strong()
                        .color(egui::Color32::from_rgba_unmultiplied(226, 214, 184, 225)),
                );
                if caption_nav_arrow(ui, "›").clicked() {
                    super::super::board_animation::request_game_nav(&mut cx.board_animator, 1);
                }
            });
        });
}

pub(super) fn render_hint_bar(ctx: &egui::Context) {
    egui::Area::new("hint_bar".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                let hint_color = egui::Color32::WHITE;
                let size = 10.5;
                let sep_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120);

                ui.label(
                    egui::RichText::new("L - Learn")
                        .size(size)
                        .color(hint_color),
                );
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(
                    egui::RichText::new("G - In Game Settings")
                        .size(size)
                        .color(hint_color),
                );
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(
                    egui::RichText::new("ESC - Exit Game")
                        .size(size)
                        .color(hint_color),
                );
            });
        });
}

#[cfg(test)]
mod tests {
    use super::menu_top;

    #[test]
    fn the_menu_is_centred_on_its_content() {
        // A 400px panel on a 1000px screen sits 300px down.
        assert_eq!(menu_top(1000.0, 400.0, 28.0, None), 300.0);
    }

    #[test]
    fn the_menu_stays_below_the_wallet_hud_and_on_screen() {
        // Centring would put the content at 150 + 28; the HUD ends at 200.
        assert_eq!(menu_top(700.0, 400.0, 28.0, Some(200.0)), 180.0);
        // A HUD above the centred position doesn't move it.
        assert_eq!(menu_top(1000.0, 400.0, 28.0, Some(60.0)), 300.0);
        // A panel taller than the screen starts at the top.
        assert_eq!(menu_top(500.0, 600.0, 28.0, None), 0.0);
    }
}
