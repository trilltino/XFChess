use super::*;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

struct PopupSession {
  id: String,
  opened_at: std::time::Instant,
  ready_logged: bool,
  step: String,
}

fn current_session_cell() -> &'static Mutex<Option<PopupSession>> {
  static CELL: Mutex<Option<PopupSession>> = Mutex::new(None);
  &CELL
}

fn new_session_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

fn begin_session(step: &str) -> String {
  let id = new_session_id();
  tracing::info!(sid = %id, event = "OPEN_POPUP_START", "[Lifecycle] OPEN_POPUP_START");
  *current_session_cell().lock() = Some(PopupSession {
    id: id.clone(),
    opened_at: std::time::Instant::now(),
    ready_logged: false,
    step: step.into(),
  });
  id
}

fn log_window_event(event: &str) {
  let guard = current_session_cell().lock();
  if let Some(s) = guard.as_ref() {
    tracing::info!(
      sid = %s.id, event = %event, elapsed_ms = s.opened_at.elapsed().as_millis() as u64,
      "[Lifecycle] {event}"
    );
  }
}

pub(crate) fn mark_session_ready(sid: &str) {
  let mut guard = current_session_cell().lock();
  if let Some(s) = guard.as_mut() {
    if s.id == sid && !s.ready_logged {
      s.ready_logged = true;
      tracing::info!(
        sid = %sid, event = "REACT_READY", elapsed_ms = s.opened_at.elapsed().as_millis() as u64,
        "[Lifecycle] REACT_READY"
      );
      return;
    }
  }
  drop(guard);
  if !sid.is_empty() && sid != "-" {
    tracing::debug!(sid = %sid, "[Lifecycle] REACT_READY for a session that is no longer current (stale popup page)");
  }
}

pub(crate) fn open_wallet_popup(app: &tauri::AppHandle) {
  open_wallet_popup_with_step(None);
  use tauri::Manager;
  app.state::<PendingTxNotify>().send_replace(());
}

pub(super) fn open_wallet_popup_for_signing(app: &tauri::AppHandle) {
  open_wallet_popup_with_step(Some("sign"));
  use tauri::Manager;
  app.state::<PendingTxNotify>().send_replace(());
}

pub(super) fn open_wallet_popup_with_step(step: Option<&str>) {
  let _operation = popup_operation().lock();
  if super::lifecycle::stopped() {
    return;
  }
  let step_name = step.unwrap_or("wallet");
  if let Some(session) = current_session_cell().lock().as_mut() {
    session.step = step_name.into();
  }
  if show_and_foreground_wallet_popup() {
    return;
  }
  if current_session_cell()
    .lock()
    .as_ref()
    .is_some_and(|session| !session.ready_logged && session.opened_at.elapsed().as_secs() < 5)
  {
    return;
  }
  kill_wallet_popup();
  let sid = begin_session(step_name);
  let wallet_url = std::env::var("XFCHESS_WALLET_URL")
    .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
  let separator = if wallet_url.contains('?') { '&' } else { '?' };
  let url = match step {
    Some(s) => format!("{wallet_url}{separator}step={s}&sid={sid}"),
    None => format!("{wallet_url}{separator}sid={sid}"),
  };
  tracing::info!(sid = %sid, "[WalletPopup] opening in system browser: {url}");
  open_in_browser(&url);
}

fn popup_operation() -> &'static Mutex<()> {
  static OPERATION: Mutex<()> = Mutex::new(());
  &OPERATION
}

