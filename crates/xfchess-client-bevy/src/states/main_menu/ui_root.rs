use super::*;

pub(super) fn main_menu_ui_wrapper(
    mut ctx: MainMenuUIContext,
    fonts: Res<FontsLoaded>,
    render: Res<crate::rendering::startup::StartupRenderStatus>,
    mut fade_started: Local<Option<f64>>,
) {
    // Upload the logo while the loading cover is still visible.
    if let Ok(egui_ctx) = ctx.contexts.ctx_mut() {
        ensure_brand_logo_texture(egui_ctx, &mut ctx.brand_logo);
    }
    if !fonts.0 {
        if let Ok(egui_ctx) = ctx.contexts.ctx_mut().cloned() {
            render_loading_screen_website(&egui_ctx, &mut ctx);
        }
        return;
    }
    match main_menu_ui(&mut ctx) {
        Ok(()) => {}
        Err(e) => {
            warn!("[MAIN_MENU] UI system error: {:?}", e);
        }
    }

    if ctx.loading_progress.complete {
        if !render.is_ready() && !ctx.loading_progress.failed {
            if let Ok(egui_ctx) = ctx.contexts.ctx_mut().cloned() {
                render_loading_screen_website(&egui_ctx, &mut ctx);
            }
            return;
        }
        if let Ok(egui_ctx) = ctx.contexts.ctx_mut() {
            // Fade the same black cover used during loading, so the scene and
            // every menu overlay appear together. This runs only on startup.
            const FADE_SECONDS: f64 = 2.0;
            let now = egui_ctx.input(|input| input.time);
            let started = *fade_started.get_or_insert(now);
            let progress = ((now - started) / FADE_SECONDS).clamp(0.0, 1.0) as f32;
            if progress < 1.0 {
                let eased = progress * progress * (3.0 - 2.0 * progress);
                let alpha = ((1.0 - eased) * 255.0).round() as u8;
                paint_startup_cover(egui_ctx, alpha);
                egui_ctx.request_repaint();
            }
        }
    }
}

pub(super) fn paint_startup_cover(ctx: &egui::Context, alpha: u8) {
    let screen = ctx.viewport_rect();
    let id = egui::Id::new("startup_black_screen");
    // Consume pointer input until the reveal is finished; hidden buttons
    // must not start a game while the renderer is still warming up.
    egui::Area::new(id)
        .order(egui::Order::Tooltip)
        .fixed_pos(screen.min)
        .default_size(screen.size())
        .movable(false)
        .show(ctx, |ui| {
            ui.allocate_rect(screen, egui::Sense::click_and_drag());
        });
    ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, id))
        .rect_filled(
            screen,
            egui::CornerRadius::ZERO,
            egui::Color32::from_black_alpha(alpha),
        );
}

pub(super) const BRAND_LOGO_PATH: &str = "assets/branding/xfchess-title.png";

fn read_menu_asset(path: &str) -> Option<Vec<u8>> {
    let mut candidates = vec![std::path::PathBuf::from(path)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(path));
        }
    }
    candidates.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("apps")
            .join("xfchess-client")
            .join(path),
    );
    candidates.iter().find_map(|path| std::fs::read(path).ok())
}

pub(in super::super) fn ensure_brand_logo_texture(
    ctx: &egui::Context,
    logo: &mut BrandLogoState,
) -> Option<egui::TextureId> {
    if let Some(texture) = logo.texture.as_ref() {
        return Some(texture.id());
    }
    if logo.loaded {
        return None;
    }
    logo.loaded = true;

    let Some(bytes) = read_menu_asset(BRAND_LOGO_PATH) else {
        warn!("[MAIN_MENU] Failed to read brand logo image: {BRAND_LOGO_PATH}");
        return None;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        warn!(
            "[MAIN_MENU] Failed to decode brand logo image at {}",
            BRAND_LOGO_PATH
        );
        return None;
    };

    const MAX_DIM: u32 = 2048;
    let decoded = if decoded.width() > MAX_DIM || decoded.height() > MAX_DIM {
        let scale = (MAX_DIM as f32 / decoded.width().max(decoded.height()) as f32).min(1.0);
        let w = (decoded.width() as f32 * scale) as u32;
        let h = (decoded.height() as f32 * scale) as u32;
        decoded.resize(w, h, image::imageops::FilterType::Triangle)
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let texture = ctx.load_texture(
        "brand_logo_screenshot",
        color_image,
        egui::TextureOptions {
            mipmap_mode: Some(egui::TextureFilter::Linear),
            ..egui::TextureOptions::LINEAR
        },
    );
    let texture_id = texture.id();
    logo.texture = Some(texture);
    Some(texture_id)
}

pub(super) fn setup_menu_camera(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    // The camera keeps its MenuCamera marker between visits, so don't filter on it.
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    debug!("[MAIN_MENU] Setting up menu camera for egui launcher");

    if let Some(camera_entity) = persistent_camera.entity {
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!(
                    "[MAIN_MENU] Setting up menu camera entity: {:?}",
                    camera_entity
                );
                *transform = Transform::from_translation(menu::BOARD_CAM)
                    .looking_at(menu::BOARD_CENTER, Vec3::Y);
                commands.entity(camera_entity).insert(MenuCamera);
            }
            Err(e) => {
                error!(
                    "[MAIN_MENU] ERROR: Persistent camera entity {:?} exists but query failed: {:?}",
                    camera_entity, e
                );
            }
        }
    } else {
        debug!("[MAIN_MENU] WARNING: Persistent camera not yet created (OnEnter runs before PreStartup).");
    }
}

