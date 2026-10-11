use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::*;

fn browser_window(hwnd: HWND) -> bool {
  unsafe {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
      return false;
    };
    let path = super::process::process_image_path(handle);
    let _ = CloseHandle(handle);
    path.is_ok_and(|path| {
      std::path::Path::new(&path).file_name().is_some_and(|name| {
        matches!(
          name.to_string_lossy().to_lowercase().as_str(),
          "chrome.exe" | "msedge.exe" | "brave.exe" | "vivaldi.exe" | "opera.exe"
        )
      })
    })
  }
}

fn find_window(title: Option<&str>, pid: u32) -> Option<HWND> {
  struct Search<'a> {
    title: Option<&'a str>,
    pid: u32,
    result: Option<HWND>,
  }
  unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
    let search = &mut *(data.0 as *mut Search<'_>);
    if let Some(title) = search.title {
      let mut buffer = [0u16; 256];
      let len = GetWindowTextW(hwnd, &mut buffer);
      if len <= 0
        || String::from_utf16_lossy(&buffer[..len as usize]) != title
        || !browser_window(hwnd)
      {
        return BOOL(1);
      }
    } else {
      let mut pid = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut pid));
      if pid != search.pid
        || !IsWindowVisible(hwnd).as_bool()
        || GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null())
      {
        return BOOL(1);
      }
    }
    search.result = Some(hwnd);
    BOOL(0)
  }
  let mut search = Search {
    title,
    pid,
    result: None,
  };
  unsafe {
    let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut _ as isize));
  }
  search.result
}

fn popup() -> Option<HWND> {
  let title = super::popup::window_title()?;
  find_window(Some(&title), 0)
}

pub(super) fn close() -> bool {
  if let Some(hwnd) = popup() {
    unsafe {
      let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    return true;
  }
  false
}

pub(super) fn hide() -> bool {
  let Some(hwnd) = popup() else {
    return false;
  };
  unsafe {
    let _ = ShowWindow(hwnd, SW_HIDE);
  }
  true
}

pub(super) fn show() -> bool {
  let Some(hwnd) = popup() else {
    return false;
  };
  unsafe {
    let _ = ShowWindow(hwnd, SW_RESTORE);
    let _ = SetForegroundWindow(hwnd);
  }
  true
}

pub(super) fn position(width: i32, height: i32) -> bool {
  let Some(hwnd) = popup() else {
    return false;
  };
  let game = find_window(None, super::lifecycle::game_pid());
  if super::lifecycle::game_pid() != 0 && game.is_none() {
    return false;
  }
  unsafe {
    let mut rect = RECT::default();
    if let Some(game) = game.filter(|game| GetWindowRect(*game, &mut rect).is_ok()) {
      // Ownership keeps the wallet above this game without covering other applications.
      SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, game.0 as isize);
      let x = rect.left + ((rect.right - rect.left - width) / 2).max(0);
      let y = rect.top + ((rect.bottom - rect.top - height) / 2).max(0);
      let _ = SetWindowPos(
        hwnd,
        None,
        x,
        y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
      );
    } else {
      let _ = SetWindowPos(
        hwnd,
        None,
        0,
        0,
        width,
        height,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
      );
    }
  }
  true
}
