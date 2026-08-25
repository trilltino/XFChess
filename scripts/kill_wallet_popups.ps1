# scripts/kill_wallet_popups.ps1
# Closes any leftover XFChess wallet-popup Chrome/Edge windows.
# The popup's document.title is "XFChess #<port>" (tauri/wallet-ui/src/App.tsx),
# so we FindWindow each likely port and post a real WM_CLOSE - the same strategy
# the Tauri app's kill_wallet_popup() uses. This is the key step that stops stale
# popups (running old JS, spamming "[SIGNER] SSE connection error") from
# surviving `just kill` / `just dev`. We must NOT kill all chrome.exe, so the
# user's normal browser windows are never touched.
$ErrorActionPreference = 'SilentlyContinue'

$src = @'
using System;
using System.Runtime.InteropServices;
public class XfWinCloser {
  [DllImport("user32.dll")]
  public static extern long FindWindow(String cls, String name);
  [DllImport("user32.dll")]
  public static extern bool PostMessage(long hWnd, int msg, int wParam, int lParam);
  public static int Closed = 0;
  public static int CloseWindows() {
    Closed = 0;
    for (int p = 7440; p <= 7485; p++) {
      long h = FindWindow(null, "XFChess #" + p);
      if (h != 0) {
        PostMessage(h, 0x0010, 0, 0); // WM_CLOSE
        Closed++;
      }
    }
    return Closed;
  }
}
'@

Add-Type $src
$closed = [XfWinCloser]::CloseWindows()
if ($closed -gt 0) {
    Write-Host "  Closed $closed leftover wallet-popup window(s)" -ForegroundColor Yellow
}