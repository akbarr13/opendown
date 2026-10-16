# OpenDown

**Open-source IDM-style download manager for Windows, written in Rust.**

Multi-connection segmented downloads with crash-safe resume · egui GUI · CLI · browser auto-grabber (Chrome/Edge extension)

<img src="docs/screenshot.png" alt="OpenDown screenshot" width="860">

## Install

- **Installer (recommended)** — `opendown-setup-*.exe` from [Releases](../../releases/latest).
  Per-user install (no admin), Start Menu shortcut, and it registers the browser
  extension for Edge & Chrome — restart your browser to activate it.
- **Portable** — `opendown-*-windows-x64.exe` from [Releases](../../releases/latest);
  run it directly, no install.
- **From source** — see [Build from source](#build-from-source).

Requirements: Windows 10/11 (x64). The binaries are unsigned — SmartScreen may warn on
first run (More info → Run anyway).

## Features

**Engine**
- Segmented downloads (up to 32 connections) with automatic single-stream fallback
- Crash-safe resume: `.part` + `.tgmeta` sidecar files — pausing or even killing the app
  mid-download loses nothing
- Per-segment retries, redirect following, speed limiter, custom User-Agent / Referer / Cookie

**GUI (egui — dark theme by default, classic light theme option)**
- IDM-style layout: menu bar, 11-button icon toolbar, full-height *Categories* tree,
  full-width status bar with speed limiter and live speed graph
- 9-column table — File Name, Q, Size, Status, Downloaded, Time left, Transfer rate,
  Last Try, Description — with per-connection segment bars, sortable headers, full-row
  selection, right-click context menu and resizable columns
- Detail panel for the selected download, Add-URL dialog, options, log pane, clipboard URL monitor
- Original assets only: icons drawn in code, UI font **Varela Round** (SIL OFL)

**CLI**
- `opendown --cli <URL> [-o DIR] [-f NAME] [-n CONNS] [--limit KB] [--resume]`

**Browser grabber**
- Local endpoint (`127.0.0.1:10081`) plus a Chrome/Edge extension (Manifest V3)
- Sniffs media/file requests, floating grab panel on pages, per-tab list in the popup,
  right-click "Download with OpenDown", IDM-style download takeover
- Grabbed downloads carry referer + cookies and auto-start (toggleable)

## Browser extension

The installer registers the extension automatically (Edge & Chrome). Manual alternatives:

- **Edge** — drag `opendown-extension-*.crx` onto `edge://extensions` (Developer mode on),
  or **Load unpacked** the `extension/` folder.
- **Chrome / other Chromium browsers** — **Load unpacked** the `extension/` folder
  (off-store `.crx` installs have been blocked since Chrome 21).
- **Firefox** — not supported (`.crx` is a Chromium format).

The extension talks to the app over localhost only:

- `GET /ping` → `{"ok":true,"app":"opendown"}`
- `POST /add` — JSON: `url`, `referer`, `filename`, `cookie`

Port (default `10081`) and grabber on/off live in the app's Options window; the popup can
change the port and toggle download takeover.

## Build from source

Any Windows Rust toolchain works:

```
cargo build --release
```

`build.bat` targets the portable [w64devkit](https://github.com/skeeto/w64devkit) +
rustup (GNU host) setup used during development — no MSVC needed. `run_gui.bat` and
`run_cli.bat` launch the built binaries (`target\release\opendown.exe`).

Release artifacts (app, `.crx`, `.zip`, NSIS installer) are produced by
`scripts/make_release.sh` into `dist/`; the installer script is `installer/opendown.nsi`.

## Data

Config and task list live in `%APPDATA%\OpenDown\` (`config.json`, `tasks.json`).

## Testing

- Engine: segmented download, no-Range fallback, pause/resume, kill-and-resume — sha256 verified
- HTTPS (native-tls) — sha256 verified
- Grabber end-to-end: extension payload → task created → downloaded file matches source (sha256)
- Installer: silent install → files + registry verified → app smoke test → clean uninstall

## License

MIT — see [LICENSE](LICENSE).

Bundled font: **Varela Round** (SIL Open Font License 1.1 — [assets/OFL.txt](assets/OFL.txt)).

OpenDown is an original project: no code or artwork from Tonec's "Internet Download
Manager" (IDM) is included — only the general UI style is inspired by it. Not affiliated
with or endorsed by Tonec Inc.
