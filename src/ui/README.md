# src/ui

Egui-based UI: account/auth panels, in-game HUD and chat, menus/popups, and the
shared style system. Distinct from [`states/`](../states/) (which owns whole screens):
`ui/` provides the widgets and panels those screens and the in-game view compose.

## Layout

| Path | Contents |
|------|----------|
| [account/](account/) | Profile display and Solana wallet panel |
| [game/](game/) | In-game HUD ([game_ui/](game/game_ui/)), chat, promotion picker, 2D board view ([game_2d/](game/game_2d/)) |
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

- The main menu lives in [states/main_menu/](../states/main_menu/); this module
  supplies reusable panels and widgets.
- Add new colors/fonts to [styles/](styles/) rather than inline literals — the theme
  is shared across menu and in-game UI.
