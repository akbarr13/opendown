#!/bin/bash
# End-to-end grabber test: simulates exactly what the browser extension POSTs to the app.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WINROOT="$(cd "$ROOT" && pwd -W)"
cd "$ROOT" || exit 1
mkdir -p demo /c/Temp
dd if=/dev/zero of=demo/grab_test.bin bs=1M count=24 status=none
python - <<'PY'
import json, os
d = os.path.join(os.environ["APPDATA"], "OpenDown")
os.makedirs(d, exist_ok=True)
cfg = {"default_dir": r"C:\Temp\opendown-grab"}
open(os.path.join(d, "config.json"), "w").write(json.dumps(cfg))
print("cfg seeded")
PY
python "$WINROOT/scripts/test_server.py" demo/grab_test.bin --port 8131 --throttle 0.05 & SRV=$!
sleep 1
OPENDOWN_SMOKE_MS=22000 ./target/debug/opendown.exe > _grab.log 2>&1 & APP=$!
sleep 6
echo "--- GET /ping:"; curl -s http://127.0.0.1:10081/ping; echo
echo "--- OPTIONS preflight:"; curl -s -o /dev/null -w "%{http_code}" -X OPTIONS http://127.0.0.1:10081/add; echo
echo "--- POST /add (extension payload):"
curl -s -X POST http://127.0.0.1:10081/add -H "Content-Type: application/json" -d '{"url":"http://127.0.0.1:8131/grab_test.bin","referer":"https://example.com/watch","filename":"grab_test.bin","cookie":"session=abc123"}'; echo
sleep 6
F=$(find /c/Temp/opendown-grab -type f -name 'grab_test.bin' 2>/dev/null | head -1)
echo "--- sha256 compare:"
if [ -n "$F" ]; then sha256sum demo/grab_test.bin "$F"; else echo "FILE MISSING"; fi
wait $APP
echo "app_exit=$?"
kill $SRV 2>/dev/null
echo "--- cleanup"
rm -f "$APPDATA/OpenDown/tasks.json" "$APPDATA/OpenDown/config.json"
rm -rf demo /c/Temp/opendown-grab _grab.log
echo CLEAN_DONE
