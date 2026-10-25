# Unified movement implementation and verification

Verified on 2026-10-23 in the working tree based on commit
`70a25c5c4e2d2fef18bb30f54f4260a74e27fadb`.
No commit or push was created. The 42 movement-related Rust files and their
hashes are recorded in `movement-source-manifest.json`. Workspace formatting
also corrected existing formatting differences outside these files.

## Implemented behavior

- Gameplay owns one `PieceMotion` transaction per moving entity. ECS advances
  it once per frame; 3D transforms and egui read the same sample, easing,
  duration and completion state. Menu and LEARN retain scene-specific timing
  while using the shared evaluator. Legacy competing movement systems are gone.
- Remote moves validate the complete UCI string against the rules position
  before mutation or a successful application result. Invalid coordinates,
  forbidden promotion suffixes, missing required promotions and unsupported
  promotion types are rejected. All four legal promotion choices are covered.
- Accepted moves record independently evaluated rules checkpoints scoped to
  game ID and board epoch. A peer cannot choose a conflicting same-ply branch
  merely by sending a FEN. A matching checkpoint can repair local corruption.
  Other casual-game conflicts trigger an asynchronous read from the configured
  backend; the complete returned history must replay legally with exact FENs.
  Results are discarded if the game, board epoch, position or gameplay state
  changed while the request was running.
- Rollup snapshots must contain a legal history through the accepted baseline,
  exact resulting FENs and the matching version head. Accepted positions reach
  authoritative board reconstruction, which clears stale motion, capture
  entities, selection, promotion and premoves. Recovery does not rewind a newer
  gameplay position or bypass spectator delay. Unknown rollup replacements
  request missing history instead of choosing an unverified branch. Snapshot
  broadcasts derive their FEN and head from the same durable history.
- Puzzle setup initializes both turn color and fullmove number from its FEN
  before applying the opponent's setup move.
- AI results apply before turn advancement, clock increments and game-result
  finalization. New searches start after finalization. Ordinary Stockfish moves
  no longer acquire a trailing NUL character during UCI normalization.
- Ordinary forward replay updates existing entities and preserves unrelated
  active tracks. Captures retain their separate fade, castling moves both
  existing pieces, and promotion retains the mover's identity. Backward steps,
  seeks and incompatible board reconstruction can still replace entities.
- Drag gestures are reset in ECS across view and board-generation changes.
  Premoves bind to color, game/session ID, board epoch, and seat game/lease
  epoch. Execution rechecks start, seat, delegation, turn and current legality.
  Game-over entry clears pending recovery and stale board replacements.

Authoritative FEN, coordinates, history, clocks, move publication and remote
application results remain independent of visual midpoint/completion. No motion
progress or visual transaction state is added to network messages.

## Validation

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |
| `cargo check -j 1 -p xfchess --all-targets` | PASS |
| `cargo check -j 1 -p xfchess --all-targets --features solana` | PASS |
| `cargo clippy -j 1 -p xfchess --all-targets --features solana -- -D warnings` | PASS |
| `cargo test -j 1 -p xfchess -- --quiet` | PASS: 394 unit + 52 integration = 446 tests |
| `cargo test -j 1 -p xfchess --features solana -- --quiet` | PASS: 463 unit + 60 integration = 523 tests; one live devnet test ignored |
| `cargo test -j 1 -p xfchess-stockfish bundled_engine -- --ignored` with `STOCKFISH_PATH` pointing to `assets/bin/stockfish.exe` | PASS: native Stockfish runtime test |

The final tests include complete remote-promotion validation; trusted same-ply
repair and rejected peer branch changes; stale recovery-task cancellation;
rollup snapshot reconstruction, history/head verification and spectator-delay
protection; both puzzle starting colors; AI plugin scheduling and UCI handling;
replay entity continuity, overlapping tracks and special moves at simulated
30/60/144 FPS; seat lease, start-barrier and delegation gates; and drag cleanup.

The egui test executes the actual square and moving-piece painters and checks
that each texture is painted once on creation, intermediate, completion and
drag frames. This is CPU paint-output testing, not GPU screenshot testing.
The publication test runs the actual local publisher and checks one gossip
message and one durable Braid enqueue through motion completion, disabled
animation and view changes. Its HTTP worker is held by a test lock.

Parallel linking initially exhausted Windows paging-file capacity. Serial
Cargo jobs completed successfully without weakening or skipping test suites.

## Remaining runtime acceptance

Manual GPU appearance, a complete interactive Stockfish match, two live network
peers, live Solana delegation/settlement, and non-Windows runtime acceptance
were not executed. The spending live-devnet test remains ignored. Automated
paint output, headless ECS execution and native Stockfish testing do not replace
those demonstrations.

Final logs are under the ignored `target/` directory:

- `fix-movement-default-complete.log`
- `fix-movement-solana-verified.log`
- `fix-movement-check-default-complete.log`
- `fix-movement-check-solana-complete.log`
- `fix-movement-clippy-verified.log`
- `fix-movement-stockfish-runtime.log`
