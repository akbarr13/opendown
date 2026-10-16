@echo off
setlocal
set "EXTID=manelgpcmclgjpecpncekbbbhjohndhp"
set "VER=1.0.0"
set "HERE=%~dp0"
set "CRX=%HERE%opendown-extension-v1.0.0.crx"

if /i "%~1"=="remove" goto remove

echo Registering OpenDown browser integration for the current user...
reg add "HKCU\Software\Microsoft\Edge\Extensions\%EXTID%" /v path /t REG_SZ /d "%CRX%" /f >nul
reg add "HKCU\Software\Microsoft\Edge\Extensions\%EXTID%" /v version /t REG_SZ /d "%VER%" /f >nul
reg add "HKCU\Software\Google\Chrome\Extensions\%EXTID%" /v path /t REG_SZ /d "%CRX%" /f >nul
reg add "HKCU\Software\Google\Chrome\Extensions\%EXTID%" /v version /t REG_SZ /d "%VER%" /f >nul
echo Done. If Edge/Chrome is already running, restart it - the extension will
echo appear on edge://extensions / chrome://extensions.
start "" msedge "edge://extensions" 2>nul
start "" chrome "chrome://extensions" 2>nul
echo.
pause
exit /b 0

:remove
echo Removing OpenDown browser integration...
reg delete "HKCU\Software\Microsoft\Edge\Extensions\%EXTID%" /f >nul 2>nul
reg delete "HKCU\Software\Google\Chrome\Extensions\%EXTID%" /f >nul 2>nul
echo Done. The extension disappears the next time Edge/Chrome starts.
pause
exit /b 0
