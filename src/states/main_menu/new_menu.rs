//! New-style main menu: full-screen 3D board background + bottom-left button list.
//!
//! The existing board with all 32 pieces in starting position is rendered in the
//! background using the primary camera. A semi-transparent panel in the bottom-left
//! lists the main navigation options.
//!
//! Press **K** to toggle back to the website-style (classic) menu.


use bevy::prelude::*;
use bevy_egui::egui;

use crate::core::{DespawnOnExit, GameMode, GameState, MenuState};
use crate::game::resources::MenuSounds;
use crate::rendering::pieces::{PieceColor, PieceMeshes, PieceType};
use crate::ui::system_params::MainMenuUIContext;

fn play_click(commands: &mut Commands, sounds: Option<&MenuSounds>) {
    if let Some(s) = sounds {
        commands.spawn(bevy::audio::AudioPlayer::new(s.menu_click.clone()));
    }
}

/// Marker for all menu-background scene entities (board squares, pieces, lights).
#[derive(Component)]
pub struct MenuBg;

/// Tracks whether background pieces have been spawned for the current MainMenu session.
#[derive(Resource, Default)]
pub struct MenuBgPiecesSpawned(pub bool);

#[derive(Resource, Default)]
pub struct MenuExitConfirm {
    pub visible: bool,
}

/// Which panel the new-style menu is currently showing.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum NewMenuPanel {
    #[default]
    Main,
    PlayOnline,
    Tournaments,
    SolanaMultiplayer,
    SolanaConnect,
    HowToPlay,
    Settings,
    Profile,
}

impl NewMenuPanel {
    fn discriminant(self) -> u8 {
        match self {
            Self::Main => 0,
            Self::PlayOnline => 1,
            Self::Tournaments => 2,
            Self::SolanaMultiplayer => 3,
            Self::SolanaConnect => 4,
            Self::HowToPlay => 5,
            Self::Settings => 6,
            Self::Profile => 7,
        }
    }
}

/// Camera world-space position for the board-view style (overridden by orbit each frame).
pub const BOARD_CAM: Vec3 = Vec3::new(3.5, 14.0, -16.0);
/// Board centre the camera looks at.
pub const BOARD_CENTER: Vec3 = Vec3::new(3.5, 0.0, 3.5);

/// Drives a slow cinematic orbit of the 3D menu camera around the board.
#[derive(Resource)]
pub struct MenuCameraOrbit {
    /// Current horizontal angle (radians).
    pub angle: f32,
    /// Distance from BOARD_CENTER on the XZ plane.
    pub radius: f32,
    /// Camera Y height.
    pub height: f32,
    /// Orbit speed (radians / second).
    pub speed: f32,
}

impl Default for MenuCameraOrbit {
    fn default() -> Self {
        Self { angle: 0.0, radius: 16.0, height: 14.0, speed: 0.10 }
    }
}

// ── Spawn systems ────────────────────────────────────────────────────────────

/// Spawn the 8×8 board squares for the menu background.
pub fn spawn_menu_bg_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(1.0, 0.1, 1.0));

    // Lichess palette: #f0d9b5 light / #b58863 dark — unlit for consistent color regardless of camera angle
    let light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.941, 0.851, 0.710),
        unlit: true,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.710, 0.533, 0.388),
        unlit: true,
        ..default()
    });

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let mat = if (file + rank) % 2 == 0 { light.clone() } else { dark.clone() };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_xyz(7.0 - file as f32, 0.0, rank as f32),
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                Name::new(format!("MenuBg-{}{}", (b'a' + file) as char, rank + 1)),
            ));
        }
    }
}

