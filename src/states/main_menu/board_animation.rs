use bevy::prelude::*;
use std::sync::OnceLock;

#[derive(Component, Clone, Copy)]
pub struct MenuBgPieceHome {
    pub file: u8,
    pub rank: u8,
}

#[derive(Component)]
pub struct MenuBgPieceAnim {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Component)]
pub struct MenuPieceFade {
    pub elapsed: f32,
    pub duration: f32,
    pub fade_in: bool,
}

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
        let alpha = if fade.fade_in { t } else { 1.0 - t };
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            // Blend while fading so the alpha actually shows.
            mat.alpha_mode = AlphaMode::Blend;
            let c = mat.base_color.with_alpha(alpha);
            mat.base_color = c;
        }
        if t >= 1.0 {
            if fade.fade_in {
                restore_piece_material(&mut materials, &mat_handle.0);
            } else {
                *vis = Visibility::Hidden;
            }
            commands.entity(e).remove::<MenuPieceFade>();
        }
    }
}

fn restore_piece_material(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
) {
    if let Some(mut mat) = materials.get_mut(handle) {
        let c = mat.base_color.with_alpha(1.0);
        mat.base_color = c;
        mat.alpha_mode = AlphaMode::Opaque;
    }
}

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

#[derive(Clone, Copy, PartialEq)]
pub enum ResetPhase {
    Idle,
    Hang(f32),
    FadeOut(f32),
    FadeIn(f32),
}

#[derive(Resource)]
pub struct BoardAnimator {
    pub ply_index: usize,
    pub move_timer: f32,
    pub reset: ResetPhase,
    pub board: [[Option<Entity>; 8]; 8],
    pub active: bool,
    pub game_index: usize,
    pub game_index_override: Option<usize>,
}

impl Default for BoardAnimator {
    fn default() -> Self {
        Self {
            ply_index: 0,
            move_timer: 2.5,
            reset: ResetPhase::Idle,
            board: [[None; 8]; 8],
            active: false,
            game_index: 0,
            game_index_override: None,
        }
    }
}

pub(super) fn request_game_nav(anim: &mut BoardAnimator, delta: i32) {
    if !matches!(anim.reset, ResetPhase::Idle | ResetPhase::Hang(_)) {
        return;
    }
    let len = super::famous_games::FAMOUS_GAMES.len() as i32;
    let wrapped = (anim.game_index as i32 + delta).rem_euclid(len) as usize;
    anim.game_index_override = Some(wrapped);
    // Next tick's `Hang(t)` branch computes `t - dt <= 0.0` and falls straight
    // into the existing fade-out/snap-home/fade-in sequence, unmodified.
    anim.reset = ResetPhase::Hang(0.0);
}

// ── Ambient board auto-play (post-Enter) ──────────────────────────────────────
//
// Replays the current game (`famous_games::FAMOUS_GAMES[game_index]`) on the
// full-size `MenuBg` board once the player presses Enter. Captured pieces are
// *hidden* (not despawned) so the whole game can loop without re-spawning:
// after the final move the board hangs on the position, then every piece
// fades out, is moved back to its `MenuBgPieceHome` while invisible, and fades
// back in — either for a loop of the same game or, here, for the next game in
// the carousel.

#[inline]
fn sq_world(file: usize, rank: usize) -> Vec3 {
    Vec3::new(7.0 - file as f32, 0.05, rank as f32)
}

#[derive(Clone, Copy)]
struct AmbientStep {
    from: (u8, u8),
    to: (u8, u8),
    capture: bool,
    ep_capture: Option<(u8, u8)>,
    castle_rook: Option<(u8, u8)>,
}

static ALL_PLANS: OnceLock<Vec<Vec<AmbientStep>>> = OnceLock::new();

fn all_plans() -> &'static [Vec<AmbientStep>] {
    ALL_PLANS
        .get_or_init(|| {
            super::famous_games::FAMOUS_GAMES
                .iter()
                .map(|g| compute_plan(g.pgn))
                .collect()
        })
        .as_slice()
}

