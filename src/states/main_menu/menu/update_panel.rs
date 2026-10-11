use super::*;

pub(super) fn render_updates_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("< Back")
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
            egui::RichText::new("Updates")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(14.0);

    // Allow long artifact names and sizes to fit beside the download icon.
    const W: f32 = 430.0;

    match &cx.update_check.status {
        UpdateStatus::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Checking latest GitHub release...")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(190, 200, 220)),
                );
            });
        }
        UpdateStatus::Available(release) => {
            ui.label(
                egui::RichText::new(format!(
                    "Version {} is available. You have {}.",
                    release.version,
                    updates::CURRENT_VERSION
                ))
                .size(11.0)
                .color(egui::Color32::from_rgb(220, 235, 255)),
            );
            if let Some(date) = release.published_date() {
                ui.label(
                    egui::RichText::new(format!("Published {date}"))
                        .size(9.5)
                        .color(egui::Color32::from_rgb(150, 165, 190)),
                );
            }
        }
        UpdateStatus::UpToDate => {
            ui.label(
                egui::RichText::new(format!(
                    "You are on the latest release: {}.",
                    updates::CURRENT_VERSION
                ))
                .size(11.0)
                .color(egui::Color32::from_rgb(170, 225, 180)),
            );
        }
        UpdateStatus::Failed(reason) => {
            ui.label(
                egui::RichText::new("Could not check GitHub releases.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(235, 175, 150)),
            );
            ui.label(
                egui::RichText::new(reason)
                    .size(9.4)
                    .color(egui::Color32::from_rgb(180, 185, 205)),
            );
        }
        UpdateStatus::Idle => {
            ui.label(
                egui::RichText::new("Check the latest release and download XFChess.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(190, 200, 220)),
            );
        }
    }

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [150.0, 30.0],
                egui::Button::new(
                    egui::RichText::new(if cx.update_check.is_checking() {
                        "Checking..."
                    } else {
                        "Check Again"
                    })
                    .size(10.4)
                    .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(60, 110, 190, 90))
                .corner_radius(5.0),
            )
            .clicked()
        {
            cx.update_check.start();
        }

        if matches!(cx.update_check.status, UpdateStatus::Available(_))
            && ui
                .add_sized(
                    [150.0, 30.0],
                    egui::Button::new(
                        egui::RichText::new("Skip This Version")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(190, 200, 220)),
                    )
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14))
                    .corner_radius(5.0),
                )
                .clicked()
        {
            cx.update_check.skip_available_version();
            *cx.menu_panel = MenuPanel::Main;
        }
    });

    ui.add_space(16.0);
    section_sized(ui, "Downloads", 14.0);
    ui.add_space(2.0);

    for platform in Platform::ALL {
        render_platform_download(ui, cx, platform, W);
        ui.add_space(9.0);
    }

    ui.add_space(8.0);
    if item_tip(
        ui,
        "Install Guide",
        "Open docs/INSTALL.md in your browser.",
        W,
    ) {
        updates::open_in_browser(updates::INSTALL_GUIDE_URL);
    }
}

pub(super) const PLATFORM_ICON_SIZE: f32 = 48.0;

pub(super) fn render_platform_download(
    ui: &mut egui::Ui,
    cx: &mut MainMenuUIContext,
    platform: Platform,
    width: f32,
) {
    let icon_px = (PLATFORM_ICON_SIZE * ui.ctx().pixels_per_point()).round() as u32;
    let icon = platform_icon_texture(ui.ctx(), platform, icon_px);
    let (asset_name, size_label, url) = match cx.update_check.available() {
        Some(release) => {
            let asset = release.asset_for(platform);
            (
                asset
                    .map(|a| a.name.as_str())
                    .unwrap_or("Latest release page"),
                asset.map(|a| a.size_label()).unwrap_or_default(),
                release.download_url_for(platform).to_string(),
            )
        }
        None => (
            "Latest release page",
            String::new(),
            updates::RELEASES_URL.to_string(),
        ),
    };

    let response = egui::Frame {
        fill: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        stroke: egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 26),
        ),
        corner_radius: egui::CornerRadius::same(8),
        inner_margin: egui::Margin::symmetric(16, 14),
        ..egui::Frame::NONE
    }
    .show(ui, |ui| {
        ui.set_width(width);
        ui.horizontal(|ui| {
            if let Some(icon) = icon {
                ui.add(egui::Image::new((
                    icon,
                    egui::vec2(PLATFORM_ICON_SIZE, PLATFORM_ICON_SIZE),
                )));
            }
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(platform.label())
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(platform.kind())
                        .size(13.0)
                        .color(egui::Color32::from_rgb(150, 165, 195)),
                );
                ui.label(
                    egui::RichText::new(if size_label.is_empty() {
                        asset_name.to_string()
                    } else {
                        format!("{asset_name} - {size_label}")
                    })
                    .size(12.0)
                    .color(egui::Color32::from_rgb(120, 130, 150)),
                );
            });
        });
    })
    .response
    .interact(egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        updates::open_in_browser(&url);
    }
}

pub(super) fn platform_icon_texture(
    ctx: &egui::Context,
    platform: Platform,
    target_px: u32,
) -> Option<egui::TextureId> {
    let id = egui::Id::new(("update_platform_icon", platform, target_px));
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture.id());
    }

    let bytes = read_asset_bytes(platform.icon_path())?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let mut rgba = decoded.to_rgba8();
    // Every mark is square, so one dimension is enough to compare against.
    if target_px > 0 && rgba.width() > target_px {
        rgba = image::imageops::resize(
            &rgba,
            target_px,
            target_px,
            image::imageops::FilterType::Lanczos3,
        );
    }
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let texture = ctx.load_texture(
        format!("update_platform_icon_{platform:?}_{target_px}"),
        color_image,
        egui::TextureOptions::LINEAR,
    );
    let texture_id = texture.id();
    ctx.data_mut(|d| d.insert_temp(id, texture));
    Some(texture_id)
}

pub(super) fn read_asset_bytes(path: &str) -> Option<Vec<u8>> {
    let mut candidates: Vec<std::path::PathBuf> = vec![path.into(), format!("./{path}").into()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(path));
        }
    }
    candidates.iter().find_map(|p| std::fs::read(p).ok())
}
