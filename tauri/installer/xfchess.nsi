; XFChess Windows installer (NSIS)
; -----------------------------------------------------------------------------
; Bundles the three binaries (game + wallet bridge + stockfish) plus assets into
; a single signed Setup.exe. Built and signed in CI by .github/workflows/release.yml.
;
; The staged payload is expected next to this script under ..\..\release\win\:
;   xfchess.exe          (game, main app)
;   xfchess-tauri.exe    (wallet bridge companion)
;   stockfish.exe        (chess engine — mandatory, release.yml fails the job
;                          before this script even runs if it couldn't be fetched)
;   assets\              (game assets)
;   wallet-ui\dist\       (built wallet-signing popup UI, served by xfchess-tauri)
; All .exe files MUST already be Authenticode-signed before makensis runs, then
; the resulting Setup.exe is signed too. See docs/PUBLISHING.md.

!define APP_NAME      "XFChess"
!define APP_PUBLISHER "trilltino"
!define APP_EXE       "xfchess.exe"
!define BRIDGE_EXE    "xfchess-tauri.exe"
!define APP_URL       "https://xfchess.com"
!ifndef APP_VERSION
  !define APP_VERSION "0.1.0"
!endif
!ifndef PAYLOAD_DIR
  !define PAYLOAD_DIR "..\..\release\win"
!endif

; Production backend endpoints baked into the launcher. Override at build time:
;   makensis /DBACKEND_URL=https://xfchess.com /DSIGNING_URL=https://xfchess.com xfchess.nsi
; nginx serves the frontend + API from the same domain (see ops/nginx/nginx.conf) —
; there's no separate api.* subdomain.
!ifndef BACKEND_URL
  !define BACKEND_URL "https://xfchess.com"
!endif
!ifndef SIGNING_URL
  !define SIGNING_URL "https://xfchess.com"
!endif

Unicode true
SetCompressor /SOLID lzma
Name "${APP_NAME} ${APP_VERSION}"
OutFile "..\..\release\XFChess-Setup-${APP_VERSION}.exe"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel admin
BrandingText "${APP_NAME} ${APP_VERSION}"

!include "MUI2.nsh"
!define MUI_ICON   "..\icons\icon.ico"
!define MUI_UNICON "..\icons\icon.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\launch.vbs"
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${APP_NAME}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  ; Kill any already-running instance first. Overwriting a locked binary below
  ; fails with a bare "Error opening file for writing <name>.exe" — no
  ; explanation that it's because a previous session's process (game or wallet
  ; bridge) is still holding the file open. Silent (nsExec, not Exec) so no
  ; console flash; ignore the exit code — "no such process" is the common,
  ; harmless case on a first-ever install.
  nsExec::Exec 'taskkill /F /IM ${APP_EXE} /T'
  nsExec::Exec 'taskkill /F /IM ${BRIDGE_EXE} /T'
  Sleep 500

  SetOutPath "$INSTDIR"

  File "${PAYLOAD_DIR}\${APP_EXE}"
  File "${PAYLOAD_DIR}\${BRIDGE_EXE}"
  File "${PAYLOAD_DIR}\stockfish.exe"

  SetOutPath "$INSTDIR\assets"
  File /r "${PAYLOAD_DIR}\assets\*.*"

  ; wallet-ui: served by xfchess-tauri itself from wallet-ui\dist next to its
  ; own exe (resolved via current_exe().parent() — see main.rs). Without this
  ; the wallet-signing popup has nowhere real to load.
  SetOutPath "$INSTDIR\wallet-ui\dist"
  File /r "${PAYLOAD_DIR}\wallet-ui\dist\*.*"

  ; Launcher: sets production endpoints, starts the wallet bridge, then the game.
  ; This completes the dev .bat (which started only the bridge).
  SetOutPath "$INSTDIR"
  FileOpen $0 "$INSTDIR\launch.bat" w
  FileWrite $0 "@echo off$\r$\n"
  FileWrite $0 "setlocal$\r$\n"
  FileWrite $0 "set SCRIPT_DIR=%~dp0$\r$\n"
  FileWrite $0 "set BACKEND_URL=${BACKEND_URL}$\r$\n"
  FileWrite $0 "set SIGNING_SERVICE_URL=${SIGNING_URL}$\r$\n"
  FileWrite $0 "start $\"XFChess Wallet$\" /D $\"%SCRIPT_DIR%$\" $\"%SCRIPT_DIR%${BRIDGE_EXE}$\"$\r$\n"
  FileWrite $0 "start $\"XFChess$\" /D $\"%SCRIPT_DIR%$\" $\"%SCRIPT_DIR%${APP_EXE}$\"$\r$\n"
  FileWrite $0 "endlocal$\r$\n"
  FileClose $0

  ; Hidden launcher: a shortcut targeting launch.bat directly makes cmd.exe
  ; flash a visible console window while it hosts the batch script, even
  ; though xfchess.exe/xfchess-tauri.exe both suppress their own console
  ; (windows_subsystem="windows" in release builds) — cmd.exe itself is what's
  ; visible, not either app. Route shortcuts through a VBScript wrapper that
  ; runs launch.bat with a hidden window style (0) instead.
  FileOpen $1 "$INSTDIR\launch.vbs" w
  FileWrite $1 "CreateObject($\"WScript.Shell$\").Run $\"$\"$\"$INSTDIR\launch.bat$\"$\"$\", 0, False$\r$\n"
  FileClose $1

  ; Second-instance launcher: sets a distinct XFCHESS_WALLET_PORT (and node
  ; identity path) before starting its own bridge+game pair, so testing
  ; multiplayer against yourself on one PC actually works — two instances
  ; launched from the plain shortcut both default to the same port, so the
  ; second bridge's HTTP server either fails outright or (now) falls back to
  ; a port neither the game nor the wallet popup can reliably discover,
  ; since nothing distinguishes "this instance's" bridge from any other's.
  ; An explicit different port sidesteps that ambiguity entirely instead of
  ; trying to guess it away after the fact. Mirrors `just dev2`'s P2 setup.
  FileOpen $2 "$INSTDIR\launch-second-instance.bat" w
  FileWrite $2 "@echo off$\r$\n"
  FileWrite $2 "setlocal$\r$\n"
  FileWrite $2 "set SCRIPT_DIR=%~dp0$\r$\n"
  FileWrite $2 "set BACKEND_URL=${BACKEND_URL}$\r$\n"
  FileWrite $2 "set SIGNING_SERVICE_URL=${SIGNING_URL}$\r$\n"
  FileWrite $2 "set XFCHESS_WALLET_PORT=7464$\r$\n"
  FileWrite $2 "set XFCHESS_NODE_KEY_PATH=%LOCALAPPDATA%\xfchess\node_key_2$\r$\n"
  FileWrite $2 "start $\"XFChess Wallet (2nd)$\" /D $\"%SCRIPT_DIR%$\" $\"%SCRIPT_DIR%${BRIDGE_EXE}$\"$\r$\n"
  FileWrite $2 "start $\"XFChess (2nd)$\" /D $\"%SCRIPT_DIR%$\" $\"%SCRIPT_DIR%${APP_EXE}$\"$\r$\n"
  FileWrite $2 "endlocal$\r$\n"
  FileClose $2

  FileOpen $3 "$INSTDIR\launch-second-instance.vbs" w
  FileWrite $3 "CreateObject($\"WScript.Shell$\").Run $\"$\"$\"$INSTDIR\launch-second-instance.bat$\"$\"$\", 0, False$\r$\n"
  FileClose $3

  ; Shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\launch.vbs" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME} (2nd Instance).lnk" "$INSTDIR\launch-second-instance.vbs" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\launch.vbs" "" "$INSTDIR\${APP_EXE}" 0

  ; Registry / Add-Remove Programs
  WriteRegStr HKLM "Software\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "URLInfoAbout" "${APP_URL}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoRepair" 1

  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\${BRIDGE_EXE}"
  Delete "$INSTDIR\stockfish.exe"
  Delete "$INSTDIR\launch.bat"
  Delete "$INSTDIR\launch.vbs"
  Delete "$INSTDIR\launch-second-instance.bat"
  Delete "$INSTDIR\launch-second-instance.vbs"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\assets"
  RMDir /r "$INSTDIR\wallet-ui"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME} (2nd Instance).lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
  DeleteRegKey HKLM "Software\${APP_NAME}"
SectionEnd
