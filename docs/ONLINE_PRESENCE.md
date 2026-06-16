# Online Presence — tracking & displaying active players

How XFChess counts who's online on the VPS backend and shows that count in the
game's main menu.

## Overview

```
Game client ──PUT /presence (heartbeat, every ~15s)──▶ Backend (VPS)
            ──GET /presence (poll count, every ~15s)──▶ PresenceStore (in-memory)
            ◀──── Vec<Presence> (online in last 5 min) ──
Main menu  ── reads OnlinePlayersState resource ──▶ "● N online"
```

Presence is **in-memory** on the backend (a `HashMap` behind an `RwLock`), with a
5-minute TTL. A player counts as online for 5 minutes after their last heartbeat.

## Backend (already complete)

- [backend/src/signing/social/presence.rs](../backend/src/signing/social/presence.rs)
  — `PresenceStore` with `upsert`, `get_all_online` (filters to entries updated in
  the last 5 min and not `Offline`), `set_offline`, `sweep_stale`.
- [backend/src/signing/social/routes.rs](../backend/src/signing/social/routes.rs)
  — wired into the router as:
  - `PUT /presence` → upsert (the heartbeat). Body is a JSON `Presence`.
  - `GET /presence` → returns `Vec<Presence>` of everyone currently online.
- Mounted in [backend/src/infrastructure/router.rs](../backend/src/infrastructure/router.rs).

### `Presence` shape

```jsonc
{
  "node_id": "…",            // stable identity (Iroh node id)
  "pubkey": "…",             // optional Solana pubkey
  "display_name": "King_dev",
  "status": "online",        // "online" | "in_game" | "offline"
  "game_id": null,           // set when status == "in_game"
  "updated_at": "2026-06-15T12:00:00Z"  // RFC3339; the server filters on this
}
```

> The backend `upsert` stores the body verbatim, so the client **must** send a
> valid RFC3339 `updated_at` or the PUT fails to deserialize.

## Game client

### HTTP helpers — [src/multiplayer/network/vps/social.rs](../src/multiplayer/network/vps/social.rs)

- `update_presence(&Presence)` → `PUT /presence` (heartbeat)
- `get_online() -> Result<Vec<Presence>, String>` → `GET /presence`

Both are re-exported from
[src/multiplayer/network/vps.rs](../src/multiplayer/network/vps.rs).

### Bevy subsystem — [src/multiplayer/social.rs](../src/multiplayer/social.rs)

- `OnlinePlayersState` resource — holds `count`, `last_sync`, and a background
  fetch receiver.
- `tick_presence_sync` system (registered in `SocialPlugin`) runs every ~15s:
  once our `node_id` is known, it spawns an `IoTaskPool` task that
  1. sends our heartbeat (`update_presence`, status `online`), then
  2. fetches the list (`get_online`) and reports `len()` back over a channel.

  The next frame drains the channel into `OnlinePlayersState.count`. All network
  I/O is off the main thread, so the frame never blocks.

### Menu display — [src/states/main_menu/new_menu.rs](../src/states/main_menu/new_menu.rs)

`render_main_panel` reads `cx.online_players.count` and renders a green dot plus
`"N online"` under the MAIN MENU header, using egui's default proportional font
(`egui::FontFamily::Proportional`) to match the rest of the menu.

The resource is exposed to the UI via `MainMenuUIContext.online_players` in
[src/ui/system_params/main_menu.rs](../src/ui/system_params/main_menu.rs).

## Caveats / future work

- **Single-node only.** The store is in-memory, so it resets on backend restart
  and does not aggregate across multiple VPS instances. Move it to Redis/SQLite
  if you ever run more than one backend node.
- **No stale sweep scheduled.** `PresenceStore::sweep_stale()` exists but isn't
  called on a timer. It's only a memory-cleanup nicety — `get_all_online` already
  filters by timestamp — but a background task in
  [backend/src/tasks/](../backend/src/tasks/) would tidy memory over time.
- **No explicit offline on quit.** Players drop off after the 5-min TTL. Call
  `set_offline` (a `DELETE`/`POST /presence/offline` route) on graceful exit for
  an instant decrement if desired.
