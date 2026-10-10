//! Claim a device seat on game start or resume. The newest claim makes prior
//! devices view-only; poll for takeover and handle seat_superseded immediately.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

use crate::multiplayer::vps_client::SeatLease;

const SEAT_RECHECK: Duration = Duration::from_secs(15);

#[derive(Resource, Default)]
pub struct SeatState {
    pub game_id: u64,
    pub epoch: i64,
    /// Another device holds this player's seat; this window is view-only.
    pub superseded: bool,
    claim_rx: Option<oneshot::Receiver<Result<SeatLease, String>>>,
    check_rx: Option<oneshot::Receiver<Result<Option<SeatLease>, String>>>,
    last_check: Option<Instant>,
}

impl SeatState {
    fn claim(&mut self, game_id: u64) {
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::multiplayer::vps_client::claim_seat(game_id));
        });
        self.game_id = game_id;
        self.claim_rx = Some(rx);
        self.last_check = Some(Instant::now());
    }
}

/// The seat belongs to another device when the lease names a different id.
pub fn lease_belongs_to_other_device(lease: Option<&SeatLease>, me: &str) -> bool {
    lease.is_some_and(|l| l.device_id != me)
}

fn claim_seat_on_game_start(
    mut started: MessageReader<crate::game::events::GameStartedEvent>,
    sync: Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut seat: ResMut<SeatState>,
) {
    for _ in started.read() {
        // Seats exist for wallet (on-chain) games only.
        let Some(game_id) = sync.game_id else {
            continue;
        };
        seat.superseded = false;
        seat.claim(game_id);
    }
}

fn track_seat(mut seat: ResMut<SeatState>) {
    if seat.game_id == 0 {
        return;
    }
    let me = crate::multiplayer::network::device_id::device_id();

    if let Some(rx) = seat.claim_rx.as_mut() {
        match rx.try_recv() {
            Ok(Ok(lease)) => {
                seat.epoch = lease.epoch;
                seat.superseded = lease.device_id != me;
                seat.claim_rx = None;
                info!(
                    "[SEAT] game {} seat held here (epoch {})",
                    seat.game_id, lease.epoch
                );
            }
            Ok(Err(e)) => {
                // Older backends / unverifiable participation: keep playing
                // under the legacy (unclaimed) rules rather than locking out.
                warn!("[SEAT] claim for game {} failed: {e}", seat.game_id);
                seat.claim_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => seat.claim_rx = None,
        }
    }

    if let Some(rx) = seat.check_rx.as_mut() {
        match rx.try_recv() {
            Ok(Ok(lease)) => {
                if lease_belongs_to_other_device(lease.as_ref(), me) && !seat.superseded {
                    warn!(
                        "[SEAT] game {} is now played on another device",
                        seat.game_id
                    );
                    seat.superseded = true;
                }
                seat.check_rx = None;
            }
            Ok(Err(_)) | Err(oneshot::error::TryRecvError::Closed) => seat.check_rx = None,
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }

    if let Some(game_id) = crate::multiplayer::network::device_id::take_seat_superseded() {
        if game_id == seat.game_id {
            seat.superseded = true;
        }
    }

    let due = seat.last_check.is_none_or(|t| t.elapsed() >= SEAT_RECHECK);
    if due && seat.claim_rx.is_none() && seat.check_rx.is_none() {
        let game_id = seat.game_id;
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::multiplayer::vps_client::get_seat(game_id));
        });
        seat.check_rx = Some(rx);
        seat.last_check = Some(Instant::now());
    }
}

fn seat_banner(mut contexts: EguiContexts, mut seat: ResMut<SeatState>) {
    if !seat.superseded {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::TopBottomPanel::top("seat_superseded_banner").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(255, 210, 90),
                "This game is being played on another device. This window is view-only.",
            );
            if ui.button("Play here instead").clicked() {
                let game_id = seat.game_id;
                seat.superseded = false;
                seat.claim(game_id);
            }
        });
    });
}

fn reset_seat(mut seat: ResMut<SeatState>) {
    *seat = SeatState::default();
}

pub struct SeatPlugin;

impl Plugin for SeatPlugin {
    fn build(&self, app: &mut App) {
        // `GameStartedEvent` is written from the menu just before the switch
        // to `InGame`, so the claim reader runs in every state.
        app.init_resource::<SeatState>()
            .add_systems(Update, claim_seat_on_game_start)
            .add_systems(
                Update,
                track_seat
                    .after(claim_seat_on_game_start)
                    .run_if(in_state(crate::core::GameState::InGame)),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                seat_banner.run_if(in_state(crate::core::GameState::InGame)),
            )
            .add_systems(OnExit(crate::core::GameState::InGame), reset_seat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_lease_naming_another_device_makes_this_window_view_only() {
        let mine = SeatLease {
            device_id: "me-0000001".into(),
            epoch: 3,
        };
        let other = SeatLease {
            device_id: "you-000002".into(),
            epoch: 4,
        };
        assert!(!lease_belongs_to_other_device(None, "me-0000001"));
        assert!(!lease_belongs_to_other_device(Some(&mine), "me-0000001"));
        assert!(lease_belongs_to_other_device(Some(&other), "me-0000001"));
    }
}
