@echo off
echo The tournament admin panel is desktop-only (no web dev server).
echo Delegating to the desktop launcher...

call "%~dp0..\..\..\scripts\start-tournament-admin.bat"
