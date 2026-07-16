# src/core

Foundational app infrastructure: the `GameState` machine, window setup, crash
reporting, and settings persistence. `CorePlugin` must be added **before** every
other XFChess plugin — it registers the states and resources they depend on.

## Role in XFChess

Every other module keys its systems off the states defined here
(`run_if(in_state(GameState::InGame))` etc.). State transitions are validated
centrally so a bug can't jump from `Auth` straight into `Paused`.

## Key files

| File | Contents |
|------|----------|
| [states.rs](states.rs) | `GameState` (`Auth, MainMenu, InGame, Paused, GameOver, MultiplayerMenu, Matching, Settings`), `GameMode`, menu sub-states, allowed-transition table |
| [plugin.rs](plugin.rs) | `CorePlugin` — panic hook, state registration, core resources |
| [crash.rs](crash.rs) / [error_handling.rs](error_handling.rs) | Crash reports with system info; non-fatal error surface |
| [settings_persistence.rs](settings_persistence.rs) | `GameSettings` load/save (graphics, audio, controls) |
| [window_config.rs](window_config.rs) | Window creation and mode handling |
| [persistent_camera.rs](persistent_camera.rs) | Camera that survives state transitions |
| [state_lifecycle.rs](state_lifecycle.rs) | Despawn-on-exit bookkeeping per state |

## Example

```rust
// states.rs — transitions are an explicit allowlist, not free-form
(GameState::MainMenu, GameState::InGame) => true,
(GameState::MainMenu, GameState::MultiplayerMenu) => true,
(GameState::MultiplayerMenu, GameState::InGame) => true, // start game from lobby
```

## Gotchas

- On `wasm32` the default state is `Auth`; on native it's `MainMenu` — don't assume
  the boot state.
- Add new state transitions to the allowlist in [states.rs](states.rs); an
  unlisted transition is rejected at runtime, which shows up as a "button does
  nothing" bug.
