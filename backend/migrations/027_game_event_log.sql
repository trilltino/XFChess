-- Migration 027: game_event_log
--
-- Durable, ordered per-game event log backing the Braid moves/resign/chat
-- transport that replaces the old in-memory p2p_relay mailbox and chat.rs's
-- own ephemeral ChatRelayState. A backend restart previously dropped both
-- entirely (see p2p_relay/README.md and chat.rs's own doc comment); this
-- table lets the in-memory Braid AppendLog hydrate from disk instead.
--
-- One append-only table serves Move, Resign, and Chat (`kind` discriminates).
-- Ping/Pong (liveness) and SessionInfo (one-time handshake, re-derivable
-- from on-chain session delegation) are deliberately NOT persisted here —
-- they're served as ephemeral, non-durable Braid PatchedDoc resources.

-- `kind` is the same discriminant `ChessMessage`'s `#[serde(tag = "type")]`
-- already produces (move / resign / offer_draw / accept_draw / decline_draw
-- on the /moves stream; chat on the /chat stream) — not constrained here so
-- adding a new ChessMessage variant never requires a migration.
CREATE TABLE IF NOT EXISTS game_event_log (
    game_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL,
    version_hash TEXT NOT NULL,
    parent_version TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_game_event_log_game ON game_event_log(game_id, seq);
