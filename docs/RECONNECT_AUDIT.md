# Reconnect audit — casual PvP and contract (wagered) games

Scope: what a client does when its link drops and comes back mid-game, for both
game kinds. Findings come from reading the code; nothing here was exercised with
two live clients.

## How the two paths differ

| | Casual PvP | Contract / wagered |
|---|---|---|
| Moves, chat | Braid durable outbox + gossip | same |
| Draw / rematch offers | gossip only | gossip only (+ on-chain `offer_draw` / `accept_draw`) |
| Link status (`P2PConnectionState`) | set by invite handshake | set by `resume::apply_plan` or the lobby |
| Opponent heartbeat | `tick_heartbeat` pings (OnlineMultiplayer only) | none; backend presence poll |
| Restart-and-rejoin | **none** | `ResumeState` via the Solana lobby "Resume game" |

## Fixed in this pass

1. **Link status never recovered.** `PeerDisconnected` set `Disconnected` mid-game;
   nothing set it back, so the "Connection lost" banner stayed up for the rest of
   the game and `opponent_disconnect_ui` (which only clears on `InGame`) never
   cleared. Now restored by `PeerConnected` for the opponent, or by any message for
   the running game (covers resumed games, which have no `peer_node_id`).
   `network/p2p.rs`
2. **Any gossip neighbour dropping flipped the status.** `NeighborDown` fires for
   every peer on every topic. Now only the opponent (or our own failed broadcast)
   counts.
3. **`connected_peers` was never pruned**, so a reconnect could be treated as
   "already connected" (`p2p.rs`, connect path). Now removed on disconnect.
4. **Stuck draw prompt.** `PendingDrawOffer` had no lifetime. It now lapses on: any
   move, game over, 60 s, link drop or recovery, leaving the game. The offerer's
   button now reflects *their own* offer ("DRAW SENT"); before, it keyed off the
   opponent's offer. The offer also named the side to move instead of the offerer.
   `game/systems/network_move.rs`, `ui/game/game_ui/moves.rs`
5. **Duplicate chat.** One message reaches a client as the optimistic local echo,
   gossip, the Braid stream, and a replay after resubscribe (the self-echo filter
   lives in memory and is empty after a restart). `ChatState` now dedupes on
   (sender, text, timestamp), is bounded at 200, is scoped to the game id, and a
   duplicate local line is not published twice. `ui/game/chat_ui.rs`
6. **Chat card drawn over the move controls.** The move list (260 px) and chat card
   (280 px) were fixed-size regardless of the panel height. They now split the
   space that is left. `ui/game/left_panel.rs`

7. **Draw offers and answers were not durable.** They travelled by gossip only, so
   an answer made while the offerer was offline was lost. They now also go on the
   Braid moves stream (the backend already stored and participant-checked
   `offer_draw` / `accept_draw` / `decline_draw`). Each carries the position (`ply`)
   and time (`at_ms`) it was made in, which gives it a unique version and lets the
   receiver reject what no longer applies: an offer lapses when the position
   moves on or its prompt times out, and an acceptance only counts if it answers an
   offer *we* made in that position, however late it arrives. Replays after a
   reconnect cannot raise stale prompts.
   `braid_chess/message.rs`, `braid_transport/draws.rs`, `network_move.rs`
8. **Casual PvP can be rejoined after a restart.** A small record is saved when a
   casual game starts; the main menu offers "Rejoin game" while it exists. The
   backend log is replayed for legality (and refused if it shows a resignation,
   agreed draw, mate, or tampering), the session is restored, and the opponent is
   found again. Wagered resume and casual rejoin share one restore path.
   `multiplayer/casual_resume.rs`, `multiplayer/session_restore.rs`
9. **Resumed games now know the opponent.** New `GET /game/{id}/nodes` returns the
   host and joiner node ids the relay recorded when a lobby game started. Resume
   and rejoin use it to set `peer_node_id` (so opponent-presence checks run) and to
   bootstrap the peer on the gossip mesh.
10. **"Refresh state" did nothing** (it called `/api/games/{id}/state`, which does
    not exist, and discarded the result). It is now "Reconnect now": it restarts
    both Braid streams from their acknowledged cursors instead of waiting out the
    backoff, retries the outbox, and re-bootstraps the opponent.

## Wagered (Solana) games — second pass

The reconnect fixes above (link recovery, chat, durable draws, "Reconnect now",
node-aware resume) are transport-level and apply to wagered games unchanged. Reading
the wagered path end to end found that the *settlement* side of draws and resignation
did not work at all:

11. **Resigning a wagered game never reached the chain.** The program has `resign`,
    but nothing in the client or backend ever called it. The game ended on screen
    and stayed `Active` on the rollup; `finalize_game` needs `Finished`, so it only
    settled when the inactivity timeout fired, and that blames whoever is *on move*.
