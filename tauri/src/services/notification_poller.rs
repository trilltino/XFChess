use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tracing::{error, info};

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// What the player has already been notified about.
#[derive(Default)]
struct NotificationState {
  last_tournament_id: Option<String>,
  last_match_found: bool,
}

/// Polls the backend for open tournaments and matchmaking results and shows a
/// desktop notification for each new one, for as long as the app runs.
pub fn start_poller(
  app: AppHandle,
  backend_url: String,
  wallet_pubkey: Arc<Mutex<Option<String>>>,
) {
  tauri::async_runtime::spawn(async move {
    let client = match reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build() {
      Ok(client) => client,
      Err(e) => {
        error!("[Poller] HTTP client unavailable: {}", e);
        return;
      }
    };
    let mut state = NotificationState::default();
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    ticker.tick().await; // skip the immediate first tick

    loop {
      ticker.tick().await;

      let Some(pubkey) = wallet_pubkey.lock().clone() else {
        continue;
      };

      match poll_tournaments(&client, &backend_url).await {
        Ok(tournaments) => {
          for t in tournaments {
            let tournament_id = t.id();
            if state.last_tournament_id.as_deref() != Some(&tournament_id) {
              state.last_tournament_id = Some(tournament_id);
              notify(
                &app,
                "Tournament Available",
                &format!(
                  "{} - {} players, {} entry fee",
                  t.name,
                  t.player_label(),
                  t.fee_label()
                ),
              );
            }
          }
        }
        Err(e) => tracing::debug!("[Poller] Tournament poll: {}", e),
      }

      match poll_matchmaking(&client, &backend_url, &pubkey).await {
        Ok(status) => {
          if status.match_found && !state.last_match_found {
            notify(
              &app,
              "Match Found",
              "An opponent has been found! Click to join.",
            );
          }
          state.last_match_found = status.match_found;
        }
        Err(e) => tracing::debug!("[Poller] Matchmaking poll: {}", e),
      }
    }
  });
}

fn notify(app: &AppHandle, title: &str, body: &str) {
  if let Err(e) = app.notification().builder().title(title).body(body).show() {
    error!("[Notification] Failed to show: {}", e);
  } else {
    info!("[Notification] {} — {}", title, body);
  }
}

#[derive(Debug, serde::Deserialize)]
struct TournamentInfo {
  tournament_id: u64,
  name: String,
  registered: usize,
  max_players: u16,
  entry_fee_lamports: u64,
  status: String,
}

impl TournamentInfo {
  fn id(&self) -> String {
    self.tournament_id.to_string()
  }

  fn player_label(&self) -> String {
    format!("{}/{}", self.registered, self.max_players)
  }

  fn fee_label(&self) -> String {
    if self.entry_fee_lamports == 0 {
      "free".to_string()
    } else {
      format!(
        "{:.4} SOL",
        self.entry_fee_lamports as f64 / 1_000_000_000.0
      )
    }
  }
}

#[derive(Debug, serde::Deserialize)]
struct MatchmakingStatus {
  match_found: bool,
}

async fn poll_tournaments(
  client: &reqwest::Client,
  backend_url: &str,
) -> Result<Vec<TournamentInfo>, reqwest::Error> {
  let url = format!("{}/api/tournaments", backend_url);
  let tournaments = client
    .get(&url)
    .send()
    .await?
    .json::<Vec<TournamentInfo>>()
    .await?;
  Ok(
    tournaments
      .into_iter()
      .filter(|t| {
        t.status.eq_ignore_ascii_case("Registration") || t.status.eq_ignore_ascii_case("Active")
      })
      .collect(),
  )
}

async fn poll_matchmaking(
  client: &reqwest::Client,
  backend_url: &str,
  pubkey: &str,
) -> Result<MatchmakingStatus, reqwest::Error> {
  let url = format!("{}/matchmaking/status/{}", backend_url, pubkey);
  client
    .get(&url)
    .send()
    .await?
    .json::<MatchmakingStatus>()
    .await
}
