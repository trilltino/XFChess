//! Menu-background board pieces for the main menu.
//!
//! Holds the ambient board state ([`BoardAnimator`]) and the smooth-slide
//! animation ([`animate_menu_pieces`]). The cinematic system
//! (`cinematic.rs`) drives which moves play; this module only owns the piece
//! components and the slide tween.

use bevy::prelude::*;

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
