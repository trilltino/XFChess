@echo off
echo The tournament admin panel is desktop-only (no web dev server).
echo Delegating to the desktop launcher...

pushd "%~dp0..\..\.."
just admin
set "LAUNCH_EXIT=%ERRORLEVEL%"
popd
exit /b %LAUNCH_EXIT%