/// Spawn all 32 pieces in starting position for the menu background.
/// Reuses the same [`PieceMeshes`] resource loaded at `Startup` by [`PiecePlugin`].
/// Runs every frame until `PieceMeshes` is available, then spawns once.
pub fn spawn_menu_bg_pieces(
    mut commands: Commands,
    piece_meshes: Option<Res<PieceMeshes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<MenuBgPiecesSpawned>,
    mut anim: ResMut<super::board_animation::BoardAnimator>,
) {
    if spawned.0 {
        return;
    }
    let Some(pm) = piece_meshes else {
        return; // retry next frame
    };

    let white_mat = materials.add(crate::rendering::pieces::white_piece_material());
    let black_mat = materials.add(crate::rendering::pieces::black_piece_material());

    const BACK: [PieceType; 8] = [
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
        PieceType::Bishop,
        PieceType::Knight,
        PieceType::Rook,
    ];
    let rot_w = Quat::IDENTITY;
    let rot_b = Quat::from_rotation_y(std::f32::consts::PI);
    // Knights face +X in the GLB, so need a 90° offset to face the opponent
    let rot_w_knight = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let rot_b_knight = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);

    for (f, &pt) in BACK.iter().enumerate() {
        let file = f as u8;
        let (wr, br) = if pt == PieceType::Knight {
            (rot_w_knight, rot_b_knight)
        } else {
            (rot_w, rot_b)
        };
        // White back rank 0
        let ew = commands.spawn((
            Mesh3d(pm.get(pt, PieceColor::White)),
            MeshMaterial3d(white_mat.clone()),
            Transform::from_xyz(7.0 - f as f32, 0.05, 0.0).with_rotation(wr),
            Visibility::Visible,
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 0 },
            super::board_animation::MenuBgPieceHome { file, rank: 0 },
        )).id();
        anim.board[0][f] = Some(ew);

        // Black back rank 7
        let eb = commands.spawn((
            Mesh3d(pm.get(pt, PieceColor::Black)),
            MeshMaterial3d(black_mat.clone()),
            Transform::from_xyz(7.0 - f as f32, 0.05, 7.0).with_rotation(br),
            Visibility::Visible,
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 7 },
            super::board_animation::MenuBgPieceHome { file, rank: 7 },
        )).id();
        anim.board[7][f] = Some(eb);
    }

    for f in 0..8usize {
        let file = f as u8;
        // White pawns rank 1
        let ewp = commands.spawn((
            Mesh3d(pm.get(PieceType::Pawn, PieceColor::White)),
            MeshMaterial3d(white_mat.clone()),
            Transform::from_xyz(7.0 - f as f32, 0.05, 1.0).with_rotation(rot_w),
            Visibility::Visible,
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 1 },
            super::board_animation::MenuBgPieceHome { file, rank: 1 },
        )).id();
        anim.board[1][f] = Some(ewp);

        // Black pawns rank 6
        let ebp = commands.spawn((
            Mesh3d(pm.get(PieceType::Pawn, PieceColor::Black)),
            MeshMaterial3d(black_mat.clone()),
            Transform::from_xyz(7.0 - f as f32, 0.05, 6.0).with_rotation(rot_b),
            Visibility::Visible,
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 6 },
            super::board_animation::MenuBgPieceHome { file, rank: 6 },
        )).id();
        anim.board[6][f] = Some(ebp);
    }

    spawned.0 = true;
    anim.active = true;
}

/// Purge any scene lights that are NOT tagged MenuBg before the menu re-spawns its own.
/// This acts as a safety net for any in-game lights that slipped through DespawnOnExit.
pub fn purge_stale_lights(
    mut commands: Commands,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
    directional: Query<Entity, (With<DirectionalLight>, Without<MenuBg>)>,
    point: Query<Entity, (With<PointLight>, Without<MenuBg>)>,
) {
    for entity in directional.iter().chain(point.iter()) {
        commands.entity(entity).despawn();
    }
    global_ambient.color = Color::WHITE;
    global_ambient.brightness = 95.0;
}

/// Spawn lights for the background board.
/// Despawns any leftover MenuBg lights first so they don't stack on re-entry.
pub fn spawn_menu_bg_lights(
    mut commands: Commands,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
    existing: Query<Entity, (With<MenuBg>, With<PointLight>)>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    global_ambient.color = Color::WHITE;
    global_ambient.brightness = 95.0;

    // Overhead point light — same as in-game "Angel Light"
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            range: 100.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.5, 20.0, 3.5),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-OverheadLight"),
    ));

    // Rim/fill light behind the black pieces so the back rank stays legible
    commands.spawn((
        PointLight {
            intensity: 900_000.0,
            range: 60.0,
            color: Color::srgb(0.72, 0.82, 1.0),
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.5, 7.0, 13.0),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-RimLight"),
    ));
}

// ── Camera & style systems ───────────────────────────────────────────────────

/// No-op kept for API compatibility — volumetric fog removed for performance.
pub fn setup_menu_fog(
    _commands: Commands,
    _cam: Res<crate::PersistentEguiCamera>,
) {
}

