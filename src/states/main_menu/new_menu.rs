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

/// Click sounds are now played globally by [`menu_click_sound`] for *any* egui
/// UI click (covering every popup — Host Game, Play vs Bot, dialogs, etc.), so
/// this per-call helper is a no-op kept so existing call sites compile unchanged.
fn play_click(_commands: &mut Commands, _sounds: Option<&MenuSounds>) {}

/// Plays `menu_click.mp3` whenever the user presses the mouse over any egui UI
/// area (menu items and every popup). The 3D board background is not an egui
/// area, so clicks there stay silent. Runs while the main menu is active.
pub(super) fn menu_click_sound(
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
    if ctx.is_pointer_over_area() {
        commands.spawn(bevy::audio::AudioPlayer::new(sounds.menu_click.clone()));
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

/// "Learn" focus mode — toggled with **L**. While active, every menu UI
/// element is hidden except the ambient board caption.
#[derive(Resource, Default)]
pub struct MenuFocusMode {
    pub active: bool,
}

/// Which panel the new-style menu is currently showing.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum NewMenuPanel {
    #[default]
    Main,
    PlayOnline,
    Puzzles,
    Tournaments,
    SolanaMultiplayer,
    SolanaConnect,
    DirectConnection,
    HowToPlay,
    Settings,
    Profile,
}

impl NewMenuPanel {
    fn discriminant(self) -> u8 {
        match self {
            Self::Main => 0,
            Self::PlayOnline => 1,
            Self::Puzzles => 2,
            Self::Tournaments => 3,
            Self::SolanaMultiplayer => 4,
            Self::SolanaConnect => 5,
            Self::DirectConnection => 6,
            Self::HowToPlay => 7,
            Self::Settings => 8,
            Self::Profile => 9,
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
    /// When true the camera holds a fixed isometric orthographic view
    /// (same projection as the TempleOS in-game camera) instead of orbiting.
    /// Toggled with **V**.
    pub ortho: bool,
}

impl Default for MenuCameraOrbit {
    fn default() -> Self {
        Self {
            angle: 0.0,
            radius: 16.0,
            height: 14.0,
            speed: 0.10,
            ortho: false,
        }
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

    // Match the in-game board exactly (see `SquareMaterials` in rendering/utils.rs):
    // lit PBR materials, Cream light squares / Green dark squares. Lit (not unlit)
    // so the board takes the same shading + piece shadows as during a game.
    let light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.93, 0.82), // Cream
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.59, 0.34), // Green
        ..default()
    });

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let mat = if (file + rank) % 2 == 0 {
                light.clone()
            } else {
                dark.clone()
            };
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
///
/// Self-healing: the guard is the actual world (are any ambient pieces present?),
/// not a one-shot bool. A bool flag can desync from reality — e.g. a transient
/// `MainMenu` exit despawns the pieces via `DespawnOnExit` but the flag stays set,
/// leaving the board empty forever. Keying off `MenuBgPieceHome` existence means
/// the pieces are always (re)spawned whenever they're missing and meshes are ready.
pub fn spawn_menu_bg_pieces(
    mut commands: Commands,
    piece_meshes: Option<Res<PieceMeshes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<MenuBgPiecesSpawned>,
    mut anim: ResMut<super::board_animation::BoardAnimator>,
    existing: Query<(), With<super::board_animation::MenuBgPieceHome>>,
) {
    if !existing.is_empty() {
        return; // ambient pieces already present
    }
    let Some(pm) = piece_meshes else {
        return; // meshes not loaded yet — retry next frame
    };

    // Each piece gets its OWN material instance (not a shared handle) so a
    // captured piece can fade its own alpha without affecting the others — see
    // `MenuPieceFade` in board_animation.rs.
    let white_mat = || crate::rendering::pieces::white_piece_material();
    let black_mat = || crate::rendering::pieces::black_piece_material();

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
        let ew = commands
            .spawn((
                Mesh3d(pm.get(pt, PieceColor::White)),
                MeshMaterial3d(materials.add(white_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 0.0).with_rotation(wr),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::board_animation::MenuBgPieceHome { file, rank: 0 },
            ))
            .id();
        anim.board[0][f] = Some(ew);

        // Black back rank 7
        let eb = commands
            .spawn((
                Mesh3d(pm.get(pt, PieceColor::Black)),
                MeshMaterial3d(materials.add(black_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 7.0).with_rotation(br),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::board_animation::MenuBgPieceHome { file, rank: 7 },
            ))
            .id();
        anim.board[7][f] = Some(eb);
    }

    for f in 0..8usize {
        let file = f as u8;
        // White pawns rank 1
        let ewp = commands
            .spawn((
                Mesh3d(pm.get(PieceType::Pawn, PieceColor::White)),
                MeshMaterial3d(materials.add(white_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 1.0).with_rotation(rot_w),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::board_animation::MenuBgPieceHome { file, rank: 1 },
            ))
            .id();
        anim.board[1][f] = Some(ewp);

        // Black pawns rank 6
        let ebp = commands
            .spawn((
                Mesh3d(pm.get(PieceType::Pawn, PieceColor::Black)),
                MeshMaterial3d(materials.add(black_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 6.0).with_rotation(rot_b),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::board_animation::MenuBgPieceHome { file, rank: 6 },
            ))
            .id();
        anim.board[6][f] = Some(ebp);
    }

    spawned.0 = true;
    // Fresh starting position ⇒ reset the replay so a (re)spawn always starts the
    // Immortal-Zugzwang sequence from move 1 rather than resuming a stale ply.
    anim.ply_index = 0;
    anim.reset = super::board_animation::ResetPhase::Idle;
    anim.move_timer = 2.5;
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

    // Match the in-game ambient tone (visual.rs: bluish, brightness 95).
    global_ambient.color = Color::srgb(0.9, 0.92, 1.0);
    global_ambient.brightness = 95.0;

    // Overhead point light — identical to the in-game "Angel Light" (game_init.rs).
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            range: 100.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.5, 20.0, 3.5),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-OverheadLight"),
    ));

    // Camera-following fill "headlamp" — identical to the in-game fill (visual.rs).
    // Tagged with the same `CameraFollowLight` marker so `update_board_fill_light`
    // (registered to also run in MainMenu) keeps it at the orbiting camera's side,
    // giving the menu the exact viewer-facing lighting a game has.
    commands.spawn((
        PointLight {
            intensity: 600_000.0,
            range: 80.0,
            color: Color::srgb(0.95, 0.96, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.5, 12.0, 3.5),
        crate::game::systems::visual::CameraFollowLight,
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-FillLight (camera-follow)"),
    ));
}

// ── Camera & style systems ───────────────────────────────────────────────────

/// No-op kept for API compatibility — volumetric fog removed for performance.
pub fn setup_menu_fog(_commands: Commands, _cam: Res<crate::PersistentEguiCamera>) {}

/// Continuously orbits the camera around BOARD_CENTER.
/// Press **V** to toggle a fixed isometric orthographic view of the board
/// (the TempleOS-style projection) instead of the orbit.
pub fn orbit_camera_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut orbit: ResMut<MenuCameraOrbit>,
    cam: Res<crate::PersistentEguiCamera>,
    mut query: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        orbit.ortho = !orbit.ortho;
        info!(
            "[MENU] Toggled menu board view to {}",
            if orbit.ortho { "orthographic" } else { "orbit" }
        );
    }

    let Some(entity) = cam.entity else { return };
    let Ok((mut t, mut proj)) = query.get_mut(entity) else {
        return;
    };

    if orbit.ortho {
        // Fixed isometric view — mirrors setup_templeos_camera: equal offsets on
        // X/Y/Z from the board centre with a FixedVertical orthographic projection.
        if !matches!(*proj, Projection::Orthographic(_)) {
            *proj = Projection::from(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: 16.0,
                },
                ..OrthographicProjection::default_3d()
            });
        }
        let offset = 5.0;
        let pos = Vec3::new(BOARD_CENTER.x + offset, offset, BOARD_CENTER.z + offset);
        *t = Transform::from_translation(pos).looking_at(BOARD_CENTER, Vec3::Y);
        return;
    }

    if !matches!(*proj, Projection::Perspective(_)) {
        *proj = Projection::default();
    }
    orbit.angle += orbit.speed * time.delta_secs();
    let x = BOARD_CENTER.x + orbit.radius * orbit.angle.cos();
    let z = BOARD_CENTER.z + orbit.radius * orbit.angle.sin();
    // Look at the true board centre so the board is horizontally centred on
    // screen, symmetric with the centred title.
    *t = Transform::from_translation(Vec3::new(x, orbit.height, z))
        .looking_at(BOARD_CENTER, Vec3::Y);
}

