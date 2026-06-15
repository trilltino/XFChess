//! Cinematic Chessboard Showcase for the main menu.
//!
//! After a short initial delay the calm orbit fades to black, cuts to a dramatic
//! camera angle, plays one curated "beautiful moment" (a move from a famous game
//! or a hand-set position), holds, then fades straight into the next moment — a
//! continuous loop that never restores the ambient board. Toggling it off (C)
//! lets the current shot finish, then fades back to the default orbit. See
//! `docs/plans/cinematic-menu-showcase.md`.
//!
//! Builds on the existing menu scene: it reuses [`MenuBgPieceAnim`] for the slide
//! animation (driven by `animate_menu_pieces`), [`BoardAnimator`] for the ambient
//! game (paused during a cinematic), and the same `Camera3d` that
//! `orbit_camera_system` normally drives (it yields while a cinematic is active).
//!
//! NOTE: camera-angle offsets and phase timings are first-pass values meant to be
//! tuned in-engine.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::GameState;
use crate::rendering::pieces::{PieceColor, PieceMeshes, PieceType};
use super::board_animation::{BoardAnimator, MenuBgPieceAnim, MenuBgPieceHome};
use super::new_menu::{MenuCameraOrbit, MenuBg, BOARD_CENTER};

// ── Tunables ────────────────────────────────────────────────────────────────

const FADE_OUT_SECS: f32 = 0.8;
const FADE_IN_SECS: f32 = 0.8;
/// Fraction of the move spent fading in from black at the start of AnimateMove.
const MOVE_FADE_IN_SECS: f32 = 0.6;
const PIECE_Y: f32 = 0.05;

// ── Phase state machine ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CinematicPhase {
    Idle,
    FadeOut,
    Cut,
    AnimateMove,
    Hold,
    FadeIn,
}

#[derive(Resource)]
pub struct MenuCinematic {
    pub phase: CinematicPhase,
    pub timer: f32,
    pub fade_alpha: f32,
    pub moment_index: usize,
    pub enabled: bool,
    /// Normalised progress through the current shot (0→1 over move+hold), for push-in.
    pub shot_t: f32,
    /// Pending promotion swap applied when the move finishes: (pawn, promoted piece).
    pub pending_promo: Option<(Entity, Entity)>,
    /// Cached last cinematic camera world position, for a seamless orbit hand-off.
    pub last_cam_pos: Vec3,
    /// World focus point for the current moment (cached at Cut to avoid re-resolving).
    pub focus: Vec3,
}

impl Default for MenuCinematic {
    fn default() -> Self {
        Self {
            phase: CinematicPhase::Idle,
            timer: 12.0,
            fade_alpha: 0.0,
            moment_index: 0,
            enabled: true,
            shot_t: 0.0,
            pending_promo: None,
            last_cam_pos: Vec3::ZERO,
            focus: BOARD_CENTER,
        }
    }
}

impl MenuCinematic {
    /// True whenever the cinematic owns the camera (everything except Idle).
    pub fn active(&self) -> bool {
        self.phase != CinematicPhase::Idle
    }
}

/// Live entity map for the cinematic's own pieces: `board[rank][file]`.
#[derive(Resource, Default)]
pub struct CinematicBoard {
    pub board: [[Option<Entity>; 8]; 8],
}

/// Marker for cinematic-spawned pieces (so they can be despawned on return).
#[derive(Component)]
pub struct CinematicPiece;

// ── Camera angle library ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CameraAngle {
    KingsideLow,
    TopDownTilted,
    CornerCloseup,
    HeroShot,
}

