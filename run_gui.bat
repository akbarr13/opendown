@echo off
rem OpenDown GUI launcher
cd /d "%~dp0"
if exist "target\release\opendown.exe" (
  "target\release\opendown.exe" %*
) else (
  "target\debug\opendown.exe" %*
)
