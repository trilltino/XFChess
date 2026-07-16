# src/ui

Egui-based UI: account/auth panels, in-game HUD and chat, menus/popups, and the
shared style system. Distinct from [`states/`](../states/) (which owns whole screens):
`ui/` provides the widgets and panels those screens and the in-game view compose.

## Layout

| Path | Contents |
|------|----------|
| [account/](account/) | Login/register ([auth.rs](account/auth.rs)), profile creation/view, Solana wallet panel |
| [game/](game/) | In-game HUD ([hud.rs](game/hud.rs)), chat, promotion picker, 2D board view ([game_2d.rs](game/game_2d.rs)) |
| [menus/](menus/) | Popups, game-over popup, stats, compliance modal, inspector |
| [styles/](styles/) | Colors, typography, shared egui component styles — use these, don't hardcode |
| [system_params/](system_params/) | Bundled `SystemParam` structs for big UI systems |
| [spectator_mode.rs](spectator_mode.rs) | Spectator UI shell |

## Example

```rust
// styles/ centralizes look & feel; UI systems pull from it:
use crate::ui::styles::{colors, typography};

// system_params/game_ui.rs bundles the many resources the HUD needs into one
// SystemParam so systems stay under Bevy's parameter limit.
```

## Gotchas

- `menus/multiplayer_menu.rs` backs the legacy `GameState::MultiplayerMenu` path and
  is a removal candidate ([docs/legacy-cleanup-audit.md](../../docs/legacy-cleanup-audit.md));
  the current menu lives in [states/main_menu/](../states/main_menu/).
- Add new colors/fonts to [styles/](styles/) rather than inline literals — the theme
  is shared across menu and in-game UI.
