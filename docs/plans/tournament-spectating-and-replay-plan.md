# Tournament Spectating & Replay — End-to-End Plan

**Status:** Phases 0–4 implemented. Phase 5 (subscribing the tournament screen to the
standings/pairings Braid resources) is not started; the detail list polls at 3s instead.

> **Correction to G4 below.** The original draft said a spectator could not leave a game in
> progress. That was wrong — the HUD has always had a working "Leave" button
> ([`spectator_overlay.rs`](../../src/multiplayer/ui/spectator_overlay.rs)). The real gap was
> narrower: Leave went to `MainMenu` and dropped the tournament context, and there was no way
> to move between games. Both are now addressed.
**Sibling doc:** [tournament-end-to-end-fix-plan.md](tournament-end-to-end-fix-plan.md) covers the
*player's* path through a tournament. This one covers the *viewer's*.

---

## 1. The goal, in product terms

Open a tournament and see it the way you'd see a real event:

- **Live** — every game currently in progress, who's playing, click to watch.
- **Exit and switch** — leave a game you're watching without leaving the tournament, and
  pick another one. Watching should feel like channel-hopping, not like launching an app.
- **Finished** — a tab of completed games with results, click to replay move by move.
- **Upcoming** — who plays whom next, and when the round closes.

The load-bearing requirement hiding in that list is **"is this game watchable right now?"**
Everything else is UI.

---

## 2. What already exists

Do not rebuild these. The spectator engine is in better shape than the surface suggests.