12. **Agreed draws were sent to the wrong layer.** `offer_draw` / `accept_draw` were
    signed in the wallet and sent to the base-layer RPC, which refuses a delegated
    game, and a failure was only logged. The UI still showed a draw. Same timeout
    outcome: one side loses an agreed draw.
13. **Race between "draw agreed" and settlement.** The acceptor ended the game
    locally and announced it at the same moment the wallet prompt opened; the
    host could start undelegating before the acceptance transaction existed.
14. **Resignations never reached the Braid log.** The publisher
    (`PublishOnlineResign`) had no sender anywhere, so a resignation travelled by
    gossip only and a later resume could not see it. Removed the dead path;
    resignations are published like the other actions.
15. **Leaving a game could award the win to the leaver.** The ESC-confirm resign
    and the draw offer named the side *to move*, not the local player.
16. **Draw offers had the wrong lifetime.** On-chain an offer stands through the
    offerer's own next move and lapses when the opponent moves; the client lapsed it
    on any move. Now identical to `apply_recorded_move`.

What changed (`multiplayer/solana/chain_actions.rs`, `lobby/game_actions.rs`):

* In a wagered game (wager > 0 or a rollup copy) a draw offer, a draw acceptance
  and a resignation are **recorded on-chain first, then announced**. A declined
  wallet prompt or a failed transaction leaves the game running, says so, and offers
  "Try again". Free rated games (no wager, never delegated) settle off-chain and
  get no prompts. Casual games are untouched.
* Wallet-signed transactions are sent to the rollup endpoint while the game is
  delegated, to the base layer otherwise, re-resolved on every attempt. Failures
  that happen before the wallet prompt (simulation, blockhash) are retried for
  about 12 s, which covers an acceptance racing its offer onto the chain.
* An in-flight action is dropped if the player has moved to another game.
* The main menu now offers "Resume game" for a wagered game still running, not only
  the Solana lobby screen.
* No program or ABI change. The backend's settlement worker already undelegates and
  finalizes any game that reaches `Finished` on the rollup, so once the account is
  right it settles even if both clients close.

**Verify on devnet before relying on it** (not testable without a wallet and a live
rollup): that a wallet-signed `resign` / `offer_draw` / `accept_draw` is accepted by
the MagicBlock endpoint for a delegated game (fee payer need not hold lamports there,
as with the backend's session-key moves), and that the simulate-before-sign step
works against it. If the endpoint refuses, the notice will say so and nothing ends
locally; the fallback is session-key variants of these three instructions, which
needs a program upgrade.

## Still open

1. **Casual rejoin does not restore the clock**; it restarts at the full time
   control. A flagged game is not recognised on rejoin (timeouts are not logged).
2. **Wagered resume does not restore the local clock either**; the chain clock keeps
   running (the lobby says so), the on-screen one restarts.
3. **Direct-connect casual games** register no node ids with the backend, so rejoin
   uses the peer id saved at game start (absent if it was never known).
4. Heartbeat pings run for `OnlineMultiplayer` only, not `MultiplayerCompetitive`.
5. **Disconnect never reaches `ClaimTimeout` from the client** (backlog P4). The
   backend settlement worker does claim expired games on the rollup.
6. A draw offer that was declined stays standing on-chain until the opponent moves
   (the program's rule); only a modified client could still accept it.
7. Spectator chat log (`spectator_overlay.rs`) has no dedup.
8. `tests/live_devnet_lifecycle.rs` does not compile with `--features solana`
   (`get_fee_for_message` bound), which fails the second configuration of
   `scripts/check_rust_safety.ps1`. Unrelated to this work.

## Deploy note

`GET /game/{id}/nodes` is backend-only. Until the backend is deployed the client
treats a 404 as "unknown" and falls back to what it saved, so nothing breaks.
Draw messages added `ply` / `at_ms` with serde defaults: old logs still decode, and
an older client simply ignores the fields.

## Needs a manual two-client check

- Drop one client's network mid-game, restore it: banner clears; an offer made
  before the drop is gone on both sides; chat shows each line once; the chat card
  no longer covers OFFER DRAW / RESIGN in a short window.
- Offer a draw, take the offerer offline, accept on the other client, bring the
  offerer back: the game ends as a draw on both.
- Offer a draw, then move before it is answered: the offer disappears on both.
- Start a casual lobby game, play a few moves, kill one client, relaunch it: the
  main menu offers "Rejoin game"; rejoining restores the board and the opponent
  sees it reconnect. Repeat with a wagered game.
- Press "Reconnect now" on the connection-lost banner.
- Wagered game on devnet: resign from the board, then from ESC-confirm while the
  opponent is on move; offer a draw and accept it; decline the wallet prompt on each.
  The game must settle (payout / draw split) without waiting for a timeout, and a
  declined prompt must leave the game running.
- Wagered game: kill one client mid-game; the main menu offers "Resume game".
