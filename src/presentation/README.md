# src/presentation

Audio presentation layer: applies the user's volume/mute settings to every playing
sound. Sound *triggering* (move clicks, menu sounds) lives with the features that own
them ([game/resources/sounds.rs](../game/resources/sounds.rs),
[states/main_menu/music.rs](../states/main_menu/music.rs)); this module is the
cross-cutting volume control.

## Key files

| File | Contents |
|------|----------|
| [audio.rs](audio.rs) | `AudioPresentationPlugin` + `apply_master_volume_system` |
| [mod.rs](mod.rs) | `PresentationPlugin` wrapper |

## Example

```rust
// audio.rs — one system applies GameSettings volume to every AudioSink
pub fn apply_master_volume_system(
    settings: Res<GameSettings>,
    mut audio_sinks: Query<&mut AudioSink>,
    mut last_settings: Local<Option<(f32, bool)>>,
) {
    let effective_volume = if settings.muted { 0.0 } else { settings.master_volume };
    for mut sink in audio_sinks.iter_mut() {
        sink.set_volume(Volume::Linear(effective_volume));
    }
}
```

## Gotchas

- New sounds only need an `AudioPlayer` + `AudioSink` entity — volume/mute is applied
  globally here, so don't multiply `master_volume` at the call site (it would apply
  twice).
