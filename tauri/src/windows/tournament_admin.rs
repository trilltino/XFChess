//! The tournament-admin window and the PRODUCTION SSH tunnel it talks through.
//!
//! Everything privileged lives behind the `tournament-admin` feature; consumer builds
//! only get the stub `open_tournament_admin`, which logs that the panel is absent.

#[cfg(feature = "tournament-admin")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "tournament-admin")]
use tauri::Manager;

// Tie SSH children to Rust app state and window/app shutdown so closed panels
// cannot leave orphan tunnels holding the port.
#[cfg(feature = "tournament-admin")]
#[derive(Default)]
pub(crate) struct AdminTunnel(Arc<Mutex<TunnelState>>);

#[cfg(feature = "tournament-admin")]
#[derive(Default)]
struct TunnelState {
  child: Option<tauri_plugin_shell::process::CommandChild>,
  target: Option<(String, String)>,
  generation: u64,
}

#[cfg(feature = "tournament-admin")]
async fn admin_health_ok(port: u16) -> bool {
  let url = format!("http://127.0.0.1:{port}/health");
  match reqwest::Client::new()
    .get(&url)
    .timeout(std::time::Duration::from_secs(3))
    .send()
    .await
  {
    Ok(r) if r.status().is_success() => r
      .json::<serde_json::Value>()
      .await
      .is_ok_and(|health| health["status"] == "ok" && health["git_sha"].is_string()),
    Ok(_) => false,
    Err(_) => false,
  }
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
pub(crate) async fn ensure_admin_tunnel(
  app: tauri::AppHandle,
  key_path: String,
  ssh_user: String,
  ssh_host: String,
  local_port: u16,
  remote_host: String,
  remote_port: u16,
) -> Result<String, String> {
  use tauri_plugin_shell::ShellExt;

  // Every value below becomes an ssh argument; one starting with "-" would be
  // parsed as an option (e.g. -oProxyCommand=…), i.e. local code execution.
  validate_tunnel_args(&key_path, &ssh_user, &ssh_host, &remote_host)?;
  if local_port != 8091 || remote_port != 8090 {
    return Err("The admin tunnel may only forward local 8091 to backend 8090".into());
  }
  let target_config = (key_path.clone(), ssh_host.clone());
  let (generation, reusable) = {
    let state = app.state::<AdminTunnel>();
    let mut slot = state.0.lock().map_err(|_| "Tunnel state unavailable")?;
    slot.generation += 1;
    (
      slot.generation,
      slot.child.is_some() && slot.target.as_ref() == Some(&target_config),
    )
  };
  let is_current = || {
    app
      .state::<AdminTunnel>()
      .0
      .lock()
      .is_ok_and(|slot| slot.generation == generation)
  };

  let owns_current = || {
    app.state::<AdminTunnel>().0.lock().is_ok_and(|slot| {
      slot.generation == generation
        && slot.child.is_some()
        && slot.target.as_ref() == Some(&target_config)
    })
  };
  if reusable && admin_health_ok(local_port).await && owns_current() {
    return Ok("reused".into());
  }
  let killed_child = {
    let state = app.state::<AdminTunnel>();
    let mut slot = state.0.lock().map_err(|_| "Tunnel state unavailable")?;
    if slot.generation != generation {
      return Err("Tunnel connection cancelled".into());
    }
    let killed = if let Some(child) = slot.child.take() {
      let _ = child.kill();
      true
    } else {
      false
    };
    slot.target = None;
    killed
  };

  if killed_child {
    for _ in 0..10 {
      if std::net::TcpStream::connect(("127.0.0.1", local_port)).is_err() {
        break;
      }
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
  }

  // Never reuse or kill a listener that the panel does not own.
  if std::net::TcpStream::connect(("127.0.0.1", local_port)).is_ok() {
    return Err(format!(
      "Port {local_port} is already in use by another process, but it is not \
       owned by this panel. Identify the process using that port and close only \
       that process before trying again."
    ));
  }

  let forward = format!("127.0.0.1:{local_port}:{remote_host}:{remote_port}");
  let target = format!("{ssh_user}@{ssh_host}");
  let (mut events, child) = app
    .shell()
    .command("ssh")
    .args([
      "-i",
      &key_path,
      "-o",
      "BatchMode=yes",
      "-o",
      "ConnectTimeout=8",
      "-o",
      "ExitOnForwardFailure=yes",
      "-o",
      "ServerAliveInterval=30",
      "-o",
      "ServerAliveCountMax=3",
      "-o",
      "StrictHostKeyChecking=yes",
      "-N",
      "-L",
      &forward,
      &target,
    ])
    .spawn()
    .map_err(|e| format!("could not start ssh: {e}"))?;

  let child_pid = child.pid();
  {
    let state = app.state::<AdminTunnel>();
    let mut slot = state.0.lock().map_err(|_| "Tunnel state unavailable")?;
    if slot.generation != generation {
      let _ = child.kill();
      return Err("Tunnel connection cancelled".into());
    }
    slot.child = Some(child);
    slot.target = Some(target_config.clone());
  }
  // Drain shell events so a full output channel cannot stall the SSH process.
  let monitor_app = app.clone();
  tauri::async_runtime::spawn(async move {
    while events.recv().await.is_some() {}
    if let Ok(mut slot) = monitor_app.state::<AdminTunnel>().0.lock() {
      if slot
        .child
        .as_ref()
        .is_some_and(|child| child.pid() == child_pid)
      {
        slot.child = None;
        slot.target = None;
      }
    }
  });

  // Poll readiness: cold SSH handshakes may exceed a fixed connection delay.
  let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
  while tokio::time::Instant::now() < deadline {
    if !is_current() {
      return Err("Tunnel connection cancelled".into());
    }
    if tokio::time::timeout_at(deadline, admin_health_ok(local_port))
      .await
      .unwrap_or(false)
      && owns_current()
    {
      return Ok("connected".into());
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
  }

  if is_current() {
    kill_admin_tunnel_inner(&app);
  }
  Err(format!(
    "SSH connected but the backend never answered /health on port {local_port} \
     within 15s. Check that the '{ssh_user}' user exists on {ssh_host}, that \
     your key is authorized, and that the backend is running."
  ))
}

#[cfg(feature = "tournament-admin")]
pub(crate) fn kill_admin_tunnel_inner(app: &tauri::AppHandle) {
  if let Some(state) = app.try_state::<AdminTunnel>() {
    if let Ok(mut slot) = state.0.lock() {
      slot.generation += 1;
      slot.target = None;
      if let Some(child) = slot.child.take() {
        let _ = child.kill();
      }
    }
  }
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
pub(crate) fn kill_admin_tunnel(app: tauri::AppHandle) {
  kill_admin_tunnel_inner(&app);
}

// The admin API key lives in the OS credential store (Windows Credential Manager,
// macOS Keychain, Secret Service), one entry per environment, never in webview storage.
#[cfg(feature = "tournament-admin")]
const CREDENTIAL_SERVICE: &str = "xfchess-tournament-admin";

#[cfg(feature = "tournament-admin")]
fn admin_token_entry(env: &str) -> Result<keyring::Entry, String> {
  if !matches!(env, "local" | "production") {
    return Err(format!("unknown admin environment {env:?}"));
  }
  keyring::Entry::new(CREDENTIAL_SERVICE, &format!("admin-token-{env}"))
    .map_err(|e| format!("credential store unavailable: {e}"))
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
pub(crate) fn admin_token_load(env: String) -> Result<Option<String>, String> {
  match admin_token_entry(&env)?.get_password() {
    Ok(token) => Ok(Some(token)),
    Err(keyring::Error::NoEntry) => Ok(None),
    Err(e) => Err(format!("could not read the saved admin key: {e}")),
  }
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
pub(crate) fn admin_token_save(env: String, token: String) -> Result<(), String> {
  if token.trim().is_empty() {
    return Err("refusing to save an empty admin key".into());
  }
  admin_token_entry(&env)?
    .set_password(&token)
    .map_err(|e| format!("could not save the admin key: {e}"))
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
pub(crate) fn admin_token_clear(env: String) -> Result<(), String> {
  match admin_token_entry(&env)?.delete_credential() {
    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
    Err(e) => Err(format!("could not remove the admin key: {e}")),
  }
}

/// Rejects tunnel parameters that ssh could parse as options or that do not look
/// like what the panel sends (a user name, a host, a loopback forward target and a
/// key file path).
#[cfg(feature = "tournament-admin")]
fn validate_tunnel_args(
  key_path: &str,
  ssh_user: &str,
  ssh_host: &str,
  remote_host: &str,
) -> Result<(), String> {
  let is_name = |s: &str| {
    !s.is_empty()
      && !s.starts_with('-')
      && s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
  };
  if ssh_user != "tunnel" {
    return Err(format!("invalid ssh user {ssh_user:?}"));
  }
  if !is_name(ssh_host) {
    return Err(format!("invalid ssh host {ssh_host:?}"));
  }
  if remote_host != "127.0.0.1" {
    return Err(format!("invalid forward host {remote_host:?}"));
  }
  let key_ok = !key_path.is_empty()
    && !key_path.starts_with('-')
    && !key_path.chars().any(|c| c.is_control() || c == '"');
  if !key_ok {
    return Err(format!("invalid key path {key_path:?}"));
  }
  Ok(())
}

/// The panel's URL: the loopback bridge, or a loopback dev server named by
/// `XFCHESS_ADMIN_DEV_URL` (hot reload). Either way it is normalised to
/// `http://localhost:<port>/tournament-admin/`, the only origin/path the admin
/// capabilities attach to.
#[cfg(feature = "tournament-admin")]
fn admin_window_url(dev_override: Option<&str>, bridge_port: u16) -> Result<tauri::Url, String> {
  let at_port = |port: u16| {
    tauri::Url::parse(&format!("http://localhost:{port}/tournament-admin/"))
      .map_err(|error| error.to_string())
  };
  let Some(dev) = dev_override.map(str::trim).filter(|d| !d.is_empty()) else {
    return at_port(bridge_port);
  };
  match tauri::Url::parse(dev) {
    Ok(url)
      if url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port().is_some() =>
    {
      tracing::warn!("[TournamentAdmin] DEV MODE — loading from {dev} (hot reload)");
      at_port(url.port().unwrap_or(bridge_port))
    }
    _ => {
      tracing::error!("[TournamentAdmin] XFCHESS_ADMIN_DEV_URL={dev} is not a loopback http URL with a port — ignoring");
      at_port(bridge_port)
    }
  }
}

/// Same origin as the panel and under `/tournament-admin/`.
#[cfg(feature = "tournament-admin")]
fn is_admin_page(url: &tauri::Url, panel: &tauri::Url) -> bool {
  url.scheme() == panel.scheme()
    && url.host_str() == panel.host_str()
    && url.port_or_known_default() == panel.port_or_known_default()
    && url.path().starts_with("/tournament-admin/")
}

#[cfg(feature = "tournament-admin")]
pub(crate) fn open_tournament_admin(app: &tauri::AppHandle) {
  // Window creation MUST run on the main thread in Tauri v2.
  let app2 = app.clone();
  let _ = app.run_on_main_thread(move || {
    let app = app2;
    let dev_override = std::env::var("XFCHESS_ADMIN_DEV_URL").ok();
    let admin_url = match admin_window_url(dev_override.as_deref(), crate::http_port()) {
      Ok(url) => url,
      Err(error) => {
        tracing::error!(%error, "Cannot construct tournament admin URL");
        return;
      }
    };
    if let Some(win) = app.get_webview_window("tournament-admin") {
      tracing::info!("[TournamentAdmin] focusing existing window");
      let _ = win.show();
      let _ = win.set_focus();
    } else {
      tracing::info!("[TournamentAdmin] creating window → {admin_url}");
      let allowed = admin_url.clone();
      match tauri::WebviewWindowBuilder::new(
        &app,
        "tournament-admin",
        tauri::WebviewUrl::External(admin_url),
      )
      // This window holds shell, tunnel and credential capabilities; it may only
      // ever show the panel itself, so a stray link cannot carry them elsewhere.
      .on_navigation(move |url| {
        let ok = is_admin_page(url, &allowed);
        if !ok {
          tracing::warn!("[TournamentAdmin] blocked navigation to {url}");
        }
        ok
      })
      .title("XFChess Tournament Admin")
      .inner_size(1200.0, 800.0)
      .min_inner_size(800.0, 600.0)
      .resizable(true)
      .decorations(false)
      .shadow(true)
      .center()
      .build()
      {
        Ok(win) => {
          crate::apply_xfchess_window_icon(&win, &app);
          // Close the SSH tunnel with the admin window so its forwarded port is released.
          let tunnel_app = app.clone();
          win.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
              kill_admin_tunnel_inner(&tunnel_app);
            }
          });
          let _ = win.show();
          let _ = win.set_focus();
        }
        Err(e) => tracing::error!("[TournamentAdmin] failed to create window: {e}"),
      }
    }
  });
}

#[cfg(not(feature = "tournament-admin"))]
pub(crate) fn open_tournament_admin(_app: &tauri::AppHandle) {
  tracing::warn!(
    "[TournamentAdmin] admin panel is not compiled into this build (needs --features tournament-admin)"
  );
}

#[cfg(all(test, feature = "tournament-admin"))]
mod tests {
  use super::{admin_window_url, is_admin_page, validate_tunnel_args};

  #[test]
  fn panel_url_is_always_localhost_under_tournament_admin() -> Result<(), Box<dyn std::error::Error>>
  {
    assert_eq!(
      admin_window_url(None, 7455)?.as_str(),
      "http://localhost:7455/tournament-admin/"
    );
    assert_eq!(
      admin_window_url(Some("http://127.0.0.1:5176/"), 7454)?.as_str(),
      "http://localhost:5176/tournament-admin/"
    );
    for rejected in [
      "https://evil.example:443/",
      "http://localhost/",
      "file:///c:/x.html",
    ] {
      assert_eq!(
        admin_window_url(Some(rejected), 7454)?.as_str(),
        "http://localhost:7454/tournament-admin/",
        "{rejected}"
      );
    }

    Ok(())
  }

  #[test]
  fn navigation_stays_on_the_panel() -> Result<(), Box<dyn std::error::Error>> {
    let panel = admin_window_url(None, 7454)?;
    let ok = |s: &str| tauri::Url::parse(s).is_ok_and(|url| is_admin_page(&url, &panel));
    assert!(ok("http://localhost:7454/tournament-admin/"));
    assert!(ok("http://localhost:7454/tournament-admin/assets/index.js"));
    assert!(!ok("http://localhost:7454/wallet"));
    assert!(!ok("http://localhost:7455/tournament-admin/"));
    assert!(!ok("https://xfchess.com/tournament-admin/"));

    Ok(())
  }

  #[test]
  fn accepts_the_panels_own_tunnel_config() {
    assert!(validate_tunnel_args(
      "C:/Users/isich/.ssh/xfchess_vps",
      "tunnel",
      "178.104.55.19",
      "127.0.0.1"
    )
    .is_ok());
  }

  #[test]
  fn rejects_values_ssh_would_read_as_options() {
    let key = "C:/k";
    assert!(validate_tunnel_args(key, "-oProxyCommand=calc", "1.2.3.4", "127.0.0.1").is_err());
    assert!(validate_tunnel_args(key, "tunnel", "-oProxyCommand=calc", "127.0.0.1").is_err());
    assert!(validate_tunnel_args(key, "tunnel", "1.2.3.4", "-x").is_err());
    assert!(validate_tunnel_args("-oProxyCommand=calc", "tunnel", "1.2.3.4", "127.0.0.1").is_err());
    assert!(validate_tunnel_args(key, "a b", "1.2.3.4", "127.0.0.1").is_err());
  }
}
