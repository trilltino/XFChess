//! Menu-background board pieces for the main menu.
//!
//! Holds the ambient board state ([`BoardAnimator`]) and the smooth-slide
//! animation ([`animate_menu_pieces`]). The cinematic system
//! (`cinematic.rs`) drives which moves play; this module only owns the piece
//! components and the slide tween.

use bevy::prelude::*;
use std::sync::OnceLock;

/// Stores the original starting square for a menu background piece.
/// Used to reset positions when the animation loops back without despawning.
#[derive(Component, Clone, Copy)]
pub struct MenuBgPieceHome {
    pub file: u8,
    pub rank: u8,
}

/// Smooth-slide animation state for a moving menu background piece.
#[derive(Component)]
pub struct MenuBgPieceAnim {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

/// Slow opacity fade for a captured menu piece. Instead of vanishing instantly,
/// a captured piece fades its (per-piece) material alpha 1→0 over `duration`,
/// then hides. Requires each menu piece to own its own material handle (see
/// `spawn_menu_bg_pieces`) so fading one never affects the others.
#[derive(Component)]
pub struct MenuPieceFade {
    pub elapsed: f32,
    pub duration: f32,
}

/// Advances capture fades: lerps each fading piece's material alpha to 0, then
/// hides it. Runs every frame while on the main menu.
pub fn animate_menu_piece_fades(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(
        Entity,
        &mut MenuPieceFade,
        &MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
) {
    for (e, mut fade, mat_handle, mut vis) in q.iter_mut() {
        fade.elapsed += time.delta_secs();
        let t = (fade.elapsed / fade.duration).clamp(0.0, 1.0);
        let alpha = 1.0 - t;
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            // Blend while fading so the alpha actually shows.
            mat.alpha_mode = AlphaMode::Blend;
            let c = mat.base_color.with_alpha(alpha);
            mat.base_color = c;
        }
        if t >= 1.0 {
            *vis = Visibility::Hidden;
            commands.entity(e).remove::<MenuPieceFade>();
        }
    }
}

/// Restores a piece's material to fully opaque (used on board reset/loop so a
/// previously-captured piece comes back solid).
fn restore_piece_material(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>) {
    if let Some(mut mat) = materials.get_mut(handle) {
        let c = mat.base_color.with_alpha(1.0);
        mat.base_color = c;
        mat.alpha_mode = AlphaMode::Opaque;
    }
}

/// Advances smooth movement animations for all in-flight menu background pieces.
/// Uses cubic smooth-step easing with a gentle arc lift, matching the in-game feel.
pub fn animate_menu_pieces(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut MenuBgPieceAnim)>,
) {
    for (entity, mut transform, mut anim) in q.iter_mut() {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / anim.duration).clamp(0.0, 1.0);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        let arc = (std::f32::consts::PI * t).sin() * 0.28;
        transform.translation = anim.start.lerp(anim.end, smooth_t) + Vec3::new(0.0, arc, 0.0);
        if t >= 1.0 {
            transform.translation = anim.end;
            commands.entity(entity).remove::<MenuBgPieceAnim>();
        }
    }
}

/// Drives the Immortal Zugzwang Game animation on the menu background board.
/// The move sequence itself is applied by the cinematic system.
#[derive(Resource)]
pub struct BoardAnimator {
    /// Index of the next ply to apply.
    pub ply_index: usize,
    /// Countdown (seconds) until the next ply is applied.
    pub move_timer: f32,
    /// Countdown after the last move before the board resets.
    pub end_pause: f32,
    /// Sparse entity map: board\[rank\]\[file\] = piece entity.
    pub board: [[Option<Entity>; 8]; 8],
    /// False until `spawn_menu_bg_pieces` populates `board`.
    pub active: bool,
}

impl Default for BoardAnimator {
    fn default() -> Self {
        Self {
            ply_index: 0,
            move_timer: 2.5,
            end_pause: 0.0,
            board: [[None; 8]; 8],
            active: false,
        }
    }
}

// Sämisch vs Nimzowitsch, Copenhagen 1923 — the Immortal Zugzwang Game.
pub(super) const ZUGZWANG_PGN: &str = "
1. d4 Nf6 2. c4 e6 3. Nf3 b6 4. g3 Bb7 5. Bg2 Be7
6. Nc3 O-O 7. O-O d5 8. Ne5 c6
9. cxd5 cxd5 10. Bf4 a6
11. Rc1 b5 12. Qb3 Nc6
13. Nxc6 Bxc6 14. h3 Qd7 15. Kh2 Nh5
16. Bd2 f5 17. Qd1 b4 18. Nb1 Bb5 19. Rg1 Bd6 20. e4 fxe4
21. Qxh5 Rxf2 22. Qg5 Raf8 23. Kh1 R8f5 24. Qe3 Bd3 25. Rce1 h6
0-1
";

// ── Ambient board auto-play (post-Enter) ──────────────────────────────────────
//
// Replays ZUGZWANG_PGN on the full-size `MenuBg` board once the player presses
// Enter. Captured pieces are *hidden* (not despawned) so the whole game can loop
// without re-spawning: at the trailing pause every piece is snapped back to its
// `MenuBgPieceHome` and revealed.

/// World position of a square on the `MenuBg` board (x = 7 − file, z = rank).
/// Matches `spawn_menu_bg_pieces` and the cinematic's `square_to_world`.
#[inline]
fn sq_world(file: usize, rank: usize) -> Vec3 {
    Vec3::new(7.0 - file as f32, 0.05, rank as f32)
}