/// Handle keyboard shortcuts on the main menu.
/// H → Guide, G → Settings, L → toggle Learn focus mode, ESC → back-navigate / exit.
pub fn menu_escape_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<NewMenuPanel>,
    mut exit_confirm: ResMut<MenuExitConfirm>,
    mut focus_mode: ResMut<MenuFocusMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        focus_mode.active = !focus_mode.active;
        return;
    }
    if focus_mode.active {
        // Any other shortcut first drops out of focus mode rather than
        // acting underneath the hidden UI.
        if keyboard.just_pressed(KeyCode::KeyH)
            || keyboard.just_pressed(KeyCode::KeyG)
            || keyboard.just_pressed(KeyCode::Escape)
        {
            focus_mode.active = false;
        }
        return;
    }
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
    render_board_caption(ctx);

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
                            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                            std::process::exit(0);
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
                NewMenuPanel::Puzzles => render_puzzles_panel(ui, cx),
                NewMenuPanel::Tournaments => render_tournaments_panel(ui, cx),
                NewMenuPanel::SolanaConnect => render_solana_connect_panel(ui, cx),
                NewMenuPanel::DirectConnection => render_direct_connection_panel(ui, cx),
                NewMenuPanel::HowToPlay => render_how_to_play_panel(ui, cx),
                NewMenuPanel::Settings => render_settings_panel(ui, cx),
                NewMenuPanel::Profile => render_profile_panel(ui, cx),
                NewMenuPanel::SolanaMultiplayer => {}
            }

            ui.set_opacity(1.0);
        });
}