impl CameraAngle {
    /// Camera offset relative to the focus point, and whether the shot is orthographic.
    /// World mapping: x = 7 - file, z = rank; White on low z, Black on high z.
    fn offset(self) -> (Vec3, bool) {
        match self {
            CameraAngle::KingsideLow => (Vec3::new(-6.0, 2.5, -6.0), false),
            CameraAngle::TopDownTilted => (Vec3::new(2.0, 14.0, -4.0), true),
            CameraAngle::CornerCloseup => (Vec3::new(5.0, 3.5, -5.0), false),
            CameraAngle::HeroShot => (Vec3::new(2.5, 1.5, -3.5), false),
        }
    }
}

// ── Curated moments ──────────────────────────────────────────────────────────

enum MomentSource {
    /// A hand-set position (FEN) plus a UCI move to animate.
    Fen { fen: &'static str, mv: &'static str },
    /// Replay `pgn` to half-move `ply` for the position, then animate ply `ply`.
    Pgn { pgn: &'static str, ply: usize },
}

struct CinematicMoment {
    angle: CameraAngle,
    source: MomentSource,
    move_secs: f32,
    hold_secs: f32,
}

// The original Immortal Game (Anderssen–Kieseritzky, London 1851).
const IMMORTAL_GAME_PGN: &str = "
1. e4 e5 2. f4 exf4 3. Bc4 Qh4+ 4. Kf1 b5 5. Bxb5 Nf6 6. Nf3 Qh6
7. d3 Nh5 8. Nh4 Qg5 9. Nf5 c6 10. g4 Nf6 11. Rg1 cxb5 12. h4 Qg6
13. h5 Qg5 14. Qf3 Ng8 15. Bxf4 Qf6 16. Nc3 Bc5 17. Nd5 Qxb2 18. Bd6 Bxg1
19. e5 Qxa1+ 20. Ke2 Na6 21. Nxg7+ Kd8 22. Qf6+ Nxf6 23. Be7# 1-0
";

fn moments() -> &'static [CinematicMoment] {
    use CameraAngle::*;
    &[
        // ── Immortal Game (PGN replay — guaranteed legal positions) ──
        CinematicMoment {
            angle: TopDownTilted, source: MomentSource::Pgn { pgn: IMMORTAL_GAME_PGN, ply: 35 },
            move_secs: 3.0, hold_secs: 1.5 },
        CinematicMoment {
            angle: HeroShot, source: MomentSource::Pgn { pgn: IMMORTAL_GAME_PGN, ply: 44 },
            move_secs: 2.5, hold_secs: 3.0 },
        // ── The Immortal Zugzwang Game — the same game the ambient board plays ──
        CinematicMoment {
            angle: CornerCloseup, source: MomentSource::Pgn { pgn: super::board_animation::ZUGZWANG_PGN, ply: 49 },
            move_secs: 2.5, hold_secs: 2.5 },
        // ── Hand-set positions (FEN) ──
        CinematicMoment {
            angle: HeroShot,
            source: MomentSource::Fen { fen: "r1bqk2r/ppp2ppp/2n5/3np3/2B5/5N2/PPPP1PPP/RNBQ1RK1 w kq - 0 1", mv: "f3g5" },
            move_secs: 2.5, hold_secs: 1.5 },
        CinematicMoment {
            angle: KingsideLow,
            source: MomentSource::Fen { fen: "rnbqk2r/pppp1ppp/5n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 1", mv: "e1g1" },
            move_secs: 2.0, hold_secs: 1.5 },
        CinematicMoment {
            angle: HeroShot,
            source: MomentSource::Fen { fen: "4k3/4P3/8/8/8/8/8/4K3 w - - 0 1", mv: "e7e8q" },
            move_secs: 2.5, hold_secs: 2.0 },
    ]
}

// ── Resolving a moment into a concrete position + move ────────────────────────

/// (board[i8;64] before the move, src square, dst square, promo piece code or 0).
struct ResolvedMove {
    board: [i8; 64],
    src: usize,
    dst: usize,
    promo: i8,
}

