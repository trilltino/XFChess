//! Main-menu music: a small looping playlist of public-domain piano pieces that
//! fade into one another, plus a discrete egui widget to see the current track,
//! skip it, or mute.
//!
//! Design:
//! - Each track plays once ([`PlaybackMode::Once`]); when it finishes (`empty()`)
//!   the next one is spawned and fades in. Skipping fades the current track out
//!   while the next fades in (a true crossfade).
//! - Volume is driven per-voice in [`drive_menu_music`] toward a target, so mute
//!   and crossfades are all just "move current volume toward target".
//! - Voices carry `DespawnOnExit(MainMenu)`, so music is strictly menu-only and
//!   stops the instant a game starts.

use bevy::audio::{AudioSinkPlayback, PlaybackMode, Volume};
use bevy::prelude::*;

use crate::core::{DespawnOnExit, GameState};

/// Default background volume (linear). Quiet — it sits under the menu.
const BASE_VOLUME: f32 = 0.30;
/// Fade rate in linear-volume units per second (≈2.5s for a full fade/crossfade).
const FADE_PER_SEC: f32 = 0.12;

/// The menu playlist + playback state. Persists across menu re-entries so the
/// track selection and mute toggle survive returning from a game.
#[derive(Resource)]
pub struct MenuMusic {
    tracks: Vec<Handle<AudioSource>>,
    titles: Vec<&'static str>,
    index: usize,
    muted: bool,
    volume: f32,
    skip_requested: bool,
    /// When true the now-playing widget is hidden (music keeps playing).
    widget_hidden: bool,
}

impl FromWorld for MenuMusic {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        // (asset path, display title)
        let entries: [(&str, &str); 5] = [
            ("audio/menu/gymnopedie_no1.mp3", "Satie — Gymnopédie No. 1"),
            ("audio/menu/gnossienne_no1.mp3", "Satie — Gnossienne No. 1"),
            (
                "audio/menu/chopin_prelude_no4.mp3",
                "Chopin — Prelude Op. 28 No. 4",
            ),
            (
                "audio/menu/chopin_preludes_1_2_3.mp3",
                "Chopin — Preludes Op. 28 Nos. 1–3",
            ),
            (
                "audio/menu/chopin_preludes_10_11_12.mp3",
                "Chopin — Preludes Op. 28 Nos. 10–12",
            ),
        ];
        Self {
            tracks: entries.iter().map(|(p, _)| asset_server.load(*p)).collect(),
            titles: entries.iter().map(|(_, t)| *t).collect(),
            index: 0,
            muted: false,
            volume: BASE_VOLUME,
            skip_requested: false,
            widget_hidden: false,
        }
    }
}

/// One playing (or fading) track. Exactly one voice is non-`outgoing` at a time
/// (the active track); skips/ends turn it `outgoing` so it fades out and despawns.
#[derive(Component)]
pub struct MenuMusicVoice {
    current: f32,
    target: f32,
    outgoing: bool,
    has_played: bool,
}

/// Spawn a fading-in voice for `index`. Starts silent; the drive system ramps it.
fn spawn_voice(commands: &mut Commands, music: &MenuMusic, index: usize) {
    let Some(handle) = music.tracks.get(index) else {
        return;
    };
    let target = if music.muted { 0.0 } else { music.volume };
    commands.spawn((
        AudioPlayer::new(handle.clone()),
        PlaybackSettings {
            mode: PlaybackMode::Once,
            volume: Volume::Linear(0.0),
            ..default()
        },
        MenuMusicVoice {
            current: 0.0,
            target,
            outgoing: false,
            has_played: false,
        },
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuMusicVoice"),
    ));
}

/// Start the playlist on entering the menu (if nothing is already playing).
pub fn start_menu_music(
    mut commands: Commands,
    music: Res<MenuMusic>,
    voices: Query<(), With<MenuMusicVoice>>,
) {
    if music.tracks.is_empty() || !voices.is_empty() {
        return;
    }
    spawn_voice(&mut commands, &music, music.index);
}

