use super::{PendingTx, PendingTxNotify};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static GAME_PID: AtomicU32 = AtomicU32::new(0);
static STOPPED: AtomicBool = AtomicBool::new(false);

pub(super) fn game_pid() -> u32 {
  GAME_PID.load(Ordering::Acquire)
}

pub(super) fn stopped() -> bool {
  STOPPED.load(Ordering::Acquire)
}

pub(crate) fn shutdown(pending: &PendingTx, notify: &PendingTxNotify) {
  STOPPED.store(true, Ordering::Release);
  if let Some(request) = pending.lock().take() {
    let _ = request.response.send(Err("Game closed".into()));
  }
  notify.send_replace(());
  super::popup::kill_wallet_popup();
  super::remove_port_files();
}

#[cfg(windows)]
pub(crate) fn attach_game(
  pid: u32,
  app: tauri::AppHandle,
  pending: PendingTx,
  notify: PendingTxNotify,
) -> Result<(), String> {
  if pid == 0 || stopped() {
    return Err("Game is not running".into());
  }
  if game_pid() == pid {
    return Ok(());
  }
  let game = super::process::GameProcess::open(pid)?;
  if GAME_PID
    .compare_exchange(0, pid, Ordering::AcqRel, Ordering::Acquire)
    .is_err()
  {
    return Err("This bridge already belongs to another game".into());
  }
  if let Err(error) = std::thread::Builder::new()
    .name("wallet-game-watch".into())
    .spawn(move || {
      if let Err(error) = game.wait() {
        eprintln!("Wallet game watcher: {error}");
      }
      shutdown(&pending, &notify);
      app.exit(0);
    })
  {
    GAME_PID.store(0, Ordering::Release);
    return Err(format!("Cannot watch game process: {error}"));
  }

  Ok(())
}

#[cfg(not(windows))]
pub(crate) fn attach_game(
  _: u32,
  _: tauri::AppHandle,
  _: PendingTx,
  _: PendingTxNotify,
) -> Result<(), String> {
  Err("Native game ownership is currently supported on Windows".into())
}
