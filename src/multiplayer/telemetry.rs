use std::time::Instant;

use bevy::prelude::*;
use bevy::window::WindowFocused;

use crate::game::events::{GameStartedEvent, MoveMadeEvent};

#[derive(Resource, Default)]
pub struct FocusTelemetry {
    blurred_since_last_local_move: bool,
    ply_count: u32,
    turn_started_at: Option<Instant>,
}

pub struct FocusTelemetryPlugin;

impl Plugin for FocusTelemetryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FocusTelemetry>()
            .add_systems(Update, (track_window_focus, report_move_blur).chain());
    }
}

fn track_window_focus(
    mut events: MessageReader<WindowFocused>,
    mut telemetry: ResMut<FocusTelemetry>,
) {
    for event in events.read() {
        if !event.focused {
            telemetry.blurred_since_last_local_move = true;
        }
    }
}

fn report_move_blur(
    mut moves: MessageReader<MoveMadeEvent>,
    mut starts: MessageReader<GameStartedEvent>,
    mut telemetry: ResMut<FocusTelemetry>,
) {
    if starts.read().next().is_some() {
        telemetry.ply_count = 0;
        telemetry.blurred_since_last_local_move = false;
        telemetry.turn_started_at = Some(Instant::now());
    }

    for mv in moves.read() {
        let now = Instant::now();
        telemetry.ply_count += 1;

        // Think time for the move just made: elapsed since the turn began
        // (the previous move applied, or game start). Resets for the next turn
        // regardless of who moved.
        let think_ms = telemetry.turn_started_at.map(|t| {
            now.saturating_duration_since(t)
                .as_millis()
                .min(u32::MAX as u128) as u32
        });
        telemetry.turn_started_at = Some(now);

        if mv.remote {
            continue;
        }
        let blurred = telemetry.blurred_since_last_local_move;
        telemetry.blurred_since_last_local_move = false;

        // Only networked games are relayed through the VPS; AI games have no
        // game_id and nothing to report against.
        let Some(game_id) = mv.game_id else { continue };

        let move_number = telemetry.ply_count;
        let color = mv.player.to_lowercase();
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                if let Err(e) = crate::multiplayer::vps_client::report_blur(
                    game_id,
                    move_number,
                    &color,
                    blurred,
                    think_ms,
                ) {
                    debug!("[telemetry] blur report failed for game {game_id}: {e}");
                }
            })
            .detach();
    }
}
