# braid_chess

The **chess application layer** over Braid-HTTP. This crate maps a chess game onto Braid
resources and provides a typed publish/subscribe API, so the rest of XFChess never touches
raw HTTP headers or version hashes — it just sends `ChessMessage`s and receives them.

Built on [`braid-http`](../braid-http) (the Rust Braid protocol implementation, a port of the
braid.org JavaScript reference).

## Role in XFChess

`braid_chess` powers the legacy Braid document stream used by **`OnlineMultiplayer`** — an online transport path alongside the on-chain
Solana path for casual (unstaked) games — and the **per-game live sub-streams** that
spectators and reconnecting players subscribe to. It is *not* used for the Solana/Ephemeral-
Rollup move path (that goes on-chain), and it sits *below* matchmaking (the lobby/`JOIN_ACK`
handshake lives in `src/multiplayer/`, not here).

## Resource model

A game is addressed as a base-agnostic path; the origin (localhost / VPS / Iroh tunnel) is
supplied by the transport, so the same `ChessResource` works everywhere.

```text
/game/{game_id}/moves     ChessStream::Moves    authoritative move log (+ resign / draw)
/game/{game_id}/clock     ChessStream::Clock    clock snapshots, pushed after each move
/game/{game_id}/engine    ChessStream::Engine   Stockfish hints, streamed per depth
/game/{game_id}/chat      ChessStream::Chat      in-game / pre-game chat
```

Splitting a game into sub-streams means a spectator can follow moves without the chat feed,
or a reconnecting client can pull the clock snapshot independently of the move history.

## Messages

[`ChessMessage`] is the full event vocabulary, serialized as JSON with a `"type"` tag:

| Variant | Payload |
|---------|---------|
| `Move` | `MovePayload` — `from`/`to`/`promotion`/`uci`/`fen_after`/`move_number`/`player` |
| `Resign` / `OfferDraw` / `AcceptDraw` / `DeclineDraw` | `{ player }` |
| `Clock` | `ClockState` — `white_ms`/`black_ms`/`timestamp_ms` |
| `EngineAnalysis` | `EngineHint` — `depth`/`score_cp`/`mate_in`/`pv`/`best_move` |
| `Chat` | `ChatPayload` — `player`/`text`/`timestamp_ms` |

## Versioning — a content-addressed move chain

Because chess is turn-based and the server is authoritative, the Braid version DAG
degenerates (intentionally) to a **linear hash chain**. Each update's version is derived from
the resulting position:

```rust
version = sha256(format!("{fen_after}:{move_number}"))[..8]   // 16 hex chars
```

Every patch lists exactly one parent (the previous version) and uses `Merge-Type: replace` —
there is no concurrent-write merge, by design. The effect is a tamper-evident, deterministic
chain of moves: the same game always produces the same version sequence. See
[`patch::version_hash`].

## API

| Type | Purpose |
|------|---------|
| [`ChessResource`] / [`ChessStream`] | Typed, base-agnostic resource refs and path round-tripping |
| [`ChessMessage`] + payloads | The event vocabulary |
| [`BraidPatch`] / [`version_hash`] | Low-level patch construction (version + parents + body) |
| [`ChessPublisher`] | PUTs events to a sub-resource, tracking the head version |
| [`ChessSubscriber`] | Opens a `Subscribe: keep-alive` stream and decodes `ChessMessage`s |

```rust,no_run
use braid_chess::{ChessPublisher, ChessSubscriber, MovePayload};

#[tokio::main]
async fn main() {
    // Publish a local move.
    let mut publisher = ChessPublisher::new("http://localhost:3000", "ABCD42").unwrap();
    let mv = MovePayload::from_uci("e2e4", "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1", 1, "alice");
    publisher.publish_move(&mv).await.unwrap();

    // Subscribe to the opponent's moves (snapshot + live tail).
    let sub = ChessSubscriber::new("http://localhost:3000", "ABCD42").unwrap();
    let (rx, _task) = sub.subscribe_moves().await.unwrap();
    while let Ok(msg) = rx.recv().await {
        match msg {
            braid_chess::ChessMessage::Move(m)  => println!("opponent played {}", m.uci),
            braid_chess::ChessMessage::Resign { player } => println!("{player} resigned"),
            _ => {}
        }
    }
}
```

## Architecture

```
src/multiplayer/   (game client: publishes local moves, applies remote ones)
        │  ChessMessage
        ▼
   braid_chess      ◄── you are here  (ChessPublisher / ChessSubscriber / ChessResource)
        │  BraidRequest (Version, Parents, Merge-Type headers)
        ▼
   braid-http        (Braid-HTTP 209 client: PUT + Subscribe)
        │
        ▼
   xfchess-braid-server / VPS relay   (append-log, fan-out to subscribers)
```

## Dependencies

`braid-http` (protocol client), `serde`/`serde_json` (messages), `sha2`+`hex` (version
hashing).

## Provenance

Original XFChess work, built on the author's Rust Braid implementation. Protocol © the Braid
working group (MIT OR Apache-2.0). See `xfchess-braid-server/ATTRIBUTION.md`.
