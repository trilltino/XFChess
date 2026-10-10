; Package the signed payload from release/win. Sign all binaries before
; makensis, then sign Setup.exe. See docs/PUBLISHING.md.

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

; Override endpoints with makensis /DBACKEND_URL=... /DSIGNING_URL=... .
; Production serves frontend and API from the same domain.
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
  ; Stop running binaries before overwriting them; missing processes are harmless.
  nsExec::Exec 'taskkill /F /IM ${APP_EXE} /T'
  nsExec::Exec 'taskkill /F /IM ${BRIDGE_EXE} /T'
  Sleep 500

  SetOutPath "$INSTDIR"

  File "${PAYLOAD_DIR}\${APP_EXE}"
  File "${PAYLOAD_DIR}\${BRIDGE_EXE}"
  File "${PAYLOAD_DIR}\stockfish.exe"

  SetOutPath "$INSTDIR\assets"
  File /r "${PAYLOAD_DIR}\assets\*.*"

  ; Ship wallet-ui/dist beside the companion executable so its signing popup can load.
  SetOutPath "$INSTDIR\wallet-ui\dist"
  File /r "${PAYLOAD_DIR}\wallet-ui\dist\*.*"

  ; Stop stale bridge/game processes before launching so cached wallet identity
  ; and bound bridge ports cannot leak between sessions.
  SetOutPath "$INSTDIR"
  FileOpen $0 "$INSTDIR\launch.bat" w
  FileWrite $0 "@echo off$\r$\n"
  FileWrite $0 "setlocal$\r$\n"
  FileWrite $0 "set SCRIPT_DIR=%~dp0$\r$\n"
  ; Strip the trailing backslash from SCRIPT_DIR before passing it to start /D.
  FileWrite $0 "set SCRIPT_DIR_Q=%SCRIPT_DIR:~0,-1%$\r$\n"
  FileWrite $0 "set BACKEND_URL=${BACKEND_URL}$\r$\n"
  FileWrite $0 "set SIGNING_SERVICE_URL=${SIGNING_URL}$\r$\n"
  FileWrite $0 "taskkill /F /IM ${BRIDGE_EXE} >nul 2>&1$\r$\n"
  FileWrite $0 "taskkill /F /IM ${APP_EXE} >nul 2>&1$\r$\n"
  FileWrite $0 "timeout /t 1 /nobreak >nul$\r$\n"
  FileWrite $0 "start $\"XFChess Wallet$\" /D $\"%SCRIPT_DIR_Q%$\" $\"%SCRIPT_DIR%${BRIDGE_EXE}$\"$\r$\n"
  FileWrite $0 "start $\"XFChess$\" /D $\"%SCRIPT_DIR_Q%$\" $\"%SCRIPT_DIR%${APP_EXE}$\"$\r$\n"
  FileWrite $0 "endlocal$\r$\n"
  FileClose $0

  ; Use the hidden VBScript wrapper so shortcuts do not flash a cmd window.
  FileOpen $1 "$INSTDIR\launch.vbs" w
  FileWrite $1 "CreateObject($\"WScript.Shell$\").Run $\"$\"$\"$INSTDIR\launch.bat$\"$\"$\", 0, False$\r$\n"
  FileClose $1

  ; Give the second instance its own bridge port and node identity, matching just dev2.
  FileOpen $2 "$INSTDIR\launch-second-instance.bat" w
  FileWrite $2 "@echo off$\r$\n"
  FileWrite $2 "setlocal$\r$\n"
  FileWrite $2 "set SCRIPT_DIR=%~dp0$\r$\n"
  FileWrite $2 "set SCRIPT_DIR_Q=%SCRIPT_DIR:~0,-1%$\r$\n"
  FileWrite $2 "set BACKEND_URL=${BACKEND_URL}$\r$\n"
  FileWrite $2 "set SIGNING_SERVICE_URL=${SIGNING_URL}$\r$\n"
  FileWrite $2 "set XFCHESS_WALLET_PORT=7464$\r$\n"
  FileWrite $2 "set XFCHESS_NODE_KEY_PATH=%LOCALAPPDATA%\xfchess\node_key_2$\r$\n"
  FileWrite $2 "start $\"XFChess Wallet (2nd)$\" /D $\"%SCRIPT_DIR_Q%$\" $\"%SCRIPT_DIR%${BRIDGE_EXE}$\"$\r$\n"
  FileWrite $2 "start $\"XFChess (2nd)$\" /D $\"%SCRIPT_DIR_Q%$\" $\"%SCRIPT_DIR%${APP_EXE}$\"$\r$\n"
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
