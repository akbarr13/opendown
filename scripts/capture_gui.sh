#!/bin/bash
# Dev helper: launch the GUI briefly, capture its window, wait for exit.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WINROOT="$(cd "$ROOT" && pwd -W)"
cd "$ROOT" || exit 1
OPENDOWN_SMOKE_MS=20000 ./target/debug/opendown.exe > _gui4.log 2>&1 &
APP=$!
sleep 7
echo "--- proc check ---"
powershell -ExecutionPolicy Bypass -File "$WINROOT/scripts/check_proc.ps1"
echo "--- window shot ---"
powershell -ExecutionPolicy Bypass -File "$WINROOT/scripts/shot.ps1"
cp -f _gui_shot.png _gui_shot_final.png 2>/dev/null
sleep 2
powershell -ExecutionPolicy Bypass -File "$WINROOT/scripts/shot.ps1"
cp -f _gui_shot.png _gui_shot_final2.png 2>/dev/null
wait $APP
echo "app_exit=$?"
cat _gui4.log
ls -la _gui_shot_final*.png 2>/dev/null