| Piece | Where | State |
|---|---|---|
| Spectator session, clock, move dispatch | [`src/multiplayer/spectator.rs`](../../src/multiplayer/spectator.rs) (432 lines) | Works |
| Broadcast-delay gating, **fail-safe** | `spectator.rs:145-152`, `resolve_spectator_delay` | Works — defaults to "delayed", only opens live gossip at delay 0 |
| Entry point | `SpectateViaLinkEvent` → `GameMode::Spectator` + `GameState::InGame` | Works |
| Deep links | `xfchess://spectate/{game_id}` | Works |
| "Watch Live" buttons | [`src/states/main_menu/screens.rs:2517`](../../src/states/main_menu/screens.rs#L2517) | Works, inside the tournament browser card |
| Game listing + username resolution | [`vps/tournament.rs:116`](../../src/multiplayer/network/vps/tournament.rs#L116) `list_tournament_games()` | Works, but see G1 |
| Delay-gated move feed | `GET /games/moves/{id}` → `get_moves_visible` ([repository.rs:392](../../backend/src/db/repository.rs#L392)) | Works |
| PGN for any game | `GET /games/{id}/pgn` ([history.rs:144](../../backend/src/signing/routes/history.rs#L144)) | Works |
| Replay engine | `ParsedPgnGameResource`, PGN modal ([modals.rs:324](../../src/states/main_menu/modals.rs#L324)), [`replay_braid.rs`](../../src/game/replay_braid.rs) | Works, but unreachable from a tournament |
| Tournament Braid resources | `/braid/tournament/{id}/{standings,pairings/N,results,…}` | Served as of the Braid consolidation; **no consumer yet** |

**Where tournament moves actually live.** This is the fact the whole plan turns on.
A tournament game is an on-chain game. `POST /record-move` validates, session-signs,
submits to the ER, *and* persists to SQLite via `add_move_simple`
([main.rs:752](../../backend/src/signing/routes/main.rs#L752)). So tournament moves are in
the `moves` table, served by `/games/moves/{id}`.

They are **not** in the Braid `game_event_log` — that's `/game/{id}/moves`, the casual-game
path. Live spectating today is HTTP polling, not Braid.

---

## 3. The gaps

### G1 — Discovery is an N+1 walk on a 10-second timer

`list_tournament_games()` fetches the tournament list, then issues **one `/bracket` request
per advertised tournament**, and keeps only matches with a `game_id`. It runs every 10s and
**only while `MenuState::Tournaments` is open**
([solana/tournament.rs:600-625](../../src/multiplayer/solana/tournament.rs#L600-L625)).

There is no per-tournament games endpoint. With 20 advertised tournaments that's 21 requests
every 10 seconds to render one card's game list.

### G2 — "Watchable" is inferred, not known

The Watch button is enabled on `match.status == "Active"`
([screens.rs:2539](../../src/states/main_menu/screens.rs#L2539)). That's the *bracket record*
status — set when the orchestrator creates the game account. It says nothing about whether
either player ever connected or a single move exists.

Click an Active game with no moves and you get an empty board and no explanation. There is
no concept of *has moves yet*, *last move at*, or *viewer count*.

### G3 — The live feed is a 2-second full-list poll

`tick_spectator_poll` re-fetches the entire move list every 2s and diffs against
`applied_move_count`. Latency is up to 2s on a game whose moves land sub-second on the ER,
and cost grows with game length × spectators.

### G4 — You cannot leave a game you are watching

The only exit is the **game-over** overlay's "Leave" → `MainMenu`
([game_over_popup.rs:897](../../src/ui/menus/game_over_popup.rs#L897)). While a spectated game
is still in progress there is no exit at all, and even the game-over exit dumps you at the
top-level menu with the tournament context gone. Channel-hopping is impossible.

### G5 — No historical games, anywhere

`get_game_history` is **per wallet**, not per tournament. Nothing lists a tournament's
finished games. Replay exists but is reachable only by pasting PGN or from your own
game-over screen.

### G6 — "What's next" is published and unread

Pairings, standings, results, and schedule-status are live Braid resources. Nothing
subscribes to them. The round-deadline countdown already renders
([screens.rs:2495](../../src/states/main_menu/screens.rs#L2495)) off the polled summary.

---

## 4. The one architectural decision

**Make a game's move stream one Braid resource that is the same thing live and afterwards.**

A spectator joining a game in progress needs *the moves so far, then the live tail*. That is
the definition of a Braid subscribe. And "the moves so far" is also exactly what replay
needs. Today those are two mechanisms (2s polling; PGN assembly) over one dataset.

Target: `record_move` also appends to a hub `AppendLog` at `game/{id}/moves`. Then:

- **Live spectating** = subscribe. Snapshot (whole game) + tail (new moves). No polling, no
  `applied_move_count` bookkeeping, no catch-up path.
- **Replay** = the same subscribe on a finished game. The snapshot *is* the game.
- **Reconnect** = free, same as the casual path already gets.

### The delay gate is the hard part

`get_moves_visible` filters by `now_ts` against the game's broadcast delay. This is a real
anti-ghosting control, and a naive subscription would bypass it. Two options:

**(a) Subscribe only at delay 0; keep polling for delayed games.** Preserves the existing
security property exactly. Two code paths, but the fail-safe stays where it is today.

**(b) Delay becomes a publisher property.** The hub append is *scheduled* `delay_secs` into
the future, so the resource simply is the delayed view and every reader is correct by
construction. One code path. Requires a rebuild-on-boot from SQLite so a restart doesn't
drop queued appends.

**Recommendation: ship (a) in Phase 3, move to (b) in Phase 3b once it's proven.** (b) is
the right end state — it deletes the reader-side filter and makes the delay unforgeable —
but it is not where you want to discover a scheduling bug.

---

## 5. Phases

Each phase is independently shippable and leaves the app better than it found it.

### Phase 0 — One endpoint that knows what's watchable *(unblocks everything)*

`GET /api/tournament/{id}/games` in
[`routes/tournament.rs`](../../backend/src/signing/routes/tournament.rs), returning per match:

```jsonc
{
  "game_id": 1234, "round": 2, "board": 3,
  "white": "…", "black": "…",
  "white_name": "alice", "black_name": "bob",
  "white_elo": 1840, "black_elo": 1795,
  "state": "live" | "upcoming" | "finished",   // computed, not the bracket enum
  "move_count": 17,                            // 0 ⇒ nobody has moved
  "last_move_at": 1755912345,                  // staleness signal
  "result": "1-0" | null,
  "broadcast_delay_secs": 0,
  "watchable": true
}
```

`state`/`watchable` are **computed server-side** from the bracket record joined against the
`moves` and `games` tables — one query, one place, both clients agree. This kills G1 and G2
together, and resolves usernames server-side so the client's per-process `USERNAME_CACHE`
([vps/tournament.rs:84](../../src/multiplayer/network/vps/tournament.rs#L84)) can go.

Client: replace `list_tournament_games()`'s bracket walk with one call per *viewed*
tournament.

### Phase 1 — A tournament detail screen with three tabs

Today tournament games are rendered inline inside a browser card. Promote to a real screen:
`MenuState::TournamentDetail { tournament_id }`.

- **Live** — `state == "live"`, sorted by board. Each row: round/board, both players with
  ELO, move count, a "last move Ns ago" chip, Watch button enabled on `watchable`.
- **Upcoming** — `state == "upcoming"`, plus next-round pairings and the round deadline.
- **Finished** — `state == "finished"` with results and a Replay button.

Disabled Watch buttons must say **why** ("waiting for first move"), not just grey out.

### Phase 2 — Spectator navigation *(fixes G4, the biggest felt gap)*

1. **Exit while live.** An always-present "Back to tournament" control in spectator mode —
   not only at game over. Needs `SpectatorSession::tournament_id` so exit returns to the
   detail screen rather than `MainMenu`.
2. **Next / previous game.** Cycle the live list without leaving spectator mode: tear down
   the session, swap `game_id`, re-run the delay lookup. `SpectatorSession` already
   supports re-entry; the missing piece is a clean `end_session()` that resets board state.
3. **Fix the game-over exit** to return to the tournament, with "watch another" offered
   alongside "leave".

> Phase 2 is the one users will notice most. Phases 0–1 make the list correct; this makes it
> feel like watching an event.

### Phase 3 — Live feed over Braid *(fixes G3)*

Append to a hub `AppendLog` at `game/{id}/moves` from the `record_move` path, and subscribe
the spectator to `/braid/game/{id}/moves` when `broadcast_delay_secs == 0`. Keep the poll for
delayed games (option (a) above).

Deletes `applied_move_count` diffing and the 2s timer for live games. Reuses
`braid_chess::ChessSubscriber`, already used for casual games.

**3b:** move the delay into scheduled publication (option (b)), then delete
`get_moves_visible`'s reader-side filter.

### Phase 4 — Replay from the same resource *(fixes G5)*

The Finished tab's Replay button subscribes to a finished game's move log, takes the
snapshot, and feeds `braid_move_log_to_parsed_pgn`
([replay_braid.rs](../../src/game/replay_braid.rs)) → `ParsedPgnGameResource`. The existing
replay UI plays it with no PGN text round-trip.

Fallback to `GET /games/{id}/pgn` for games predating the append-log.

### Phase 5 — "What's next", and the first consumer of the tournament Braid resources

Subscribe the detail screen to `/braid/tournament/{id}/standings`, `…/pairings/{round}`, and
`…/results`. Standings and the bracket update live as games finish, with no polling.

This is also what makes the Braid consolidation visible to a user for the first time — those
resources are currently served and read by nobody, and `spawn_swiss_subscription`
([solana/tournament.rs:341](../../src/multiplayer/solana/tournament.rs#L341)) is defined and
never called. Phase 5 either calls it or replaces it with an HTTP-209 subscription.

---

## 6. Test plan

| Rung | Check |
|---|---|
| 0 | `/api/tournament/{id}/games` returns correct `state`/`move_count` for a live, an unstarted, and a finished match |
| 0 | One request renders a tournament's game list — assert no `/bracket` fan-out |
| 1 | Unstarted match shows "waiting for first move", button disabled |
| 2 | Watch → back → watch another, twice, without a board-state leak between games |
| 2 | Exit mid-game returns to the tournament, not `MainMenu` |
| 3 | A spectator joining at move 20 sees all 20 then live moves |
| 3 | **A delayed game never emits a move earlier than its delay** — regression-test this directly; it's the anti-ghosting control |
| 4 | Replay of a finished tournament game matches its `/pgn` output move for move |
| 5 | Standings update in the UI on result recording with no client poll |

---

## 7. What you need to decide

1. **Delay handling** — stage (a)→(b) as recommended, or go straight to scheduled
   publication? (b) is fewer moving parts at rest, more risk in flight.
2. **Detail screen vs. expanded card.** A real screen is more work and much more room; the
   current inline list will not hold three tabs.
3. **Viewer counts.** Not in this plan. If you want "137 watching", it wants a presence
   resource, and it is its own phase.
4. **Does the web frontend get this too?** `xfchessdotcom/` has no Braid client at all today.
   The endpoints from Phase 0 are plain JSON and would work there immediately; Phases 3–5
   would need a JS Braid client.

---

## 8. Order of value

If you only do part of this: **Phase 0 + 1 + 2.** That gives a correct, honest game list and
the ability to hop between games, which is the actual product ask. Phases 3–5 are the
architectural payoff — less polling, one dataset for live and replay, and the tournament
Braid resources finally earning their keep — but nobody's experience is blocked on them.
