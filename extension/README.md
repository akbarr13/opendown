# OpenDown Integration Module (browser extension)

Browser integration for the **OpenDown** desktop download manager — auto-grabber,
context-menu downloads and IDM-style download takeover. Chrome / Edge (Manifest V3).

## Install (current browsers)

Browsers now block extensions installed outside their official stores (2024+ security
policies) — off-store `.crx` installs and registry-based installs end up disabled with
no way to enable them. The reliable path is **Load unpacked**:

1. Open `edge://extensions` (or `chrome://extensions`).
2. Turn ON **Developer mode**.
3. Click **Load unpacked** and select this `extension/` folder — or the `unpacked`
   copy shipped with the installer at
   `%LOCALAPPDATA%\Programs\OpenDown\extension\unpacked`.
4. Keep the OpenDown app running while you browse (the grabber listens on
   `127.0.0.1:10081` by default; change it in the app Options or in the popup).

Notes:

- The extension stays enabled while **Developer mode is ON** (Chrome 133+ disables
  unpacked extensions when it is turned off).
- A "developer mode extensions" warning at browser startup is normal and harmless.
- The `.crx` package is kept for Chromium forks that still accept off-store installs.
- Firefox is not supported (`.crx` / unpacked is a Chromium format).

## Features

- **Media sniffing** — watches network requests for video/audio/files, badge shows the count.
- **Popup list** — per-tab list of sniffed media with per-item and "Download all" buttons.
- **Floating panel** — pages with media get a small "OpenDown" panel with a grab button.
- **Context menus** — right-click a link / video / page: "Download with OpenDown".
- **Download takeover** — media/file downloads started in the browser are cancelled and
  re-routed to OpenDown (toggle in the popup).

## Files

- `manifest.json` — MV3 manifest
- `background.js` — service worker (sniffing, context menus, takeover, app bridge)
- `content.js` / `content.css` — floating panel
- `popup.html` / `popup.js` — toolbar popup
- `icons/` — generated icons

The extension talks to the app over plain HTTP on localhost (`POST /add`, `GET /ping`).
No data leaves your machine.
