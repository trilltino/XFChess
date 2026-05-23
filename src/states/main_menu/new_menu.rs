//! New-style main menu: full-screen 3D board background + bottom-left button list.
//!
//! The existing board with all 32 pieces in starting position is rendered in the
//! background using the primary camera. A semi-transparent panel in the bottom-left
//! lists the main navigation options.
//!
//! Press **K** to toggle back to the website-style (classic) menu.

use bevy::light::{FogVolume, VolumetricFog, VolumetricLight};
use bevy::prelude::*;
use bevy_egui::egui;

use crate::core::{DespawnOnExit, GameMode as CoreGameMode, GameState, MenuState};
use crate::rendering::pieces::{PieceColor, PieceMeshes, PieceType};
use crate::ui::system_params::MainMenuUIContext;

/// Marker for all menu-background scene entities (board squares, pieces, lights).
#[derive(Component)]
pub struct MenuBg;

/// Tracks whether background pieces have been spawned for the current MainMenu session.
#[derive(Resource, Default)]
pub struct MenuBgPiecesSpawned(pub bool);

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

    // Lichess palette: #f0d9b5 light / #b58863 dark
    let light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.941, 0.851, 0.710),
        perceptual_roughness: 0.8,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.710, 0.533, 0.388),
        perceptual_roughness: 0.8,
        ..default()
    });

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let mat = if (file + rank) % 2 == 0 { light.clone() } else { dark.clone() };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_xyz(file as f32, 0.0, rank as f32),
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

    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.91, 0.85),
        perceptual_roughness: 0.45,
        ..default()
    });
    let black_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.12, 0.08),
        perceptual_roughness: 0.45,
        ..default()
    });

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
            Transform::from_xyz(f as f32, 0.05, 0.0).with_rotation(wr),
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 0 },
        )).id();
        anim.board[0][f] = Some(ew);

        // Black back rank 7
        let eb = commands.spawn((
            Mesh3d(pm.get(pt, PieceColor::Black)),
            MeshMaterial3d(black_mat.clone()),
            Transform::from_xyz(f as f32, 0.05, 7.0).with_rotation(br),
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 7 },
        )).id();
        anim.board[7][f] = Some(eb);
    }

    for f in 0..8usize {
        let file = f as u8;
        // White pawns rank 1
        let ewp = commands.spawn((
            Mesh3d(pm.get(PieceType::Pawn, PieceColor::White)),
            MeshMaterial3d(white_mat.clone()),
            Transform::from_xyz(f as f32, 0.05, 1.0).with_rotation(rot_w),
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 1 },
        )).id();
        anim.board[1][f] = Some(ewp);

        // Black pawns rank 6
        let ebp = commands.spawn((
            Mesh3d(pm.get(PieceType::Pawn, PieceColor::Black)),
            MeshMaterial3d(black_mat.clone()),
            Transform::from_xyz(f as f32, 0.05, 6.0).with_rotation(rot_b),
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            super::board_animation::MenuBgPiecePos { file, rank: 6 },
        )).id();
        anim.board[6][f] = Some(ebp);
    }

    spawned.0 = true;
    anim.active = true;
}

/// Spawn directional key light and point fill light for the background board.
pub fn spawn_menu_bg_lights(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 3_500.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.4, 0.0)),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-KeyLight"),
    ));

    commands.spawn((
        PointLight {
            intensity: 700.0,
            range: 22.0,
            shadows_enabled: false,
            color: Color::srgb(0.65, 0.80, 1.0),
            ..default()
        },
        Transform::from_xyz(3.5, 5.5, 3.5),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-FillLight"),
    ));

    // Red directional light for volumetric god-rays over the board.
    // shadows_enabled is required by VolumetricLight.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.9, 0.08, 0.08),
            illuminance: 2_000.0,
            shadows_enabled: true,
            ..default()
        },
        VolumetricLight,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.2, 0.4, 0.0)),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-FogLight"),
    ));

    // Fog volume sized to encompass the board and a few units above it.
    commands.spawn((
        FogVolume::default(),
        Transform::from_xyz(3.5, 4.0, 3.5).with_scale(Vec3::new(10.0, 8.0, 10.0)),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-FogVolume"),
    ));
}

// ── Camera & style systems ───────────────────────────────────────────────────

/// Attaches `VolumetricFog` to the persistent camera for the cinematic red fog effect.
/// Runs once on `OnEnter(MainMenu)`.
pub fn setup_menu_fog(
    mut commands: Commands,
    cam: Res<crate::PersistentEguiCamera>,
) {
    if let Some(entity) = cam.entity {
        commands.entity(entity).insert(VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        });
    }
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
    if let Some(entity) = cam.entity {
        if let Ok(mut t) = query.get_mut(entity) {
            *t = Transform::from_translation(Vec3::new(x, orbit.height, z))
                .looking_at(BOARD_CENTER, Vec3::Y);
        }
    }
}

