#!/usr/bin/env bash
# Tray + grab regression test. Uses an isolated APPDATA and scripts/tray_sim.ps1,
# which posts the same tray-callback messages the Windows shell sends (msg 6002),
# with the hidden-icons flyout opened first when needed.
set -u
cd "$(dirname "$0")/.."
PS="powershell -NoProfile -ExecutionPolicy Bypass -File"
TA='C:/Users/pipu/AppData/Local/Temp/odtray'
TB=/c/Users/pipu/AppData/Local/Temp/odtray

[ -f target/release/opendown.exe ] || { echo "build release first"; exit 1; }
kill_all() { taskkill /F /IM opendown.exe >/dev/null 2>&1; sleep 1; }
count_tasks() { python -c "import json;print(len(json.load(open(r'C:/Users/pipu/AppData/Local/Temp/odtray/OpenDown/tasks.json'))))" 2>/dev/null || echo 0; }
post1() { curl -s -m 3 -X POST http://127.0.0.1:10081/add -H "Content-Type: application/json" -d "$1" >/dev/null; }

echo "== B1: --minimized starts off-screen (expect POS=(-32000,-32000)) =="
kill_all; rm -rf "$TB"; mkdir -p "$TB"
APPDATA="$TA" ./target/release/opendown.exe --minimized >/dev/null 2>&1 &
sleep 2.5
$PS scripts/win_list.ps1

echo "== B2: hidden app still processes grabs (expect tasks: 1) =="
post1 '{"url":"http://127.0.0.1:9/h1.bin","filename":"h1.bin","referer":"","cookie":""}'
sleep 2.5
echo "tasks: $(count_tasks)"

echo "== B3: tray click -> Show (expect on-screen POS) =="
$PS scripts/tray_sim.ps1 -Action show 2>&1 | tail -2
sleep 1.3
$PS scripts/win_list.ps1

echo "== B4: WM_CLOSE hides; grab while hidden (expect tasks: 2) =="
$PS scripts/win_close.ps1 | head -2
sleep 1.5
post1 '{"url":"http://127.0.0.1:9/h2.bin","filename":"h2.bin","referer":"","cookie":""}'
sleep 2.5
echo "tasks: $(count_tasks)"

echo "== B5: tray right-click -> Exit (expect process gone) =="
$PS scripts/tray_sim.ps1 -Action exit 2>&1 | tail -3
sleep 2
if tasklist 2>/dev/null | grep -qi opendown; then echo "STILL RUNNING (!!)"; else echo "PROCESS_GONE_OK"; fi

kill_all; rm -rf "$TB"
echo TRAY_TEST_DONE