/// Continuously orbits the camera around BOARD_CENTER.
pub fn orbit_camera_system(
    time: Res<Time>,
    mut orbit: ResMut<MenuCameraOrbit>,
    cam: Res<crate::PersistentEguiCamera>,
    mut query: Query<&mut Transform, With<Camera3d>>,
) {
    orbit.angle += orbit.speed * time.delta_secs();
    let x = BOARD_CENTER.x + orbit.radius * orbit.angle.cos();
    let z = BOARD_CENTER.z + orbit.radius * orbit.angle.sin();
    // Look at a point slightly left of board centre so the board sits right-of-centre in screen space
    let look_target = Vec3::new(BOARD_CENTER.x - 1.2, BOARD_CENTER.y, BOARD_CENTER.z);
    if let Some(entity) = cam.entity {
        if let Ok(mut t) = query.get_mut(entity) {
            *t = Transform::from_translation(Vec3::new(x, orbit.height, z))
                .looking_at(look_target, Vec3::Y);
        }
    }
}

/// Handle keyboard shortcuts on the main menu.
/// H → Guide, G → Settings, ESC → back-navigate / exit.
pub fn menu_escape_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<NewMenuPanel>,
    mut exit_confirm: ResMut<MenuExitConfirm>,
) {
    if keyboard.just_pressed(KeyCode::KeyH) {
        *panel = NewMenuPanel::HowToPlay;
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        *panel = NewMenuPanel::Settings;
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        if *panel == NewMenuPanel::Main {
            exit_confirm.visible = true;
        } else {
            *panel = NewMenuPanel::Main;
        }
    }
}

// ── egui panel ───────────────────────────────────────────────────────────────

/// Render the bottom-left button list.
/// Modals (AI setup, controls popup) are rendered by the caller in `main_menu.rs`.
pub fn render_new_style_panel(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    // Corner logos whenever a wallet is connected (any panel)
    if cx.player_identity.username.is_some() || *cx.new_menu_panel == NewMenuPanel::SolanaConnect {
        render_corner_logos(ctx, cx);
    }

    render_title_logo(ctx, cx);
    render_hint_bar(ctx);

    // ── Per-panel fade-in ────────────────────────────────────────────────────
    // Detect panel changes via egui temp storage; when the panel changes,
    // remove the new panel's animation state so it restarts from 0→1.
    let current = *cx.new_menu_panel;
    let prev_id = egui::Id::new("xfc_prev_panel");
    let prev: NewMenuPanel = ctx.data(|d| d.get_temp(prev_id).unwrap_or_default());
    if prev != current {
        ctx.data_mut(|d| {
            d.insert_temp(prev_id, current);
            d.remove::<bool>(egui::Id::new(("panel_fade", current.discriminant())));
        });
    }
    let alpha = ctx.animate_bool_with_time(
        egui::Id::new(("panel_fade", current.discriminant())),
        true,
        0.15,
    );

    // ── Exit confirmation dialog ─────────────────────────────────────────────
    if cx.exit_confirm.visible {
        egui::Window::new("##exit_confirm")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(300.0, 120.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(18, 18, 18, 250),
                corner_radius: egui::CornerRadius::same(6),
                stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 60, 60)),
                inner_margin: egui::Margin::same(24),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Are you sure you want to exit?").size(15.0).color(egui::Color32::WHITE).strong());
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        if ui.add_sized(
                            [110.0, 34.0],
                            egui::Button::new(egui::RichText::new("Exit").size(13.0).color(egui::Color32::WHITE).strong())
                                .fill(egui::Color32::from_rgb(160, 50, 50))
                                .corner_radius(4.0),
                        ).clicked() {
                            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                            std::process::exit(0);
                        }
                        ui.add_space(12.0);
                        if ui.add_sized(
                            [110.0, 34.0],
                            egui::Button::new(egui::RichText::new("Cancel").size(13.0))
                                .fill(egui::Color32::from_rgba_unmultiplied(70, 70, 70, 220))
                                .corner_radius(4.0),
                        ).clicked() {
                            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                            cx.exit_confirm.visible = false;
                        }
                    });
                });
            });
    }

    egui::Window::new("##xfc_new_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .min_size(egui::vec2(340.0, 360.0))
        .anchor(egui::Align2::LEFT_CENTER, egui::vec2(36.0, 0.0))
        .frame(egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            inner_margin: egui::Margin::same(28),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.set_opacity(alpha);
            match current {
                NewMenuPanel::Main => render_main_panel(ui, cx),
                NewMenuPanel::PlayOnline => render_play_online_panel(ui, cx),
                NewMenuPanel::Tournaments => render_tournaments_panel(ui, cx),
                NewMenuPanel::SolanaConnect => render_solana_connect_panel(ui, cx),
                NewMenuPanel::HowToPlay => render_how_to_play_panel(ui, cx),
                NewMenuPanel::Settings => render_settings_panel(ui, cx),
                NewMenuPanel::Profile => render_profile_panel(ui, cx),
                NewMenuPanel::SolanaMultiplayer => {}
            }

            ui.set_opacity(1.0);
        });
}

