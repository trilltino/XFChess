# Tournament Production-Readiness Plan

**Goal:** an operator with permission issues a tournament from the admin surface;
players join by **signing an on-chain transaction**; matches are created, played over
P2P + Ephemeral Rollups, advanced through the bracket, and paid out — with no human in
the loop after the tournament starts. Verified locally first, then on Hetzner.

**Status of this document:** written 2026-08-18 against `feat/chromeos-release`.
Every claim below was traced through the code; file:line references are the evidence.

---

## 1. Verdict up front

The **contracts are ready**. The **payout path is ready**. The **game, P2P and ER
layers work**. What is missing is the *connective tissue*: the backend never calls the
registration instruction, never assigns game IDs automatically, and never calls the
Swiss on-chain instructions at all. Three humans-in-the-loop and one unauthenticated
endpoint stand between today and the flow described above.

| Layer | Maturity | Blocking issues |
|---|---|---|
| Solana program | **High** — complete, tested (24 test targets) | Non-power-of-2 brackets unsupported |
| Prize distribution | **High** — automated, anti-cheat gated, approval-gated | Depends on `fund_sol_prize` being called (see F2) |
| Deploy / ops | **High** — UFW, forward-only SSH tunnel, TLS, smoke tests | Admin API key has a debug fallback (F6) |
| Game client (1v1) | **High** — ER delegation, P2P, settlement all live | — |
| Game client (tournament) | **Medium** — polls, enters games, delegates to ER | Blocked on game_id (F3); gossip client is dead code (F7) |
| Backend tournament flow | **Low** — the weak link | F1, F2, F3, F4, F5 |
| Cost model / benchmark | **Medium** — excellent ix coverage, wrong cost formula | Rent omitted entirely (F9) — the dominant cost |

---

## 2. Critical findings

These are ordered by severity. F1–F3 each independently block the target flow.

### F1 — Registration bypasses the smart contract entirely, unauthenticated