fn open_in_browser(url: &str) {
  let ts = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let sep = if url.contains('?') { '&' } else { '?' };
  let url_ts = format!("{url}{sep}_t={ts}");

  #[cfg(windows)]
  {
    // CREATE_NO_WINDOW prevents spawned Windows console tools from showing a console.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn get_chromium_default_browser() -> Option<String> {
      let output = std::process::Command::new("reg")
        .args([
          "query",
          r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
          "/v",
          "ProgId",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
      let stdout = String::from_utf8_lossy(&output.stdout);
      let prog_id = stdout
        .lines()
        .find(|l| l.contains("ProgId"))?
        .split_whitespace()
        .last()?;

      let hkcr = format!(r"HKCR\{}\shell\open\command", prog_id);
      let output = std::process::Command::new("reg")
        .args(["query", &hkcr, "/ve"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
      let stdout = String::from_utf8_lossy(&output.stdout);

      let path_str = stdout.lines().find(|l| l.contains("REG_SZ"))?;
      let idx = path_str.find("REG_SZ")?;
      let cmd = path_str[idx + 6..].trim();
      let path = if let Some(quoted) = cmd.strip_prefix('"') {
        let end_quote = quoted.find('"')?;
        quoted[..end_quote].to_string()
      } else {
        let path_part = cmd
          .split(" --")
          .next()
          .unwrap_or(cmd)
          .split(" %")
          .next()
          .unwrap_or(cmd);
        path_part.trim().to_string()
      };
      let lower = path.to_lowercase();
      if lower.contains("chrome.exe")
        || lower.contains("msedge.exe")
        || lower.contains("brave.exe")
        || lower.contains("vivaldi.exe")
        || lower.contains("opera.exe")
      {
        return Some(path);
      }
      None
    }

    let chromium_browser = get_chromium_default_browser().or_else(|| {
      ["PROGRAMFILES", "PROGRAMFILES(X86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .flat_map(|root| {
          [
            "Google/Chrome/Application/chrome.exe",
            "Microsoft/Edge/Application/msedge.exe",
          ]
          .map(|relative| std::path::PathBuf::from(&root).join(relative))
        })
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
    });
    if super::lifecycle::stopped() {
      return;
    }
    if let Some(chromium_browser) = chromium_browser {
      if std::path::Path::new(&chromium_browser).exists() {
        let app_flag = format!("--app={}", url_ts);
        match Command::new(&chromium_browser)
          .args([
            &app_flag,
            &format!("--window-size={WALLET_POPUP_WIDTH},{WALLET_POPUP_HEIGHT}"),
          ])
          .spawn()
        {
          Ok(child) => {
            drop(child);
            spawn_wallet_popup_resize_watcher();
            return;
          }
          Err(e) => tracing::warn!("[WalletPopup] failed to spawn {chromium_browser}: {e}"),
        }
      }
    }

    // Fall back to default browser via open::that() if not Chromium or spawn failed
    tracing::info!("[WalletPopup] Opening in default browser (not chromium --app)");
    let _ = open::that(&url_ts);
  }
  #[cfg(not(windows))]
  {
    let _ = open::that(&url_ts);
  }
}

pub(crate) fn kill_wallet_popup() {
  #[cfg(windows)]
  {
    let opening = current_session_cell()
      .lock()
      .as_ref()
      .is_some_and(|session| !session.ready_logged && session.opened_at.elapsed().as_secs() < 10);
    let attempts = if opening && super::lifecycle::stopped() {
      50
    } else {
      1
    };
    for _ in 0..attempts {
      if super::native::close() {
        break;
      }
      if attempts > 1 {
        std::thread::sleep(std::time::Duration::from_millis(100));
      }
    }
  }
  *current_session_cell().lock() = None;
  *wallet_popup_hidden_at_cell().lock() = None;
}

fn wallet_popup_hidden_at_cell() -> &'static Mutex<Option<std::time::Instant>> {
  static CELL: Mutex<Option<std::time::Instant>> = Mutex::new(None);
  &CELL
}

pub(crate) fn hide_wallet_popup() {
  #[cfg(windows)]
  if super::native::hide() {
    *wallet_popup_hidden_at_cell().lock() = Some(std::time::Instant::now());
  }
}

fn show_and_foreground_wallet_popup() -> bool {
  #[cfg(windows)]
  if super::native::show() {
    *wallet_popup_hidden_at_cell().lock() = None;
    return true;
  }
  false
}

const WALLET_POPUP_WIDTH: i32 = 460;
const WALLET_POPUP_HEIGHT: i32 = 720;

fn resize_wallet_popup_window(width: i32, height: i32) -> bool {
  #[cfg(windows)]
  {
    super::native::position(width, height)
  }
  #[cfg(not(windows))]
  {
    let _ = (width, height);
    false
  }
}

pub(super) fn window_title() -> Option<String> {
  current_session_cell()
    .lock()
    .as_ref()
    .map(|session| format!("XFChess #{} [{}]", http_port(), &session.id[..8]))
}

pub(super) fn is_session(sid: &str) -> bool {
  current_session_cell()
    .lock()
    .as_ref()
    .is_some_and(|session| session.id == sid)
}

pub(super) fn current_step() -> Option<String> {
  current_session_cell()
    .lock()
    .as_ref()
    .map(|session| session.step.clone())
}

const POPUP_WINDOW_POLL_INTERVAL_MS: u64 = 200;
const POPUP_WINDOW_POLL_ATTEMPTS: u32 = 150;

fn spawn_wallet_popup_resize_watcher() {
  let sid = current_session_cell()
    .lock()
    .as_ref()
    .map(|session| session.id.clone());
  tauri::async_runtime::spawn(async move {
    let mut found_once = false;
    for _ in 0..POPUP_WINDOW_POLL_ATTEMPTS {
      if super::lifecycle::stopped() || !sid.as_deref().is_some_and(is_session) {
        return;
      }
      tokio::time::sleep(std::time::Duration::from_millis(
        POPUP_WINDOW_POLL_INTERVAL_MS,
      ))
      .await;
      if resize_wallet_popup_window(WALLET_POPUP_WIDTH, WALLET_POPUP_HEIGHT) {
        if !found_once {
          log_window_event("WINDOW_FOUND");
        }
        found_once = true;
        break;
      }
    }
    if !found_once {
      log_window_event("WINDOW_NOT_FOUND_TIMEOUT");
      tracing::warn!("[WalletPopup] gave up waiting for popup window to enforce its size");
    }
  });
}

const HIDDEN_POPUP_IDLE_KILL_SECS: u64 = 15 * 60;

pub(crate) fn spawn_wallet_popup_idle_reaper() {
  tauri::async_runtime::spawn(async move {
    loop {
      tokio::time::sleep(std::time::Duration::from_secs(60)).await;
      let _operation = popup_operation().lock();
      let should_kill = wallet_popup_hidden_at_cell()
        .lock()
        .is_some_and(|at| at.elapsed().as_secs() >= HIDDEN_POPUP_IDLE_KILL_SECS);
      if should_kill {
        tracing::info!(
          "[WalletPopup] hidden popup idle for {HIDDEN_POPUP_IDLE_KILL_SECS}s — killing for real"
        );
        kill_wallet_popup();
        *wallet_popup_hidden_at_cell().lock() = None;
      }
    }
  });
}
