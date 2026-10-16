# Dev scripts (Windows / MSYS-oriented)

Helper scripts used while developing OpenDown. They assume you run them from a
Git-Bash / MSYS shell on Windows, from anywhere (paths are resolved relative to the
script location).

- `test_server.py` - Range-capable throttled HTTP server used by the tests
- `demo_capture.sh` - seeds 3 downloads against local test servers, launches the GUI
  with test hooks, captures screenshots mid-download
- `capture_gui.sh` - quick GUI smoke + window capture
- `grabber_test.sh` - end-to-end browser-grabber test (pings the local endpoint,
  POSTs an extension-style payload, verifies sha256)
- `shot.ps1` - PowerShell window-capture helper (PrintWindow)
- `check_proc.ps1` / `list_windows.ps1` - small diagnostics helpers
