//! Immortal Zugzwang Game animation for the main menu background board.
//!
//! Uses `nimzovich_engine` to parse the PGN and convert SAN moves to square
//! coordinates, then drives the 3D background pieces every N seconds.

use bevy::prelude::*;
use nimzovich_engine::{do_move, new_game, parse_pgn, san_to_move, KING_ID, PAWN_ID};

use crate::core::{DespawnOnExit, GameState};

/// Marks a menu-background piece and records its current board square.
#[derive(Component, Clone, Copy)]
pub struct MenuBgPiecePos {
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

/// Precomputed movements for one half-move (ply).
#[derive(Clone, Debug)]
pub struct PlyData {
    /// List of (from_file, from_rank, to_file, to_rank) entity movements.
    /// Castling produces 2 entries (king + rook); normal moves produce 1.
    pub movements: Vec<(u8, u8, u8, u8)>,
    /// En-passant captured pawn square (only set for en-passant captures).
    pub ep_capture: Option<(u8, u8)>,
}

/// Drives the Immortal Zugzwang Game animation on the menu background board.
#[derive(Resource)]
pub struct BoardAnimator {
    /// All precomputed plies for the game (empty until `init_board_animator` runs).
    pub plies: Vec<PlyData>,
    /// Index of the next ply to apply.
    pub ply_index: usize,
    /// Countdown (seconds) until the next ply is applied.
    pub move_timer: f32,
    /// Seconds between plies.
    pub move_interval: f32,
    /// Countdown after the last move before the board resets.
    pub end_pause: f32,
    /// Sparse entity map: board\[rank\]\[file\] = piece entity.
    pub board: [[Option<Entity>; 8]; 8],
    /// False until `spawn_menu_bg_pieces` populates `board`.
    pub active: bool,
    /// True once plies have been precomputed.
    pub initialized: bool,
}

impl Default for BoardAnimator {
    fn default() -> Self {
        Self {
            plies: Vec::new(),
            ply_index: 0,
            move_timer: 2.5,
            move_interval: 2.0,
            end_pause: 0.0,
            board: [[None; 8]; 8],
            active: false,
            initialized: false,
        }
    }
}

// Sämisch vs Nimzowitsch, Copenhagen 1923 — the Immortal Zugzwang Game.
const ZUGZWANG_PGN: &str = "
1. d4 Nf6 2. c4 e6 3. Nf3 b6 4. g3 Bb7 5. Bg2 Be7
6. Nc3 O-O 7. O-O d5 8. Ne5 c6
9. cxd5 cxd5 10. Bf4 a6
11. Rc1 b5 12. Qb3 Nc6
13. Nxc6 Bxc6 14. h3 Qd7 15. Kh2 Nh5
16. Bd2 f5 17. Qd1 b4 18. Nb1 Bb5 19. Rg1 Bd6 20. e4 fxe4
21. Qxh5 Rxf2 22. Qg5 Raf8 23. Kh1 R8f5 24. Qe3 Bd3 25. Rce1 h6
0-1
";

/// One-time system: parse the PGN and precompute (src, dst) for every ply.
/// Runs in `Update` so it executes after all plugins are fully initialised.
pub fn init_board_animator(mut anim: ResMut<BoardAnimator>) {
    if anim.initialized {
        return;
    }
    anim.initialized = true;
    anim.plies = precompute_plies();
    if anim.plies.is_empty() {
        error!("[ANIM] PGN precomputation returned 0 plies — static board will show, no animation");
    } else {
        info!("[ANIM] Immortal Zugzwang Game loaded: {} plies", anim.plies.len());
    }
}

fn precompute_plies() -> Vec<PlyData> {
    let parsed = match parse_pgn(ZUGZWANG_PGN) {
        Ok(p) => p,
        Err(e) => {
            error!("[ANIM] PGN parse error: {:?}", e);
            return Vec::new();
        }
    };

    let mut game = new_game();
    let mut plies = Vec::new();

    for san in &parsed.moves {
        let ep_before = game.en_passant_target;

        let (src, dst, _promo) = match san_to_move(&mut game, san) {
            Ok(m) => m,
            Err(e) => {
                error!("[ANIM] san_to_move '{}': {:?}", san, e);
                break;
            }
        };

        let piece = game.board[src as usize];
        let piece_type = piece.abs();
        let color: i8 = if piece > 0 { 1 } else { -1 };

        let mut movements: Vec<(u8, u8, u8, u8)> = Vec::new();
        let mut ep_capture: Option<(u8, u8)> = None;

        // Primary piece movement
        movements.push((
            (src % 8) as u8,
            (src / 8) as u8,
            (dst % 8) as u8,
            (dst / 8) as u8,
        ));

        // Castling: king jumps 2 squares → also slide the rook
        if piece_type == KING_ID && (dst - src).abs() == 2 {
            let (rook_src, rook_dst): (i8, i8) = if color > 0 {
                if dst == 6 { (7, 5) } else { (0, 3) }   // white kingside / queenside
            } else {
                if dst == 62 { (63, 61) } else { (56, 59) } // black kingside / queenside
            };
            movements.push((
                (rook_src % 8) as u8,
                (rook_src / 8) as u8,
                (rook_dst % 8) as u8,
                (rook_dst / 8) as u8,
            ));
        }

        // En passant: pawn lands on ep square → remove the captured pawn
        if piece_type == PAWN_ID {
            if let Some(ep_target) = ep_before {
                if dst == ep_target {
                    let ep_sq: i8 = if color > 0 { dst - 8 } else { dst + 8 };
                    ep_capture = Some(((ep_sq % 8) as u8, (ep_sq / 8) as u8));
                }
            }
        }

        do_move(&mut game, src, dst, true);
        plies.push(PlyData { movements, ep_capture });
    }

    plies
}

/// Advances the animation one ply at a time; handles resets when the game ends.
pub fn animate_board_system(
    mut commands: Commands,
    time: Res<Time>,
    mut anim: ResMut<BoardAnimator>,
    transforms: Query<&Transform>,
    piece_entities: Query<Entity, With<MenuBgPiecePos>>,
    mut spawned: ResMut<super::new_menu::MenuBgPiecesSpawned>,
) {
    if !anim.active || !anim.initialized {
        return;
    }

    // No plies loaded (PGN parse failed) — show static board, no animation.
    if anim.plies.is_empty() {
        return;
    }

    // ── Game over: pause then reset ──────────────────────────────────────────
    if anim.ply_index >= anim.plies.len() {
        // Clamp so end_pause is always a visible delay (never flicker on reset).
        if anim.end_pause <= 0.0 {
            anim.end_pause = 4.0;
        }
        anim.end_pause -= time.delta_secs();
        if anim.end_pause <= 0.0 {
            for entity in piece_entities.iter() {
                commands.entity(entity).despawn();
            }
            anim.board = [[None; 8]; 8];
            anim.ply_index = 0;
            anim.move_timer = 3.0;
            anim.active = false;
            spawned.0 = false;
        }
        return;
    }

    // ── Countdown between plies ──────────────────────────────────────────────
    anim.move_timer -= time.delta_secs();
    if anim.move_timer > 0.0 {
        return;
    }
    anim.move_timer = anim.move_interval;

    let ply = anim.plies[anim.ply_index].clone();
    anim.ply_index += 1;

    // ── En-passant: remove the captured pawn that sits beside the destination ─
    if let Some((ep_file, ep_rank)) = ply.ep_capture {
        if let Some(entity) = anim.board[ep_rank as usize][ep_file as usize].take() {
            commands.entity(entity).despawn();
        }
    }

    // ── Apply each movement in the ply (1 for normal, 2 for castling) ────────
    for (from_file, from_rank, to_file, to_rank) in &ply.movements {
        let (ff, fr, tf, tr) = (*from_file, *from_rank, *to_file, *to_rank);

        // Capture: despawn whatever is on the destination square
        if let Some(captured) = anim.board[tr as usize][tf as usize].take() {
            commands.entity(captured).despawn();
        }

        // Move the piece entity with a smooth slide animation
        if let Some(entity) = anim.board[fr as usize][ff as usize].take() {
            anim.board[tr as usize][tf as usize] = Some(entity);
            let start = transforms.get(entity).map(|t| t.translation)
                .unwrap_or(Vec3::new(ff as f32, 0.05, fr as f32));
            let end = Vec3::new(tf as f32, 0.05, tr as f32);
            commands.entity(entity).insert(MenuBgPieceAnim { start, end, elapsed: 0.0, duration: 0.55 });
        }
    }

    // ── After final ply, start the end-pause countdown ───────────────────────
    if anim.ply_index >= anim.plies.len() {
        anim.end_pause = 6.0;
    }
}
