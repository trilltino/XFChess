# Binary Tools

## Purpose

The `bin` directory contains standalone binary tools for XFChess development, debugging, and utilities. These executables run independently from the main game and provide specialized functionality for developers and power users.

## Impact on Game

These tools assist with:
- **Debugging**: Monitor transactions and game events
- **Development**: Test features without full game startup
- **Analysis**: Inspect game states and network traffic
- **Utilities**: Helper scripts for common tasks

## Available Binaries

### `debugger`

| Property | Value |
|----------|-------|
| File | [`debugger.rs`](debugger.rs) |
| Purpose | Transaction and event log debugger |
| Usage | `./xfchess-debugger --game-id 12345` |

The debugger tool monitors rollup transactions and game events:
- Real-time log tailing
- Transaction validation
- Event filtering by game ID
- WebSocket remote monitoring (optional)

#### Debugger CLI Arguments

```bash
./xfchess-debugger \
  --game-id 12345 \           # Game to monitor
  --log-file ./debug.log \    # Output file
  --websocket-port 9000 \     # Optional WebSocket server
  --follow                    # Continuous monitoring
```

#### Example Output

```
╔══════════════════════════════════════════╗
║     XFChess Transaction Debugger         ║
╚══════════════════════════════════════════╝
Game ID: 12345
Log file: ./debug.log

[2024-01-15 14:32:01] GAME_START: White vs Black
[2024-01-15 14:32:15] MOVE: e2e4 (White)
[2024-01-15 14:32:28] MOVE: e7e5 (Black)
[2024-01-15 14:33:05] VALIDATION: Move legal
```

## Building Binaries

### Build All Binaries

```bash
cargo build --release --bins
```

### Build Specific Binary

```bash
cargo build --release --bin debugger
```

### Run Binary

```bash
# Via cargo
cargo run --bin debugger -- --game-id 12345

# Direct execution
./target/release/xfchess-debugger --game-id 12345
```

## Adding New Binaries

To add a new binary tool:

1. Create file in `src/bin/` (e.g., `src/bin/my_tool.rs`)
2. Add to `Cargo.toml`:

```toml
[[bin]]
name = "my_tool"
path = "src/bin/my_tool.rs"
```

3. Implement with standard CLI pattern:

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "my_tool")]
struct Args {
    #[arg(long)]
    option: String,
}

fn main() {
    let args = Args::parse();
    // Tool implementation
}
```

## Dependencies

Common dependencies for binaries:
- [`clap`](https://docs.rs/clap) - Command-line argument parsing
- [`tracing`](https://docs.rs/tracing) - Structured logging
- [`tokio`](https://docs.rs/tokio) - Async runtime (if needed)

## Related Components

- [`multiplayer`](../multiplayer/README.md) - Network debugging
- [`solana`](../solana/README.md) - Transaction monitoring
- [`game`](../game/README.md) - Game state inspection
