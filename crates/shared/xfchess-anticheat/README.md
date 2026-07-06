# xfchess-anticheat

Server-side, post-game anti-cheat analysis. The backend feeds finished games in; the
crate runs a Stockfish subprocess over the moves, extracts behavioral features, scores
them against ELO-calibrated baselines, and emits a structured report per game plus
cross-game aggregation per player.

## Pipeline

```
GameRecord ──ingest──► engine/stockfish (per-ply evals)
                            │
                       features/  accuracy · timing · blur · complexity
                            │
                       scoring/   per-signal scores → SideAnalysis
                            │
                       elo_baseline.rs  (what's normal at this rating?)
                            │
                       report/ ──► AcReport          cross_game/ ──► longitudinal evidence
```

Entry point: `analyse_game(game, cfg)` — async, but internally drives a blocking
Stockfish subprocess for the duration; call it from a dedicated worker task (the
backend does this from its background job queue).

## Modules

| Module | Contents |
|--------|----------|
| `ingest.rs`, `types.rs` | `GameRecord` input shape, `PlyEval`, `SignalValues`, `AcReport` |
| `engine/` | Stockfish process management (`StockfishHandle`), depth/movetime control |
| `features/` | Signal extraction: move accuracy vs engine, move-time distributions, browser blur events, position complexity |
| `scoring/` | Converts raw signals into calibrated per-side scores |
| `elo_baseline.rs` | Expected signal ranges by rating band, so strong play at high ELO isn't flagged |
| `cross_game/` | Aggregates per-game reports into per-player evidence over time |
| `report/` | Final `AcReport` assembly |
| `config.rs` | `AcConfig`: Stockfish path, analysis depth, movetime, thresholds |
| `metrics.rs` | Prometheus counters/histograms for analysis throughput and flag rates |
| `error.rs` | `AcResult` error type |

## Operational notes

- Requires a Stockfish binary on the host; the path comes from `AcConfig`.
- Analysis cost scales with depth × game length — budget worker concurrency
  accordingly (one Stockfish process per in-flight analysis).
- Reports are advisory evidence for the dispute flow; nothing here auto-bans.
