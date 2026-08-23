# Tournament Flow — End-to-End Fix Plan

**Date:** 2026-08-21
**Status:** PROPOSED
**Scope:** The complete player journey for a multi-round tournament, client and backend
**Scenario used throughout:** 16-player single-elimination (4 rounds, 15 matches)

---

## 1. Verdict

The **backend runs a tournament correctly end to end.** Registration, seeding,
bracket generation, on-chain match creation, deterministic game IDs, settlement,
winner advancement, and prize distribution all work and are tested.

The **client can't follow it past the first game.** Not because the logic is
wrong, but because the wire that tells the client "here is your match" was never
connected. Everything downstream of that wire — the match handler, the waiting
room, the result display — is fully written and unreachable.

A 2-player tournament hides this completely: one match, which is also the final.
At 16 players you need round-to-round transition four times, and it fails at the
first one.

---

## 2. Lifecycle trace

| Phase | What happens | State |
|---|---|---|
| **A. Create** | Panel → affordability pre-flight → `initialize_tournament` + `initialize_escrow` + `initialize_shards` | **Works** |
| **B. Fund prize** | `fund_sol_prize`; contract rejects registration until locked | **Works** |
| **C. Register** | Client signs `register_player`, then `POST /confirm-join` verifies the tx on-chain and returns a slot | **Fixed 2026-08-21** |
| **D. Start** | Scheduler fires on fill or `scheduled_at`; seeds by ELO, generates bracket, `initialize_match` × 15, assigns deterministic game IDs for round 1 | **Works** |
| **E. Match → client** | Backend has `GET /my-match` ready and correct | **BROKEN — no client poll** |
| **F. Play** | Session keys, Iroh P2P, ER delegation, sub-second moves | **Works once entered** |
| **G. Game ends** | Settlement worker → `finalize_game` → ELO + payout → `record_match_result` + `advance_winner` on-chain → store advances winner → next match gets both players → **game ID auto-assigned** | **Works** |
| **H. Between rounds** | Player should see "you won, waiting for next opponent" | **BROKEN — dead UI** |
| **I. Next round** | Client should pick up the new game ID and re-enter | **BROKEN — same as E** |
| **J. Complete** | Final → `Completed` → prize distributor pays top 3 | **Works** |

Note phase G: the backend **already** slots the winner into their round-2 match
and assigns that match a game ID the moment both feeder matches finish
(`record_result` → `assign_ready_game_ids`). The next match genuinely exists and
is playable. Nothing tells the player.

---

## 3. The gaps, with evidence

### G1 — `TournamentMatchAssignedEvent` has no producer *(fatal)*

