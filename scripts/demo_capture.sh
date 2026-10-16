#!/bin/bash
# Demo capture: seed 3 downloads -> run the GUI with test hooks -> screenshot mid-download.
# Requires a debug build (cargo build) and Python on PATH.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WINROOT="$(cd "$ROOT" && pwd -W)"
cd "$ROOT" || exit 1
mkdir -p demo /c/Temp
dd if=/dev/zero of=demo/video_clip.mp4 bs=1M count=48 status=none
dd if=/dev/zero of=demo/song.mp3 bs=1M count=48 status=none
dd if=/dev/zero of=demo/archive.zip bs=1M count=48 status=none
python - <<'PY'
import json, os
p = os.path.join(os.environ["APPDATA"], "OpenDown", "tasks.json")
os.makedirs(os.path.dirname(p), exist_ok=True)
d = r"C:\Temp\opendown-demo"
tasks = [
    {"url": "http://127.0.0.1:8123/video_clip.mp4", "filename": "video_clip.mp4", "save_dir": d, "connections": 8, "referer": ""},
    {"url": "http://127.0.0.1:8124/song.mp3", "filename": "song.mp3", "save_dir": d, "connections": 8, "referer": ""},
    {"url": "http://127.0.0.1:8125/archive.zip", "filename": "archive.zip", "save_dir": d, "connections": 8, "referer": ""},
]
open(p, "w").write(json.dumps(tasks, indent=2))
print("seeded:", p)
PY
python "$WINROOT/scripts/test_server.py" demo/video_clip.mp4 --port 8123 --throttle 0.5 & S1=$!
python "$WINROOT/scripts/test_server.py" demo/song.mp3 --port 8124 --throttle 0.5 & S2=$!
python "$WINROOT/scripts/test_server.py" demo/archive.zip --port 8125 --throttle 0.5 & S3=$!
sleep 1
OPENDOWN_SMOKE_MS=20000 OPENDOWN_AUTOSTART=1 OPENDOWN_AUTOSEL=1 ./target/debug/opendown.exe > _gui5.log 2>&1 & APP=$!
sleep 8
powershell -ExecutionPolicy Bypass -File "$WINROOT/scripts/shot.ps1" | tail -3
cp -f _gui_shot.png _gui_demo.png 2>/dev/null
sleep 3
powershell -ExecutionPolicy Bypass -File "$WINROOT/scripts/shot.ps1" | tail -3
cp -f _gui_shot.png _gui_demo2.png 2>/dev/null
wait $APP
echo "app_exit=$?"
kill $S1 $S2 $S3 2>/dev/null
echo "--- log:"
tail -5 _gui5.log
ls -la _gui_demo*.png 2>/dev/null
echo "--- cleanup"
rm -f "$APPDATA/OpenDown/tasks.json"
rm -rf demo /c/Temp/opendown-demo
echo CLEAN_DONE
