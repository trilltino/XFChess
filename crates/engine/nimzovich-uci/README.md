# nimzovich-uci

A minimal, synchronous [UCI](https://www.chessprogramming.org/UCI) protocol adapter
around [`nimzovich_engine`](../nimzovich_engine/), used for engine-vs-engine match
testing with `cutechess-cli` and for loading the engine into any UCI-speaking GUI.

## Design

Single binary (`src/main.rs`), no async runtime. The engine's search is time-bounded
internally, so the adapter's `stop` command is a no-op — the engine always returns
within the budget it was given. This keeps the adapter tiny and deterministic for
regression matches.

## Usage

```bash
cargo build --release -p nimzovich-uci

# Engine-vs-engine regression match
cutechess-cli \
  -engine cmd=target/release/nimzovich-uci name=nimzovich-new \
  -engine cmd=path/to/baseline-nimzovich-uci name=nimzovich-base \
  -each proto=uci tc=10+0.1 -rounds 100
```

Strength changes to `nimzovich_engine` (search, evaluation, book) should be validated
with a match here before merging.