/// Title logo pinned to top-center — shown on all panels while the 3D menu is active.
fn render_title_logo(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    super::ensure_brand_logo_texture(ctx, &mut cx.brand_logo);
    let Some(ref handle) = cx.brand_logo.texture else { return };
    let [w, h] = handle.size();
    let display_h = 150.0_f32;
    let display_w = (w as f32 / h as f32) * display_h;
    egui::Area::new("title_logo".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 20.0))
        .show(ctx, |ui| {
            ui.add(egui::Image::new(egui::load::SizedTexture::new(handle.id(), [display_w, display_h])));
        });
}

/// Keyboard hint bar pinned to bottom-right — always shown while the 3D menu is active.
fn render_hint_bar(ctx: &egui::Context) {
    egui::Area::new("hint_bar".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let hint_color = egui::Color32::WHITE;
                let size = 13.0;
                let sep_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120);

                ui.label(egui::RichText::new("H - Bring up Guide").size(size).color(hint_color));
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(egui::RichText::new("G - In Game Settings").size(size).color(hint_color));
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(egui::RichText::new("ESC - Exit Game").size(size).color(hint_color));
            });
        });
}

/// Small logos pinned above the hint bar — shown whenever a wallet is connected.
fn render_corner_logos(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    super::ensure_solana_logos(ctx, &mut cx.solana_logos);

    egui::Area::new("corner_logos".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -52.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(ref tex) = cx.solana_logos.texture1 {
                    let [w, h] = tex.size();
                    let dh = 32.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), [dw, dh])));
                    ui.add_space(8.0);
                }
                if let Some(ref tex) = cx.solana_logos.texture2 {
                    let [w, h] = tex.size();
                    let dh = 32.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), [dw, dh])));
                }
            });
        });
}

fn render_main_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
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
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60)),
    );
    ui.add_space(10.0);
    ui.add_space(6.0);

    let snd = cx.menu_sounds.as_deref();

    if item(ui, "Play Against a Computer", W) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_ai_setup = true;
    }
    ui.add_space(SP);

    if item_expandable(ui, "Play Online", W) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::PlayOnline;
    }
    ui.add_space(SP);

    if item(ui, "PGN Replay", W) {
        play_click(&mut cx.commands, snd);
        *cx.core_mode = GameMode::PgnReplay;
        cx.next_state.set(GameState::InGame);
    }
    ui.add_space(SP);

    if item(ui, "XFChess.com", W) {
        play_click(&mut cx.commands, snd);
        if let Err(e) = webbrowser::open("https://xfchess.com") {
            tracing::warn!("[Menu] Failed to open XFChess.com: {}", e);
        }
    }
    ui.add_space(SP);
}

fn render_play_online_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    // Back arrow + "Play Online" label styled the same as the Main Menu header
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("‹ Back")
                    .size(10.0)
                    .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
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
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60)),
    );
    ui.add_space(10.0);

    let snd = cx.menu_sounds.as_deref();

    if item(ui, "Create Lobby", W) {
        play_click(&mut cx.commands, snd);
        cx.menu_state.set(MenuState::HostConfig);
    }
    ui.add_space(SP);

    if item(ui, "Join Lobby", W) {
        play_click(&mut cx.commands, snd);
        // Trigger immediate poll so the lobby list is fresh on arrival
        if let Some(ref mut vps) = cx.p2p_vps_state {
            vps.last_poll = None;
        }
        cx.menu_state.set(MenuState::BraidLobby);
    }
    ui.add_space(SP);

    if item(ui, "Spectator", W) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_spectator_popup = true;
    }
    ui.add_space(SP + 4.0);

    if item_expandable(ui, "Tournaments", W) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::Tournaments;
    }
    ui.add_space(SP);

    if item_expandable(ui, "Solana Multiplayer", W) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::SolanaConnect;
    }
}

