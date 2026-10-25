# Unified movement audit

Baseline: `70a25c5c4e2d2fef18bb30f54f4260a74e27fadb`. The working tree already
contained the motion migration and premove changes when this task was opened;
those changes are preserved and completed in place. No AGENTS.md was found in
the repository or its ancestor directories.

## Ownership and callers

* `game/systems/shared.rs`: `execute_move` is authoritative. It updates Piece,
  captures (including en passant), castling rook, promotion, SAN/history, FEN,
  pending turns and MoveMadeEvent. Animation descriptors are local commands.
* `input/board.rs`: both picking observers and egui use selection and move
  validation here. The shared premove resource is an intent, not a move.
* `game/ai/systems/mod.rs`: Stockfish results validate then call execute_move.
* `game/systems/network_move.rs`: remote, spectator and scripted puzzle moves
  validate and call execute_move. Expected FEN comparison, deduplication and
  acknowledgements happen here, independently of animation.
* `multiplayer/systems/control.rs`: trusted casual resync responses replace FEN.
  Visual transactions and premoves must be invalidated and ECS rebuilt too.
* `game/systems/spectate_sync.rs`: initial/full snapshots must rebuild ECS;
  consecutive moves can use NetworkMoveEvent without originating local moves.
  `multiplayer/spectator.rs` also dispatches polled, live-feed and Braid-resync
  moves through NetworkMoveEvent, with its existing confirmation and delay gates.
  These sources inherit the per-piece catch-up policy without transport changes.
* `multiplayer/resume.rs`: restores verified FEN and turn after FEN piece spawn;
  scene entry already resets visual generation. No protocol change is needed.
* `puzzle/session.rs`: FEN setup and scripted UCI responses use the existing
  network-move application path; player responses retain puzzle validation.
* `game/systems/visual.rs`: capture sinking/knockback/tilt/scale stays separate;
  flush_pending_turn is authoritative and must never wait for motion.
* `game/systems/promotion.rs`: mesh replacement must also follow promotions
  applied by AI/remote moves, retaining picking, layers and visual markers.
* `ui/game/game_2d/render.rs`: projects board samples with current geometry;
  suppresses each moving entity's static destination and paints overlays.
* `game/replay/board.rs`: rebuilt entities animate single forward steps; seeks
  and backwards steps snap. Castling requires two descriptors. En passant must
  identify the moving pawn, not the victim. Replay emits no gameplay moves.
* `states/main_menu/board_animation.rs`: PGN showcase retains planning, pacing,
  fades and resets, and uses the common sampler for piece travel.
* `xf_animate/animation/{components,systems}.rs` and `sequence/playback.rs`:
  LEARN retains sequence timing and scale, with common sampling and disjoint
  active/idle translation writers.

## Scheduling and lifecycle

`core/schedule.rs` orders Receive, Session, Input, Validation, Execution, Visual,
Send. EguiPrimaryContextPass follows Update. Local egui input must be buffered
until Update so authority and publication are processed in that ordering.
Execution flushes turns and computes results immediately; Visual alone advances
the canonical PieceMotion per entity, then presents it in 3D. Egui only reads.
Scene entry/exit, snapshot replacement and replay reconstruction retire old
motion generations. View switching only changes visibility/projection.

## Search results and gaps found before further edits

PieceMoveAnimation, animate_piece_movement and the global PieceAnim2D timer
were already removed by the incoming working tree. MenuBgPieceAnim and
MiniMoveAnimation remain scene adapters calling MotionStyle::sample. Other
lerps/smoothstep expressions belong to cameras or fades, not piece travel.

The initial default cargo check passed. Initial library tests exposed replay
castling pairing, empty legal caches after successive accepted moves, incomplete
test entities, invalid test FENs and a floating point test endpoint. Further
inspection found snapshot-to-ECS reconciliation, 3D promotion visuals, shared
replay 2D rendering, premove identity/promotion, retarget lift continuity and
first-frame scheduling still needed attention. These are required integration
work, not reasons to change move wire formats or settlement code.