fn resolve(moment: &CinematicMoment) -> Option<ResolvedMove> {
    match &moment.source {
        MomentSource::Fen { fen, mv } => {
            let cb = nimzovich_engine::CompactBoard::from_fen(fen);
            let (src, dst, promo) = parse_uci(mv)?;
            Some(ResolvedMove { board: cb.squares, src, dst, promo })
        }
        MomentSource::Pgn { pgn, ply } => {
            use nimzovich_engine::{do_move, new_game, parse_pgn, san_to_move};
            let parsed = parse_pgn(pgn).ok()?;
            if *ply >= parsed.moves.len() {
                return None;
            }
            let mut game = new_game();
            for san in &parsed.moves[0..*ply] {
                let (s, d, _) = san_to_move(&mut game, san).ok()?;
                do_move(&mut game, s, d, true);
            }
            // Snapshot the position BEFORE the featured ply.
            let mut board = [0i8; 64];
            for (i, slot) in board.iter_mut().enumerate() {
                *slot = game.board[i];
            }
            let (s, d, promo) = san_to_move(&mut game, &parsed.moves[*ply]).ok()?;
            Some(ResolvedMove { board, src: s as usize, dst: d as usize, promo })
        }
    }
}

/// Parse a UCI move ("e2e4" / "e7e8q") into (src, dst, promo-code). Promo code uses
/// engine piece ids: 5=Q 4=R 3=B 2=N, 0 = none.
fn parse_uci(mv: &str) -> Option<(usize, usize, i8)> {
    let b = mv.as_bytes();
    if b.len() < 4 {
        return None;
    }
    let sf = (b[0] as i32 - 'a' as i32) as usize;
    let sr = (b[1] as i32 - '1' as i32) as usize;
    let df = (b[2] as i32 - 'a' as i32) as usize;
    let dr = (b[3] as i32 - '1' as i32) as usize;
    if sf > 7 || sr > 7 || df > 7 || dr > 7 {
        return None;
    }
    let promo = match b.get(4) {
        Some(b'q') | Some(b'Q') => 5,
        Some(b'r') | Some(b'R') => 4,
        Some(b'b') | Some(b'B') => 3,
        Some(b'n') | Some(b'N') => 2,
        _ => 0,
    };
    Some((sr * 8 + sf, dr * 8 + df, promo))
}

// ── Piece spawning helpers ───────────────────────────────────────────────────

#[inline]
fn square_to_world(file: usize, rank: usize) -> Vec3 {
    Vec3::new(7.0 - file as f32, PIECE_Y, rank as f32)
}

fn type_from_code(code: i8) -> Option<(PieceType, PieceColor)> {
    let color = if code > 0 { PieceColor::White } else { PieceColor::Black };
    let pt = match code.abs() {
        1 => PieceType::Pawn,
        2 => PieceType::Knight,
        3 => PieceType::Bishop,
        4 => PieceType::Rook,
        5 => PieceType::Queen,
        6 => PieceType::King,
        _ => return None,
    };
    Some((pt, color))
}