fn render_tournaments_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("‹ Back")
                    .size(10.0)
                    .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
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
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60)),
    );
    ui.add_space(10.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;
    let snd = cx.menu_sounds.as_deref();

    if item(ui, "Join Tournament", W) {
        play_click(&mut cx.commands, snd);
        cx.menu_state.set(MenuState::Tournaments);
    }
    ui.add_space(SP);

    if item(ui, "Spectate Tournament", W) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_spectator_popup = true;
    }
}

fn render_how_to_play_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(egui::RichText::new("‹ Back").size(10.0).color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)))
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("How to Play").size(16.5).color(egui::Color32::WHITE).strong());
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical()
        .max_height(420.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(280.0);

            section(ui, "XFChess Modes");
            bullet(ui, "Play vs Computer — Choose difficulty 1–8 and time control.");
            bullet(ui, "Play Online — Host or join a P2P lobby with a friend.");
            bullet(ui, "Tournaments — Compete in Swiss-format brackets.");
            bullet(ui, "Solana Multiplayer — Wager SOL on the outcome. Connect a wallet to unlock.");

            ui.add_space(8.0);
            section(ui, "Controls");
            bullet(ui, "Left-click — Select and move pieces.");
            bullet(ui, "K — Toggle between 3D board menu and classic menu.");
            bullet(ui, "Escape — Return to menu from a game.");
        });
}

fn render_settings_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("‹ Back")
                    .size(10.0)
                    .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
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
    const SP: f32 = 6.0;

    let snd = cx.menu_sounds.as_deref();

    section(ui, "Controls");

    if item(ui, "Keyboard Shortcuts", W) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_controls_popup = true;
    }
    ui.add_space(SP + 4.0);

    section(ui, "Admin");

    if item(ui, "Tournament Admin", W) {
        play_click(&mut cx.commands, snd);
        std::thread::spawn(|| {
            let port: u16 = std::env::var("XFCHESS_WALLET_PORT")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(7454);
            if let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
            {
                let _ = client
                    .post(format!("http://127.0.0.1:{}/api/open-tournament-admin", port))
                    .send();
            }
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(10.1)
            .color(egui::Color32::from_rgb(120, 180, 255))
            .strong(),
    );
    ui.add_space(3.0);
}

fn bullet(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("·").size(9.8).color(egui::Color32::from_rgb(100, 140, 200)));
        ui.label(egui::RichText::new(text).size(9.4).color(egui::Color32::from_rgb(200, 200, 210)));
    });
    ui.add_space(2.0);
}

fn render_solana_connect_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Back button + header
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("‹ Back")
                    .size(10.0)
                    .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
        }
        ui.label(
            egui::RichText::new("Solana Multiplayer")
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
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60)),
    );
    ui.add_space(10.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    let wallet_connected = cx.player_identity.username.is_some();

    // Connect Wallet is always the first item
    let connect_label = if wallet_connected { "Wallet Connected" } else { "Connect Wallet" };
    if ui.add_sized(
        [W, 40.0],
        egui::Button::new(
            egui::RichText::new(connect_label)
                .size(11.6)
                .color(egui::Color32::WHITE)
                .strong()
                .family(egui::FontFamily::Proportional),
        )
        .fill(if wallet_connected {
            egui::Color32::from_rgb(30, 110, 60)
        } else {
            egui::Color32::from_rgb(50, 120, 200)
        })
        .corner_radius(6.0)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40))),
    ).clicked() {
        play_click(&mut cx.commands, cx.menu_sounds.as_deref());
        // Clear stale receivers so poll fires immediately on next frame
        cx.wallet_bridge.status_rx = None;
        cx.wallet_bridge.balance_rx = None;
        cx.wallet_bridge.enabled = true;
        cx.wallet_bridge.timer = 5.0;
        cx.wallet_bridge.show_connect_overlay = true;
        // Signal the Tauri bridge to open the wallet popup in Chrome
        std::thread::spawn(|| {
            use std::io::Write;
            use std::net::TcpStream;
            let base: u16 = std::env::var("XFCHESS_WALLET_PORT")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(7454);
            for offset in 2u16..=11 {
                let port = base.saturating_sub(offset);
                if let Ok(mut s) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
                    let _ = s.write_all(b"OPEN");
                    break;
                }
            }
        });
    }

    // Wagered options only shown once wallet is connected
    if wallet_connected {
        ui.add_space(SP + 6.0);

        let snd = cx.menu_sounds.as_deref();

        if item(ui, "Wagered PVP", W) {
            play_click(&mut cx.commands, snd);
            #[cfg(feature = "solana")]
            if cx.player_identity.has_onchain_profile {
                cx.menu_state.set(crate::core::MenuState::SolanaLobby);
            } else {
                std::thread::spawn(|| {
                    let _ = reqwest::blocking::Client::new()
                        .post("http://127.0.0.1:7454/api/open-profile-step")
                        .send();
                });
            }
            #[cfg(not(feature = "solana"))]
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        if item(ui, "Find Wagered Game", W) {
            play_click(&mut cx.commands, snd);
            #[cfg(feature = "solana")]
            cx.menu_state.set(crate::core::MenuState::SolanaLobby);
            #[cfg(not(feature = "solana"))]
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
    } else {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Connect a Solana wallet to access wagered games")
                .size(8.6)
                .color(egui::Color32::from_rgb(130, 130, 150))
                .italics(),
        );
    }
}