/// Title logo — pinned top-center and fades out when clicked. Not draggable
/// (the welcome card is the draggable element). Shown on all panels while the
/// 3D menu is active.
fn render_title_logo(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    super::ensure_brand_logo_texture(ctx, &mut cx.brand_logo);

    // Image when the asset loaded; otherwise fall back to styled text so the
    // title is never blank if the logo file can't be found at runtime.
    let image = cx.brand_logo.texture.as_ref().map(|handle| {
        let [w, h] = handle.size();
        let display_h = 150.0_f32;
        (handle.id(), (w as f32 / h as f32) * display_h, display_h)
    });

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
            let resp = match image {
                Some((id, display_w, display_h)) => ui
                    .add(egui::Image::new(egui::load::SizedTexture::new(
                        id,
                        [display_w, display_h],
                    )))
                    .interact(egui::Sense::click()),
                None => ui
                    .vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("XFCHESS")
                                .size(60.0)
                                .strong()
                                .color(egui::Color32::from_rgb(235, 238, 245)),
                        );
                        ui.label(
                            egui::RichText::new("COMPETITIVE CHESS SERVER")
                                .size(15.0)
                                .color(egui::Color32::from_rgba_unmultiplied(200, 205, 215, 200)),
                        );
                    })
                    .response
                    .interact(egui::Sense::click()),
            };

            // A click starts the fade-out.
            if resp.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(clicked_id, true));
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
}

/// Caption under the ambient board naming the game it replays
/// (Sämisch vs. Nimzowitsch, Copenhagen 1923 — the Immortal Zugzwang Game).
/// The board is horizontally centred on screen, so bottom-center sits under it.
pub(super) fn render_board_caption(ctx: &egui::Context) {
    egui::Area::new("board_caption".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -64.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Sämisch vs. Nimzowitsch, Copenhagen 1923")
                    .size(13.0)
                    .italics()
                    .color(egui::Color32::from_rgba_unmultiplied(216, 202, 168, 185)),
            );
        });
}

/// Keyboard hint bar pinned to bottom-right — always shown while the 3D menu is active.
fn render_hint_bar(ctx: &egui::Context) {
    egui::Area::new("hint_bar".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                let hint_color = egui::Color32::WHITE;
                let size = 10.5;
                let sep_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120);

                ui.label(
                    egui::RichText::new("H - Bring up Guide")
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
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(
                    egui::RichText::new("F11 - Minimise / Maximise")
                        .size(size)
                        .color(hint_color),
                );
                ui.label(egui::RichText::new("|").size(size).color(sep_color));
                ui.label(
                    egui::RichText::new("L - Learn")
                        .size(size)
                        .color(hint_color),
                );
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
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        tex.id(),
                        [dw, dh],
                    )));
                    ui.add_space(8.0);
                }
                if let Some(ref tex) = cx.solana_logos.texture2 {
                    let [w, h] = tex.size();
                    let dh = 32.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        tex.id(),
                        [dw, dh],
                    )));
                }
            });
        });
}

