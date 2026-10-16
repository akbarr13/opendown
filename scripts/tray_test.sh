#!/usr/bin/env bash
# Regression test for the tray features of the release build:
#   - "--minimized" starts with the window hidden
#   - WM_CLOSE hides the window but keeps OpenDown running (close-to-tray)
set -u
cd "$(dirname "$0")/.."
ROOT=$(pwd -W 2>/dev/null || pwd)
BIN=./target/release/opendown.exe
PS="powershell -NoProfile -ExecutionPolicy Bypass -File"
WEXE="$ROOT\\target\\release\\opendown.exe"

[ -f "$BIN" ] || { echo "build release first: cargo build --release"; exit 1; }

kill_all() { taskkill /F /IM opendown.exe >/dev/null 2>&1; sleep 1; }

echo "== minimized start (expect the app window VIS=False) =="
kill_all
"$BIN" --minimized >/dev/null 2>&1 &
sleep 2.5
$PS scripts/win_list.ps1

echo "== close-to-tray (expect VIS=False after WM_CLOSE, process still alive) =="
kill_all
powershell -NoProfile -Command "Start-Process -FilePath '$WEXE'"
sleep 4
$PS scripts/win_close.ps1
sleep 2
$PS scripts/win_list.ps1
echo "-- process alive? --"
tasklist 2>/dev/null | grep -i opendown | head -3

kill_all
echo "TRAY_TEST_DONE"