fn render_profile_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Header row
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("‹ Back")
                    .size(10.0)
                    .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Profile")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(16.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    let connected = cx.wallet_bridge.enabled && cx.wallet_bridge.known_pubkey.is_some();

    if connected {
        // Username
        let name = cx.player_identity.display_name().to_string();
        ui.label(
            egui::RichText::new(&name)
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong()
                .family(egui::FontFamily::Proportional),
        );
        ui.add_space(4.0);

        // ELO
        let elo = cx.player_identity.display_elo();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("ELO").size(9.0).color(egui::Color32::from_rgb(120, 140, 170)));
            ui.label(egui::RichText::new(&elo).size(11.0).color(egui::Color32::from_rgb(200, 220, 255)).strong());
        });
        ui.add_space(2.0);

        // Country
        if let Some(ref country) = cx.player_identity.country {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Country").size(9.0).color(egui::Color32::from_rgb(120, 140, 170)));
                ui.label(egui::RichText::new(country).size(11.0).color(egui::Color32::from_rgb(200, 210, 200)));
            });
            ui.add_space(2.0);
        }

        // Wallet pubkey (shortened)
        if let Some(ref pk) = cx.wallet_bridge.known_pubkey.clone() {
            let short = format!("{}...{}", &pk[..6.min(pk.len())], &pk[pk.len().saturating_sub(4)..]);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Wallet").size(9.0).color(egui::Color32::from_rgb(120, 140, 170)));
                ui.label(egui::RichText::new(&short).size(9.5).color(egui::Color32::from_rgb(160, 180, 160)).monospace());
            });
        }

        // SOL balance — always shown once connected
        {
            let data = cx.wallet_bridge.data.lock().unwrap();
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Balance").size(9.0).color(egui::Color32::from_rgb(120, 140, 170)));
                let sol_text = format!("{:.4} SOL", data.sol_balance);
                let sol_color = if data.sol_balance > 0.0 {
                    egui::Color32::from_rgb(140, 220, 140)
                } else {
                    egui::Color32::from_rgb(140, 140, 150)
                };
                ui.label(egui::RichText::new(&sol_text).size(11.0).color(sol_color));
                if let Some(usd) = data.usd_balance {
                    if usd > 0.0 {
                        ui.label(egui::RichText::new(format!("(${:.2})", usd)).size(9.0).color(egui::Color32::from_rgb(140, 160, 140)));
                    }
                }
            });
        }

        ui.add_space(SP + 6.0);

        // Disconnect
        if ui.add_sized(
            [W, 34.0],
            egui::Button::new(
                egui::RichText::new("Disconnect Wallet")
                    .size(10.8)
                    .color(egui::Color32::WHITE)
                    .family(egui::FontFamily::Proportional),
            )
            .fill(egui::Color32::from_rgb(100, 40, 40))
            .corner_radius(6.0),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            cx.wallet_bridge.enabled = false;
            cx.wallet_bridge.known_pubkey = None;
            cx.wallet_bridge.timer = 0.0;
            cx.wallet_bridge.status_rx = None;
            cx.wallet_bridge.balance_rx = None;
            if let Ok(mut d) = cx.wallet_bridge.data.lock() {
                d.sol_balance = 0.0;
                d.usd_balance = None;
            }
            *cx.player_identity = crate::states::main_menu::PlayerIdentity::default();
        }
        ui.add_space(SP);

        // Create Profile — shown when wallet is connected but no backend profile found
        if cx.player_identity.username.is_none() {
            ui.add_space(SP);
            ui.label(
                egui::RichText::new("No profile found for this wallet.")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(200, 160, 80))
                    .italics(),
            );
            ui.add_space(4.0);
            if ui.add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Create Profile")
                        .size(11.0)
                        .color(egui::Color32::WHITE)
                        .strong()
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgb(130, 80, 20))
                .corner_radius(6.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 200, 80, 80))),
            ).clicked() {
                play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                let _ = webbrowser::open("http://localhost:5174/create-profile");
            }
            ui.add_space(SP);
        }

        // Refresh balance
        if ui.add_sized(
            [W, 30.0],
            egui::Button::new(
                egui::RichText::new("Refresh")
                    .size(10.0)
                    .color(egui::Color32::from_rgb(180, 200, 220))
                    .family(egui::FontFamily::Proportional),
            )
            .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12))
            .corner_radius(6.0),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            // Force a status re-poll and clear balance_rx so balance is re-fetched too
            cx.wallet_bridge.timer = 5.0;
            cx.wallet_bridge.balance_rx = None;
            // Reset balance display so user sees it's refreshing
            if let Ok(mut d) = cx.wallet_bridge.data.lock() {
                d.sol_balance = 0.0;
                d.usd_balance = None;
            }
        }
    } else {
        // Not connected — direct user to the correct place
        ui.label(
            egui::RichText::new("No wallet connected")
                .size(13.0)
                .color(egui::Color32::from_rgb(160, 160, 180))
                .italics(),
        );
        ui.add_space(SP + 4.0);
        ui.label(
            egui::RichText::new("Connect your wallet from the Solana Multiplayer menu.")
                .size(9.2)
                .color(egui::Color32::from_rgb(120, 130, 150)),
        );
        ui.add_space(SP + 4.0);

        if ui.add_sized(
            [W, 34.0],
            egui::Button::new(
                egui::RichText::new("Go to Solana Multiplayer →")
                    .size(10.5)
                    .color(egui::Color32::from_rgb(140, 180, 255))
                    .family(egui::FontFamily::Proportional),
            )
            .fill(egui::Color32::from_rgba_unmultiplied(40, 80, 160, 60))
            .corner_radius(6.0)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 140, 255, 80))),
        ).clicked() {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
        }
    }
}

