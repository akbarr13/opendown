# OpenDown Integration Module (browser extension)

Browser integration for the **OpenDown** desktop download manager — auto-grabber, context-menu
downloads and IDM-style download takeover. Chrome / Edge (Manifest V3).

## Load (unpacked)

1. Open Chrome/Edge -> `chrome://extensions` (or `edge://extensions`).
2. Enable **Developer mode** (top-right).
3. Click **Load unpacked** and select this `extension` folder.
4. Start the OpenDown app (grabber listens on `127.0.0.1:10081` by default; change in
   Options or in the extension popup if needed).

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
