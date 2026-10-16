@echo off
rem OpenDown GUI launcher (starts the app detached, this console exits)
cd /d "%~dp0"
if exist "target\release\opendown.exe" (
  start "" "target\release\opendown.exe" %*
) else (
  start "" "target\debug\opendown.exe" %*
)