/// Alpha announcement card shown on the startup (main) menu. Starts docked to
/// the far right of the screen, locked in place (not draggable). Dismissable;
/// stays closed for the rest of the session once closed.
fn render_welcome_panel(
    ctx: &egui::Context,
    _commands: &mut Commands,
    _sounds: Option<&MenuSounds>,
) {
    let welcome_closed_id = egui::Id::new("startup_welcome_closed");
    if ctx.data(|d| d.get_temp::<bool>(welcome_closed_id).unwrap_or(false)) {
        return;
    }

    let panel_frame = egui::Frame {
        corner_radius: egui::CornerRadius::same(8),
        fill: egui::Color32::from_rgba_unmultiplied(8, 8, 12, 240),
        stroke: egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        ),
        inner_margin: egui::Margin::symmetric(18, 16),
        ..egui::Frame::NONE
    };
    // Pinned to the far right (vertically centred) and locked in place — the
    // welcome card is anchored and not draggable.
    egui::Window::new("xfchess_welcome_panel")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .movable(false)
        .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-20.0, 0.0))
        .fixed_size([300.0, 380.0])
        .frame(panel_frame)
        .show(ctx, |ui| {
            // Header: title + close button.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("XFChess Alpha")
                        .size(17.0)
                        .color(egui::Color32::from_rgb(100, 200, 255))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.add(
                        egui::Button::new(
                            // Plain "X" — the menu fonts have no "✕" glyph.
                            egui::RichText::new("X")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(180, 180, 180)),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    );
                    if close.clicked() {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(welcome_closed_id, true));
                    }
                    if close.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
            });

            ui.add_space(8.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // Scrollable body so longer copy (and images) never overflow the card.
            egui::ScrollArea::vertical()
                .max_height(270.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // A few short welcome paragraphs. `para` is a small local helper
                    // for consistent body text + spacing.
                    let para = |ui: &mut egui::Ui, text: &str| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(210, 215, 225)),
                        );
                        ui.add_space(8.0);
                    };

                    // First line: bold "XFChess Alpha" inline (egui RichText can't
                    // mix weights in one label, so compose it across segments).
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let seg = |t: &str| {
                            egui::RichText::new(t)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(210, 215, 225))
                        };
                        ui.label(seg("Welcome to "));
                        ui.label(seg("XFChess Alpha").strong());
                        ui.label(seg(" — thanks for being here early."));
                    });
                    ui.add_space(8.0);
                    para(
                        ui,
                        "Play 3D chess against the engine, friends, or players online.",
                    );
                    para(
                        ui,
                        "Compete in matches and tournaments built for fast, fair \
                         competition.",
                    );
                    para(
                        ui,
                        "Expect changes, report bugs, and help shape what XFChess becomes.",
                    );

                    ui.add_space(2.0);
                    // ── Image slot ───────────────────────────────────────────────
                    // Drop announcement screenshots here once their egui textures are
                    // available (load them into a State resource like `SolanaLogoState`,
                    // then render with:
                    //   let [w, h] = tex.size();
                    //   ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    //       tex.id(), [width, width * h as f32 / w as f32])));
                    // See `render_corner_logos` for the existing texture pattern.

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Pick a mode on the left to begin.")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(100, 200, 255))
                            .strong(),
                    );
                });
        });
}

fn render_main_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Startup welcome card, pinned to the far right of the screen.
    render_welcome_panel(ui.ctx(), &mut cx.commands, cx.menu_sounds.as_deref());

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
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    // Live online-player count (refreshed every ~15s by the social subsystem).
    let online = cx.online_players.count;
    ui.horizontal(|ui| {
        // Paint the status dot directly — the menu fonts (Cinzel/OpenSans) have no
        // "●" glyph, so a text bullet would render as a missing-glyph box.
        let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 11.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot.center(), 4.0, egui::Color32::from_rgb(120, 220, 140));
        ui.label(
            egui::RichText::new(format!("{online} online"))
                .size(11.0)
                .color(egui::Color32::from_rgb(200, 220, 210))
                .family(egui::FontFamily::Proportional),
        );
    });
    ui.add_space(6.0);

    let snd = cx.menu_sounds.as_deref();

    if item_tip(
        ui,
        "Play Against a Computer",
        "Play offline against the engine — pick your side, difficulty, and time control.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_ai_setup = true;
    }
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Play Online",
        "Host or join a live game against a friend or a matched opponent.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::PlayOnline;
    }
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Puzzles",
        "Solve tactics puzzles, or take on puzzle challenges to earn rewards.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::Puzzles;
    }
    ui.add_space(SP);

    if item_tip(
        ui,
        "PGN Replay",
        "Load a PGN and step through any game move by move.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        *cx.core_mode = GameMode::PgnReplay;
        cx.next_state.set(GameState::InGame);
    }
    ui.add_space(SP);

    // TempleOS tribute mode — dev builds only (`--features templeos`).
    #[cfg(feature = "templeos")]
    {
        if item_tip(
            ui,
            "TempleOS",
            "Single-player game in the retro TempleOS-style isometric view.",
            W,
        ) {
            play_click(&mut cx.commands, snd);
            *cx.view_mode = crate::game::view_mode::ViewMode::TempleOS;
            *cx.core_mode = GameMode::SinglePlayer;
            cx.next_state.set(GameState::InGame);
        }
        ui.add_space(SP);
    }

    if item_tip(
        ui,
        "XFChess.com",
        "Open the XFChess website in your browser.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        // Point at the locally deployed site when XFCHESS_WEB_URL is set (dev
        // stack exports http://localhost:5173); otherwise the public site.
        let url =
            std::env::var("XFCHESS_WEB_URL").unwrap_or_else(|_| "https://xfchess.com".to_string());
        if let Err(e) = webbrowser::open(&url) {
            tracing::warn!("[Menu] Failed to open {}: {}", url, e);
        }
    }
    ui.add_space(SP);
}

