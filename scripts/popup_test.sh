#!/usr/bin/env bash
# OpenDown popup-windows E2E test -> grab -> confirm popup -> download -> complete window.
# Also verifies confirm_downloads=false auto-start path and debug-cleanliness.
# Note: tests the click path by synthesizing real mouse input (click_at.ps1).
set -u
cd "$(dirname "$0")/.."
PS="powershell -NoProfile -ExecutionPolicy Bypass -File"
TA='C:/Users/pipu/AppData/Local/Temp/odwin'
TB=/c/Users/pipu/AppData/Local/Temp/odwin

echo "=== TEST A: app in TRAY + grab -> confirm popup -> full flow ==="
taskkill /F /IM opendown.exe >/dev/null 2>&1
# self-contained test server + file
mkdir -p "$TB/srv"
head -c 3000000 /dev/urandom > "$TB/srv/testfile.zip"
SPID=$(netstat -ano 2>/dev/null | grep ":8123" | grep -i listen | awk '{print $NF}' | head -1)
[ -n "${SPID:-}" ] && taskkill /F /PID $SPID >/dev/null 2>&1
python -m http.server 8123 --directory 'C:/Users/pipu/AppData/Local/Temp/odwin/srv' >/dev/null 2>&1 &
sleep 1.5
rm -rf "$TB/OpenDown"; mkdir -p "$TB/OpenDown"
printf '{"default_dir":"C:/Users/pipu/AppData/Local/Temp/odwin/dl","speed_preset":2}' > "$TB/OpenDown/config.json"
rm -f "$TB/dl/Compressed/testfile.zip"
APPDATA="$TA" ./target/release/opendown.exe --minimized >/dev/null 2>&1 &
sleep 3
echo "-- app started minimized; windows:"
timeout 60 $PS scripts/win_list.ps1 | head -4
curl -s -m 3 -X POST http://127.0.0.1:10081/add -H "Content-Type: application/json" -d '{"url":"http://127.0.0.1:8123/testfile.zip","filename":"testfile.zip","referer":"","cookie":""}' >/dev/null
sleep 2
echo "-- after grab (expect 'start download' window VISIBLE on screen):"
LINE=$(timeout 60 $PS scripts/win_list.ps1 | grep "start download" | head -1)
echo "$LINE"
XY=$(echo "$LINE" | grep -o 'POS=([-0-9,]*' | tr -d 'POS=(')
PX=${XY%,*}; PY=${XY#*,}
timeout 45 $PS scripts/click_at.ps1 -X $((PX + 331)) -Y $((PY + 188)) -Hwnd $(echo "$LINE" | grep -o 'H=[0-9]*' | tr -d 'H=') >/dev/null
sleep 8
echo "-- after click + download (expect file + complete window):"
ls -la "$TB/dl/Compressed/" 2>/dev/null | grep testfile && sha256sum "$TB/dl/Compressed/testfile.zip" | awk '{print $1}'
timeout 60 $PS scripts/win_list.ps1 | head -4
echo "-- win.log exists? (should NOT, no WINDEBUG):"
ls "$TB/OpenDown/win.log" 2>/dev/null || echo "no win.log (clean) ✓"

echo "=== TEST B: confirm OFF -> auto start, no popup ==="
taskkill /F /IM opendown.exe >/dev/null 2>&1
rm -rf "$TB/OpenDown"; mkdir -p "$TB/OpenDown"
printf '{"default_dir":"C:/Users/pipu/AppData/Local/Temp/odwin/dl","speed_preset":2,"confirm_downloads":false}' > "$TB/OpenDown/config.json"
rm -f "$TB/dl/Compressed/testfile.zip"
APPDATA="$TA" ./target/release/opendown.exe >/dev/null 2>&1 &
sleep 3
curl -s -m 3 -X POST http://127.0.0.1:10081/add -H "Content-Type: application/json" -d '{"url":"http://127.0.0.1:8123/testfile.zip","filename":"testfile.zip","referer":"","cookie":""}' >/dev/null
sleep 2
echo "-- windows (expect NO 'start download', expect download running/complete):"
timeout 60 $PS scripts/win_list.ps1 | head -4
sleep 7
ls -la "$TB/dl/Compressed/" 2>/dev/null | grep testfile && echo "TEST B file ✓"
taskkill /F /IM opendown.exe >/dev/null 2>&1
SPID=$(netstat -ano 2>/dev/null | grep ":8123" | grep -i listen | awk '{print $NF}' | head -1)
[ -n "${SPID:-}" ] && taskkill /F /PID $SPID >/dev/null 2>&1
echo FINAL_DONE
