# src/states

Per-screen plugins: one module per `GameState` variant (main menu, pause, game over,
settings, tournament menu). The state *machine* lives in
[core/states.rs](../core/states.rs); this module owns what each screen shows and does.

## Key files

| File | Contents |
|------|----------|
| [main_menu/](main_menu/) | The main menu: screen layout ([screens.rs](main_menu/screens.rs), [new_menu.rs](main_menu/new_menu.rs)), modals, cinematic + board animation backdrop, music |
| [pause.rs](pause.rs) | Pause overlay (`GameState::Paused`) |
| [game_over.rs](game_over.rs) | Result screen (`GameState::GameOver`) |
| [settings.rs](settings.rs) | Settings screen (`GameState::Settings`) |
| [tournament_menu.rs](tournament_menu.rs) | Tournament browser/registration UI |

## Example

```rust
// Each screen scopes its systems and entities to its state:
app.add_systems(OnEnter(GameState::Paused), spawn_pause_menu)
   .add_systems(Update, pause_menu_buttons.run_if(in_state(GameState::Paused)));
// UI entities carry a despawn-on-exit marker so leaving the state cleans up.
```

## Gotchas

- Entities spawned for a screen must be tagged for despawn-on-exit (see
  [core/state_lifecycle.rs](../core/state_lifecycle.rs)) or they leak into the next
  state.
- New screens also need their transitions added to the allowlist in
  [core/states.rs](../core/states.rs) — otherwise navigation to them is silently
  rejected.
