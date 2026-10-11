---
name: bevy
description: Implement XFChess Bevy ECS, plugins, schedules, state transitions, assets and rendering. Use for the native game; verify examples against the workspace pin before applying them.
license: MIT
metadata:
  tier: '1'
  area: ecs
  bevy_version: '0.19'
  compatibility: opencode,claude-code,cursor
  source: chrisgliddon/bevy-skills
  source_commit: b1b4da5744ebbd5c526342b2351967411cd5ca61
---

# Bevy ECS and scheduling

## When to use this skill

- Setting up a new Bevy app or library plugin.
- Deciding which schedule (`Startup`, `Update`, `FixedUpdate`, `PostUpdate`, ...) a system belongs in.
- Writing an exclusive system that needs `&mut World`.
- Hitting schedule cycles, elevated ambiguity diagnostics, or deferred-command order bugs.
- Asking "where should X run?"

## Canonical pattern

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GamePlugin)
        .run();
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Score(0))
            .add_systems(Startup, spawn_world)
            .add_systems(Update, (read_input, apply_gravity).chain())
            .add_systems(FixedUpdate, simulate_physics)
            .add_systems(PostUpdate, sync_transforms);
    }
}

#[derive(Resource)]
struct Score(u32);

fn spawn_world(mut commands: Commands) {
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 4.0, 8.0)));
}

fn read_input(_keys: Res<ButtonInput<KeyCode>>) {}
fn apply_gravity(_t: Res<Time>) {}
fn simulate_physics(_t: Res<Time<Fixed>>) {}
fn sync_transforms(_q: Query<&mut Transform>) {}

// Exclusive system — full mutable World access, runs alone.
fn rebuild_index(world: &mut World) {
    let count = world.entities().len();
    world.insert_resource(EntityCount(count));
}

#[derive(Resource)]
struct EntityCount(u32);
```

## Schedule cheat sheet

| Schedule | Use for |
|---|---|
| `Startup` | One-shot setup. Runs once before `Update`. |
| `PreUpdate` | Reading input, networking ingress, frame-start bookkeeping. |
| `Update` | Per-frame game logic. Runs at render rate. **Default for game systems.** |
| `PostUpdate` | Reactive bookkeeping after `Update` (transform propagation lives here). |
| `Last` | Anything that must run after `PostUpdate` (rare). |
| `FixedFirst` / `FixedPreUpdate` / `FixedUpdate` / `FixedPostUpdate` / `FixedLast` | Deterministic, tick-rate work: physics, networking simulation, gameplay state. Runs zero or more times per render frame to catch up to `Time<Fixed>`. |

**Rule of thumb:** if a system must produce the same result for the same inputs regardless of frame rate, put it in `FixedUpdate`. Otherwise `Update`.

## XFChess integration

The imported core guide was written for Bevy 0.19. XFChess pins 0.20.0;
use the installed crate sources and existing game code for version-sensitive APIs.
This is reference material, not an instruction to downgrade or upgrade the engine.

- Read `src/core/schedule.rs`, `src/core/state_lifecycle.rs` and the target plugin.
  Preserve the existing system sets and state-scoped cleanup.
- Define ordering when downstream work needs prior mutations or deferred commands;
  serialization through shared mutable access does not choose logical order.
- Frame-driven UI and authoritative chess state have different timing needs.
  Preserve current game scheduling rather than moving everything into FixedUpdate.
- Keep render-world passes in render schedules. Reuse owned assets/materials and
  cancel screen-owned asynchronous work during state exit.
- Distinguish buffered messages from observer events; inspect the actual event type
  before selecting a reader, writer or observer API.
- Use the `egui` skill for immediate-mode UI. Check asset/loading, picking, animation,
  cameras, audio and rendering against the pinned sources when touching those areas.

## Sources

- [Upstream core guide](https://github.com/chrisgliddon/bevy-skills/tree/b1b4da5744ebbd5c526342b2351967411cd5ca61/skills/bevy-core-concepts)
- [Bevy API](https://docs.rs/bevy/latest/bevy/): select the installed version, or use local crate sources if online versioned docs are unavailable.
- [Bevy migration guides](https://bevy.org/learn/migration-guides/)
- [Bevy examples](https://github.com/bevyengine/bevy/tree/main/examples): match the workspace release before copying snippets.
