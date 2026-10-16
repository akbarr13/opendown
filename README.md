# OpenDown

An IDM-style download manager — **original implementation in Rust**: multi-connection
segmented HTTP/HTTPS downloads with crash-safe resume, an egui GUI, a CLI, and a browser
grabber with a Chrome/Edge extension. No IDM (Tonec) code or assets are used anywhere —
only the general layout and workflow are inspired by it.

![OpenDown screenshot](docs/screenshot.png)

## Features

**Engine**
- Segmented downloads (1–32 connections) with `Range` requests, automatic
  single-stream fallback when the server does not support ranges
- Crash-safe resume: `.part` + `.tgmeta` files, survives kills, pause/resume at any time
- Per-segment retries, redirect following, speed limiter, custom User-Agent / Referer / Cookie

**GUI (egui, dark charcoal theme with light-theme option)**
- IDM-style layout: menu bar (Tasks / File / Downloads / View / Registration / Help),
  11-button icon toolbar, full-height "Categories" tree (All Downloads + subfolders +
  Unfinished / Finished / Grabber projects / Queues), full-width status bar with
  speed-limiter dropdown and live speed sparkline
- 9-column download table: File Name, Q, Size, Status, Downloaded (per-connection segment
  bars), Time left, Transfer rate, Last Try, Description — full-row click, sortable
  headers, right-click context menu, resizable columns, zebra striping
- Selected-download detail panel with per-segment visualization and speed graph
- Add-URL dialog, options window, log pane, clipboard URL monitor
- Original assets only: icons are drawn in code, rounded font is **Varela Round** (OFL)

**Browser grabber**
- Built-in localhost endpoint (`127.0.0.1:10081` by default) for the OpenDown browser extension
- Media auto-grabber: sniffs video/audio/file requests, floating grab panel on pages,
  per-tab media list in the popup, right-click menus, IDM-style download takeover
- Grabbed downloads are created with referer + cookies and auto-started (toggleable)

## Quick start

- `run_gui.bat` — launch the GUI
- `run_cli.bat "URL" -o DIR -n 16` — command-line download
- `installer/opendown.nsi` — Windows setup script (per-user install; registers the
  browser extension for Edge/Chrome on the next browser start)

## Browser extension (opendown\extension)

1. Open `chrome://extensions` (or `edge://extensions`)
2. Enable **Developer mode**, click **Load unpacked**, select the `extension` folder
3. Keep the OpenDown app running while browsing

The extension talks to the app over localhost only:

- `GET /ping` -> `{"ok":true,"app":"opendown"}`
- `POST /add` with JSON: `{"url":"...", "referer":"...", "filename":"...", "cookie":"..."}`

Port and grabber on/off live in the app Options window; the extension popup can change
the port and toggle download takeover.

## Build from source

Rust `x86_64-pc-windows-gnu` host toolchain + portable w64devkit (no MSVC needed):

```
build.bat          # or:
cargo build --release
```

Output: `target\release\opendown.exe`

## Data

Config and task list live in `%APPDATA%\OpenDown\` (`config.json`, `tasks.json`).

## Verified behavior

- Engine: 4/4 tests — segmented download, no-Range fallback, pause/resume,
  kill-and-resume, sha256 identical
- HTTPS via native-tls (Cloudflare test file, ~3 MB, sha256 verified)
- Grabber end-to-end: extension payload (curl) -> task created -> downloaded file sha256
  identical to source (24 MB test)

## License

MIT — see [LICENSE](LICENSE).

Bundled font: **Varela Round** by The Varela Round Project Authors, used under the
SIL Open Font License 1.1 (see [assets/OFL.txt](assets/OFL.txt)).

OpenDown is an original project: no code or artwork from Tonec's "Internet Download
Manager" (IDM) is included — only the general UI style is inspired by it. OpenDown is
not affiliated with or endorsed by Tonec Inc.
