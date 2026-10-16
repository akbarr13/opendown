$procs = Get-Process opendown -ErrorAction SilentlyContinue
if (-not $procs) { Write-Output "NO_PROCESS"; exit 0 }
foreach ($p in $procs) {
  $title = ""
  try { $title = $p.MainWindowTitle } catch {}
  Write-Output ("PID={0} HWND={1} TITLE='{2}'" -f $p.Id, $p.MainWindowHandle, $title)
}