fn compute_plan(pgn: &str) -> Vec<AmbientStep> {
    use nimzovich_engine::{do_move, new_game_no_tt, parse_pgn, san_to_move};
    let Ok(parsed) = parse_pgn(pgn) else {
        return Vec::new();
    };
    // No search ever runs on this Game (just replaying a fixed decorative
    // PGN), so skip the multi-GB transposition table `new_game` allocates.
    let mut game = new_game_no_tt();
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
    let plan = &all_plans()[anim.game_index];
    if plan.is_empty() {
        return;
    }

    // How long the board hangs on the final position before resetting, and how
    // long each side of the reset crossfade takes.
    const HANG_SECS: f32 = 15.0;
    const CROSSFADE_SECS: f32 = 1.5;

    // End-of-game reset: hang → fade out → snap home invisible → fade in → loop.
    match anim.reset {
        ResetPhase::Idle => {
            if anim.ply_index >= plan.len() {
                anim.reset = ResetPhase::Hang(HANG_SECS);
                return;
            }
        }
        ResetPhase::Hang(t) => {
            let t = t - time.delta_secs();
            if t > 0.0 {
                anim.reset = ResetPhase::Hang(t);
            } else {
                // Fade out every piece still on the board (captured pieces are
                // already hidden).
                for (e, _, _, vis, _) in reset_q.iter_mut() {
                    if *vis != Visibility::Hidden {
                        commands
                            .entity(e)
                            .remove::<MenuBgPieceAnim>()
                            .insert(MenuPieceFade {
                                elapsed: 0.0,
                                duration: CROSSFADE_SECS,
                                fade_in: false,
                            });
                    }
                }
                anim.reset = ResetPhase::FadeOut(CROSSFADE_SECS);
            }
            return;
        }
        ResetPhase::FadeOut(t) => {
            let t = t - time.delta_secs();
            if t > 0.0 {
                anim.reset = ResetPhase::FadeOut(t);
            } else {
                // Everything is invisible now — snap pieces home at alpha 0 and
                // fade the starting position back in. Resolve which game plays
                // next: a manual nav request wins, otherwise advance to the
                // next game in the list (wrapping around).
                let next = anim
                    .game_index_override
                    .take()
                    .unwrap_or((anim.game_index + 1) % super::famous_games::FAMOUS_GAMES.len());
                anim.game_index = next;
                anim.board = [[None; 8]; 8];
                for (e, home, mut tr, mut v, mat) in reset_q.iter_mut() {
                    commands
                        .entity(e)
                        .remove::<MenuBgPieceAnim>()
                        .remove::<MenuPieceFade>();
                    // Start fully transparent so the fade-in has no one-frame flash.
                    if let Some(mut m) = materials.get_mut(&mat.0) {
                        m.alpha_mode = AlphaMode::Blend;
                        let c = m.base_color.with_alpha(0.0);
                        m.base_color = c;
                    }
                    tr.translation = sq_world(home.file as usize, home.rank as usize);
                    *v = Visibility::Visible;
                    commands.entity(e).insert(MenuPieceFade {
                        elapsed: 0.0,
                        duration: CROSSFADE_SECS,
                        fade_in: true,
                    });
                    anim.board[home.rank as usize][home.file as usize] = Some(e);
                }
                anim.reset = ResetPhase::FadeIn(CROSSFADE_SECS);
            }
            return;
        }
        ResetPhase::FadeIn(t) => {
            let t = t - time.delta_secs();
            if t > 0.0 {
                anim.reset = ResetPhase::FadeIn(t);
            } else {
                anim.reset = ResetPhase::Idle;
                anim.ply_index = 0;
                anim.move_timer = 2.0;
            }
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_famous_games_parse_completely() {
        for g in super::super::famous_games::FAMOUS_GAMES {
            let parsed = nimzovich_engine::parse_pgn(g.pgn).expect("pgn should parse");
            let plan = compute_plan(g.pgn);
            assert_eq!(
                plan.len(),
                parsed.moves.len(),
                "{:?}: only resolved {}/{} plies",
                g.caption,
                plan.len(),
                parsed.moves.len()
            );
        }
    }
}

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
                fade_in: false,
            });
        }
    }
    if let Some((ef, er)) = step.ep_capture {
        if let Some(cap) = anim.board[er as usize][ef as usize].take() {
            commands.entity(cap).insert(MenuPieceFade {
                elapsed: 0.0,
                duration: FADE_SECS,
                fade_in: false,
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
