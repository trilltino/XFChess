//! Background notification poller for the XFChess Tauri app.
//!
//! Uses a dedicated OS thread with its own single-threaded Tokio runtime so
//! it can be started from the Tauri `setup` closure before Tauri's async
//! runtime handle is fully initialised.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tracing::{error, info};

/// State tracking for deduplication — only notify once per event.
#[derive(Default, Clone)]
pub struct NotificationState {
  pub last_tournament_id: Option<String>,
  pub last_match_found: bool,
  pub last_game_invite: Option<String>,
}

/// Starts the background poller on a dedicated thread with its own runtime.
///
/// `wallet_pubkey` is the *live* shared wallet state (not a one-time
/// snapshot) — the app always starts disconnected, so reading it once at
/// startup would mean the poller silently never runs for the whole session.
/// Safe to call from the Tauri `setup` closure.
pub fn start_poller(app: AppHandle, backend_url: String, wallet_pubkey: Arc<Mutex<Option<String>>>) {
  let state = Arc::new(Mutex::new(NotificationState::default()));

  std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("[Poller] Failed to build tokio runtime");

    rt.block_on(async move {
      let mut ticker = tokio::time::interval(Duration::from_secs(30));
      ticker.tick().await; // skip the immediate first tick

      loop {
        ticker.tick().await;

        let pubkey = match wallet_pubkey.lock().unwrap().clone() {
          Some(p) => p,
          None => continue,
        };

        // ── Poll tournaments ─────────────────────────────────────────
        match poll_tournaments(&backend_url).await {
          Ok(tournaments) => {
            for t in tournaments {
              let mut s = state.lock().unwrap();
              if s.last_tournament_id.as_deref() != Some(&t.id) {
                s.last_tournament_id = Some(t.id.clone());
                drop(s);
                notify(
                  &app,
                  "Tournament Available",
                  &format!("{} — {} players, {} entry fee", t.name, t.players, t.fee),
                );
              }
            }
          }
          Err(e) => tracing::debug!("[Poller] Tournament poll: {}", e),
        }

        // ── Poll matchmaking status ──────────────────────────────────
        match poll_matchmaking(&backend_url, &pubkey).await {
          Ok(status) => {
            let mut s = state.lock().unwrap();
            if status.match_found && !s.last_match_found {
              s.last_match_found = true;
              drop(s);
              notify(
                &app,
                "Match Found",
                "An opponent has been found! Click to join.",
              );
            } else if !status.match_found {
              s.last_match_found = false;
            }
          }
          Err(e) => tracing::debug!("[Poller] Matchmaking poll: {}", e),
        }
      }
    });
  });
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn notify(app: &AppHandle, title: &str, body: &str) {
  if let Err(e) = app.notification().builder().title(title).body(body).show() {
    error!("[Notification] Failed to show: {}", e);
  } else {
    info!("[Notification] {} — {}", title, body);
  }
}

#[derive(Debug, serde::Deserialize)]
struct TournamentInfo {
  id: String,
  name: String,
  players: u32,
  fee: String,
}

#[derive(Debug, serde::Deserialize)]
struct MatchmakingStatus {
  match_found: bool,
}

async fn poll_tournaments(backend_url: &str) -> Result<Vec<TournamentInfo>, reqwest::Error> {
  let url = format!("{}/tournaments/active", backend_url);
  reqwest::get(&url)
    .await?
    .json::<Vec<TournamentInfo>>()
    .await
}

async fn poll_matchmaking(
  backend_url: &str,
  pubkey: &str,
) -> Result<MatchmakingStatus, reqwest::Error> {
  let url = format!("{}/matchmaking/status/{}", backend_url, pubkey);
  reqwest::get(&url).await?.json::<MatchmakingStatus>().await
}
