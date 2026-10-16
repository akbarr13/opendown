#!/bin/bash
# Build release artifacts into dist/: exe, extension .crx + .zip, SHA256SUMS.txt.
# The extension signing key (extension.pem) is generated on first pack and reused;
# it is gitignored - keep it safe, it owns the extension ID for future updates.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
VER="0.1.0"
EXT_VER="1.0.0"
[ -d "$HOME/tools/w64devkit/bin" ] && export PATH="$HOME/tools/w64devkit/bin:$PATH"
[ -d "$HOME/.cargo/bin" ] && export PATH="$HOME/.cargo/bin:$PATH"

mkdir -p dist || exit 1

echo "== build =="
cargo build --release || exit 1
cp -f target/release/opendown.exe "dist/opendown-v${VER}-windows-x64.exe"
ls -la "dist/opendown-v${VER}-windows-x64.exe"

echo "== release smoke test =="
OPENDOWN_SMOKE_MS=8000 ./target/release/opendown.exe > _rel_smoke.log 2>&1 &
APP=$!
sleep 4
echo -n "smoke ping: "; curl -s --max-time 3 http://127.0.0.1:10081/ping; echo
wait $APP; echo "smoke exit=$?"
rm -f _rel_smoke.log

echo "== pack extension (.crx) =="
EXTWIN="$(cd extension && pwd -W)"
KEY=""
[ -f extension.pem ] && KEY="--pack-extension-key=$(cd "$ROOT" && pwd -W)/extension.pem"
rm -f extension.crx
PACKED=""
for cand in \
  "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" \
  "/c/Program Files/Microsoft/Edge/Application/msedge.exe" \
  "/c/Program Files/Google/Chrome/Application/chrome.exe" \
  "/c/Program Files (x86)/Google/Chrome/Application/chrome.exe" ; do
  [ -f "$cand" ] || continue
  echo "trying: $cand"
  "$cand" --pack-extension="$EXTWIN" $KEY --no-first-run --no-default-browser-check >/dev/null 2>&1 &
  for i in $(seq 1 20); do [ -f extension.crx ] && break; sleep 1; done
  if [ -f extension.crx ]; then PACKED="$cand"; echo "packed with: $cand"; break; fi
done
[ -z "$PACKED" ] && echo "CRX PACK FAILED"
[ -f extension.crx ] && mv -f extension.crx "dist/opendown-extension-v${EXT_VER}.crx"
[ -f extension.pem ] && echo "signing key at extension.pem (gitignored - keep it!)"
[ -f "dist/opendown-extension-v${EXT_VER}.crx" ] && python -c "print('crx magic:', open('dist/opendown-extension-v${EXT_VER}.crx','rb').read(4))"

echo "== zip (load-unpacked fallback) =="
python - <<'PY'
import zipfile, os
base = "extension"
out = "dist/opendown-extension-v1.0.0.zip"
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for dp, dns, fns in os.walk(base):
        for fn in fns:
            fp = os.path.join(dp, fn)
            z.write(fp, os.path.relpath(fp, base))
print("zip:", os.path.getsize(out), "bytes")
PY

echo "== checksums =="
cd dist && sha256sum opendown-v${VER}-windows-x64.exe opendown-extension-v${EXT_VER}.crx opendown-extension-v${EXT_VER}.zip > SHA256SUMS.txt && cat SHA256SUMS.txt
echo DONE
