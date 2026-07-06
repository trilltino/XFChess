# puzzle — server-verified tactics mode

Client-side puzzle play loop. The client is deliberately a **renderer and input
collector, never the judge**: every solution move is verified server-side, one move at
a time, so the full solution line never reaches the client until it has been earned.

## Flow

1. The menu sets `PendingPuzzleRequest`, which triggers `GET /puzzle/next` against the
   backend.
2. On load, the position spawns from FEN, the app transitions to `InGame`, and the
   opponent's setup move plays out so the player is facing the tactic.
3. Each player move posts to `POST /puzzle/move`. The server verifies it and returns
   either the opponent's reply (correct — revealed only now), a retry (wrong), or
   completion.
4. Completion updates the player's puzzle rating server-side.

## Why server-authoritative

Keeping verification on the backend means puzzle solutions are not embedded in the
client binary, ratings can't be farmed by inspecting network payloads ahead of time,
and the same puzzle API serves any future client (web, mobile) identically.

## Contents

Single-module implementation (`mod.rs`): request/response types matching the backend
puzzle routes, the pending-request resource, and the systems driving steps 1–4 above,
all gated to the puzzle game state.
