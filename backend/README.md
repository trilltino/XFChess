# XFChess Backend

## Purpose

The Backend provides server-side services for XFChess, including game indexing, P2P network observation, and HTTP API endpoints. It acts as a sidecar node that observes the Iroh gossip network to track active games and provide queryable game state.

## Impact on Game

This backend enables:
- **Game Discovery**: Query active and historical games via REST API
- **Network Observation**: Monitors P2P network traffic for game events
- **Game Indexing**: Maintains searchable index of all games
- **Observer Mode**: Spectator functionality without direct P2P participation
- **Relay Services**: Fallback connectivity for players behind NAT

## Architecture/Key Components

### Application State

| Component | Purpose |
|-----------|---------|
| [`AppState`](src/main.rs:18) | Shared application state including Iroh node and game index |
| [`GameRecord`](src/main.rs:30) | Indexed game data structure |
| [`Indexed Games`](src/main.rs:25) | In-memory storage of observed games |

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check for monitoring |
| `/games` | GET | List all indexed games |
| `/games/:id` | GET | Get specific game by ID |
| `/observe` | POST | Start observing a game node |

### Network Components

| Component | Purpose |
|-----------|---------|
| Iroh Node | P2P networking node for gossip protocol participation |
| Gossip Topic | `XFChess-0.5-SOL` topic for game event broadcast |
| Network Observer | Async task processing gossip events |

### Event Types

| Event | Description |
|-------|-------------|
| `game_start` | New game created with players and stake |
| `move_made` | Chess move recorded |
| `game_end` | Game concluded with winner |
| `NeighborUp` | New peer discovered |
| `NeighborDown` | Peer disconnected |

## Usage

### Running the Backend

```bash
# Development
cargo run --bin backend

# Production
cargo run --release --bin backend
```

### API Usage Examples

#### Health Check

```bash
curl http://localhost:3000/health
# Response: OK
```

#### List Games

```bash
curl http://localhost:3000/games
```

```json
[
  {
    "id": "game-12345",
    "players": ["node-abc", "node-def"],
    "stake_amount": 0.5,
    "start_time": 1705319521,
    "moves": ["e2e4", "e7e5", "Nf3"],
    "end_time": null,
    "winner": null
  }
]
```

#### Get Specific Game

```bash
curl http://localhost:3000/games/game-12345
```

#### Start Observing

```bash
curl -X POST http://localhost:3000/observe \
  -H "Content-Type: application/json" \
  -d '{"observer_node_id": "node-xyz"}'
```

## Architecture Flow

```
┌─────────────────┐     ┌──────────────────┐
│   XFChess       │     │   XFChess        │
│   Player 1      │◄───►│   Player 2       │
│   (Iroh Node)   │ P2P │   (Iroh Node)    │
└────────┬────────┘     └────────┬─────────┘
         │                       │
         └───────────┬───────────┘
                     │ Gossip
                     ▼
            ┌─────────────────┐
            │   Backend       │
            │   (Observer)    │
            │                 │
            │ • Indexes games │
            │ • Tracks moves  │
            │ • Stores state  │
            └────────┬────────┘
                     │ HTTP
                     ▼
            ┌─────────────────┐
            │   API Clients   │
            │   (Spectators)  │
            └─────────────────┘
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [`axum`](https://docs.rs/axum) | HTTP web framework |
| [`tokio`](https://docs.rs/tokio) | Async runtime |
| [`serde`](https://docs.rs/serde) | JSON serialization |
| [`tracing`](https://docs.rs/tracing) | Logging and instrumentation |
| [`braid-iroh`](../../crates/braid-iroh/README.md) | P2P networking |
| [`iroh-gossip`](../../crates/iroh-gossip/README.md) | Gossip protocol |

## Configuration

Environment variables (future enhancement):

```bash
XFCHESS_BACKEND_PORT=3000          # HTTP server port
XFCHESS_BACKEND_LOG_LEVEL=info     # Logging level
XFCHESS_BACKEND_DB_PATH=./data.db  # SQLite database path
```

## Database Schema (Planned)

```sql
-- Games table
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    player_white TEXT NOT NULL,
    player_black TEXT NOT NULL,
    stake_amount REAL,
    start_time INTEGER,
    end_time INTEGER,
    winner TEXT,
    final_fen TEXT
);

-- Moves table
CREATE TABLE moves (
    id INTEGER PRIMARY KEY,
    game_id TEXT REFERENCES games(id),
    move_number INTEGER,
    move_san TEXT,
    timestamp INTEGER
);
```

## Related Modules

- [`crates/braid-iroh`](../../crates/braid-iroh/README.md) - P2P networking
- [`crates/iroh-gossip`](../../crates/iroh-gossip/README.md) - Gossip protocol
- [`src/multiplayer`](../../src/multiplayer/README.md) - Client-side multiplayer
- [`shared`](../../crates/shared/README.md) - Shared message types

## Deployment

### Docker (Future)

```dockerfile
FROM rust:1.75
WORKDIR /app
COPY . .
RUN cargo build --release
EXPOSE 3000
CMD ["./target/release/backend"]
```

### Systemd Service

```ini
[Unit]
Description=XFChess Backend
After=network.target

[Service]
Type=simple
User=xfchess
WorkingDirectory=/opt/xfchess
ExecStart=/opt/xfchess/backend
Restart=always

[Install]
WantedBy=multi-user.target
```