// ── egui panel ───────────────────────────────────────────────────────────────

/// Render the bottom-left button list.
/// Modals (AI setup, controls popup) are rendered by the caller in `main_menu.rs`.
pub fn render_new_style_panel(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    // Corner logos only when Solana Multiplayer panel is open
    if *cx.new_menu_panel == NewMenuPanel::SolanaConnect {
        render_corner_logos(ctx, cx);
    }

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

    egui::Window::new("##xfc_new_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .min_size(egui::vec2(280.0, 320.0))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(36.0, -36.0))
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
                NewMenuPanel::SolanaMultiplayer => {}
            }

            ui.set_opacity(1.0);
        });
}

/// Small logos pinned to bottom-right — always shown while the 3D menu is active.
fn render_corner_logos(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    super::ensure_solana_logos(ctx, &mut cx.solana_logos);

    egui::Area::new("corner_logos".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
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
    const W: f32 = 244.0;
    const SP: f32 = 4.0;

    if item(ui, "Play Against a Computer", W) {
        cx.competitive_menu.show_ai_setup = true;
    }
    ui.add_space(SP);

    if item_expandable(ui, "Play Online", W) {
        *cx.new_menu_panel = NewMenuPanel::PlayOnline;
    }
    ui.add_space(SP);

    if item_expandable(ui, "How to Play", W) {
        *cx.new_menu_panel = NewMenuPanel::HowToPlay;
    }
    ui.add_space(SP);

    if item_expandable(ui, "Settings", W) {
        *cx.new_menu_panel = NewMenuPanel::Settings;
    }
    ui.add_space(SP);

    if item(ui, "XFChess.com", W) {
        if let Err(e) = webbrowser::open("https://xfchess.com") {
            tracing::warn!("[Menu] Failed to open XFChess.com: {}", e);
        }
    }
    ui.add_space(SP);

    if item(ui, "Exit", W) {
        std::process::exit(0);
    }
}

fn render_play_online_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Back button + section header
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("←")
                    .size(10.1)
                    .color(egui::Color32::from_rgb(140, 160, 200)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            *cx.new_menu_panel = NewMenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Play Online")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(14.0);

    const W: f32 = 244.0;
    const SP: f32 = 4.0;

    if item(ui, "Create Lobby", W) {
        cx.menu_state.set(MenuState::HostConfig);
    }
    ui.add_space(SP);

    if item(ui, "Join Lobby", W) {
        // Trigger immediate poll so the lobby list is fresh on arrival
        if let Some(ref mut vps) = cx.p2p_vps_state {
            vps.last_poll = None;
        }
        cx.menu_state.set(MenuState::BraidLobby);
    }
    ui.add_space(SP);

    if item(ui, "Spectator", W) {
        cx.competitive_menu.show_spectator_popup = true;
    }
    ui.add_space(SP + 4.0);

    if item_expandable(ui, "Tournaments", W) {
        *cx.new_menu_panel = NewMenuPanel::Tournaments;
    }
    ui.add_space(SP);

    if item_expandable(ui, "Solana Multiplayer", W) {
        *cx.new_menu_panel = NewMenuPanel::SolanaConnect;
    }
}

fn render_tournaments_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(
                egui::RichText::new("←")
                    .size(10.1)
                    .color(egui::Color32::from_rgb(140, 160, 200)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Tournaments")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(14.0);

    const W: f32 = 244.0;
    const SP: f32 = 4.0;

    if item(ui, "Join Tournament", W) {
        cx.menu_state.set(MenuState::Tournaments);
    }
    ui.add_space(SP);

    if item(ui, "Spectate Tournament", W) {
        cx.competitive_menu.show_spectator_popup = true;
    }
}

fn render_how_to_play_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(egui::RichText::new("←").size(10.1).color(egui::Color32::from_rgb(140, 160, 200)))
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
        ).clicked() {
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
                egui::RichText::new("←")
                    .size(10.1)
                    .color(egui::Color32::from_rgb(140, 160, 200)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
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

    const W: f32 = 244.0;
    const SP: f32 = 4.0;

    section(ui, "Controls");

    if item(ui, "Keyboard Shortcuts", W) {
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
                egui::RichText::new("←")
                    .size(10.1)
                    .color(egui::Color32::from_rgb(140, 160, 200)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        ).clicked() {
            *cx.new_menu_panel = NewMenuPanel::PlayOnline;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Solana")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(14.0);

    const W: f32 = 244.0;
    const SP: f32 = 4.0;

    let wallet_connected = cx.player_identity.username.is_some();

    // Connect Wallet is always the first item
    let connect_label = if wallet_connected { "Wallet Connected ✓" } else { "Connect Wallet" };
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
        // Always open the wallet popup — allows reconnect or wallet switch
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

    // Rest of options only shown once connected
    if wallet_connected {
        ui.add_space(SP + 6.0);

        if item(ui, "Non-Wagered", W) {
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        if item(ui, "Wagered PVP", W) {
            #[cfg(feature = "solana")]
            cx.menu_state.set(crate::core::MenuState::SolanaLobby);
        }
        ui.add_space(SP);

        if item(ui, "Wager Search", W) {
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        if item(ui, "Create A Game", W) {
            cx.menu_state.set(crate::core::MenuState::HostConfig);
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

/// Full-screen Solana splash: pure black background, two logos bottom-right.
pub fn render_solana_splash(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    // Ensure textures are loaded
    super::ensure_solana_logos(ctx, &mut cx.solana_logos);

    // Black full-screen background
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            // ← back button top-left
            if ui.add(
                egui::Button::new(
                    egui::RichText::new("←")
                        .size(16.5)
                        .color(egui::Color32::from_rgb(140, 160, 200)),
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
/// Shown when a Solana wallet is connected.
pub fn render_wallet_hud(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    let display_name = cx.player_identity.display_name().to_string();
    let is_guest = cx.player_identity.username.is_none();

    #[cfg(feature = "solana")]
    let (sol_balance, usd_balance) = if let Some(ref solana_state) = cx.solana_state {
        let sol = solana_state.balance;
        let usd = solana_state.cached_usd_balance;
        (sol, usd)
    } else {
        (0.0, None)
    };

    #[cfg(not(feature = "solana"))]
    let (sol_balance, usd_balance) = (0.0, None::<f64>);

    // Only show if wallet is connected (has a real username or solana state with pubkey)
    #[cfg(feature = "solana")]
    let connected = cx.solana_state.as_ref().and_then(|s| s.wallet_pubkey).is_some();
    #[cfg(not(feature = "solana"))]
    let connected = false;

    if !connected && is_guest {
        return;
    }

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
                        // Username
                        ui.label(
                            egui::RichText::new(&display_name)
                                .size(10.5)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Balance
                        if let Some(usd) = usd_balance {
                            ui.label(
                                egui::RichText::new(format!("${:.2}", usd))
                                    .size(10.1)
                                    .color(egui::Color32::from_rgb(20, 241, 149)),
                            );
                            ui.label(
                                egui::RichText::new(format!("({:.3} SOL)", sol_balance))
                                    .size(8.3)
                                    .color(egui::Color32::from_rgb(150, 150, 170)),
                            );
                        } else if sol_balance > 0.0 {
                            ui.label(
                                egui::RichText::new(format!("{:.3} SOL", sol_balance))
                                    .size(10.1)
                                    .color(egui::Color32::from_rgb(20, 241, 149)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("0 SOL")
                                    .size(10.1)
                                    .color(egui::Color32::from_rgb(150, 150, 170)),
                            );
                        }
                    });
                });
        });
}

/// Same as [`item`] but draws a `›` chevron on the right to signal expansion.
fn item_expandable(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);
    let chevron_col = egui::Color32::from_rgb(120, 140, 180);
    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(16.5)
                .color(btn_text)
                .family(egui::FontFamily::Proportional)
        )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(width, 36.0)),
    );
    if resp.hovered() {
        let r = resp.rect;
        ui.painter().rect_filled(
            r.expand(1.0),
            egui::CornerRadius::same(4),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 11),
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.left(), r.center().y - 10.0),
                egui::vec2(3.0, 20.0),
            ),
            egui::CornerRadius::same(2),
            egui::Color32::from_rgb(90, 160, 255),
        );
    }
    // Draw chevron at the right edge regardless of hover
    let r = resp.rect;
    ui.painter().text(
        egui::pos2(r.right() - 10.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(24.0),
        chevron_col,
    );
    resp.clicked()
}

/// A transparent button with a left-side accent bar on hover.
fn item(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    let btn_text = egui::Color32::from_rgb(218, 218, 232);
    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(16.5)
                .color(btn_text)
                .family(egui::FontFamily::Proportional)
        )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(width, 36.0)),
    );
    if resp.hovered() {
        let r = resp.rect;
        ui.painter().rect_filled(
            r.expand(1.0),
            egui::CornerRadius::same(4),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 11),
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.left(), r.center().y - 10.0),
                egui::vec2(3.0, 20.0),
            ),
            egui::CornerRadius::same(2),
            egui::Color32::from_rgb(90, 160, 255),
        );
    }
    resp.clicked()
}
