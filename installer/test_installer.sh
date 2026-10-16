#!/bin/bash
# Silent end-to-end test of the built installer.
set -u
cd /c/Users/pipu/projects/opendown || exit 1
SETUP="dist/opendown-setup-v0.1.0.exe"
TESTDIR='C:\Users\pipu\projects\opendown\_instest'
echo "== silent install =="
"$SETUP" /S "/D=$TESTDIR"
echo "install_rc=$?"
sleep 2
echo "== installed tree =="
ls _instest/ 2>/dev/null | head -12
echo "--- extension/:"
ls _instest/extension/ 2>/dev/null
echo "== registry (browser auto-install must be ABSENT, meta keys must exist) =="
reg query "HKCU\Software\Microsoft\Edge\Extensions\manelgpcmclgjpecpncekbbbhjohndhp" >/dev/null 2>&1 && echo "EDGE_KEY_PRESENT(!!)" || echo "EDGE_KEY_ABSENT_OK"
reg query "HKCU\Software\Google\Chrome\Extensions\manelgpcmclgjpecpncekbbbhjohndhp" >/dev/null 2>&1 && echo "CHROME_KEY_PRESENT(!!)" || echo "CHROME_KEY_ABSENT_OK"
reg query "HKCU\Software\OpenDown" 2>&1 | tail -3
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" 2>&1 | tail -2
echo "== smoke: installed exe =="
OPENDOWN_SMOKE_MS=7000 ./_instest/opendown.exe > _inst_smoke.log 2>&1 &
APP=$!
sleep 4
echo -n "ping: "; curl -s --max-time 3 http://127.0.0.1:10081/ping; echo
wait $APP; echo "smoke_exit=$?"
rm -f _inst_smoke.log
echo "== silent uninstall =="
./_instest/uninstall.exe /S
echo "uninstall_rc=$?"
sleep 6
echo "== after =="
if [ -d _instest ]; then echo "DIR_STILL_EXISTS:"; ls _instest 2>/dev/null; else echo "DIR_GONE"; fi
reg query "HKCU\Software\Microsoft\Edge\Extensions\manelgpcmclgjpecpncekbbbhjohndhp" >/dev/null 2>&1 && echo "EDGE_KEY_STILL" || echo "EDGE_KEY_GONE"
reg query "HKCU\Software\Google\Chrome\Extensions\manelgpcmclgjpecpncekbbbhjohndhp" >/dev/null 2>&1 && echo "CHROME_KEY_STILL" || echo "CHROME_KEY_GONE"
reg query "HKCU\Software\OpenDown" >/dev/null 2>&1 && echo "META_KEY_STILL" || echo "META_KEY_GONE"
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" >/dev/null 2>&1 && echo "ARP_STILL" || echo "ARP_GONE"
rm -rf _instest 2>/dev/null
echo TEST_DONE