`POST /tournament/{id}/join` ([backend/src/signing/routes/tournament.rs:963](../../backend/src/signing/routes/tournament.rs#L963))
accepts `{"player": "<any string>", "elo": <any number>}` and pushes it straight into
the SQLite store. It never calls `register_player`, never touches escrow, never
verifies a signature.

The route is mounted at [infrastructure/router.rs:51-52](../../backend/src/infrastructure/router.rs#L51-L52)
with **no auth layer whatsoever** — not `require_api_key`, not `require_relay_or_jwt`.
Anyone who knows a tournament ID can `curl` arbitrary pubkeys into any bracket, at any
claimed ELO, without paying an entry fee.

The on-chain instruction that does this correctly — ELO range check, cross-shard
duplicate check, capacity check, entry-fee escrow transfer, shard slotting — already
exists and is tested:
[programs/xfchess-game/src/tournament_ix/registration/register.rs](../../programs/xfchess-game/src/tournament_ix/registration/register.rs).
It is simply never invoked. There is no `register_player_ix` call site anywhere in the
backend.

### F2 — The backend and the contract disagree on what the prize pool *is*

Two incompatible models are live simultaneously:

- **On-chain model** (correct, deliberate): the operator locks a guaranteed prize
  *before* registration opens via `fund_sol_prize`/`fund_usdc_prize`. Entry fees are a
  **refundable deposit**, swept to `host_treasury` as operator revenue when the
  tournament starts ([lifecycle/start.rs:165-177](../../programs/xfchess-game/src/tournament_ix/lifecycle/start.rs#L165-L177)).
  Entry fees are explicitly *not* prize money.
- **Backend model**: `join_tournament` accumulates
  `t.prize_pool += entry_fee - platform_fee` ([tournament.rs:1049](../../backend/src/signing/routes/tournament.rs#L1049)) —
  i.e. it believes entry fees *fund* the pool.

Consequence: if the operator never calls `fund_sol_prize`, on-chain `prize_pool` stays
0 while the backend's copy grows with each entrant. `distribute_tournament_prizes` then
hard-fails on `require!(tournament.prize_pool > 0)`
([prizes/distribute.rs:52](../../programs/xfchess-game/src/tournament_ix/prizes/distribute.rs#L52)),
the distributor logs and retries forever, and players who paid see no prize. The
backend's number is also what gates the 5-SOL manual-approval threshold — so the gate
is calibrated against a figure that does not correspond to any real escrow balance.

**The on-chain model is the right one.** The backend must adopt it, not the reverse.

### F3 — A tournament match cannot start without a human

`handle_tournament_match_assigned` ([src/multiplayer/solana/tournament.rs:675-684](../../src/multiplayer/solana/tournament.rs#L675-L684))
bails out with *"game_id is None — skipping session setup"* unless the match already
carries a `game_id`. That field is only ever set by:

- `POST /admin/tournament/{id}/set-match-game-id` ([tournament.rs:826](../../backend/src/signing/routes/tournament.rs#L826)), admin-only; or
- menu option `6` in the `tournament_admin` / `vps_admin` CLIs.

Nothing assigns game IDs automatically. Every single match of every round needs an
operator to type it in. This is the single largest gap between "tournament starts" and
"players actually play."

### F4 — Bracket advancement is manual (single-elim) or absent on-chain (Swiss)

- **Single-elimination**: `POST /admin/tournament/{id}/record-result` does mirror
  `record_match_result` + `advance_winner` on-chain ([tournament.rs:703-747](../../backend/src/signing/routes/tournament.rs#L703-L747)),
  but it is admin-gated and **neither frontend ever calls it**. The settlement worker
  already knows the `tournament_id` of every settled game
  ([tasks/settlement_worker.rs:972](../../backend/src/tasks/settlement_worker.rs#L972)) and would be the
  natural trigger — it just doesn't do it.
- **Swiss**: `record_swiss_result_ix`, `advance_round_ix` and
  `complete_swiss_tournament_ix` have **zero call sites** in the entire backend. Swiss
  tournaments run purely in SQLite; their on-chain `Tournament` account stays frozen at
  `current_round = 0` forever.

Both on-chain mirror calls are also fire-and-forget: a failed tx is logged and
execution continues, so the store and the chain diverge silently with no reconciliation.

### F5 — `SwissOrchestrator` is dead code built for exactly this job

[backend/src/signing/swiss/orchestrator.rs](../../backend/src/signing/swiss/orchestrator.rs) was written to
create games per pairing, sign `record_swiss_result` on game end, and push standings.
Its own module doc admits it is never spawned because no producer sends it an
`OrchestratorEvent`; [tasks/mod.rs:13-24](../../backend/src/tasks/mod.rs#L13-L24) repeats the warning and
asks that it be wired properly rather than blind-spawned. **Most of WS-C is finishing
this file, not writing it.**

### F6 — `ADMIN_API_KEY` silently defaults to `"dev"` in debug builds

[infrastructure/auth_middleware.rs:29-40](../../backend/src/infrastructure/auth_middleware.rs#L29-L40).
Release builds correctly return 503 when unset. Debug builds accept `X-API-Key: dev`.
Acceptable locally; a hazard if a debug binary is ever deployed. Needs an explicit
startup assertion tied to `APP_ENV`, not just build profile.

### F7 — The game's gossip tournament client is dead code

`TournamentMultiplayerPlugin` ([src/multiplayer/tournament/mod.rs](../../src/multiplayer/tournament/mod.rs)) —
the braid-iroh gossip client for live pairings/standings — is **never registered with
the Bevy app**. Only the HTTP-polling `solana::tournament::TournamentClientPlugin` is
live ([src/multiplayer/mod.rs:98](../../src/multiplayer/mod.rs#L98)). "Real-time tournament updates
via gossip" is currently polling. This is a *quality* gap, not a blocker — decide
deliberately whether to wire it (WS-F) or delete it.

### F9 — The cost model omits rent, which *is* the cost

`crates/solana/er-cu-benchmark` is in much better shape than expected on coverage: it
already has instruction builders for **38 instructions**, including
`register_player_ix`, `advance_round_ix`, `complete_swiss_tournament_ix`,
`distribute_tournament_prizes_ix`, `initialize_match_ix` and `advance_winner_ix` — several
of which **the backend itself still lacks** (F1, F4). WS-A/C should lift these rather
than write them twice.

The *cost formula*, however, is wrong in four ways, and the first is severe.

**1. Rent is not modelled at all.** `generate_cost_report`
([cost_reporter.rs:94-100](../../crates/solana/er-cu-benchmark/src/cost_reporter.rs#L94-L100)) totals
exactly three things: base tx fee, a flat priority fee, and ER session fees. Account
rent — the money that actually leaves the signer's wallet — appears nowhere.

For tournaments this inverts the entire picture. Modelled from the real account layouts
(`(128 + data_len) × 6960` lamports, Solana's rent-exempt formula):

| Account | Size | Rent each | Count @ 256 players | Subtotal |
|---|---|---|---|---|
| `TournamentPlayersShard` | 4,765 B | 0.0341 SOL | 4 | **0.136 SOL** |
| `TournamentMatch` | 184 B | 0.00217 SOL | 255 | **0.554 SOL** |
| `Game` | ~366 B | ~0.0034 SOL | 255 | **~0.877 SOL** |
| `Tournament` + escrow | — | — | 2 | ~0.005 SOL |
| **Rent total** | | | | **~1.57 SOL** |
| Tx fees (~1,500 txs × 15k lam) | | | | ~0.023 SOL |
| ER session fees (255 × 300k lam) | | | | ~0.077 SOL |

**Rent is roughly 16× the fees the current report shows, and the report shows only the
fees.** The 0.0341 SOL/shard figure is corroborated by the program's own comment
("1 shard, ~0.034 SOL",
[lifecycle/initialize_shards.rs](../../programs/xfchess-game/src/tournament_ix/lifecycle/initialize_shards.rs)),
which is good evidence the formula is right. These are *modelled* numbers to be
replaced by measured ones in WS-D — not measurements.

**2. Rent refunds are not modelled either.** `close_tournament` and `finalize_game`
return rent to the payer. Gross cost and net cost differ by most of that 1.57 SOL, and
an operator needs both: gross = working capital required up front, net = actual cost
per tournament.

**3. The `paid_instructions` allowlist is stale.**
[cost_reporter.rs:55-72](../../crates/solana/er-cu-benchmark/src/cost_reporter.rs#L55-L72) omits
`initialize_match`, `advance_winner`, `advance_round`, `complete_swiss_tournament`,
`distribute_tournament_prizes`, `fund_sol_prize`, `close_tournament`,
`leave_tournament`, `claim_tournament_prize`, `withdraw_treasury`, the
`initialize_shards_small/medium` variants, and both `global_*` game instructions. Any
instruction not on this list is silently counted as **free** — so all 255
`initialize_match` calls in a 256-player bracket currently contribute zero to the total.

**4. Priority fee arithmetic is wrong.** The report charges a flat `10_000` lamports per
tx ([cost_reporter.rs:95](../../crates/solana/er-cu-benchmark/src/cost_reporter.rs#L95)), but the
transactions actually set `DEFAULT_CU_PRICE = 10_000` **micro-lamports per CU**
([lib.rs:121](../../crates/solana/er-cu-benchmark/src/lib.rs#L121)). Real priority fee is
`cu_price × cu_used ÷ 1_000_000` — about 2,000 lamports for a 200k-CU transaction, not
10,000. The model over-charges cheap instructions and under-charges expensive ones.

**5. ER move costs: two models that contradict each other, neither measured.**

The benchmark asserts ER transactions are free — *"Base fee per TX on ER: 0 lamports"*
([cost_reporter.rs:4-6](../../crates/solana/er-cu-benchmark/src/cost_reporter.rs#L4-L6)) — and
`record_move` is absent from `paid_instructions`, so every move on the rollup
contributes exactly zero to the report.

The **program disagrees**. `apply_recorded_move` charges `RECORD_RESULT_COST` = 5,000
lamports into `Game.fees_advanced` on **every single move**
([moves_ix/apply.rs:54-57](../../programs/xfchess-game/src/moves_ix/apply.rs#L54-L57)), and
`mark_undelegated` adds `UNDELEGATE_COST` + `ER_SESSION_FEE_LAMPORTS`
([lifecycle/transitions.rs:62-72](../../programs/xfchess-game/src/lifecycle/transitions.rs#L62-L72)).
A 40-move game therefore accrues:

```
create 5,000 + join 5,000 + delegate 5,000
      + 40 moves × 5,000 = 200,000
      + undelegate 5,000 + ER session 300,000
      = 520,000 lamports (0.00052 SOL) per game
```

One model says a move costs 5,000 lamports; the other says it costs 0. **Both are
assumptions — neither has ever been measured.** `constants.rs` says so explicitly of
the session fee: *"an ER infrastructure cost, not a Solana base-layer tx fee, so it's
invisible to any on-chain instruction."* Invisible means unverified.

Three consequences:

- **The CU logger tracks only compute units.** `cu_logger.rs` records `cu_consumed` and
  nothing else — no pre/post lamport balances anywhere. Nothing in the benchmark
  measures money actually leaving a wallet, on either layer.
- **Tournament games silently drop the accrual entirely.** Tournament matches are
  created with `wager_amount = 0`
  ([src/multiplayer/solana/tournament.rs:767](../../src/multiplayer/solana/tournament.rs#L767)), and
  `settle_finished_game` skips its whole payout block when `wager_amount == 0` — the
  comment states plainly that `fees_advanced` "is simply never reimbursed — the account
  just closes with that data discarded"
  ([lifecycle/settlement.rs:47-53](../../programs/xfchess-game/src/lifecycle/settlement.rs#L47-L53)).
  So for **every tournament game** the operator absorbs the real ER cost while the
  on-chain accounting records it and throws it away. At 255 games that is ~0.13 SOL of
  genuine cost appearing in no ledger anywhere.
- **No term for game duration.** `ER_COMMIT_FREQUENCY_MS = 30_000` means the rollup
  commits state to the base layer every 30 seconds while a game is delegated. A 3-minute
  blitz game is ~6 commits; a 30-minute rapid game is ~60. The 300,000-lamport session
  fee is modelled as **flat per undelegation**, with no dependence on how long the game
  was delegated or how many commits occurred. If commit cost is real and per-commit,
  cost scales with wall-clock duration and the model has no variable for it — which
  matters enormously for tournaments, where time control is an operator choice.

**Also stale:** `SOL_GBP_RATE = 60.0` is hardcoded
([lib.rs:130](../../crates/solana/er-cu-benchmark/src/lib.rs#L130)) while the backend already
maintains a live rate (`signing/routes/rates.rs`). And there is no **per-signer**
attribution — the user-facing question is "what does this cost *the operator* vs *each
player*", and the report emits one aggregate number.

### F8 — Non-power-of-2 brackets unsupported

`VALID_PLAYER_COUNTS = [2,4,8,16,32,64,128,256]` is enforced in both the contract
([lifecycle/initialize.rs:13](../../programs/xfchess-game/src/tournament_ix/lifecycle/initialize.rs#L13)) and the
backend ([tournament.rs:263](../../backend/src/signing/routes/tournament.rs#L263)). A 12-player
single-elimination bracket needs bye support in the bracket math. Swiss can already run
12 players today via `max_players = 16, min_players = 12`.

---

## 3. What "done" looks like

```
OPERATOR (holds admin credential + vps_authority key)
  │
  ├─ 1. POST /admin/tournament/create ──────────► initialize_tournament
  │                                               initialize_escrow
  │                                               initialize_shards
  │
  ├─ 2. POST /admin/tournament/{id}/fund-prize ─► fund_sol_prize   ◄── NEW, gates registration
  │
  └─ 3. Registration opens (status: Registration)

PLAYER (holds wallet)
  │
  ├─ 4. GET  /tournament/{id}/build-register-tx ─► unsigned register_player tx  ◄── NEW
  ├─ 5. signs in wallet, submits to chain ───────► escrow += entry_fee
  └─ 6. POST /tournament/{id}/confirm-join ──────► backend verifies tx on-chain,  ◄── NEW
                                                   reconciles store from chain

SCHEDULER (automatic)
  ├─ 7. fill or scheduled_at reached ────────────► start_tournament (sweeps fees)
  ├─ 8. initialize_match × N                      (already implemented)
  └─ 9. auto-assign game_id + create Game PDAs ──► session_create_game/join  ◄── NEW

PLAY (automatic, existing infrastructure)
  ├─ 10. clients enter game, connect via Iroh P2P, delegate to ER
  ├─ 11. moves recorded on ER (sub-second)
  └─ 12. checkmate/resign/timeout → undelegate → finalize_game

ADVANCE (automatic)
  ├─ 13. settlement worker sees settled tournament game ◄── NEW trigger
  ├─ 14a. single-elim → record_match_result + advance_winner
  ├─ 14b. Swiss     → record_swiss_result → advance_round → complete_swiss_tournament
  └─ 15. next round's matches get game IDs → back to step 10

PAYOUT (automatic, already implemented)
  └─ 16. prize distributor → anti-cheat gate → distribute_tournament_prizes
```

Six new pieces (marked NEW). Everything else exists.

---

## 4. Permission & authority model

This is the part to get right before writing any code, because it determines every
route signature.

### 4.1 The four principals

| Principal | Credential | May do | Enforced by |
|---|---|---|---|
| **Operator (human)** | `ADMIN_API_KEY` + SSH tunnel | create/cancel/fund tournaments, override results, approve large payouts | `require_api_key` + network isolation |
| **VPS authority (machine)** | `vps_authority` keypair | sign `initialize_*`, `start_tournament`, `initialize_match`, `record_match_result` | on-chain `constraint = authority.key() == vps_authority::ID` |
| **Player** | wallet signature / JWT | `register_player`, `leave_tournament`, play moves | on-chain `Signer` + `require_relay_or_jwt` |
| **Anyone (crank)** | none | `advance_round`, `complete_swiss_tournament`, `distribute_tournament_prizes` | safe by construction — on-chain state proves preconditions |

The permissionless crank tier is a deliberate strength: a tournament cannot be frozen
by the operator going offline. Preserve it.

### 4.2 Three layers of gating for admin instructions

1. **Network** — `/admin/*` is never publicly routable. UFW blocks 8090; nginx does not
   proxy `/admin`. The Tauri admin panel reaches it through a forward-only SSH tunnel
   user (`PermitOpen 127.0.0.1:8090`, nologin shell) set up at
   [ops/scripts/deploy.ps1:164-196](../../ops/scripts/deploy.ps1#L164-L196). This is already correct.
2. **Application** — `require_api_key` (constant-time compare) on every admin router.
   Already correct except F6.
3. **Chain** — the ultimate authority. Even a fully compromised backend cannot forge a
   result it isn't authorised for, cannot drain escrow (destinations are constrained to
   recorded winners), and cannot touch the program itself (upgrade authority is a
   separate key). Five pairwise-distinct authorities, CI-enforced
   ([constants.rs:213-232](../../programs/xfchess-game/src/constants.rs#L213-L232)).

### 4.3 Required changes

- **P0** Add `require_relay_or_jwt` to `tournament_routes()` and assert the JWT's wallet
  matches the `player` field in the body. Today a valid credential for one wallet — or
  none at all — can act as any wallet.
- **P0** Startup assertion: if `APP_ENV=production`, refuse to boot without a
  `ADMIN_API_KEY` of ≥32 chars. Kills F6 regardless of build profile.
- **P1** Rate-limit `build-register-tx` per wallet.
- **P1** Audit-log every admin mutation (who, what, when, resulting tx signature) to a
  new `admin_audit` table. Currently admin actions leave only tracing logs.
- **P2** Move `vps_authority` to a hardware-backed or at minimum separately-rotated key
  before mainnet. Per existing project memory the devnet keys were exposed in git
  history and still need rotation.

---

## 5. Workstreams

Ordered by dependency. WS-A and WS-B are the critical path.

### WS-A — Make registration real *(P0, ~2–3 days)*

Turns F1 + F2 into the signed on-chain flow.

1. **`GET /tournament/{id}/build-register-tx?player=<pubkey>`** — returns an unsigned
   `register_player` transaction. Mirror the existing `build-leave-tx` handler
   ([tournament.rs:1598](../../backend/src/signing/routes/tournament.rs#L1598)) — the account-list
   assembly (tournament PDA, profile PDA, escrow PDA, shards 0–3, host_treasury) is the
   same shape. Add `register_player_ix` to `signing/solana/instructions.rs`.
2. **`POST /tournament/{id}/confirm-join`** — takes a tx signature, confirms it on
   chain, reads the resulting `TournamentPlayersShard`, and reconciles the SQLite record
   **from chain state**. The store stops being a source of truth for the player list.
3. **Delete the prize-pool accumulation** at [tournament.rs:1049](../../backend/src/signing/routes/tournament.rs#L1049).
   Replace the store's `prize_pool` with a value read from the on-chain `Tournament`
   account so the distributor's threshold logic operates on real escrow.
4. **`POST /admin/tournament/{id}/fund-prize`** — new admin route wrapping
   `fund_sol_prize`. Registration must be impossible until this has run for any paid
   tournament (the contract already enforces this — the backend must stop pretending
   otherwise).
5. Keep the ELO/ban/KYC gates that currently live in `join_tournament`; run them
   *before* handing out the unsigned tx, so a rejected player never pays gas.
6. Add auth per §4.3.

**Done when:** a player with no backend credential cannot appear in a bracket, and a
player who signs the tx appears in both the shard PDA and the store with matching data.

### WS-B — Automate game creation *(P0, ~2–3 days)*

Kills F3, the "human types every game ID" gap.

1. On `start_tournament` success and on every subsequent round advance, for each
   `TournamentMatch` that has both players and no `game_id`: derive a deterministic
   `game_id` (e.g. `hash(tournament_id, round, match_index)` truncated to u64 — must be
   collision-free and reproducible so both clients agree), persist it via the existing
   `set_match_game_id` store method, and mirror on-chain.
2. Drive `session_create_game` / `session_join_game` from the scheduler rather than
   from each client's `IoTaskPool` task, or at minimum make the client path idempotent
   and retried — today a dropped call leaves one side without a session key.
3. Ensure `authorize_tournament_session` has run for both players before their first
   match. This is what makes tournament play popup-free; it currently has no automatic
   trigger point.

**Done when:** a tournament started with 4 registered players produces 2 playable
matches with zero operator input.

### WS-C — Automate bracket advancement *(P0, ~3–4 days)*

Kills F4 + F5.

1. **Add the missing producer.** In `settlement_worker`, after a game with
   `tournament_id = Some(id)` reaches `Settled`, emit the result. This is the one change
   that resurrects `SwissOrchestrator` — it already knows what to do with a `GameEnded`
   event.
2. **Single-elim path:** call `record_match_result` + `advance_winner` from that
   trigger instead of from the admin route. Keep the admin route as a manual override.
3. **Swiss path:** wire `record_swiss_result_ix`, then have the round-completion check
   call `advance_round_ix`, then `complete_swiss_tournament_ix` on the final round. All
   three instructions exist and are tested; they need call sites and instruction
   builders in `signing/solana/instructions.rs`.
4. **Replace fire-and-forget with durable retry.** Add a small `pending_onchain_tx`
   table: enqueue, retry with backoff, alert on repeated failure. No tournament
   state-changing tx should be lost because one RPC call timed out.
5. **Reconciliation job.** Periodically diff on-chain `Tournament`/`TournamentMatch`
   against the store and heal drift. `sync_tournament_status`
   ([tournament.rs:1712](../../backend/src/signing/routes/tournament.rs#L1712)) already does the read —
   it just needs to run on a timer instead of on an admin POST.

**Done when:** a 4-player single-elim tournament and a 4-player Swiss tournament both
run start→finish with no admin calls after creation, and on-chain state matches the
store at every step.

### WS-D — Cost model & capacity prediction *(P0, ~3–4 days)*

Fixes F9 and produces the deliverable: **an accurate, per-signer cost prediction for
each bracket size (8/16/32/64/128/256), validated against measured devnet reality.**

**D.1 — Fix the cost formula** (`cost_reporter.rs`)
1. **Add rent accounting.** Every `init`-ing instruction must record the rent it paid.
   Read it from the transaction's pre/post balances rather than recomputing — that way
   the number is measured, not modelled, and stays correct if account layouts change.
2. **Track rent refunds separately** so the report emits both `gross_sol` (capital
   required) and `net_sol` (cost after `close_tournament`/`finalize_game` refunds).
3. **Replace the `paid_instructions` allowlist with a denylist.** The current
   fail-open design silently zero-rates any instruction someone forgets to add. Invert
   it: everything on the base layer is paid unless explicitly marked ER-free. This
   makes the *next* new instruction correct by default.
4. **Fix priority-fee math** to `cu_price × cu_used ÷ 1_000_000`.
5. **Per-signer attribution.** Tag each logged tx with its fee payer and group the
   report by principal: `vps_authority` (operator), each player wallet, cranker. This is
   the number that actually answers "what will this cost the signers."
6. **Live SOL/GBP rate** from the backend's `rates.rs` rate cache, with the hardcoded
   60.0 as an explicit, labelled fallback.

**D.1b — Measure the ER layer empirically** *(the part with no existing ground truth)*

Everything about ER cost is currently assumed. Replace each assumption with a
measurement, in this order:

1. **Balance-delta instrumentation.** Extend `cu_logger` to snapshot lamport balances
   around every transaction — fee payer and session key, on **both** layers. This single
   change converts the entire suite from "CU meter" to "cost meter" and is the
   prerequisite for everything below.
2. **Is a move actually free?** Delegate a game, record N moves (N = 10, 50, 200), and
   diff the session key's ER balance. This settles the 5,000-vs-0 contradiction with
   data. Run at several N so a per-move cost is distinguishable from a fixed one.
3. **Is the session fee flat?** Vary time-delegated independently of move count — hold a
   game delegated for 1, 5 and 30 minutes with identical move counts, then compare. If
   the 300,000-lamport figure moves with duration, it is a per-commit cost
   (`ER_COMMIT_FREQUENCY_MS`) and the model needs a duration term. If it holds flat,
   the current constant is vindicated and we can say so with evidence.
4. **Who actually pays commits?** Identify whether base-layer commit transactions during
   delegation debit our fee payer, the ER validator, or neither. This is the largest
   unknown in the whole model.
5. **Reconcile `fees_advanced` against reality.** Compare the program's accrued figure
   to measured lamport movement per game. If they diverge, correct the constants
   (`RECORD_RESULT_COST`, `ER_SESSION_FEE_LAMPORTS`) — they are reimbursement amounts
   taken from real wager pots in 1v1, so an inaccurate constant over- or under-charges
   real players at settlement.
6. **Price the tournament free-game absorption.** Quantify what the operator eats per
   tournament given `fees_advanced` is discarded for zero-wager games, and surface it as
   an explicit line item in the report rather than an invisible loss. Then decide
   deliberately: absorb it, or fund tournament games from the entry-fee sweep.

**Done when:** every ER constant in `constants.rs` is annotated with the measurement
that justifies it and the date it was taken, and moves are priced from data rather than
from a doc comment.

**D.2 — Extend scenario coverage**
- `run_swiss_tournament_flow` exists and is solid; add a
  `run_single_elimination_flow` covering `initialize_match` × N, `record_match_result`,
  `advance_winner` through every round — the path with the most transactions and
  currently zero cost coverage.
- Parameterise both over `{8, 16, 32, 64, 128, 256}`.

**D.3 — Build the predictive model**
Derive a closed-form estimator from the measured per-instruction costs:

```
cost(N, format, moves_avg, time_control) =
      rent_fixed
    + rent_shards(N)              # ceil(N/64) × 0.0341 SOL
    + rent_matches(N)             # (N-1) × measured
    + rent_games(N)               # (N-1) × measured
    + tx_fees(N, format)          # per-instruction measured × count
    + er_move_cost(N, moves_avg)  # ← from D.1b.2, currently unknown
    + er_session_fees(N, time_control)  # ← flat or duration-scaled, from D.1b.3
    - rent_refunds(N)
```

Note the last two terms take `moves_avg` and `time_control` as inputs. The current
model has neither variable, which is precisely why it cannot answer "does a 30-minute
rapid tournament cost more than a 3-minute blitz one." Until D.1b lands, both terms are
placeholders.
Ship it as a library function plus a `predict-cost` binary, so the operator can ask
"what does a 64-player Swiss cost me?" **before** funding anything. Then validate:
predicted vs measured must agree within a stated tolerance (target ±5%) at every size
actually run.

**D.4 — Wire predictions into the product**
- `POST /admin/tournament/create` returns the predicted operator cost and required
  working capital, and **refuses to create** a tournament the `vps_authority` wallet
  cannot afford. Running out of SOL midway through `initialize_match × 255` leaves a
  half-built bracket — a failure mode worth designing out rather than debugging later.
- Registration info surfaces the predicted per-player cost so entry fees can be set
  above it.

**Done when:** `predict-cost --players 256 --format single-elim` matches a real devnet
run within tolerance, broken down per signer.

### WS-E — Local end-to-end verification *(P0, ~2 days)*

Cannot be skipped — F1–F4 mean this flow has never actually run unattended.

1. Extend `backend/tests/swiss_tournament_e2e.rs` to cover the new signed-registration
   path against `solana-program-test`.
2. New integration test: full 4-player single-elim, driven only by the scheduler,
   asserting on-chain state after each transition.
3. Manual multi-client run — see §6.

### WS-F — Hetzner staging + production *(P0, ~1–2 days)*

See §7.

### WS-G — Gossip vs polling decision *(P1, ~2 days)*

Resolve F7. Either register `TournamentMultiplayerPlugin` and make gossip the primary
pairing/standings transport (falling back to polling), or delete the module. Leaving
two parallel implementations where one is silently dead is the worst of both. The
backend side (`tournament_gossip.rs`, `BracketFired` broadcast) is already live, so
wiring is the smaller job — but it needs real two-client testing.

### WS-H — Hardening & polish *(P1–P2)*

- Admin audit log (§4.3).
- Non-power-of-2 brackets with byes (F8) — contract + backend + bracket math.
- Rotate the exposed devnet authority keys before any mainnet consideration.
- Runbook update: [docs/runbooks/tournament-lifecycle.md](../runbooks/tournament-lifecycle.md) still
  describes the manual flow and will be wrong once WS-A–C land.

---

## 6. Local verification plan

Run in this order. Do not proceed to Hetzner until every step passes.

### Phase 1 — contract level
```bash
cargo build-sbf                              # required before the -test suites
cargo test -p xfchess-game                   # all 24 test targets
cargo test -p xfchess-game --test tournament_registration_e2e_tests
cargo test -p xfchess-game --test tournament_swiss_completion_tests
```

### Phase 2 — backend level
```bash
cargo test -p backend
cargo run --bin signing-server               # devnet RPC, local SQLite
```
Then, with `curl`, verify the **negative** cases first — these are the F1 regressions:
- `POST /tournament/1/join` with a bogus pubkey and no auth → **must now 401**.
- `GET /tournament/1/build-register-tx` for an ELO-ineligible player → **must 403 before
  returning a tx**.
- `register_player` submitted for a tournament with no funded prize → **must fail
  on-chain with `PrizeNotFunded`**.

### Phase 3 — full local stack, 2 clients, 2-player tournament
```bash
scripts\run_offline.bat                      # backend + monitoring
cargo run --features solana                  # client A
cargo run --features solana                  # client B (second machine or profile)
```
Checklist:
- [ ] Operator creates + funds a 2-player tournament via the admin panel/CLI
- [ ] Both clients see it in the tournament list
- [ ] Both register via wallet signature; escrow balance increases by 2 × entry fee
- [ ] Scheduler auto-starts; entry fees sweep to `host_treasury`
- [ ] `initialize_match` fires; **game_id assigned automatically** (WS-B)
- [ ] Both clients enter the game; Iroh P2P connects; `requires_delegation` triggers ER
      delegation ([src/multiplayer/solana/tournament.rs:769](../../src/multiplayer/solana/tournament.rs#L769))
- [ ] Moves land on the ER sub-second; `MoveEvent`s visible in logs
- [ ] Checkmate → undelegate → `finalize_game` → `Settled`
- [ ] **Advancement fires automatically** (WS-C) → tournament `Completed`
- [ ] Prize distributor pays the winner within one 60s tick
- [ ] `/metrics` shows no stuck workers

### Phase 4 — 4-player, both formats
Repeat Phase 3 with 4 players, single-elim (2 rounds) and Swiss (3 rounds). This is the
first test that exercises **round-to-round** advancement, which is the part that has
never run unattended.

Watch specifically for: session-key expiry between rounds (2h per-game delegation), ER
re-delegation on round 2, and the `round_boards_reported` bitmap clearing correctly.

Run every phase with the CU logger attached (WS-D) so these local runs already produce
cost data — rungs 1–3 of the escalation ladder in §8.2 are exactly these scenarios, and
there is no reason to run them twice.

---

## 7. Hetzner deployment plan

The ops tooling is genuinely mature — the risk here is procedural, not technical.

### 7.1 Staging first
[docs/ENVIRONMENTS.md](../ENVIRONMENTS.md) already specifies a staging slice on the same box
(separate systemd unit, port 8091, separate DB dir, `APP_ENV=production`). **Stand this
up before touching prod** — the tournament changes touch money movement, and staging is
where a wrong `host_treasury` gets caught.

```powershell
ops\scripts\deploy.ps1 -Environment staging
```

### 7.2 Pre-deploy gates
- [ ] `ADMIN_API_KEY` set, ≥32 chars, unique per environment (F6 fix asserts this)
- [ ] `RELAY_SHARED_SECRET` set — otherwise the dual-accept guard is JWT-only
- [ ] `vps_authority` funded with devnet SOL (it pays rent for Tournament/Escrow/Shards)
- [ ] `host_treasury` is the intended wallet — **verify by pubkey, not by variable name**
- [ ] Prize-release threshold reviewed (`PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS`, default 5 SOL)
- [ ] Program redeployed to devnet if any contract change landed — several fixes in
      project memory (ER recovery, Swiss completion) are **built and tested but never
      devnet-deployed**; confirm the deployed program matches `HEAD`

### 7.3 Deploy
```powershell
ops\scripts\deploy.ps1 -Domain xfchess.com
```
Then the mandatory smoke test — and per project memory, **verify `git_sha`, not just
that the service is "active"**; a stale duplicate systemd unit once served old code for
two hours while reporting healthy:
```bash
curl -fsS https://xfchess.com/health | grep -q '"status":"ok"'   # check the SHA field
curl -fsS -o /dev/null -w '%{http_code}' https://xfchess.com/readyz
```

### 7.4 Admin access on prod
The admin panel does **not** reach `/admin/*` over the public internet. Open the
forward-only tunnel, then point the panel at the local port:
```bash
ssh -N -L 8091:127.0.0.1:8090 tunnel@<host>
```
Verify from the box that `/admin/*` is genuinely unreachable publicly:
```bash
curl -sS -o /dev/null -w '%{http_code}' https://xfchess.com/admin/tournament/1   # expect 404/502, never 200
```

### 7.5 First production tournament
Run a **free-entry, zero-prize 2-player tournament** on devnet first. No money at risk,
but it exercises the entire path including the on-chain calls. Only after that passes
should a paid tournament be created — and the first paid one should have an entry fee
small enough that a total loss is acceptable.

---

## 8. The culminating test — full-scale cost validation

This is what the whole plan builds toward: **one real, instrumented run of every bracket
size against live devnet, producing a cost table accurate enough to price entry fees.**
Nothing before this point proves the economics; this does.

### 8.1 Preconditions
- WS-A through WS-E complete; staging green.
- `predict-cost` produces a number for every size (WS-D.3).
- Benchmark master wallet funded. **Budget from the prediction, then double it** — a
  256-player run is modelled at ~1.6 SOL gross before refunds, and a failed run
  mid-bracket strands rent in accounts you must then reclaim.
- Devnet program matches `HEAD` (per §7.2 — several fixes in project memory are built
  and tested but never devnet-deployed).

### 8.2 The escalation ladder
Run in strict order. **Each rung must land within tolerance of its prediction before
funding the next** — this is what stops a formula error from being discovered at 256
players with real SOL committed.

| Rung | Players | Format | What it first proves |
|---|---|---|---|
| 1 | 2 | single-elim | End-to-end path; baseline per-instruction CU |
| 2 | 4 | single-elim | Multi-round advancement; `advance_winner` |
| 3 | 4 | Swiss | `record_swiss_result` → `advance_round` → `complete_swiss` on-chain |
| 4 | 8 | single-elim | 3 rounds; prediction model's first real extrapolation test |
| 5 | 16 | single-elim | Batching behaviour of `initialize_match` (20/tx) |
| 6 | 32 | Swiss | Shard-boundary-adjacent standings math |
| 7 | 64 | single-elim | **Single-shard ceiling** (`SHARD_CAPACITY = 64`) |
| 8 | 128 | single-elim | **2-shard path** (`initialize_shards_medium`) |
| 9 | 256 | single-elim | **4-shard path**; 255 matches; peak cost |

Rungs 7–9 are the ones that have never run. The shard-count transitions (64→128→256)
are where the cost curve steps rather than scales smoothly, and where
`initialize_shards_small`/`medium`/full diverge.

### 8.3 What each run must capture
Per rung, emit a JSON artifact containing:
- Total CU, per-instruction CU (min/max/avg — not just avg; the max is what sets the
  CU limit you must request).
- **Rent paid, rent refunded, net rent**, per account type.
- Tx fees and priority fees, computed with the corrected formula.
- **Measured** ER cost: session-key balance delta per game, split into per-move and
  per-session components; number of undelegations; number of commits observed.
- **Total moves played** and mean moves/game — the ER cost driver.
- Reconciliation: on-chain `fees_advanced` vs measured lamport movement, per game.
- **Per-signer totals**: operator, each player, cranker.
- Predicted vs actual, with % delta.
- Wall-clock duration per phase (registration, start, per round, payout), and
  **per-game delegated duration** — needed to separate flat from per-commit ER cost.
- Any tx that failed and had to be retried — retries are real cost.

### 8.4 The deliverable
A committed cost table, checked into the repo so it can be diffed when the program
changes:

| Players | Format | Operator gross | Operator net | ER cost | Per player | Duration |
|---|---|---|---|---|---|---|
| 8 | single-elim | | | | | |
| 16 | single-elim | | | | | |
| 32 | Swiss | | | | | |
| 64 | single-elim | | | | | |
| 128 | single-elim | | | | | |
| 256 | single-elim | | | | | |

Plus: minimum entry fee that covers per-player cost at each size, the working capital
the operator must hold before opening registration, and — if D.1b.3 shows ER cost scales
with delegated duration — a **cost-by-time-control** table, since that turns time control
from a purely sporting choice into an economic one.

### 8.5 Failure handling
If a rung diverges from prediction by more than tolerance, **stop and fix the model** —
do not proceed up the ladder. A prediction that is wrong at 32 players will be
catastrophically wrong at 256, and the whole point of the ladder is to catch that while
it is still cheap.

Keep `consolidate_funds` (already in the benchmark's binaries) ready to sweep child
wallets after each rung so SOL is not stranded across dozens of test keypairs.

---

## 9. Acceptance criteria

The flow is production-ready when all of these hold:

1. An unauthenticated `POST /tournament/{id}/join` returns 401. *(F1)*
2. A player appears in a bracket **only** after an on-chain `register_player`. *(F1)*
3. The store's prize pool equals the on-chain escrow's prize portion. *(F2)*
4. A tournament with 4+ players runs start→payout with **zero admin calls** after
   creation and funding. *(F3, F4)*
5. Both formats reach `Completed` **on-chain**, not just in SQLite. *(F4)*
6. Killing the backend mid-round does not lose tournament state — on restart,
   reconciliation heals the store from chain, and a third-party crank can still advance
   the round. *(F5)*
7. Every admin mutation is attributable in an audit log. *(§4.3)*
8. `/health` reports the deployed `git_sha` matching the intended commit. *(§7.3)*
9. `predict-cost` agrees with a measured devnet run within ±5% at every bracket size
   from 8 to 256, broken down per signer. *(F9, §8)*
10. `create` refuses to open a tournament the operator wallet cannot fund. *(WS-D.4)*
11. Every ER cost constant is backed by a dated measurement, and the on-chain
    `fees_advanced` accrual reconciles with measured lamport movement. *(F9.5, WS-D.1b)*

---

## 10. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Store↔chain drift during migration | Players in one, not the other | Chain is authoritative; reconciliation job (WS-C.5); staging first |
| Deterministic `game_id` collision | Two matches share a Game PDA | Include tournament_id + round + match_index; assert uniqueness on insert |
| Session-key expiry between rounds | Wallet popup mid-tournament, or a stalled match | Test explicitly in Phase 4; consider re-authorising at round start |
| ER validator unavailable mid-tournament | Match stuck delegated | `request_force_undelegate` path exists and is tested — but **verify it's deployed to devnet** |
| Entry fees swept before a round fails | Players paid, tournament broken | `cancel_tournament` refunds from `host_treasury` (needs its co-signature) — rehearse this on staging |
| Exposed devnet authority keys | Forged results, drained fees | Rotate before mainnet; per memory this is still open |
| Operator wallet drains mid-bracket at 256 players | Half-built tournament, stranded rent | Pre-flight affordability check (WS-D.4); escalation ladder (§8.2) |
| Cost model wrong at scale | Entry fees priced below cost — every tournament loses money | Ladder stops on first out-of-tolerance rung (§8.5) |
| ER move cost is non-zero after all | Long games cost far more than modelled; blitz vs rapid economics differ | Measure before pricing (WS-D.1b.2); treat 0 as unproven until then |
| ER session fee scales with delegated time | Rapid/classical tournaments uneconomic at blitz-derived prices | D.1b.3 varies duration independently of move count |
| `fees_advanced` constants are wrong | 1v1 players over/under-charged from their own wager pot at settlement | D.1b.5 reconciles accrual against measurement |

---

## 11. Sequencing summary

```
WS-A (registration) ──┐
                      ├──► WS-E (local e2e) ──► WS-F (staging) ──► WS-F (prod)
WS-B (game creation) ─┤                                                 │
WS-C (advancement) ───┘                                                 ▼
                                                          §8 CULMINATING TEST
WS-D (cost model) ────────────────────────────────────────────────►  (8→256)

WS-G (gossip), WS-H (hardening) — parallel, non-blocking
```

Critical path: **WS-A → WS-B → WS-C → WS-E → WS-F → §8**. Roughly 13–18 focused days.

WS-A, WS-B and WS-D are mutually independent and can run in parallel — **WS-D can start
immediately**, since fixing the cost formula needs no backend changes and its
instruction builders are already written. Starting it first also de-risks the rest: it
is the workstream whose output determines whether the economics work at all.
