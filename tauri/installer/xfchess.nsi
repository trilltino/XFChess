; XFChess Windows installer (NSIS)
; -----------------------------------------------------------------------------
; Bundles the three binaries (game + wallet bridge + stockfish) plus assets into
; a single signed Setup.exe. Built and signed in CI by .github/workflows/release.yml.
;
; The staged payload is expected next to this script under ..\..\release\win\:
;   xfchess.exe          (game, main app)
;   xfchess-tauri.exe    (wallet bridge companion)
;   stockfish.exe        (chess engine)
;   assets\              (game assets)
;   tournament-admin\dist\ (admin UI, served by the bridge)
; All .exe files MUST already be Authenticode-signed before makensis runs, then
; the resulting Setup.exe is signed too. See DISTRIBUTION.md.

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
; nginx serves the frontend + API from the same domain (see deploy/nginx/nginx.conf) —
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
!define MUI_FINISHPAGE_RUN "$INSTDIR\launch.bat"
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${APP_NAME}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"

  File "${PAYLOAD_DIR}\${APP_EXE}"
  File "${PAYLOAD_DIR}\${BRIDGE_EXE}"
  File /nonfatal "${PAYLOAD_DIR}\stockfish.exe"

  SetOutPath "$INSTDIR\assets"
  File /r "${PAYLOAD_DIR}\assets\*.*"

  ; Tournament-admin UI dist served by the wallet bridge (optional)
  SetOutPath "$INSTDIR\tournament-admin\dist"
  File /nonfatal /r "${PAYLOAD_DIR}\tournament-admin\dist\*.*"

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

  ; Shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\launch.bat" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\launch.bat" "" "$INSTDIR\${APP_EXE}" 0

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
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\assets"
  RMDir /r "$INSTDIR\tournament-admin"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
  DeleteRegKey HKLM "Software\${APP_NAME}"
SectionEnd