Declared at [tournament.rs:263](../../src/multiplayer/solana/tournament.rs#L263),
registered at :639, consumed by `handle_tournament_match_assigned` at :656.
**Zero writers anywhere in `src/`.**

That handler does all the work: session create/join, P2P connect, ER delegation,
`GameState::InGame`. It can never run. This breaks round 1 as well as every
later round — the only reason a 2-player tournament appears to work is that
players enter via other paths.

### G2 — Nothing polls `/my-match` *(fatal)*

`poll_timer` is documented as "seconds since last my-match poll". No code calls
the endpoint. `poll_bracket_fired` depends on `bracket_fired_rx`, fed only by
the gossip subscriber in `TournamentMultiplayerPlugin` — which is never
registered with the Bevy app (F7 in the readiness plan).

### G3 — The waiting room is dead code

`waiting_for_next_match` is assigned `false` exactly once
([:780](../../src/multiplayer/solana/tournament.rs#L780)) and `true` **never**.
Both render sites — [screens.rs:2205](../../src/states/main_menu/screens.rs#L2205)
and [game_ui.rs:1654](../../src/ui/game/game_ui.rs#L1654) — are unreachable. The
panel is complete, styled, and has its copy written.

`last_match_result` is **never assigned at all**, so the "Result: 1-0" line
inside it would be blank even if the panel could show.

### G4 — `active_game_id` is never cleared

Set at :779, never reset. Stale after a match ends; the player's client still
believes it is in the previous game.

### G5 — `found: false` is ambiguous

[`match_for_player`](../../backend/src/signing/storage/tournament.rs#L322) returns
`None` when the next match's opponent slot is still empty (the `?` on
`player_black`). So "you won, your round-2 opponent is still playing" and "you
are not in this tournament" are **the same response**. The client cannot tell a
waiting player from a stranger.

### G6 — No auto-forfeit for single-elimination *(bracket-stalling)*

`tournament_forfeit.rs` starts with `let Some(sd) = t.swiss_data.as_ref() else
{ continue };` — it is **Swiss-only**. In a 16-player single-elim, one player who
closes the app in round 2 stalls that match, and therefore that half of the
bracket, **forever**. There is no deadline either: `round_deadline_at` lives in
`SwissStorageData` and has no single-elim equivalent.

For a 16-player bracket this is close to guaranteed to happen.

### G7 — Private-tournament passwords are unenforced

`/confirm-join` takes `{player, elo, signature}` and checks no password.
`password_hash` is read in exactly two places, both to display an "is_private"
badge. The prompt is decorative.

### G8 — Non-power-of-2 brackets unsupported

`VALID_PLAYER_COUNTS = [2,4,8,16,32,64,128,256]`, enforced on-chain and in the
backend. A 12-player single-elim needs bye support in the bracket math. 16 is
fine; this only bites on odd sizes.

---

## 4. How it should work

The current client is written as if it were **event-driven** (gossip messages,
Bevy events) but has no event source, so it renders nothing. Rather than bolt a
producer onto each dead path, make the client a **thin renderer of one
authoritative backend state**, polled on a timer. One request, one state machine,
one place to debug.

### 4.1 Player state machine

```
NotRegistered
     │ register (on-chain + confirm-join)
     ▼
  Registered ───────── tournament starts ─────────┐
     │                                            │
     │                                            ▼
     │                                      AwaitingOpponent
     │                                    (in bracket, opponent TBD)
     │                                            │ both feeders done
     ▼                                            ▼
 (cancelled)                                 MatchReady
                                             (game_id assigned)
                                                  │ auto-enter
                                                  ▼
                                               Playing
                                                  │ settle
                                    ┌─────────────┴─────────────┐
                                    ▼                           ▼
                               Eliminated                  AwaitingOpponent
                              (show placing)               (won, next round)
                                                                │ final won
                                                                ▼
                                                             Champion
                                                          (prize pending/paid)
```

Every state above maps to UI that **already exists** except `Eliminated` and
`Champion`, which need a small placing/prize panel.

### 4.2 One endpoint to drive it

New `GET /tournament/{id}/my-status?player=<pubkey>` returning everything the
client needs in one call:

```json
{
  "state": "AwaitingOpponent",
  "registered": true,
  "tournament_status": "Active",
  "round": 2,
  "total_rounds": 4,
  "match": { "game_id": 123, "opponent": "…", "your_color": "white", "status": "Pending" },
  "blocked_by": { "round": 2, "matches_remaining": 3 },
  "last_result": { "round": 1, "result": "win", "opponent": "…" },
  "placing": null,
  "prize_lamports": null
}
```

This resolves **G5** by construction: `AwaitingOpponent` and `NotRegistered` stop
being the same response. `blocked_by` gives the waiting room something honest to
show ("3 matches left in round 2") instead of an indefinite spinner.

Keep `/my-match` as-is for backward compatibility; `/my-status` supersedes it.

---

## 5. Implementation

### Phase 1 — Make the client follow the tournament *(fixes G1–G5; the critical path)*

> **IMPLEMENTED 2026-08-21.** Backend: `PlayerState`/`player_status` in
> `storage/tournament.rs` + `GET /tournament/{id}/my-status`, covered by 4 unit
> tests. Client: `poll_my_tournament_status` in `TournamentClientPlugin` now
> emits `TournamentMatchAssignedEvent` (the missing producer), drives
> `waiting_for_next_match` / `last_match_result`, clears `active_game_id`
> between rounds, and renders eliminated/champion panels with placing + prize.
> 209 backend unit tests + 18 e2e green; client compiles clean. Not yet run
> against a live multi-round tournament — see §6 rung 2.

**Backend**
1. Add `GET /tournament/{id}/my-status` per §4.2. Derives from data the store
   already holds — bracket position, match state, completion, placing. No new
   storage.
2. Have `match_for_player` distinguish "assigned but opponent TBD" from "no
   match" internally so `my-status` can report `AwaitingOpponent` precisely.

**Client**
3. New `poll_my_status` system in `TournamentClientPlugin`, on the existing
   `poll_timer` (~3 s while in a tournament, backing off when idle).
4. On `MatchReady` with a `game_id` → **emit `TournamentMatchAssignedEvent`**.
   This gives the dead handler its producer and lights up the entire existing
   path: session setup, P2P, ER, `InGame`. **One line fixes rounds 1 through 4.**
5. On `AwaitingOpponent` → set `waiting_for_next_match = true` and populate
   `last_match_result`, lighting up the panel that is already built.
6. Clear `active_game_id` and `waiting_for_next_match` on entering a new match
   (fixes G4).
7. Add `Eliminated` / `Champion` panels — placing, prize, and a "back to lobby"
   action.

**Done when:** a 4-player single-elim runs start→finish with both clients only
ever clicking "Join" — round 2 begins automatically for the winner, and the
loser sees an elimination screen rather than a silent drop to the menu.

### Phase 2 — Stop brackets stalling *(fixes G6; required before any real 16-player run)*

> **IMPLEMENTED 2026-08-21** (item 8). `tournament_forfeit.rs` now handles
> single-elimination alongside Swiss: it scans the current round's incomplete
> matches with both players seated, forfeits a player offline past
> `FORFEIT_GRACE`, records the result (which advances the winner and assigns
> the next match's game ID), and mirrors `record_result` + `advance_winner`
> on-chain best-effort. Forfeits at most one side per match per tick, so a
> match where *both* players are absent doesn't advance a no-show into the
> next round. Items 9–10 (per-round deadlines, panel surfacing) still open.

8. Generalise `tournament_forfeit.rs` beyond Swiss: for single-elim, scan the
   current round's incomplete matches with both players seated, and forfeit a
   player continuously offline past `FORFEIT_GRACE`.
9. Give single-elim a per-round deadline (mirroring Swiss's
   `round_deadline_at`), with the existing admin `set-round-deadline` route
   extended to cover it. On expiry: forfeit the absent side, or void if neither
   showed.
10. Surface both in the panel — a stalled match should be visible, with a manual
    forfeit override (the admin route already exists).

**Done when:** a 4-player single-elim where one player closes their client
mid-round still completes and pays out, without an operator intervening.

### Phase 3 — Correctness and polish

> **Item 11 IMPLEMENTED 2026-08-21.** The feature was worse than "unenforced" —
> it was inert in *both* directions: `CreateTournamentReq` had no password
> field, so `password_hash` was never written by any route, `is_private` was
> permanently false, and the client's password prompt could never appear.
> Now: `password` on create (argon2-hashed, same scheme as private P2P
> lobbies), verified in `/confirm-join` after signature verification and before
> any roster mutation, passed through from the game client, and settable in the
> admin panel. Covered by `private_tournament_rejects_bad_password`.
>
> **Item 13 ASSESSED, not actioned.** `TournamentMultiplayerPlugin` confirmed
> dead (only hits are its own definition — 590 lines). Left in place but its
> module doc now says so unmistakably, including a warning that it exports a
> `TournamentClientPlugin` **with the same name as the live one**, which is a
> real import footgun. Deleting 590 lines is your call, not mine.

11. ~~Enforce the private-tournament password in `/confirm-join` (**G7**)~~ — done.
12. Non-power-of-2 brackets with byes (**G8**) — contract, backend, bracket math.
    Only needed if you want 12-player events.
13. Decide gossip vs polling (F7): either register `TournamentMultiplayerPlugin`
    and make gossip the live-update transport with polling as fallback, or delete
    it. Phase 1 makes polling authoritative, so deleting is now the cheaper,
    honest option.

---

## 6. Test plan

Each rung must pass before funding the next.

| # | Scenario | First proves |
|---|---|---|
| 1 | 2-player, free entry | Registration + settlement (regression) |
| 2 | 4-player, free | **Round-to-round transition** — the thing that has never worked |
| 3 | 4-player, one player quits mid-round | Phase 2 forfeit path |
| 4 | 8-player, free | 3 rounds; elimination UI at multiple depths |
| 5 | 16-player, free | Target scenario; 4 rounds, 15 matches |
| 6 | 16-player, paid, small entry fee | Escrow, prize split across top 3 |

Rungs 1–2 are local (`just dev2`, two clients). 4+ needs scripted clients or
`fill-bots`.

---

## 7. What you need to decide

1. **Phase 1 now?** It's the critical path and it's mostly wiring — the handler,
   the endpoint, and the UI all exist.
2. **Phase 2 before or after a real 16-player run?** My recommendation: **before.**
   With 16 people, someone closing their laptop is near-certain, and today that
   stalls the bracket permanently with no recovery short of manual admin action.
3. **G7 — enforce the password, or drop the prompt?**
4. **G8 — do you need non-power-of-2 sizes?** If 8/16/32 is fine, skip it.

---

## 8. Why the 2-player test passed

Worth stating plainly so this isn't mistaken for a regression: a 2-player
tournament is a single match which is also the final. Registration completes,
the game runs, settlement pays out, the tournament marks `Completed`. No
round-to-round transition is exercised, no elimination screen, no waiting room,
no forfeit risk across rounds. Everything G1–G6 covers is invisible at N=2 and
mandatory at N=16.
