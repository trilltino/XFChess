use super::*;

pub(super) fn main_menu_ui_wrapper(mut ctx: MainMenuUIContext) {
    match main_menu_ui(&mut ctx) {
        Ok(()) => {}
        Err(e) => {
            warn!("[MAIN_MENU] UI system error: {:?}", e);
        }
    }
}

pub(super) fn render_menu_modal_backdrop(ctx: &egui::Context, id: &'static str) {
    let screen_rect = ctx.input(|i| i.content_rect());
    egui::Area::new(id.into())
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

pub(super) const BRAND_LOGO_PATH: &str = "assets/branding/xfchess-title.png";

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

    // Try several locations (mirrors the font loader): CWD-relative both with and
    // without a leading "./", then next to the executable for packaged builds.
    let mut candidates: Vec<std::path::PathBuf> = vec![
        BRAND_LOGO_PATH.into(),
        format!("./{}", BRAND_LOGO_PATH).into(),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(BRAND_LOGO_PATH));
        }
    }
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else {
        warn!(
            "[MAIN_MENU] Failed to read brand logo image; tried {:?}",
            candidates
        );
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
    let current_substate = if let Some(ref menu_state_res) = ctx.current_menu_state {
        *menu_state_res.get()
    } else {
        crate::core::MenuState::Main
    };

    let egui_ctx = ctx.contexts.ctx_mut()?.clone();

    // Always render the main menu (keeps 3D board visible behind all popups)
    render_website_menu(&egui_ctx, ctx);

    let popup_frame = crate::ui::styles::StyledPanel::popup();

    if current_substate == crate::core::MenuState::BraidLobby {
        egui::Window::new("p2p_lobby_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 480.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_braid_lobby_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::Tournaments {
        egui::Window::new("tournaments_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([480.0, 520.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_tournament_browser_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::HostConfig {
        egui::Window::new("host_config_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([380.0, 320.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_host_p2p_config_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::P2PWaiting {
        egui::Window::new("p2p_waiting_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([340.0, 260.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_p2p_waiting_screen(ui, ctx);
            });
    }

    #[cfg(feature = "solana")]
    if current_substate == crate::core::MenuState::SolanaLobby {
        egui::Window::new("solana_lobby_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([460.0, 500.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                ui_solana_lobby(ui, ctx);
            });
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
        if let Ok(data) = std::fs::read(path) {
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
        if let Ok(data) = std::fs::read(path) {
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
        if let Ok(data) = std::fs::read(path) {
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

    if ctx_menu.focus_mode.active {
        menu::render_board_caption(ctx, ctx_menu);
        return;
    }

    let active_modal = ctx_menu.competitive_menu.active_modal();
    if active_modal.is_some() {
        render_menu_modal_backdrop(ctx, "main_menu_modal_backdrop");
    }

    render_menu(ctx, ctx_menu);

    render_wallet_hud(ctx, ctx_menu);

    if active_modal == Some(MainMenuModal::AiSetup) {
        render_ai_setup_modal(
            ctx,
            &mut ctx_menu.competitive_menu,
            &mut ctx_menu.ai_config,
            &mut ctx_menu.core_mode,
            &mut ctx_menu.next_state,
            &mut ctx_menu.active_time_control,
        );
    }

    if active_modal == Some(MainMenuModal::Spectator) {
        let cached_games = if let Some(vps) = &ctx_menu.p2p_vps_state {
            vps.cached_games.clone()
        } else {
            Vec::new()
        };
        #[cfg(feature = "solana")]
        let sol_usd_rate = ctx_menu
            .sol_usd_wager_rate
            .as_ref()
            .and_then(|r| r.snapshot())
            .map(|s| s.usd_per_sol);
        #[cfg(not(feature = "solana"))]
        let sol_usd_rate: Option<f64> = None;
        render_spectator_popup(
            ctx,
            &mut ctx_menu.competitive_menu,
            &cached_games,
            &mut ctx_menu.spectate_events,
            sol_usd_rate,
        );
    }

    if active_modal == Some(MainMenuModal::Controls) {
        render_controls_popup(ctx, &mut ctx_menu.competitive_menu);
    }

    if active_modal == Some(MainMenuModal::PgnInput) {
        render_pgn_input_modal(
            ctx,
            &mut ctx_menu.competitive_menu,
            &mut ctx_menu.core_mode,
            &mut ctx_menu.next_state,
            &mut ctx_menu.commands,
        );
    }
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
