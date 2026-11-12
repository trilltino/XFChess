//! Shared, content-sized modal shell. Egui centres the complete measured card
//! and blocks input to every layer behind it.

use bevy_egui::egui;

pub fn show<R>(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    frame: egui::Frame,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    egui::Modal::new(egui::Id::new(id))
        .frame(frame)
        .backdrop_color(egui::Color32::from_black_alpha(150))
        .show(ctx, |ui| {
            ui.set_width(width);
            contents(ui)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        }
    }

    #[test]
    fn dialog_stays_centered_when_its_contents_grow() {
        let ctx = egui::Context::default();
        for height in [100.0, 330.0] {
            let mut rect = egui::Rect::NOTHING;
            for _ in 0..3 {
                let mut output = ctx.run_ui(input(vec![]), |root| {
                    let ctx = root.ctx();
                    rect = show(
                        ctx,
                        "centered",
                        300.0,
                        egui::Frame::NONE.inner_margin(20),
                        |ui| {
                            ui.set_min_height(height);
                            ui.label("Waiting for opponent");
                        },
                    )
                    .response
                    .rect;
                });
                output.textures_delta.clear();
            }
            assert!(
                (rect.center() - egui::pos2(400.0, 300.0)).length() < 1.0,
                "{rect:?}"
            );
            assert!(rect.width() <= 341.0, "{rect:?}");
        }
    }

    #[test]
    fn background_buttons_are_blocked_until_the_dialog_closes() {
        let ctx = egui::Context::default();
        let mut clicks = 0;
        let mut button = egui::Rect::NOTHING;
        let mut frame = |open: bool, events| {
            let mut output = ctx.run_ui(input(events), |root| {
                let ctx = root.ctx();
                egui::Area::new(egui::Id::new("background"))
                    .fixed_pos(egui::pos2(40.0, 100.0))
                    .show(ctx, |ui| {
                        let response = ui.button("Host a game");
                        button = response.rect;
                        clicks += usize::from(response.clicked());
                    });
                if open {
                    show(ctx, "blocking", 300.0, egui::Frame::NONE, |ui| {
                        ui.label("Local login");
                    });
                }
            });
            output.textures_delta.clear();
            button.center()
        };
        for _ in 0..3 {
            frame(true, vec![]);
        }
        let position = frame(true, vec![]);
        let click = |pressed| {
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        frame(true, click(true));
        frame(true, click(false));
        assert_eq!(clicks, 0);

        // Egui clears the previous modal layer on the following pass.
        for _ in 0..3 {
            let mut output = ctx.run_ui(input(vec![]), |root| {
                let ctx = root.ctx();
                egui::Area::new(egui::Id::new("background"))
                    .fixed_pos(egui::pos2(40.0, 100.0))
                    .show(ctx, |ui| {
                        let _ = ui.button("Host a game");
                    });
            });
            output.textures_delta.clear();
        }
        for pressed in [true, false] {
            let mut output = ctx.run_ui(input(click(pressed)), |root| {
                let ctx = root.ctx();
                egui::Area::new(egui::Id::new("background"))
                    .fixed_pos(egui::pos2(40.0, 100.0))
                    .show(ctx, |ui| {
                        clicks += usize::from(ui.button("Host a game").clicked());
                    });
            });
            output.textures_delta.clear();
        }
        assert_eq!(clicks, 1);
    }
}