pub(super) fn ensure_menu_camera_setup(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<MenuCamera>)>,
    mut commands: Commands,
    menu_camera_query: Query<Entity, With<MenuCamera>>,
) {
    if menu_camera_query.is_empty() {
        if let Some(camera_entity) = persistent_camera.entity {
            match camera_query.get_mut(camera_entity) {
                Ok(mut transform) => {
                    info!("[MAIN_MENU] Initializing menu camera (late setup)");
                    *transform = Transform::from_translation(menu::BOARD_CAM)
                        .looking_at(menu::BOARD_CENTER, Vec3::Y);
                    commands.entity(camera_entity).insert(MenuCamera);
                }
                Err(e) => {
                    error!(
                        "[MAIN_MENU] ERROR: Late setup failed to query camera: {:?}",
                        e
                    );
                }
            }
        }
    }
}

pub(super) fn main_menu_ui(
    ctx: &mut MainMenuUIContext,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let egui_ctx = ctx.contexts.ctx_mut()?.clone();
    ctx.dialogs.active = dialogs::resolve(ctx);
    if ctx.dialogs.close_requested {
        dialogs::dismiss(ctx);
        render_website_menu(&egui_ctx, ctx);
        // A related operation (such as cancelling a wager) can still be in
        // progress. Keep its dialog visible rather than flashing the menu.
        if ctx.dialogs.is_open() && ctx.loading_progress.complete {
            dialogs::render_selected(&egui_ctx, ctx);
        }
        return Ok(());
    }
    render_website_menu(&egui_ctx, ctx);
    ctx.dialogs.active = dialogs::resolve(ctx);
    if ctx.loading_progress.complete {
        dialogs::render_selected(&egui_ctx, ctx);
    }
    Ok(())
}

pub(super) fn try_setup_fonts(mut contexts: EguiContexts, mut loaded: ResMut<FontsLoaded>) {
    if loaded.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return; // egui not ready yet — retry next frame
    };

    let mut fonts = egui::FontDefinitions::default();

    // Load Cinzel (title / heading font — classical serif)
    let cinzel_paths = [
        "assets/fonts/Cinzel-Regular.ttf",
        "./assets/fonts/Cinzel-Regular.ttf",
    ];
    for path in &cinzel_paths {
        if let Some(data) = read_menu_asset(path) {
            fonts.font_data.insert(
                "Cinzel".to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            info!("[MAIN_MENU] Loaded Cinzel font from {}", path);
            break;
        }
    }

    let cinzel_bold_paths = [
        "assets/fonts/Cinzel-Bold.ttf",
        "./assets/fonts/Cinzel-Bold.ttf",
    ];
    for path in &cinzel_bold_paths {
        if let Some(data) = read_menu_asset(path) {
            fonts.font_data.insert(
                "CinzelBold".to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            info!("[MAIN_MENU] Loaded Cinzel Bold font from {}", path);
            break;
        }
    }

    let opensans_paths = [
        "assets/fonts/OpenSans-VariableFont_wdth,wght.ttf",
        "./assets/fonts/OpenSans-VariableFont_wdth,wght.ttf",
    ];
    let mut body_loaded = false;
    for path in &opensans_paths {
        if let Some(data) = read_menu_asset(path) {
            fonts.font_data.insert(
                "OpenSans".to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            info!("[MAIN_MENU] Loaded OpenSans font from {}", path);
            body_loaded = true;
            break;
        }
    }

    // Use Cinzel first and OpenSans for unsupported characters.
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    if fonts.font_data.contains_key("Cinzel") {
        proportional.insert(0, "Cinzel".to_owned());
    }
    if body_loaded {
        // OpenSans sits after Cinzel for fallback coverage
        let pos = if fonts.font_data.contains_key("Cinzel") {
            1
        } else {
            0
        };
        proportional.insert(pos, "OpenSans".to_owned());
    }

    // Register a named family so UI code can request Cinzel explicitly
    if fonts.font_data.contains_key("Cinzel") {
        fonts.families.insert(
            egui::FontFamily::Name("Cinzel".into()),
            vec!["Cinzel".to_owned()],
        );
    }
    if fonts.font_data.contains_key("CinzelBold") {
        fonts.families.insert(
            egui::FontFamily::Name("CinzelBold".into()),
            vec!["CinzelBold".to_owned()],
        );
    }

    ctx.set_fonts(fonts);
    loaded.0 = true;
    info!("[MAIN_MENU] Fonts registered successfully");
}

pub(super) fn render_website_menu(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    if !ctx_menu.loading_progress.complete {
        render_loading_screen_website(ctx, ctx_menu);
        return;
    }

    if ctx_menu.focus_mode.active && !ctx_menu.dialogs.is_open() {
        menu::render_board_caption(ctx, ctx_menu);
        return;
    }

    render_menu(ctx, ctx_menu);
    render_wallet_hud(ctx, ctx_menu);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Returning to the menu re-runs setup on a camera that still carries its
    /// MenuCamera marker from the last visit; it must still be repositioned.
    #[test]
    fn menu_camera_is_reset_on_every_visit() -> Result<(), Box<dyn std::error::Error>> {
        let mut world = World::new();
        let camera = world
            .spawn((
                Camera3d::default(),
                Transform::from_xyz(9.0, 9.0, 9.0),
                MenuCamera,
            ))
            .id();
        world.insert_resource(crate::PersistentEguiCamera {
            entity: Some(camera),
        });
        world
            .run_system_once(setup_menu_camera)
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(
            world
                .get::<Transform>(camera)
                .ok_or("camera transform missing")?
                .translation,
            menu::BOARD_CAM
        );

        Ok(())
    }
}
