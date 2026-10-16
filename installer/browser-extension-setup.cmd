@echo off
setlocal
set "EXTID=manelgpcmclgjpecpncekbbbhjohndhp"
set "HERE=%~dp0"
set "UNPACKED=%HERE%unpacked"

if /i "%~1"=="remove" goto remove

echo ============================================================
echo  OpenDown browser extension - setup helper
echo ============================================================
echo.
echo  Current browsers block automatic (off-store) extension installs,
echo  so the extension needs to be enabled once, manually:
echo.
echo    1. In the browser, open   edge://extensions   or   chrome://extensions
echo    2. Turn ON "Developer mode"
echo    3. Click "Load unpacked" and select this folder:
echo       %UNPACKED%
echo       ^(the path is already in your clipboard - just paste it^)
echo    4. Done. Keep "Developer mode" ON while using the extension.
echo.
echo  A "developer mode extensions" warning on browser startup is normal.
echo.
echo Opening the folder and the browser extension page now...
echo %UNPACKED%| clip
start "" explorer "%UNPACKED%"
start "" msedge "edge://extensions" 2>nul
start "" chrome "chrome://extensions" 2>nul
echo.
pause
exit /b 0

:remove
echo Removing old automatic-install registry entries (v0.1.0 installer method)...
reg delete "HKCU\Software\Microsoft\Edge\Extensions\%EXTID%" /f >nul 2>nul
reg delete "HKCU\Software\Google\Chrome\Extensions\%EXTID%" /f >nul 2>nul
echo Done. If the extension still shows as disabled in the browser, remove it on
echo edge://extensions / chrome://extensions and restart the browser.
pause
exit /b 0