/// One pre-resolved ply: where the piece moves, plus capture / castling / en
/// passant side-effects. Pure data (no `Entity`) so it can be cached globally.
#[derive(Clone, Copy)]
struct AmbientStep {
    from: (u8, u8),
    to: (u8, u8),
    /// A piece sits on the destination square and must be hidden.
    capture: bool,
    /// En-passant: square of the pawn to hide (file, rank).
    ep_capture: Option<(u8, u8)>,
    /// Castling: rook (from_file, to_file) on the king's rank.
    castle_rook: Option<(u8, u8)>,
}

static AMBIENT_PLAN: OnceLock<Vec<AmbientStep>> = OnceLock::new();

/// Parse ZUGZWANG_PGN once into a flat list of resolved plies, computed lazily.
fn ambient_plan() -> &'static [AmbientStep] {
    AMBIENT_PLAN.get_or_init(zugzwang_steps).as_slice()
}

fn zugzwang_steps() -> Vec<AmbientStep> {
    use nimzovich_engine::{do_move, new_game, parse_pgn, san_to_move};
    let Ok(parsed) = parse_pgn(ZUGZWANG_PGN) else {
        return Vec::new();
    };
    let mut game = new_game();
    let mut steps = Vec::with_capacity(parsed.moves.len());
    for san in &parsed.moves {
        let Ok((s, d, _promo)) = san_to_move(&mut game, san) else {
            break;
        };
        let (su, du) = (s as usize, d as usize);
        let (sf, sr) = (su % 8, su / 8);
        let (df, dr) = (du % 8, du / 8);
        let mover = game.board[su];
        let is_pawn = mover.abs() == 1;
        let is_king = mover.abs() == 6;
        let dest_occupied = game.board[du] != 0;
        // Pawn moving diagonally onto an empty square ⇒ en passant.
        let ep_capture = if is_pawn && sf != df && !dest_occupied {
            Some((df as u8, sr as u8))
        } else {
            None
        };
        // King moving two files ⇒ castling; slide the matching rook too.
        let castle_rook = if is_king && (df as i32 - sf as i32).abs() == 2 {
            if df == 6 {
                Some((7u8, 5u8))
            } else {
                Some((0u8, 3u8))
            }
        } else {
            None
        };
        steps.push(AmbientStep {
            from: (sf as u8, sr as u8),
            to: (df as u8, dr as u8),
            capture: dest_occupied,
            ep_capture,
            castle_rook,
        });
        do_move(&mut game, s, d, true);
    }
    steps
}

/// Drives the Immortal-Zugzwang replay on the ambient `MenuBg` board. Self-arms
/// via `anim.active`, set once `spawn_menu_bg_pieces` populates the board map.
pub fn animate_ambient_board(
    time: Res<Time>,
    mut commands: Commands,
    mut anim: ResMut<BoardAnimator>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut reset_q: Query<(
        Entity,
        &MenuBgPieceHome,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    if !anim.active {
        return;
    }
    let plan = ambient_plan();
    if plan.is_empty() {
        return;
    }

    // Trailing pause after the final ply, then snap everything home and loop.
    if anim.end_pause > 0.0 {
        anim.end_pause -= time.delta_secs();
        if anim.end_pause <= 0.0 {
            anim.board = [[None; 8]; 8];
            for (e, home, mut t, mut v, mat) in reset_q.iter_mut() {
                commands
                    .entity(e)
                    .remove::<MenuBgPieceAnim>()
                    .remove::<MenuPieceFade>();
                // Captured pieces faded to transparent — restore them solid.
                restore_piece_material(&mut materials, &mat.0);
                t.translation = sq_world(home.file as usize, home.rank as usize);
                *v = Visibility::Visible;
                anim.board[home.rank as usize][home.file as usize] = Some(e);
            }
            anim.ply_index = 0;
            anim.move_timer = 2.0;
        }
        return;
    }

    if anim.ply_index >= plan.len() {
        anim.end_pause = 4.0;
        return;
    }

    anim.move_timer -= time.delta_secs();
    if anim.move_timer > 0.0 {
        return;
    }
    anim.move_timer = 1.6;

    let step = plan[anim.ply_index];
    apply_ambient_step(&mut commands, &mut anim, step);
    anim.ply_index += 1;
}

/// Applies one ply to `anim.board`: hides captures and inserts slide tweens.
fn apply_ambient_step(commands: &mut Commands, anim: &mut BoardAnimator, step: AmbientStep) {
    let (sf, sr) = (step.from.0 as usize, step.from.1 as usize);
    let (df, dr) = (step.to.0 as usize, step.to.1 as usize);

    // Captured pieces fade out slowly rather than vanishing instantly.
    const FADE_SECS: f32 = 1.2;
    if step.capture {
        if let Some(cap) = anim.board[dr][df].take() {
            commands.entity(cap).insert(MenuPieceFade {
                elapsed: 0.0,
                duration: FADE_SECS,
            });
        }
    }
    if let Some((ef, er)) = step.ep_capture {
        if let Some(cap) = anim.board[er as usize][ef as usize].take() {
            commands.entity(cap).insert(MenuPieceFade {
                elapsed: 0.0,
                duration: FADE_SECS,
            });
        }
    }
    if let Some(e) = anim.board[sr][sf].take() {
        anim.board[dr][df] = Some(e);
        commands.entity(e).insert(MenuBgPieceAnim {
            start: sq_world(sf, sr),
            end: sq_world(df, dr),
            elapsed: 0.0,
            duration: 1.2,
        });
    }
    if let Some((rf, rt)) = step.castle_rook {
        let (rf, rt) = (rf as usize, rt as usize);
        if let Some(re) = anim.board[sr][rf].take() {
            anim.board[sr][rt] = Some(re);
            commands.entity(re).insert(MenuBgPieceAnim {
                start: sq_world(rf, sr),
                end: sq_world(rt, sr),
                elapsed: 0.0,
                duration: 1.2,
            });
        }
    }
}
