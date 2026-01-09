//! Pawn Promotion UI
//!
//! Displays a dialog when a pawn reaches the end of the board,
//! allowing the player to choose which piece to promote to.

use crate::game::resources::{PendingPromotion, PromotionSelected};
use crate::rendering::pieces::PieceType;
use crate::ui::styles::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// System to display the pawn promotion selection UI
pub fn promotion_ui_system(
    mut contexts: EguiContexts,
    pending_promotion: Res<PendingPromotion>,
    mut promotion_messages: MessageWriter<PromotionSelected>,
) {
    if !pending_promotion.is_active() {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let Some(entity) = pending_promotion.pawn_entity else {
        return;
    };
    let Some(position) = pending_promotion.position else {
        return;
    };
    let Some(color) = pending_promotion.color else {
        return;
    };

    // Create a modal overlay
    egui::Area::new(egui::Id::new("promotion_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ui.ctx().screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
            );
        });

    egui::Window::new("Promote Pawn")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_MID)
                .corner_radius(12.0)
                .inner_margin(20.0)
                .stroke(egui::Stroke::new(2.0, UiColors::BORDER)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Choose Promotion Piece")
                        .size(20.0)
                        .color(UiColors::TEXT_PRIMARY)
                        .strong(),
                );
                ui.add_space(15.0);

                ui.horizontal(|ui| {
                    // Piece options
                    let pieces = [
                        (PieceType::Queen, "♕", "Queen"),
                        (PieceType::Rook, "♖", "Rook"),
                        (PieceType::Bishop, "♗", "Bishop"),
                        (PieceType::Knight, "♘", "Knight"),
                    ];

                    for (piece_type, symbol, _name) in pieces {
                        let button = egui::Button::new(
                            egui::RichText::new(symbol)
                                .size(48.0)
                                .color(UiColors::TEXT_PRIMARY),
                        )
                        .min_size(egui::vec2(70.0, 70.0))
                        .fill(UiColors::BG_DARK);

                        if ui.add(button).clicked() {
                            promotion_messages.write(PromotionSelected {
                                entity,
                                position,
                                color,
                                promoted_to: piece_type,
                            });
                        }
                        ui.add_space(5.0);
                    }
                });
            });
        });
}
