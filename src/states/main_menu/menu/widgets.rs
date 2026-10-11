use super::*;

#[cfg(feature = "solana")]
pub(super) fn complete_logout(handle: &tokio::runtime::Handle) {
    use crate::multiplayer::solana::global_session_manager::GlobalSessionKeyManager;

    // Remove the local no-popup credential before any subsequent connect can
    // reload it; backend revocation and bridge cleanup continue in background.
    let _ = GlobalSessionKeyManager::delete();
    let port = crate::multiplayer::network::vps::wallet_bridge_port();
    handle.spawn_blocking(move || {
        crate::multiplayer::network::vps::logout();
        let client = reqwest::blocking::Client::new();
        let _ = client
            .post(format!("http://127.0.0.1:{port}/wallet/disconnect"))
            .send();
    });
}

pub(in super::super) fn menu_click_sound(
    mut contexts: bevy_egui::EguiContexts,
    mouse: Res<ButtonInput<MouseButton>>,
    sounds: Option<Res<MenuSounds>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(sounds) = sounds else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if ctx.is_pointer_over_egui() {
        crate::game::resources::sounds::play_sfx(&mut commands, sounds.menu_click.clone());
    }
}

pub(super) fn section(ui: &mut egui::Ui, title: &str) {
    section_sized(ui, title, 10.1);
}

pub(super) fn section_sized(ui: &mut egui::Ui, title: &str, size: f32) {
    ui.label(
        egui::RichText::new(title)
            .size(size)
            .color(egui::Color32::from_rgb(120, 180, 255))
            .strong(),
    );
    ui.add_space(3.0);
}

pub(super) fn item_expandable(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    item_expandable_tip(ui, label, "", width)
}

pub(super) fn item_expandable_tip(ui: &mut egui::Ui, label: &str, tip: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);
    let chevron_col = egui::Color32::from_rgb(120, 140, 180);

    // Reserve background shape slot BEFORE adding the button so the highlight
    // renders behind the text (not on top of it).
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let accent_idx = ui.painter().add(egui::Shape::Noop);

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(18.0)
                .color(btn_text)
                .family(egui::FontFamily::Proportional),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(width, 40.0)),
    );

    let r = resp.rect;
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().set(
            bg_idx,
            egui::Shape::rect_filled(
                r.expand(1.0),
                egui::CornerRadius::same(4),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 11),
            ),
        );
        ui.painter().set(
            accent_idx,
            egui::Shape::rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(r.left(), r.center().y - 11.0),
                    egui::vec2(3.0, 22.0),
                ),
                egui::CornerRadius::same(2),
                egui::Color32::from_rgb(90, 160, 255),
            ),
        );
    }

    // Chevron always visible
    ui.painter().text(
        egui::pos2(r.right() - 10.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(24.0),
        chevron_col,
    );
    if !tip.is_empty() {
        resp.clone().on_hover_text(tip);
    }
    resp.clicked()
}

pub(super) fn item(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    item_tip(ui, label, "", width)
}

pub(super) fn item_tip(ui: &mut egui::Ui, label: &str, tip: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);

    // Reserve background shape slots BEFORE the button so highlights render behind text.
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let accent_idx = ui.painter().add(egui::Shape::Noop);

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(18.0)
                .color(btn_text)
                .family(egui::FontFamily::Proportional),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(width, 40.0)),
    );

    let r = resp.rect;
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().set(
            bg_idx,
            egui::Shape::rect_filled(
                r.expand(1.0),
                egui::CornerRadius::same(4),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18),
            ),
        );
        ui.painter().set(
            accent_idx,
            egui::Shape::rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(r.left(), r.center().y - 11.0),
                    egui::vec2(3.0, 22.0),
                ),
                egui::CornerRadius::same(2),
                egui::Color32::from_rgb(90, 160, 255),
            ),
        );
    }

    if !tip.is_empty() {
        resp.clone().on_hover_text(tip);
    }
    resp.clicked()
}