/// Full-screen Solana splash: pure black background, two logos bottom-right.
pub fn render_solana_splash(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    // Ensure textures are loaded
    super::ensure_solana_logos(ctx, &mut cx.solana_logos);

    // Black full-screen background
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            if ui.add(
                egui::Button::new(
                    egui::RichText::new("Back")
                        .size(13.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            ).clicked() {
                *cx.new_menu_panel = NewMenuPanel::Main;
            }
        });

    // Logos anchored to bottom-right via a floating Area
    egui::Area::new("solana_logos_area".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -24.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(ref tex) = cx.solana_logos.texture1 {
                    let [w, h] = tex.size();
                    let dh = 72.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), [dw, dh])));
                    ui.add_space(16.0);
                }
                if let Some(ref tex) = cx.solana_logos.texture2 {
                    let [w, h] = tex.size();
                    let dh = 72.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), [dw, dh])));
                }
            });
        });
}

/// Render username + wallet balance in the top-right corner of the main menu.
/// Clicking the balance section cycles through SOL → USD → GBP.
pub fn render_wallet_hud(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    let display_name = cx.player_identity.display_name().to_string();
    let is_guest = cx.player_identity.username.is_none();

    #[cfg(feature = "solana")]
    let sol_balance = cx.solana_state.as_ref().map(|s| s.balance).unwrap_or(0.0);
    #[cfg(not(feature = "solana"))]
    let sol_balance = cx.wallet_bridge.data.lock().map(|d| d.sol_balance).unwrap_or(0.0);

    #[cfg(feature = "solana")]
    let connected = cx.solana_state.as_ref().and_then(|s| s.wallet_pubkey).is_some();
    #[cfg(not(feature = "solana"))]
    let connected = false;

    if !connected && is_guest {
        return;
    }

    let (sol_usd_rate, sol_gbp_rate) = cx.wallet_bridge.data.lock()
        .map(|d| (d.sol_usd_rate, d.sol_gbp_rate))
        .unwrap_or((0.0, 0.0));

    // 0 = SOL, 1 = USD, 2 = GBP — persisted in egui temp storage across frames.
    let currency_id = egui::Id::new("balance_currency");
    let currency_mode = ctx.data(|d| d.get_temp::<u8>(currency_id).unwrap_or(0));

    let (balance_text, balance_color) = match currency_mode {
        1 => {
            if sol_usd_rate > 0.0 {
                (format!("${:.2}", sol_balance * sol_usd_rate), egui::Color32::from_rgb(20, 241, 149))
            } else {
                (format!("{:.3} SOL", sol_balance), egui::Color32::from_rgb(20, 241, 149))
            }
        }
        2 => {
            if sol_gbp_rate > 0.0 {
                (format!("£{:.2}", sol_balance * sol_gbp_rate), egui::Color32::from_rgb(20, 241, 149))
            } else {
                (format!("{:.3} SOL", sol_balance), egui::Color32::from_rgb(20, 241, 149))
            }
        }
        _ => (format!("{:.3} SOL", sol_balance), egui::Color32::from_rgb(20, 241, 149)),
    };

    egui::Area::new("wallet_hud".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
        .show(ctx, |ui| {
            egui::Frame {
                corner_radius: egui::CornerRadius::same(8),
                fill: egui::Color32::from_rgba_unmultiplied(20, 20, 25, 220),
                stroke: egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
                inner_margin: egui::Margin::symmetric(14, 10),
                ..egui::Frame::NONE
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&display_name)
                            .size(10.5)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Clickable balance — cycles SOL → USD → GBP on each click.
                    let bal_resp = ui.add(
                        egui::Button::new(
                            egui::RichText::new(&balance_text)
                                .size(10.1)
                                .color(balance_color)
                                .strong(),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    );
                    if bal_resp.clicked() {
                        ctx.data_mut(|d| d.insert_temp(currency_id, (currency_mode + 1) % 3));
                    }
                    if bal_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
            });
        });
}

