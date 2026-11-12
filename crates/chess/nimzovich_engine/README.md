# nimzovich_engine

The XFChess chess engine: a minimax AI with alpha-beta pruning, transposition tables,
and iterative deepening, modularized from the Salewski engine lineage. One crate serves
two very different builds — the full `std` search engine and the `no_std` move-generation
core that ends up inside the Solana program.

## Module map

| Module | Contents |
|--------|----------|
| `board.rs`, `bitset.rs` | Board representation and bit sets |
| `move_gen/` | Legal move generation (tables initialized once at startup) |
| `evaluation/` | Static evaluation (PeSTO tables in `pesto.rs`, positional terms) |
| `api/` | Engine entry points used by the game client and backend |
| `hash.rs` | Zobrist hashing / transposition table |
| `book.rs` | Opening book |
| `perft.rs` | Move-generation correctness benchmarks |
| `pgn.rs` | PGN import/export |
| `on_chain.rs`, `on_chain_moves.rs`, `on_chain_attack.rs` | The `no_std` subset: `CompactBoard` (68-byte packed board) and allocation-light move legality used on-chain |
| `constants.rs`, `error.rs` | Shared constants and error types |

## Build modes

- **`features = ["std", "search"]`** — full engine with time-bounded search. This is
  what `src/engine/` (game client) and the backend link.
- **default (`no_std`)** — board + move generation only. `chess-logic-on-chain`
  re-exports this subset into the Solana program, so these modules must stay
  allocator-free apart from what the `alloc` feature explicitly permits.

Keep the feature boundary clean: nothing under `on_chain*.rs` or the core move-gen path
may depend on `std`-only functionality.

## Testing

Perft suites validate move-generation correctness. For playing-strength regression,
drive the engine through [`nimzovich-uci`](../nimzovich-uci/) under cutechess-cli.
