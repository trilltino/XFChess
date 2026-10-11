---
name: chess-engine
description: Integrate XFChess Stockfish UCI, nimzovich rules/search, Shakmaty PGN/SAN and Lichess puzzles. Use for engine subprocesses, legal moves, analysis, notation and puzzle import.
metadata:
  provenance: docs-derived
---

# Chess engines and data

Read `src/game/ai/`, `src/engine/`, `src/puzzle/`, backend puzzle/anti-cheat modules,
and the pinned nimzovich dependency. Keep the game, analysis engine and on-chain
validation contracts aligned; transport arrival is not proof of a legal move.

- Preserve board orientation, square indexing, side-to-move, castling, en passant
  and promotion conventions at each boundary. Parse and validate full UCI/FEN/PGN
  representations rather than assuming a four-character move or standard opening.
- Implement the actual UCI lifecycle: handshake, readiness, position, bounded search,
  bestmove, stop/quit and child reaping. Bound reads and handle engine exits.
- Scope search results to the position and request that produced them. Cancel or
  reject stale results after a new game, changed position or shutdown.
- Keep Stockfish executable lookup and deployment in the desktop-packaging skill.
  CPU-heavy search/anti-cheat work needs a bounded worker pool outside UI/runtime workers.
- Use Shakmaty for the existing SAN/PGN interpretation boundary and nimzovich for
  existing game/program rules. Do not replace the rule engine as part of a notation fix.
- For Lichess puzzle import, verify dataset/API schema, ID, FEN, move ordering and
  rate limits. Preserve import idempotency and storage layout; retain the supplied
  puzzle's initial move/setup convention when constructing the playable position.
- Keep analysis judgments and operational telemetry separate from deterministic
  legality; one engine score alone should not redefine a game's authoritative result.

Use representative castling, en passant, promotions, terminal positions and malformed
data for affected boundaries. Run a real engine handshake/search when changing UCI.

Sources: [Stockfish documentation](https://official-stockfish.github.io/docs/stockfish-wiki/),
[Shakmaty](https://docs.rs/shakmaty/latest/shakmaty/),
[Lichess API](https://lichess.org/api),
[Lichess puzzle database](https://database.lichess.org/#puzzles).
