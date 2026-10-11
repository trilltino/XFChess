use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
  OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_WIN32,
  PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};

/// The executable path of the process behind `handle`, which must have been opened
/// with `PROCESS_QUERY_LIMITED_INFORMATION`.
pub(super) fn process_image_path(handle: HANDLE) -> windows::core::Result<String> {
  // Windows paths are at most 32,767 UTF-16 units; kept on the heap, not the stack.
  let mut path = vec![0u16; 32_768];
  let mut len = path.len() as u32;
  // SAFETY: `path` is a writable buffer of `len` units that outlives the call.
  unsafe {
    QueryFullProcessImageNameW(
      handle,
      PROCESS_NAME_WIN32,
      windows::core::PWSTR(path.as_mut_ptr()),
      &mut len,
    )?;
  }
  Ok(String::from_utf16_lossy(&path[..len as usize]))
}

pub(super) struct GameProcess(OwnedHandle);

impl GameProcess {
  pub(super) fn open(pid: u32) -> Result<Self, String> {
    unsafe {
      let handle = OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
        false,
        pid,
      )
      .map_err(|error| error.to_string())?;
      let owned = OwnedHandle::from_raw_handle(handle.0);
      let name = process_image_path(handle).map_err(|error| error.to_string())?;
      if !std::path::Path::new(&name)
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("xfchess.exe"))
      {
        return Err("Only the XFChess game may own the wallet bridge".into());
      }
      Ok(Self(owned))
    }
  }

  pub(super) fn wait(&self) -> Result<(), String> {
    let result = unsafe { WaitForSingleObject(HANDLE(self.0.as_raw_handle()), u32::MAX) };
    if result == WAIT_OBJECT_0 {
      Ok(())
    } else {
      Err(format!("Game process wait failed: {}", result.0))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rejects_a_process_that_is_not_the_game() {
    assert!(GameProcess::open(std::process::id()).is_err());
  }

  #[test]
  fn child_fixture() {
    if std::env::var_os("XFCHESS_WALLET_TEST_CHILD").is_some() {
      std::thread::sleep(std::time::Duration::from_millis(500));
    }
  }

  #[test]
  fn waits_for_the_game_process_to_exit() {
    let directory =
      std::env::temp_dir().join(format!("xfchess-wallet-process-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join("xfchess.exe");
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    let mut child = std::process::Command::new(&executable)
      .args(["--exact", "wallet::process::tests::child_fixture"])
      .env("XFCHESS_WALLET_TEST_CHILD", "1")
      .spawn()
      .unwrap();
    let game = GameProcess::open(child.id()).unwrap();
    game.wait().unwrap();
    assert!(child.wait().unwrap().success());
    drop(game);
    std::fs::remove_file(executable).unwrap();
    std::fs::remove_dir(directory).unwrap();
  }
}