/// Drives fades, advances to the next track on natural end, and handles skips.
pub fn drive_menu_music(
    time: Res<Time>,
    mut commands: Commands,
    mut music: ResMut<MenuMusic>,
    mut voices: Query<(Entity, &mut MenuMusicVoice, &mut AudioSink)>,
) {
    let dt = time.delta_secs();
    let step = FADE_PER_SEC * dt;
    let mut advance = false;

    for (entity, mut voice, mut sink) in voices.iter_mut() {
        // The active voice tracks the live mute state; outgoing voices fade to 0.
        if !voice.outgoing {
            voice.target = if music.muted { 0.0 } else { music.volume };
        }
        // Ease current volume toward target and apply it.
        if voice.current < voice.target {
            voice.current = (voice.current + step).min(voice.target);
        } else if voice.current > voice.target {
            voice.current = (voice.current - step).max(voice.target);
        }
        sink.set_volume(Volume::Linear(voice.current));

        if !sink.empty() {
            voice.has_played = true;
        }

        if voice.outgoing {
            // Faded out — drop it.
            if voice.current <= 0.001 {
                commands.entity(entity).despawn();
            }
        } else if voice.has_played && sink.empty() {
            // Active track finished on its own — move to the next.
            commands.entity(entity).despawn();
            advance = true;
        }
    }

    // Skip: fade the active voice out and crossfade into the next.
    if music.skip_requested {
        music.skip_requested = false;
        for (_e, mut voice, _s) in voices.iter_mut() {
            if !voice.outgoing {
                voice.outgoing = true;
                voice.target = 0.0;
            }
        }
        advance = true;
    }

    if advance && !music.tracks.is_empty() {
        music.index = (music.index + 1) % music.tracks.len();
        spawn_voice(&mut commands, &music, music.index);
    }
}

/// Discrete now-playing widget: a play/mute dot, the track title, and a skip
/// button, anchored to the bottom-left. Hidden during the pre-Enter splash.
pub fn menu_music_widget(
    mut contexts: bevy_egui::EguiContexts,
    mut music: ResMut<MenuMusic>,
    intro: Res<super::MenuIntro>,
) {
    use bevy_egui::egui;

    if intro.awaiting_enter || music.widget_hidden {
        return; // keep the splash screen clean / honour a closed widget
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let title = music.titles.get(music.index).copied().unwrap_or("");

    egui::Area::new("menu_music_widget".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(28.0, -22.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Play/mute dot (painted — the menu fonts have no music glyph).
                let (dot, resp) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::click());
                if music.muted {
                    ui.painter().circle_stroke(
                        dot.center(),
                        4.0,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 120, 130)),
                    );
                } else {
                    ui.painter().circle_filled(
                        dot.center(),
                        4.0,
                        egui::Color32::from_rgb(120, 200, 140),
                    );
                }
                if resp.clicked() {
                    music.muted = !music.muted;
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(title)
                        .size(11.0)
                        .color(egui::Color32::from_rgba_unmultiplied(205, 205, 215, 140)),
                );

                ui.add_space(4.0);
                // Skip → next track (chevron renders fine in OpenSans).
                let next = ui.add(
                    egui::Button::new(
                        egui::RichText::new("›")
                            .size(15.0)
                            .color(egui::Color32::from_rgba_unmultiplied(205, 205, 215, 160)),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                );
                if next.clicked() {
                    music.skip_requested = true;
                }
                if next.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                ui.add_space(2.0);
                // Close → hide the widget for the session (music keeps playing).
                let close = ui.add(
                    egui::Button::new(
                        egui::RichText::new("X")
                            .size(11.0)
                            .color(egui::Color32::from_rgba_unmultiplied(205, 205, 215, 120)),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                );
                if close.clicked() {
                    music.widget_hidden = true;
                }
                if close.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        });
}