fn rotation_for(pt: PieceType, color: PieceColor) -> Quat {
    let base = if color == PieceColor::White {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_y(std::f32::consts::PI)
    };
    if pt == PieceType::Knight {
        if color == PieceColor::White {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        } else {
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        }
    } else {
        base
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_piece(
    commands: &mut Commands,
    pm: &PieceMeshes,
    white_mat: &Handle<StandardMaterial>,
    black_mat: &Handle<StandardMaterial>,
    code: i8,
    file: usize,
    rank: usize,
    visible: bool,
) -> Option<Entity> {
    let (pt, color) = type_from_code(code)?;
    let mat = if color == PieceColor::White { white_mat } else { black_mat };
    let vis = if visible { Visibility::Visible } else { Visibility::Hidden };
    let e = commands
        .spawn((
            Mesh3d(pm.get(pt, color)),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(square_to_world(file, rank)).with_rotation(rotation_for(pt, color)),
            vis,
            MenuBg,
            DespawnOnExit(GameState::MainMenu),
            CinematicPiece,
        ))
        .id();
    Some(e)
}

// ── Director: advances the timeline and sets up / tears down each cinematic ────

#[allow(clippy::too_many_arguments)]
pub fn cinematic_director(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cine: ResMut<MenuCinematic>,
    mut anim: ResMut<BoardAnimator>,
    mut orbit: ResMut<MenuCameraOrbit>,
    mut cine_board: ResMut<CinematicBoard>,
    mut commands: Commands,
    piece_meshes: Option<Res<PieceMeshes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient_q: Query<(Entity, &MenuBgPieceHome, &mut Transform, &mut Visibility), Without<CinematicPiece>>,
    cine_pieces_q: Query<Entity, With<CinematicPiece>>,
) {
    // Press C to toggle the cinematic showcase. Disabling lets the current shot
    // finish naturally, then no new ones start (pure orbit).
    if keyboard.just_pressed(KeyCode::KeyC) {
        cine.enabled = !cine.enabled;
    }
    let dt = time.delta_secs();
    cine.timer -= dt;

    match cine.phase {
        CinematicPhase::Idle => {
            if cine.enabled && cine.timer <= 0.0 {
                cine.phase = CinematicPhase::FadeOut;
                cine.timer = FADE_OUT_SECS;
            }
        }
        CinematicPhase::FadeOut => {
            cine.fade_alpha = (1.0 - cine.timer / FADE_OUT_SECS).clamp(0.0, 1.0);
            if cine.timer <= 0.0 {
                cine.fade_alpha = 1.0;
                cine.phase = CinematicPhase::Cut;
                cine.timer = 0.0;
            }
        }
        CinematicPhase::Cut => {
            // Everything here happens during the fully-black frame.
            let Some(pm) = piece_meshes.as_deref() else { return; };
            let all = moments();
            let moment = &all[cine.moment_index % all.len()];

            // 1) Pause the ambient game and hide its pieces.
            anim.active = false;
            for (_e, _home, _t, mut vis) in ambient_q.iter_mut() {
                *vis = Visibility::Hidden;
            }

            // 2) Despawn any leftover cinematic pieces and clear the map.
            for e in cine_pieces_q.iter() {
                commands.entity(e).despawn();
            }
            cine_board.board = [[None; 8]; 8];

            if let Some(rm) = resolve(moment) {
                let white_mat = materials.add(crate::rendering::pieces::white_piece_material());
                let black_mat = materials.add(crate::rendering::pieces::black_piece_material());

                // 3) Spawn the position.
                for sq in 0..64usize {
                    let code = rm.board[sq];
                    if code == 0 {
                        continue;
                    }
                    let (file, rank) = (sq % 8, sq / 8);
                    if let Some(e) = spawn_piece(&mut commands, pm, &white_mat, &black_mat, code, file, rank, true) {
                        cine_board.board[rank][file] = Some(e);
                    }
                }

                // Cache the focus point (the move's destination square) for the camera.
                cine.focus = Vec3::new(7.0 - (rm.dst % 8) as f32, 0.5, (rm.dst / 8) as f32);

                // 4) Queue the featured move (animated over move_secs).
                queue_move(&mut commands, &mut cine, &mut cine_board, pm, &white_mat, &black_mat, &rm, moment.move_secs);

                cine.timer = moment.move_secs;
            } else {
                // Resolve failed (bad data) — skip straight to fade-in.
                cine.timer = 0.0;
            }
            cine.shot_t = 0.0;
            cine.phase = CinematicPhase::AnimateMove;
        }
        CinematicPhase::AnimateMove => {
            let total = moments()[cine.moment_index % moments().len()].move_secs.max(0.01);
            let elapsed = total - cine.timer.max(0.0);
            // Fade in from black over the first part of the move.
            cine.fade_alpha = (1.0 - elapsed / MOVE_FADE_IN_SECS).clamp(0.0, 1.0);
            cine.shot_t = (elapsed / total).clamp(0.0, 1.0) * 0.5;
            if cine.timer <= 0.0 {
                // Apply a pending promotion (swap pawn → promoted piece).
                if let Some((pawn, promoted)) = cine.pending_promo.take() {
                    commands.entity(pawn).insert(Visibility::Hidden);
                    commands.entity(promoted).insert(Visibility::Visible);
                }
                let hold = moments()[cine.moment_index % moments().len()].hold_secs;
                cine.phase = CinematicPhase::Hold;
                cine.timer = hold;
            }
        }
        CinematicPhase::Hold => {
            cine.fade_alpha = 0.0;
            cine.shot_t = (cine.shot_t + dt * 0.05).min(1.0);
            if cine.timer <= 0.0 {
                if cine.enabled {
                    // Continuous loop: advance to the next moment and fade straight
                    // into it. The ambient board is NOT restored between shots — the
                    // showcase runs position-to-position until toggled off.
                    cine.moment_index = cine.moment_index.wrapping_add(1);
                    cine.phase = CinematicPhase::FadeOut;
                    cine.timer = FADE_OUT_SECS;
                } else {
                    // Disabled (C pressed): fade back to the ambient board / orbit.
                    cine.phase = CinematicPhase::FadeIn;
                    cine.timer = FADE_IN_SECS;
                }
            }
        }
        CinematicPhase::FadeIn => {
            cine.fade_alpha = (cine.timer / FADE_IN_SECS).clamp(0.0, 1.0);
            if cine.timer <= 0.0 {
                // Black frame: tear down cinematic, restore the ambient game.
                for e in cine_pieces_q.iter() {
                    commands.entity(e).despawn();
                }
                cine_board.board = [[None; 8]; 8];
                cine.pending_promo = None;

                anim.board = [[None; 8]; 8];
                for (entity, home, mut transform, mut vis) in ambient_q.iter_mut() {
                    commands.entity(entity).remove::<MenuBgPieceAnim>();
                    transform.translation = square_to_world(home.file as usize, home.rank as usize);
                    *vis = Visibility::Visible;
                    anim.board[home.rank as usize][home.file as usize] = Some(entity);
                }
                anim.ply_index = 0;
                anim.move_timer = 3.0;
                anim.end_pause = 0.0;
                anim.active = true;

                // Resume the orbit from the cinematic camera's bearing (no spin).
                orbit.angle = (cine.last_cam_pos.z - BOARD_CENTER.z)
                    .atan2(cine.last_cam_pos.x - BOARD_CENTER.x);

                cine.fade_alpha = 0.0;
                cine.phase = CinematicPhase::Idle;
                cine.moment_index = cine.moment_index.wrapping_add(1);
                // 10 / 12.5 / 15s, cycling — no rng dependency.
                cine.timer = 10.0 + (cine.moment_index % 3) as f32 * 2.5;
            }
        }
    }
}

/// Inserts the slide animation(s) for the featured move: the mover, plus rook
/// (castling), captured piece removal (incl. en passant), and a queued promotion.
#[allow(clippy::too_many_arguments)]
fn queue_move(
    commands: &mut Commands,
    cine: &mut MenuCinematic,
    cine_board: &mut CinematicBoard,
    pm: &PieceMeshes,
    white_mat: &Handle<StandardMaterial>,
    black_mat: &Handle<StandardMaterial>,
    rm: &ResolvedMove,
    move_secs: f32,
) {
    let (sf, sr) = (rm.src % 8, rm.src / 8);
    let (df, dr) = (rm.dst % 8, rm.dst / 8);
    let mover = rm.board[rm.src];
    let is_pawn = mover.abs() == 1;
    let is_king = mover.abs() == 6;

    // Normal capture on the destination square.
    if let Some(cap) = cine_board.board[dr][df].take() {
        commands.entity(cap).despawn();
    }
    // En passant: pawn moves diagonally onto an empty square.
    if is_pawn && sf != df && rm.board[rm.dst] == 0 {
        let ep_rank = if mover > 0 { dr.wrapping_sub(1) } else { dr + 1 };
        if ep_rank < 8 {
            if let Some(cap) = cine_board.board[ep_rank][df].take() {
                commands.entity(cap).despawn();
            }
        }
    }

    // Slide the moving piece.
    if let Some(e) = cine_board.board[sr][sf].take() {
        cine_board.board[dr][df] = Some(e);
        commands.entity(e).insert(MenuBgPieceAnim {
            start: square_to_world(sf, sr),
            end: square_to_world(df, dr),
            elapsed: 0.0,
            duration: move_secs,
        });

        // Promotion: spawn the promoted piece (hidden) on the destination; the
        // director swaps pawn → promoted when the slide finishes.
        if rm.promo != 0 {
            let promo_code = if mover > 0 { rm.promo } else { -rm.promo };
            if let Some(pe) = spawn_piece(commands, pm, white_mat, black_mat, promo_code, df, dr, false) {
                cine.pending_promo = Some((e, pe));
                cine_board.board[dr][df] = Some(pe);
            }
        }
    }

    // Castling: king moved two files → slide the rook too (same rank).
    if is_king && (df as i32 - sf as i32).abs() == 2 {
        let (rsf, rdf) = if df == 6 { (7usize, 5usize) } else { (0usize, 3usize) };
        if let Some(re) = cine_board.board[sr][rsf].take() {
            cine_board.board[sr][rdf] = Some(re);
            commands.entity(re).insert(MenuBgPieceAnim {
                start: square_to_world(rsf, sr),
                end: square_to_world(rdf, sr),
                elapsed: 0.0,
                duration: move_secs,
            });
        }
    }
}

// ── Camera: drives the Camera3d while a cinematic is active ───────────────────

pub fn cinematic_camera_system(
    mut cine: ResMut<MenuCinematic>,
    cam: Res<crate::PersistentEguiCamera>,
    mut query: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    if !cine.active() {
        return;
    }
    let Some(entity) = cam.entity else { return };
    let Ok((mut t, mut proj)) = query.get_mut(entity) else { return };

    let all = moments();
    let moment = &all[cine.moment_index % all.len()];
    let (offset, ortho) = moment.angle.offset();

    // Focus the featured destination square (cached at Cut). Gentle push-in.
    let focus = cine.focus;
    let pos = focus + offset * (1.0 - 0.10 * cine.shot_t);

    if ortho {
        if !matches!(*proj, Projection::Orthographic(_)) {
            *proj = Projection::from(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical { viewport_height: 12.0 },
                ..OrthographicProjection::default_3d()
            });
        }
    } else if !matches!(*proj, Projection::Perspective(_)) {
        *proj = Projection::default();
    }

    *t = Transform::from_translation(pos).looking_at(focus, Vec3::Y);
    cine.last_cam_pos = pos;
}

// ── Fade overlay (egui) ──────────────────────────────────────────────────────

pub fn cinematic_fade_overlay(mut contexts: EguiContexts, cine: Res<MenuCinematic>) {
    if cine.fade_alpha <= 0.001 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return; };
    let a = (cine.fade_alpha.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Area::new("cinematic_fade".into())
        .order(egui::Order::Foreground)
        .interactable(false) // never eat menu clicks
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let r = ctx.screen_rect();
            ui.painter().rect_filled(r, egui::CornerRadius::same(0), egui::Color32::from_black_alpha(a));
        });
}

/// Reset on leaving the menu so re-entry starts clean.
pub fn reset_cinematic_on_exit(
    mut cine: ResMut<MenuCinematic>,
    mut cine_board: ResMut<CinematicBoard>,
) {
    *cine = MenuCinematic::default();
    cine_board.board = [[None; 8]; 8];
}
