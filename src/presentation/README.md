# Presentation Module

## Purpose

The Presentation module handles audio and sensory feedback systems for XFChess. It manages game sounds, ambient audio, and haptic feedback to create an immersive chess playing experience.

## Impact on Game

This module enhances gameplay through:
- **Sound Effects**: Audio feedback for moves, captures, check, and game end
- **Ambient Audio**: Background music and atmospheric sounds
- **Board Themes**: Audio themes that match visual board themes
- **Audio Cues**: Notifications for turn changes and timer warnings
- **Accessibility**: Audio indicators for players with visual impairments

## Architecture/Key Components

### Audio Plugin

| Component | Purpose |
|-----------|---------|
| [`PresentationPlugin`](mod.rs:7) | Entry point for all presentation systems |
| [`AudioPresentationPlugin`](audio.rs) | Manages audio playback and sound events |

### Sound Categories

| Category | Sounds |
|----------|--------|
| **Movement** | Piece move, piece drag start/end |
| **Combat** | Piece capture, check announcement |
| **Game Events** | Game start, victory, defeat, draw |
| **UI** | Button click, menu open/close |
| **Ambient** | Background music (theme-dependent) |

### Board Theme Audio

| Theme | Audio Style |
|-------|-------------|
| Classic | Traditional chess sounds |
| TempleOS | Retro 8-bit style audio |
| King | Premium orchestral sounds |

## Usage

### Playing a Sound

```rust
use bevy::audio::*;

fn play_move_sound(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
) {
    let sound = asset_server.load("sounds/move_piece.mp3");
    audio.play(sound);
}
```

### Audio on Game Events

```rust
fn on_piece_moved(
    mut events: EventReader<PieceMoved>,
    audio: Res<Audio>,
    assets: Res<GameAssets>,
) {
    for _ in events.read() {
        audio.play(assets.sounds.move_sound.clone());
    }
}
```

### Theme-Specific Audio

```rust
fn set_audio_theme(
    theme: BoardTheme,
    mut audio_settings: ResMut<AudioSettings>,
) {
    audio_settings.theme = match theme {
        BoardTheme::TempleOS => AudioTheme::Retro8Bit,
        BoardTheme::King => AudioTheme::Orchestral,
        _ => AudioTheme::Standard,
    };
}
```

## Dependencies

- [`bevy::audio`](https://docs.rs/bevy/latest/bevy/audio/index.html) - Audio playback system
- [`rodio`](https://docs.rs/rodio) - Underlying audio library (via Bevy)

## Related Modules

- [`assets`](../assets/README.md) - Sound asset loading
- [`game`](../game/README.md) - Game events trigger sounds
- [`rendering`](../rendering/README.md) - Visual themes match audio themes

## Audio File Locations

```
assets/
├── game_sounds/
│   ├── move_piece.mp3
│   ├── capture_piece.mp3
│   └── board_themes/
│       ├── King.mp3
│       └── temple_os.mp3
```

## Performance Notes

- Audio assets are preloaded to prevent latency
- Spatial audio not used (board is static viewpoint)
- Volume ducking during multiplayer voice chat
