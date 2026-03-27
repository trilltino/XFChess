# XFChess — Project Reference

> Decentralized chess on Solana with MagicBlock Ephemeral Rollups for sub-second in-game moves.

---

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [Repository Layout](#repository-layout)
4. [Architecture](#architecture)
   - [High-Level Flow](#high-level-flow)
   - [Component Map](#component-map)
5. [On-Chain Program](#on-chain-program)
   - [Program IDs & Constants](#program-ids--constants)
   - [PDAs](#pdas)
   - [Account Schemas](#account-schemas)
   - [Instructions](#instructions)
6. [VPS Signing Server](#vps-signing-server)
   - [HTTP API](#http-api)
   - [Session Lifecycle](#session-lifecycle)
7. [Client (Bevy App)](#client-bevy-app)
   - [Module Tree](#module-tree)
   - [Multiplayer Subsystem](#multiplayer-subsystem)
   - [P2P Network Protocol](#p2p-network-protocol)
   - [Ephemeral Rollup Manager](#ephemeral-rollup-manager)
   - [VPS Client](#vps-client)
8. [MagicBlock Ephemeral Rollup Flow](#magicblock-ephemeral-rollup-flow)
9. [Game Lifecycle](#game-lifecycle)
10. [Wallet Integration (Tauri)](#wallet-integration-tauri)
11. [Crates](#crates)
12. [Configuration & Environment](#configuration--environment)
13. [Build & Run](#build--run)
14. [Business Model](#business-model)

---

## Overview

XFChess is a peer-to-peer chess game where:

- **Game state** is recorded on **Solana devnet** via an Anchor program.
- **In-game moves** are processed at sub-second latency on a **MagicBlock Ephemeral Rollup (ER)** — a temporary sidechain that commits back to devnet when the game ends.
- **Player discovery and move sync** happen over a **P2P gossip network** (Iroh/QUIC + Braid).
- A **VPS signing server** holds per-game session keypairs so moves can be submitted on-chain without requiring wallet popups for every move.
- Wagers are held in a **lamport escrow PDA** and paid out automatically when `finalize_game` runs.
- **ELO ratings** and win/loss stats are stored in per-player `PlayerProfile` PDAs.

---

## Technology Stack

| Layer | Technology |
|---|---|
| Game engine | Bevy 0.18 (Rust, native desktop) |
| Blockchain | Solana (devnet) |
| Smart contracts | Anchor 0.32.1 |
| Ephemeral rollups | MagicBlock ER SDK 0.8.5 |
| P2P transport | Iroh (QUIC) + Braid gossip |
| Signing server | Axum (Rust HTTP), SQLite |
| Wallet popups | Tauri v2 + React + `@solana/wallet-adapter` |
| Chess engine | Shakmaty (host) + custom no-std on-chain variant |
| UI | Bevy Egui |

---

## Repository Layout

```
XFChess/
├── src/                        # Bevy client application
│   ├── main.rs                 # Entry point, Bevy App assembly
│   ├── lib.rs                  # Lib crate, GameConfig resource
│   ├── engine/                 # Pure chess logic (board state, move gen)
│   ├── game/                   # Bevy game systems and events
│   ├── multiplayer/            # P2P + Solana + ER integration
│   │   ├── mod.rs              # MultiplayerPlugin, top-level systems
│   │   ├── network/            # P2P (Iroh/Braid), protocol messages
│   │   ├── rollup/             # ER manager, bridge, VPS client, magicblock
│   │   ├── solana/             # Wallet state, lobby, Tauri signer
│   │   ├── ui/                 # Transaction debugger
│   │   └── wager_state/        # Wager flow state machine
│   ├── solana/                 # Client-side instruction builders
│   │   └── instructions.rs     # All IX builders (create_game, record_move, …)
│   ├── rendering/              # Bevy 3D board & piece rendering
│   ├── states/                 # Menu, game-over, pause screen states
│   └── ui/                     # In-game HUD panels
│
├── programs/
│   └── xfchess-game/           # Anchor program (on-chain)
│       └── src/
│           ├── lib.rs          # Program entrypoint, declare_id!
│           ├── constants.rs    # PDA seeds, AI authority
│           ├── errors.rs       # XfchessGameError codes
│           ├── state/          # Account schemas (Game, PlayerProfile, …)
│           └── instructions/   # One file per instruction handler
│
├── backend/
│   └── src/signing/            # VPS signing server (Axum)
│       ├── mod.rs              # Router builder
│       ├── routes.rs           # HTTP handlers
│       ├── solana.rs           # IX builders used by server
│       ├── store.rs            # SQLite session store
│       ├── feepayer.rs         # Fee-payer keypair pool
│       ├── auth.rs             # JWT issuer
│       └── config.rs           # SigningConfig (from env)
│
├── crates/                     # Internal workspace crates
│   ├── braid-core/             # Braid P2P state sync protocol
│   ├── braid-iroh/             # Iroh QUIC transport adapter
│   ├── chess-logic-shared/     # Shared chess types
│   ├── chess-logic-on-chain/   # no-std chess logic for the program
│   └── shakmaty-no-std/        # Shakmaty fork for on-chain use
│
├── tauri/                      # Tauri v2 wrapper for wallet signing
│   ├── src/main.rs             # Tauri shell
│   └── wallet-ui/              # React + @solana/wallet-adapter
│
├── Cargo.toml                  # Workspace root
├── Anchor.toml                 # Anchor config (cluster, programs)
└── scripts/                    # run_multiplayer.bat, etc.
```

---

## Architecture

### High-Level Flow

```
┌──────────────┐          ┌──────────────┐
│  Player A    │          │  Player B    │
│  (Bevy app)  │          │  (Bevy app)  │
└──────┬───────┘          └──────┬───────┘
       │   P2P (Iroh/Braid gossip)│
       └──────────────────────────┘
              │          │
       ┌──────▼──────────▼──────┐
       │   VPS Signing Server   │  ← holds session keypairs
       │   (Axum, SQLite)        │
       └──────────┬─────────────┘
                  │  HTTP (JSON)
       ┌──────────▼─────────────┐
       │  MagicBlock ER Validator│  ← sub-second moves
       │  devnet-eu.magicblock.app│
       └──────────┬─────────────┘
                  │  commit on game end
       ┌──────────▼─────────────┐
       │   Solana Devnet        │  ← authoritative state
       │   xfchess-game program │
       └────────────────────────┘
```

### Component Map

```
Bevy App
 ├─ MultiplayerPlugin
 │   ├─ P2PConnectionPlugin          network/p2p.rs
 │   ├─ EphemeralRollupPlugin        rollup/manager.rs
 │   ├─ RollupNetworkBridgePlugin    rollup/bridge.rs
 │   ├─ SolanaIntegrationPlugin      solana/integration/
 │   └─ SolanaLobbyPlugin            solana/lobby.rs
 ├─ EphemeralMvpPlugin               rollup/mvp_plugin.rs
 └─ WagerPlugin                      wager_state/

VPS Backend
 ├─ SessionStore  (SQLite)
 ├─ FeepayerPool  (pre-funded keypairs)
 └─ Routes        (Axum)

On-Chain Program (xfchess-game, Anchor)
 ├─ Game PDA
 ├─ MoveLog PDA
 ├─ SessionDelegation PDA
 ├─ PlayerProfile PDA
 └─ WagerEscrow PDA
```

---

## On-Chain Program

### Program IDs & Constants

| Name | Value |
|---|---|
| Program ID | `FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX` |
| MagicBlock Delegation Program | `DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh` |
| MagicBlock Magic Context | `MagicContext1111111111111111111111111111111` |
| MagicBlock Magic Program | `Magic11111111111111111111111111111111111111` |
| AI Authority | `AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp` |
| Network | Solana devnet |
| ER Endpoint | `https://devnet-eu.magicblock.app` |

### PDAs

| Account | Seeds | Description |
|---|---|---|
| `Game` | `["game", game_id.to_le_bytes()]` | Core game state |
| `MoveLog` | `["move_log", game_id.to_le_bytes()]` | Full move history |
| `PlayerProfile` | `["profile", player_pubkey]` | ELO, stats |
| `WagerEscrow` | `["escrow", game_id.to_le_bytes()]` | Lamport escrow |
| `SessionDelegation` | `["session_delegation", game_id.to_le_bytes()]` | VPS session key auth |

### Account Schemas

**`Game`**
```rust
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub status: GameStatus,      // WaitingForOpponent | Active | Finished | Expired
    pub result: GameResult,      // None | Winner(Pubkey) | Draw
    pub fen: String,             // Current board FEN (max 100 chars)
    pub move_count: u16,
    pub turn: u8,
    pub created_at: i64,
    pub updated_at: i64,
    pub wager_amount: u64,       // in lamports
    pub wager_token: Option<Pubkey>,
    pub game_type: GameType,     // PvP | PvAI
    pub bump: u8,
}
```

**`PlayerProfile`**
```rust
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    pub elo: u16,                // Default: 1200, auto-created on first game
}
```

**`SessionDelegation`**
```rust
pub struct SessionDelegation {
    pub game_id: u64,
    pub player: Pubkey,
    pub session_key: Pubkey,     // VPS keypair authorised to sign moves
    pub expires_at: i64,
    pub max_batch_len: u16,
    pub enabled: bool,
    pub bump: u8,
}
```

### Instructions

| Instruction | Signer | Description |
|---|---|---|
| `init_profile` | player | Create `PlayerProfile` PDA (ELO=1200) |
| `create_game` | player (white) | Create `Game` + `MoveLog` + `WagerEscrow`; auto-creates `PlayerProfile` |
| `join_game` | player (black) | Set `game.black`; status → `Active`; auto-creates `PlayerProfile` |
| `record_move` | session key or player | Append move to `MoveLog`, update `game.fen` |
| `commit_move_batch` | session key | Batch-commit multiple moves (ER optimised) |
| `authorize_session_key` | player | Write VPS session pubkey to `SessionDelegation` |
| `revoke_session_key` | player | Disable `SessionDelegation` |
| `delegate_game` | player | Delegate `Game` + `MoveLog` PDAs to MagicBlock ER |
| `undelegate_game` | VPS session key | Commit ER state back to devnet; release accounts |
| `finalize_game` | fee-payer (VPS) | Set `Finished`, pay wager escrow, update ELO |
| `withdraw_expired_wager` | white | Recover escrowed SOL after game expires |

> **Note:** `undelegate_game` has **no payer identity check** — the VPS session key may call it without a wallet popup.

---

## VPS Signing Server

The signing server (`backend/src/signing/`) is an Axum HTTP service that:

- Generates and stores a per-game **session keypair** in SQLite.
- Acts as **fee-payer** for ER transactions (from a pre-funded pool).
- Signs `record_move` and `commit_move_batch` on the ER with the session key.
- On game end: calls `undelegate_game` on the ER and `finalize_game` on devnet.

### HTTP API

| Method | Path | Description |
|---|---|---|
| `POST` | `/auth/issue` | Issue JWT for authenticated clients |
| `POST` | `/session/create` | Create session keypair for `game_id`; returns `session_pubkey` |
| `POST` | `/session/activate` | Submit wallet-signed setup TX (create_game + authorize_session_key) |
| `POST` | `/session/sign` | Sign a pre-built TX with the session key and submit |
| `GET` | `/session/status/:game_id` | Query session active status |
| `POST` | `/move/record` | Build + sign + submit `record_move` IX on ER |
| `POST` | `/game/undelegate` | Submit `undelegate_game` IX on ER (commits state to devnet) |
| `POST` | `/game/finalize` | Submit `finalize_game` IX on devnet (payout + ELO) |

### Session Lifecycle

```
1. client calls /session/create          → VPS generates session keypair, stores in SQLite
2. client builds create_game + join_game TX with authorize_session_key
3. wallet signs the TX (Tauri popup)
4. client calls /session/activate        → VPS submits to chain, session is live
5. game starts; moves via /move/record   → VPS signs with session key on ER (no popup)
6. game ends; client calls /game/undelegate → VPS commits ER → devnet
7. wait ~3s, then /game/finalize         → VPS sets Finished, pays wager, updates ELO
```

---

## Client (Bevy App)

### Module Tree

```
src/
 ├─ engine/          Pure chess: ChessEngine resource, move generation
 ├─ game/            GamePlugin: board events, game-over detection, history
 ├─ input/           Mouse/keyboard input → MoveMadeEvent
 ├─ rendering/       3D board mesh, piece sprites, animations
 ├─ states/          AppState FSM (MainMenu, Multiplayer, Playing, GameOver, Pause)
 ├─ ui/              Egui panels: clock, captured pieces, eval bar
 ├─ singleplayer/    AI opponent integration (Stockfish via braid_stockfish_ai)
 ├─ solana/          Client-side IX builders (instructions.rs)
 └─ multiplayer/     See below
```

### Multiplayer Subsystem

```
multiplayer/
 ├─ mod.rs               MultiplayerPlugin, top-level Bevy systems
 │                        feed_local_moves_to_rollup
 │                        handle_session_info_from_network
 │                        finalize_game_on_end
 │                        emit_game_ended_event
 │
 ├─ network/
 │   ├─ p2p.rs            P2PConnectionPlugin: Iroh node, peer discovery
 │   ├─ braid.rs          BraidP2PConfig, gossip topic management
 │   ├─ protocol.rs       NetworkMessage enum (Move, BatchPropose, SessionInfo, …)
 │   └─ game_id_store.rs  Global game_id singleton
 │
 ├─ rollup/
 │   ├─ manager.rs        EphemeralRollupManager resource + EphemeralRollupPlugin
 │   │                     Tracks committed_fen, pending_batch, status, game_id
 │   ├─ bridge.rs         RollupNetworkBridgePlugin
 │   │                     handle_game_start_delegation  (wallet popup)
 │   │                     handle_game_end_undelegation  (VPS, no popup)
 │   │                     retry_pending_delegation
 │   │                     process_batch_commit_requests
 │   ├─ magicblock.rs     MagicBlockResolver resource, delegation IX builders
 │   ├─ session_keys.rs   SessionKeyManager resource (local ephemeral keypair)
 │   ├─ vps_client.rs     Blocking HTTP client for all VPS endpoints
 │   └─ mvp_plugin.rs     EphemeralMvpPlugin (broadcast session info)
 │
 ├─ solana/
 │   ├─ integration/      SolanaIntegrationPlugin + SolanaIntegrationState
 │   │   ├─ state.rs       wallet_pubkey, session_keypair, opponent_pubkey
 │   │   ├─ systems.rs     session auth polling, handshake
 │   │   └─ rpc.rs         RPC helpers
 │   ├─ lobby.rs           SolanaLobbyPlugin: game creation / joining flow
 │   ├─ addon.rs           SolanaWallet, CompetitiveMatchState, SolanaProfile
 │   └─ tauri_signer.rs    sign_and_send_via_tauri() — wallet popup via IPC
 │
 ├─ wager_state/           WagerPlugin: wager amount tracking, escrow flow
 └─ ui/
     └─ tx_debugger.rs    TransactionDebuggerPlugin (--debug flag)
```

### P2P Network Protocol

`NetworkMessage` variants exchanged over the Iroh gossip topic:

| Variant | Purpose |
|---|---|
| `Move` | Relay a single move to the opponent |
| `BatchPropose` | Propose a batch of moves for ER commit |
| `BatchAccept` / `BatchReject` | Batch consensus |
| `BatchConfirmation` | Confirmed on-chain (tx_sig + new_fen) |
| `SessionInfo` | Exchange VPS session pubkeys |
| `TxMessage` / `TxSignature` | Raw TX bytes for multi-sig flows |
| `Committed` | Notify peer that a batch landed on-chain |
| `ResyncRequest` / `ResyncResponse` | State resync after disconnect |
| `GameInvite` / `InviteResponse` | Out-of-band matchmaking |
| `GameStart` | Confirm white/black assignment and initial FEN |

### Ephemeral Rollup Manager

`EphemeralRollupManager` (resource in `rollup/manager.rs`) maintains the ER state machine:

```
GameStateStatus: Synced → Pending → Committing → OutOfSync
```

Key fields:

- `game_id` — Solana on-chain game ID (u64, LE bytes of timestamp)
- `is_creator` — `true` = white player
- `committed_fen` — last FEN confirmed on-chain
- `committed_turn` — turn number of last commit
- `pending_batch` — buffered moves not yet sent to VPS
- `session_keys` — `(white_session_pubkey, black_session_pubkey)`

Batch flush triggers when `max_batch_size` (10) is reached or `flush_interval` (10 s) elapses.

### VPS Client

`rollup/vps_client.rs` — all calls are **blocking** (run inside `IoTaskPool` tasks):

| Function | Endpoint | Description |
|---|---|---|
| `create_session` | `POST /session/create` | Get/create session pubkey |
| `activate_session` | `POST /session/activate` | Submit wallet-signed TX |
| `record_move` | `POST /move/record` | Submit single move on ER |
| `sign_and_submit` | `POST /session/sign` | Submit pre-built TX via session key |
| `vps_undelegate_game` | `POST /game/undelegate` | Commit ER → devnet |
| `vps_finalize_game` | `POST /game/finalize` | Finalize game on devnet |
| `session_status` | `GET /session/status/:id` | Check session is live |

Base URL from `SIGNING_SERVICE_URL` env var, default: `https://unrejuvenated-philologically-trudi.ngrok-free.dev`

---

## MagicBlock Ephemeral Rollup Flow

```
devnet                           ER (devnet-eu.magicblock.app)
────────────────────────────────────────────────────────────
create_game  ──────────────────►
join_game    ──────────────────►
authorize_session_key ─────────►
delegate_game ─────────────────►  Game + MoveLog PDAs now live on ER

                                  record_move (VPS session key)  ← move 1
                                  record_move (VPS session key)  ← move 2
                                  …                              ← move N

undelegate_game (on ER) ────────► ER commits state back to devnet ──────►
                                                                 (3s wait)
finalize_game ◄────────────────── game.status = Finished, wager paid, ELO updated
```

Key points:

- **Delegation** requires the **player's wallet** to sign (one Tauri popup at game start).
- **Move recording** on the ER uses only the **VPS session key** — zero wallet popups per move.
- **Undelegation** uses the **VPS session key** (no auth check on the instruction).
- **Finalization** is fee-paid by the VPS from its pre-funded feepayer pool.

---

## Game Lifecycle

```
1. MATCHMAKING
   Player A: create_game (wallet popup) → game_pda, game_id
   Player B: join_game   (wallet popup) → game.status = Active

2. SESSION SETUP
   Both players: /session/create → get session_pubkey from VPS
   Both players: authorize_session_key (bundled in create/join TX)
   P2P: exchange SessionInfo messages to share session pubkeys

3. DELEGATION
   White player: delegate_game (wallet popup) → Game + MoveLog on ER
   RollupNetworkBridge: handle_game_start_delegation fires

4. GAMEPLAY (ER, sub-second)
   Each move: VPS /move/record → ER tx, no wallet popup
   Periodic batch sync via BatchPropose consensus over P2P

5. GAME END (timeout / checkmate / resignation)
   Bevy: GameOverState → emit_game_ended_event → GameEndedEvent
   finalize_game_on_end: force-flush pending batch
   handle_game_end_undelegation (IoTaskPool async task):
     a. vps_undelegate_game  (ER tx, VPS session key)
     b. sleep 3s
     c. vps_finalize_game    (devnet tx, VPS fee-payer)
   → game.status = Finished, wager paid, ELO updated

6. RESULT
   PlayerProfile.elo updated on-chain
   WagerEscrow paid to winner (or split on draw)
```

---

## Wallet Integration (Tauri)

A small Tauri v2 shell wraps the Bevy window. When a wallet signature is needed:

1. Bevy calls `tauri_signer::sign_and_send_via_tauri(rpc_url, wallet_pubkey, &[ix], &[])`.
2. The Tauri webview opens with a React page using `@solana/wallet-adapter`.
3. The user approves in Phantom / Solflare.
4. Signature is returned to Bevy via Tauri IPC; the TX is submitted to devnet.

Wallet popups occur **exactly three times** per competitive game:
1. `create_game` (white)
2. `join_game` (black)
3. `delegate_game` (white, at game start)

Everything after that uses the VPS session key.

---

## Crates

| Crate | Purpose |
|---|---|
| `braid-core` | Braid CRDT-like state sync, patch/version protocol |
| `braid-iroh` | Iroh QUIC node + Braid gossip topic adapter |
| `braid_uri` | Braid node URI parsing |
| `chess-logic-shared` | Shared chess types used by both client and on-chain |
| `chess-logic-on-chain` | no-std chess logic (Shakmaty fork, for on-chain move validation) |
| `shakmaty-no-std` | no-std Shakmaty for BPF target |
| `shakmaty-host-vendored` | Full Shakmaty for host-side engine |
| `shared` | Common serialisation types across crates |
| `iroh-gossip` | Forked iroh-gossip with XFChess adjustments |
| `xfchess-ai-service` | Stockfish AI HTTP service |
| `solana-chess-client` | Legacy Solana client helpers |

---

## Configuration & Environment

### Client

| Env / CLI Flag | Default | Description |
|---|---|---|
| `--game-id` | — | On-chain game ID (u64) |
| `--player-color` | — | `white` or `black` |
| `--rpc-url` | `https://api.devnet.solana.com` | Solana RPC |
| `--p2p-port` | `5001` | Iroh P2P listen port |
| `--wager-amount` | — | SOL to stake |
| `--debug` | `false` | Enable transaction debugger |
| `SIGNING_SERVICE_URL` | ngrok URL | VPS base URL |
| `XFCHESS_IDENTITY` | — | Path to Iroh identity key file |

### VPS Backend

Configured via `.env` or environment:

| Variable | Description |
|---|---|
| `PROGRAM_ID` | On-chain program pubkey |
| `SOLANA_RPC_URL` | Devnet RPC endpoint |
| `ER_RPC_URL` | MagicBlock ER endpoint |
| `FEE_PAYER_KEYS` | Comma-separated base58 keypairs (pre-funded) |
| `JWT_SECRET` | Secret for JWT signing |
| `DATABASE_URL` | SQLite path (`sqlite:sessions.db`) |
| `PORT` | HTTP listen port (default `3000`) |

---

## Build & Run

### On-chain program

```bash
# Build
cargo build-sbf --package xfchess-game --features move-validation --release

# Deploy (requires ~10 SOL devnet)
solana program deploy target/deploy/xfchess_game.so \
  --url devnet \
  --program-id target/deploy/xfchess_game-keypair.json
```

### VPS signing server

```bash
cargo build --release -p backend --bin signing-server
./target/release/signing-server
```

### Client

```bash
# Single player / AI
cargo run --features solana

# Two local instances for testing (see scripts/run_multiplayer.bat)
scripts\run_multiplayer.bat
```

### Feature flags

| Flag | Effect |
|---|---|
| `solana` | Enable all Solana/ER/wallet integration |
| *(none)* | Pure local chess, no Solana dependencies |

---

## Business Model

### Subscription Pricing

XFChess operates on a **flat £4.99/month subscription**. Players pay for platform access and keep 100% of their wager winnings.

This classification matters legally and financially:

- **Skill-based utility** — a flat access fee does not constitute a rake (a cut of stakes), which means the platform avoids classification as a gambling operator under UK law.
- **Tax efficiency** — by not taking a percentage of stakes, we fall outside the scope of the **Remote Gaming Duty (40%)** introduced in the UK in April, and instead pay the standard **19–25% Corporation Tax** on profit.

### Unit Economics

| Item | Value |
|---|---|
| Monthly subscription | £4.99 |
| Server cost per game | ~£0 (Iroh P2P; no relay server needed) |
| VPS signing server (shared) | Fixed low cost, does not scale with game count |
| Gross margin | ~90% |

Because Iroh handles all P2P move-sync directly between clients, we bear **zero marginal server cost per game**. The VPS signing server is a small fixed cost shared across all sessions, not a per-game expense.

### Why This Works for Players

- Players pay a predictable "club membership" fee.
- They retain **100% of wager winnings** — no house cut.
- On-chain escrow via the `WagerEscrow` PDA ensures payouts are trustless and automatic.

### Revenue Scalability

Because marginal cost per game is effectively zero, revenue scales linearly with subscriber count while operating costs remain flat. A 10× increase in players produces roughly a 10× increase in profit with no infrastructure changes required.