/// Same as [`item`] but draws a `›` chevron on the right to signal expansion.
fn item_expandable(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);
    let chevron_col = egui::Color32::from_rgb(120, 140, 180);

    // Reserve background shape slot BEFORE adding the button so the highlight
    // renders behind the text (not on top of it).
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let accent_idx = ui.painter().add(egui::Shape::Noop);

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(22.0)
                .color(btn_text)
                .family(egui::FontFamily::Proportional)
        )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(width, 46.0)),
    );

    let r = resp.rect;
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().set(bg_idx, egui::Shape::rect_filled(
            r.expand(1.0),
            egui::CornerRadius::same(4),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 11),
        ));
        ui.painter().set(accent_idx, egui::Shape::rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.left(), r.center().y - 13.0),
                egui::vec2(3.0, 26.0),
            ),
            egui::CornerRadius::same(2),
            egui::Color32::from_rgb(90, 160, 255),
        ));
    }

    // Chevron always visible
    ui.painter().text(
        egui::pos2(r.right() - 10.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(28.0),
        chevron_col,
    );
    resp.clicked()
}

/// A transparent button with a left-side accent bar on hover.
fn item(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);

    // Reserve background shape slots BEFORE the button so highlights render behind text.
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let accent_idx = ui.painter().add(egui::Shape::Noop);

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(22.0)
                .color(btn_text)
                .family(egui::FontFamily::Proportional)
        )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(width, 46.0)),
    );

    let r = resp.rect;
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().set(bg_idx, egui::Shape::rect_filled(
            r.expand(1.0),
            egui::CornerRadius::same(4),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        ));
        ui.painter().set(accent_idx, egui::Shape::rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.left(), r.center().y - 13.0),
                egui::vec2(3.0, 26.0),
            ),
            egui::CornerRadius::same(2),
            egui::Color32::from_rgb(90, 160, 255),
        ));
    }

    resp.clicked()
}