fn render_play_online_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    // Back arrow + "Play Online" label styled the same as the Main Menu header
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
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
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    let snd = cx.menu_sounds.as_deref();

    if item(ui, "Create Lobby", W) {
        play_click(&mut cx.commands, snd);
        cx.p2p_host.direct_mode = false;
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
    ui.add_space(SP);

    if item_expandable_tip(
        ui,
        "Direct Connection",
        "This works by using an ID. You copy it and send it to your friend — no login, no lobby list.",
        W,
    ) {
        play_click(&mut cx.commands, snd);
        *cx.new_menu_panel = NewMenuPanel::DirectConnection;
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

/// Raw node-ID P2P connection — no account, no login, not listed anywhere.
/// Distinct from "Create Lobby"/"Join Lobby" above, which go through the
/// VPS-backed public lobby directory. This is chess-player language, not
/// programmer language: "This works by using an ID. You copy it and send it
/// to your friend." See docs/plans/identity-implementation-plan.md.
fn render_direct_connection_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
        }
        ui.label(
            egui::RichText::new("Direct Connection")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(10.0);

    ui.label(
        egui::RichText::new(
            "Play a friend directly — no account, no lobby list. One of you hosts and shares an ID; the other pastes it in to connect.",
        )
        .size(12.0)
        .color(egui::Color32::from_rgb(200, 200, 215)),
    );
    ui.add_space(16.0);

    let snd = cx.menu_sounds.as_deref();

    ui.label(
        egui::RichText::new("HOST")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 220, 140))
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Start a game and get an ID to send to your friend.")
            .size(11.0)
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    ui.add_space(6.0);
    if item(ui, "Host a Game", W) {
        play_click(&mut cx.commands, snd);
        cx.p2p_host.direct_mode = true;
        cx.menu_state.set(MenuState::HostConfig);
    }

    ui.add_space(20.0);
    ui.label(
        egui::RichText::new("JOIN")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 180, 255))
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Paste the ID your friend sent you.")
            .size(11.0)
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    ui.add_space(6.0);

    if let Some(p2p_ui) = cx.p2p_ui.as_mut() {
        ui.add(
            egui::TextEdit::singleline(&mut p2p_ui.peer_input)
                .hint_text("Friend's node ID")
                .desired_width(W),
        );
        ui.add_space(8.0);

        if let Some(err) = &p2p_ui.error_message {
            ui.label(
                egui::RichText::new(err)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(240, 120, 120)),
            );
            ui.add_space(6.0);
        }

        if item(ui, "Connect", W) {
            play_click(&mut cx.commands, snd);
            match p2p_ui.validate_node_id() {
                Ok(()) => {
                    p2p_ui.clear_error();
                    let peer_node_id = p2p_ui.peer_input.trim().to_string();
                    if let Some(connect_events) = cx.connect_events.as_mut() {
                        connect_events.write(
                            crate::multiplayer::network::p2p::ConnectToPeerEvent { peer_node_id },
                        );
                    }
                }
                Err(e) => p2p_ui.set_error(e),
            }
        }
    }

    if let Some(p2p_state) = cx.p2p_state.as_ref() {
        let status_text = match &p2p_state.status {
            crate::multiplayer::network::p2p::P2PConnectionStatus::Connecting => {
                Some(("Connecting…", egui::Color32::GOLD))
            }
            crate::multiplayer::network::p2p::P2PConnectionStatus::Connected => {
                Some(("Connected!", egui::Color32::from_rgb(120, 220, 140)))
            }
            crate::multiplayer::network::p2p::P2PConnectionStatus::Error(msg) => {
                Some((msg.as_str(), egui::Color32::from_rgb(240, 120, 120)))
            }
            _ => None,
        };
        if let Some((text, color)) = status_text {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(text).size(12.0).color(color));
        }
    }
}

