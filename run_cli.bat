@echo off
rem OpenDown CLI launcher
cd /d "%~dp0"
if exist "target\release\opendown.exe" (
  "target\release\opendown.exe" --cli %*
) else (
  "target\debug\opendown.exe" --cli %*
)
