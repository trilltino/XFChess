# Chess rules and Stockfish

Stockfish is the only AI opponent and analysis engine in the native game.
There is no engine picker or alternate-engine fallback. The eight difficulty
levels retain their time budgets and target strengths; targets are clamped to
the running engine's advertised UCI range.

`libs/chess/xfchess-rules` provides Shakmaty positions, legal moves, SAN, PGN,
annotations, and the existing 68-byte contract board encoding. The client,
backend, and benchmark tools use these rules without starting a process.
The Solana program alone retains its pinned Nimzovich rules dependency, with
default features and search disabled. Host applications depend on the program
with its default features disabled, using only its interface and state types.

Game endings follow the program: checkmate, stalemate, insufficient material
as the contract defines it (`Game::is_insufficient_material`, narrower than
Shakmaty's rule) and an automatic draw after 100 halfmoves without a capture or
pawn move. Threefold repetition is not detected on chain, so the client does not
end games on it either. `rules_compatibility` checks both rule sets agree.

`libs/chess/xfchess-stockfish` owns each child process on a worker thread.
It uses a 15-second startup deadline and a search deadline of the move budget
plus five seconds. Failed or illegal responses discard the process and retry
the same position once. A second failure pauses the AI game and its clock,
offering Retry or Return to menu. Searches carry a game identifier and position;
stale responses cannot play a move after navigation or a reset. Dropping a
request cancels it; dropping the worker terminates and reaps its process.

Optional evaluation uses a separate worker at unrestricted strength, with a
100 ms budget per position. Scores are normalized to White's perspective;
mate scores remain distinct from centipawns. Missing values appear as pending
or unavailable. Analysis is disabled in competitive online games and unrevealed
puzzles, and never blocks the opponent's worker or pauses gameplay on failure.

Set `STOCKFISH_PATH` to override executable discovery. Otherwise the desktop
game looks in `assets/bin`, beside the executable, and in packaged resources.
Android executes `libstockfish.so` from its extracted `nativeLibraryDir`; see
[the Android release gate](../runbooks/android-release.md).

```text
cargo test -p xfchess-rules -p xfchess-stockfish
cargo test -p xfchess-stockfish bundled_engine -- --ignored
cargo test -p xfchess-game --test rules_compatibility
pwsh -NoProfile -File scripts/check_engine_dependencies.ps1
```

The bundled-engine test requires the platform's native binary. Desktop release
jobs run it against the exact staged Stockfish executable. Contract tests run
separately from application tests so workspace feature unification cannot hide
an accidental engine dependency in the game or backend.