fn render_puzzles_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    // Back arrow + "Puzzles" header (matches the other sub-panels).
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
        }
        ui.label(
            egui::RichText::new("Puzzles")
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
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    let snd = cx.menu_sounds.as_deref();

    // ── Play section ───────────────────────────────────────────────────────
    ui.label(
        egui::RichText::new("PLAY")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 180, 255))
            .strong(),
    );
    ui.add_space(4.0);
    if item(ui, "Solve Puzzles", W) {
        play_click(&mut cx.commands, snd);
        // No reward in Solve mode, so Guests can play too — fall back to the
        // local node ID as an identifier when there's no wallet/account.
        let wallet = cx
            .player_identity
            .pubkey_str
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(crate::multiplayer::network::identity::node_id_b58);
        cx.commands
            .insert_resource(crate::puzzle::PendingPuzzleRequest {
                mode: crate::puzzle::PuzzleMode::Solve,
                wallet,
            });
    }
    ui.add_space(SP * 2.0);

    // ── Earn section ───────────────────────────────────────────────────────
    ui.label(
        egui::RichText::new("EARN")
            .size(11.0)
            .color(egui::Color32::from_rgb(120, 220, 140))
            .strong(),
    );
    ui.add_space(4.0);
    if item(ui, "Puzzles (Earn)", W) {
        play_click(&mut cx.commands, snd);
        let wallet = cx.player_identity.pubkey_str.clone().unwrap_or_default();
        cx.commands
            .insert_resource(crate::puzzle::PendingPuzzleRequest {
                mode: crate::puzzle::PuzzleMode::Earn,
                wallet,
            });
    }
}

fn render_tournaments_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
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
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
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
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            play_click(&mut cx.commands, cx.menu_sounds.as_deref());
            *cx.new_menu_panel = NewMenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("How to Play")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical()
        .max_height(420.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(280.0);

            section(ui, "XFChess Modes");
            bullet(
                ui,
                "Play vs Computer — Choose difficulty 1–8 and time control.",
            );
            bullet(ui, "Play Online — Host or join a P2P lobby with a friend.");
            bullet(ui, "Tournaments — Compete in Swiss-format brackets.");
            bullet(
                ui,
                "Solana Multiplayer — Wager SOL on the outcome. Connect a wallet to unlock.",
            );

            ui.add_space(8.0);
            section(ui, "Controls");
            bullet(ui, "Left-click — Select and move pieces.");
            bullet(ui, "K — Toggle between 3D board menu and classic menu.");
            bullet(ui, "Escape — Return to menu from a game.");
        });
}

fn render_settings_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
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

    let snd = cx.menu_sounds.as_deref();

    section(ui, "Controls");

    if item(ui, "Keyboard Shortcuts", W) {
        play_click(&mut cx.commands, snd);
        cx.competitive_menu.show_controls_popup = true;
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
        ui.label(
            egui::RichText::new("·")
                .size(9.8)
                .color(egui::Color32::from_rgb(100, 140, 200)),
        );
        ui.label(
            egui::RichText::new(text)
                .size(9.4)
                .color(egui::Color32::from_rgb(200, 200, 210)),
        );
    });
    ui.add_space(2.0);
}

fn render_solana_connect_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Back button + header
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
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
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    let wallet_connected = cx.wallet_bridge.enabled && cx.wallet_bridge.known_pubkey.is_some();

    // Connect Wallet is always the first item
    let connect_label = if wallet_connected {
        "Wallet Connected"
    } else {
        "Connect Wallet"
    };
    if ui
        .add_sized(
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
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
            )),
        )
        .clicked()
    {
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
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7454);
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
            {
                if let Some(lobby) = cx.solana_lobby.as_mut() {
                    lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                    lobby.allow_create = true;
                    // Fresh entry from the main menu should always show the
                    // create-game form, not a stale WaitingForOpponent/Success
                    // left over from an earlier create attempt this session.
                    lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
                }
                // Always open the lobby screen. If the on-chain profile isn't
                // ready yet, the profile check (profile_check.rs) opens the
                // Tauri profile step on its own as soon as it resolves — do NOT
                // fire a popup from here (that was the "click Wagered PVP ->
                // stray login popup" bug).
                cx.menu_state.set(crate::core::MenuState::SolanaLobby);
            }
            #[cfg(not(feature = "solana"))]
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        if item(ui, "Find Wagered Game", W) {
            play_click(&mut cx.commands, snd);
            #[cfg(feature = "solana")]
            {
                if let Some(lobby) = cx.solana_lobby.as_mut() {
                    lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Browse;
                    // Finding a game is join/browse only — hide the Create tab.
                    lobby.allow_create = false;
                    lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
                }
                cx.menu_state.set(crate::core::MenuState::SolanaLobby);
            }
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
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
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
            ui.label(
                egui::RichText::new("ELO")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 140, 170)),
            );
            ui.label(
                egui::RichText::new(&elo)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(200, 220, 255))
                    .strong(),
            );
        });
        ui.add_space(2.0);

        // Lichess ELO — a distinct, clearly-labeled second stat. Never
        // merged with the on-chain ELO above.
        if let Some(lichess_elo) = cx.player_identity.lichess_elo {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("LICHESS")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(lichess_elo.to_string())
                        .size(11.0)
                        .color(egui::Color32::from_rgb(200, 255, 220))
                        .strong(),
                );
                if cx.player_identity.lichess_verified {
                    ui.label(
                        egui::RichText::new("✓")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(120, 220, 140)),
                    );
                }
            });
            ui.add_space(2.0);
        }

        // Country
        if let Some(ref country) = cx.player_identity.country {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Country")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(country)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(200, 210, 200)),
                );
            });
            ui.add_space(2.0);
        }

        // Wallet pubkey (shortened)
        if let Some(ref pk) = cx.wallet_bridge.known_pubkey.clone() {
            let short = format!(
                "{}...{}",
                &pk[..6.min(pk.len())],
                &pk[pk.len().saturating_sub(4)..]
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Wallet")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(&short)
                        .size(9.5)
                        .color(egui::Color32::from_rgb(160, 180, 160))
                        .monospace(),
                );
            });
        }

        // SOL balance — always shown once connected
        {
            let data = cx.wallet_bridge.data.lock().unwrap();
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Balance")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                let sol_text = format!("{:.4} SOL", data.sol_balance);
                let sol_color = if data.sol_balance > 0.0 {
                    egui::Color32::from_rgb(140, 220, 140)
                } else {
                    egui::Color32::from_rgb(140, 140, 150)
                };
                ui.label(egui::RichText::new(&sol_text).size(11.0).color(sol_color));
                if let Some(usd) = data.usd_balance {
                    if usd > 0.0 {
                        ui.label(
                            egui::RichText::new(format!("(${:.2})", usd))
                                .size(9.0)
                                .color(egui::Color32::from_rgb(140, 160, 140)),
                        );
                    }
                }
            });
        }

        // Connect Lichess — links the on-chain profile to a Lichess account
        // via the backend's existing PKCE OAuth flow (backend/src/signing/routes/lichess_oauth.rs).
        // The backend's own callback page completes the exchange server-side
        // and this game client just needs to open the browser; the next
        // periodic /auth/me poll picks up the new lichess_blitz/verified
        // fields once linking finishes — no dedicated listener needed here.
        if cx.player_identity.lichess_elo.is_none() {
            ui.add_space(SP);
            if ui
                .add_sized(
                    [W, 34.0],
                    egui::Button::new(
                        egui::RichText::new("Connect Lichess")
                            .size(11.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                            .family(egui::FontFamily::Proportional),
                    )
                    .fill(egui::Color32::from_rgb(30, 90, 60))
                    .corner_radius(6.0),
                )
                .clicked()
            {
                play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                if let Some(pubkey) = cx.wallet_bridge.known_pubkey.clone() {
                    let base = crate::multiplayer::network::vps::vps_base();
                    std::thread::spawn(move || {
                        let client = reqwest::blocking::Client::new();
                        // Base58 pubkeys are alphanumeric only — no percent-encoding needed.
                        let url = format!("{base}/api/auth/lichess/init?wallet_pubkey={pubkey}");
                        match client
                            .get(url)
                            .send()
                            .and_then(|r| r.json::<serde_json::Value>())
                        {
                            Ok(body) => {
                                if let Some(auth_url) = body["auth_url"].as_str() {
                                    let _ = webbrowser::open(auth_url);
                                } else {
                                    tracing::warn!(
                                        "[Lichess] /init response missing auth_url: {body}"
                                    );
                                }
                            }
                            Err(e) => tracing::warn!("[Lichess] /init request failed: {e}"),
                        }
                    });
                }
            }
        }

        ui.add_space(SP + 6.0);

        // Disconnect
        if ui
            .add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Disconnect Wallet")
                        .size(10.8)
                        .color(egui::Color32::WHITE)
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgb(100, 40, 40))
                .corner_radius(6.0),
            )
            .clicked()
        {
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
            if ui
                .add_sized(
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
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 200, 80, 80),
                    )),
                )
                .clicked()
            {
                play_click(&mut cx.commands, cx.menu_sounds.as_deref());
                let _ = webbrowser::open("http://localhost:5174/create-profile");
            }
            ui.add_space(SP);
        }

        // Refresh balance
        if ui
            .add_sized(
                [W, 30.0],
                egui::Button::new(
                    egui::RichText::new("Refresh")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(180, 200, 220))
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12))
                .corner_radius(6.0),
            )
            .clicked()
        {
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

        if ui
            .add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Go to Solana Multiplayer →")
                        .size(10.5)
                        .color(egui::Color32::from_rgb(140, 180, 255))
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(40, 80, 160, 60))
                .corner_radius(6.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(80, 140, 255, 80),
                )),
            )
            .clicked()
        {
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
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("Back")
                            .size(13.0)
                            .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                )
                .clicked()
            {
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
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        tex.id(),
                        [dw, dh],
                    )));
                    ui.add_space(16.0);
                }
                if let Some(ref tex) = cx.solana_logos.texture2 {
                    let [w, h] = tex.size();
                    let dh = 72.0_f32;
                    let dw = (w as f32 / h as f32) * dh;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        tex.id(),
                        [dw, dh],
                    )));
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
    let sol_balance = cx
        .wallet_bridge
        .data
        .lock()
        .map(|d| d.sol_balance)
        .unwrap_or(0.0);

    #[cfg(feature = "solana")]
    let connected = cx
        .solana_state
        .as_ref()
        .and_then(|s| s.wallet_pubkey)
        .is_some();
    #[cfg(not(feature = "solana"))]
    let connected = false;

    if !connected && is_guest {
        return;
    }

    let (sol_usd_rate, sol_gbp_rate) = cx
        .wallet_bridge
        .data
        .lock()
        .map(|d| (d.sol_usd_rate, d.sol_gbp_rate))
        .unwrap_or((0.0, 0.0));

    // 0 = USD (default — the primary display currency throughout the app),
    // 1 = SOL, 2 = GBP — persisted in egui temp storage across frames.
    let currency_id = egui::Id::new("balance_currency");
    let currency_mode = ctx.data(|d| d.get_temp::<u8>(currency_id).unwrap_or(0));

    let (balance_text, balance_color) = match currency_mode {
        1 => (
            format!("{:.3} SOL", sol_balance),
            egui::Color32::from_rgb(20, 241, 149),
        ),
        2 => {
            if sol_gbp_rate > 0.0 {
                (
                    format!("£{:.2}", sol_balance * sol_gbp_rate),
                    egui::Color32::from_rgb(20, 241, 149),
                )
            } else {
                (
                    format!("{:.3} SOL", sol_balance),
                    egui::Color32::from_rgb(20, 241, 149),
                )
            }
        }
        _ => {
            if sol_usd_rate > 0.0 {
                (
                    format!("${:.2}", sol_balance * sol_usd_rate),
                    egui::Color32::from_rgb(20, 241, 149),
                )
            } else {
                (
                    format!("{:.3} SOL", sol_balance),
                    egui::Color32::from_rgb(20, 241, 149),
                )
            }
        }
    };

    egui::Area::new("wallet_hud".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
        .show(ctx, |ui| {
            egui::Frame {
                corner_radius: egui::CornerRadius::same(8),
                fill: egui::Color32::from_rgba_unmultiplied(20, 20, 25, 220),
                stroke: egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                ),
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
    item_expandable_tip(ui, label, "", width)
}

/// Like [`item_expandable`] but shows `tip` as a hover tooltip when non-empty.
fn item_expandable_tip(ui: &mut egui::Ui, label: &str, tip: &str, width: f32) -> bool {
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
                .family(egui::FontFamily::Proportional),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(width, 46.0)),
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
                    egui::pos2(r.left(), r.center().y - 13.0),
                    egui::vec2(3.0, 26.0),
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
        egui::FontId::proportional(28.0),
        chevron_col,
    );
    if !tip.is_empty() {
        resp.clone().on_hover_text(tip);
    }
    resp.clicked()
}

/// A transparent button with a left-side accent bar on hover.
fn item(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    item_tip(ui, label, "", width)
}

/// Like [`item`] but shows `tip` as a hover tooltip when non-empty.
fn item_tip(ui: &mut egui::Ui, label: &str, tip: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);

    // Reserve background shape slots BEFORE the button so highlights render behind text.
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let accent_idx = ui.painter().add(egui::Shape::Noop);

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(22.0)
                .color(btn_text)
                .family(egui::FontFamily::Proportional),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(width, 46.0)),
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
                    egui::pos2(r.left(), r.center().y - 13.0),
                    egui::vec2(3.0, 26.0),
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
