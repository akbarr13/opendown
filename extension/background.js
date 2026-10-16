// OpenDown Integration Module — background service worker.
// Sends grabbed downloads to the OpenDown desktop app at http://127.0.0.1:<port>/add

const DEFAULT_PORT = 10081;
const MEDIA_RE = /\.(mp4|m4v|mkv|webm|mov|avi|flv|wmv|ts|m3u8|mpd|mp3|m4a|aac|flac|wav|ogg|opus|zip|rar|7z|tar|gz|bz2|xz|iso|img|exe|msi|dmg|apk|pdf|epub)(\?|#|$)/i;

let settings = { port: DEFAULT_PORT, takeover: true };
let tabMedia = {}; // tabId -> [{url, size, type, ts}]

chrome.storage.sync.get({ port: DEFAULT_PORT, takeover: true }, (s) => {
  settings.port = s.port;
  settings.takeover = s.takeover;
});

function appUrl(path) {
  return "http://127.0.0.1:" + settings.port + path;
}

async function sendToApp(payload) {
  try {
    const r = await fetch(appUrl("/add"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const j = await r.json().catch(() => ({ ok: r.ok }));
    flashBadge(j && j.ok !== false ? "OK" : "ERR", j && j.ok !== false ? "#2e7d32" : "#c62828");
    return j;
  } catch (e) {
    flashBadge("OFF", "#c62828");
    return { ok: false, error: String(e) };
  }
}

function flashBadge(text, color) {
  chrome.action.setBadgeBackgroundColor({ color });
  chrome.action.setBadgeText({ text });
  setTimeout(() => {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (tabs && tabs[0]) updateMediaBadge(tabs[0]);
    });
  }, 2500);
}

// ---- context menus ----
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({ id: "opendown-link", title: "Download with OpenDown", contexts: ["link"] });
  chrome.contextMenus.create({ id: "opendown-media", title: "Download this media with OpenDown", contexts: ["video", "audio"] });
  chrome.contextMenus.create({ id: "opendown-page", title: "Download page URL with OpenDown", contexts: ["page"] });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  const url = info.linkUrl || info.srcUrl || info.pageUrl;
  if (!url) return;
  withCookies(url, (cookie) => {
    sendToApp({ url, referer: (tab && tab.url) || "", cookie });
  });
});

function withCookies(url, cb) {
  try {
    chrome.cookies.getAll({ url }, (cookies) => {
      const cookie = (cookies || []).map((c) => c.name + "=" + c.value).join("; ");
      cb(cookie);
    });
  } catch (e) {
    cb("");
  }
}

// ---- media sniffing ----
function addMedia(tabId, url, size, type) {
  const list = tabMedia[tabId] || (tabMedia[tabId] = []);
  if (list.some((m) => m.url === url)) return;
  if (list.length >= 100) list.shift();
  list.push({ url, size: size || 0, type: type || "", ts: Date.now() });
  chrome.action.setBadgeBackgroundColor({ color: "#2a5cc2" });
  chrome.action.setBadgeText({ text: String(list.length), tabId });
  chrome.tabs.sendMessage(tabId, { type: "media-updated", count: list.length }, () => void chrome.runtime.lastError);
}

chrome.webRequest.onBeforeRequest.addListener(
  (d) => {
    if (d.tabId < 0) return;
    if (MEDIA_RE.test(d.url)) addMedia(d.tabId, d.url, 0, d.type);
  },
  { urls: ["<all_urls>"] },
  []
);

chrome.webRequest.onHeadersReceived.addListener(
  (d) => {
    if (d.tabId < 0) return;
    const h = {};
    (d.responseHeaders || []).forEach((x) => (h[(x.name || "").toLowerCase()] = x.value || ""));
    const ct = (h["content-type"] || "").toLowerCase();
    const cl = parseInt(h["content-length"] || "0", 10) || 0;
    if (ct.startsWith("video/") || ct.startsWith("audio/")) {
      addMedia(d.tabId, d.url, cl, d.type);
    } else if (ct.indexOf("octet-stream") >= 0 && cl > 262144 && MEDIA_RE.test(d.url)) {
      addMedia(d.tabId, d.url, cl, d.type);
    }
  },
  { urls: ["<all_urls>"] },
  ["responseHeaders"]
);

function updateMediaBadge(tab) {
  if (!tab || tab.id == null || tab.id < 0) return;
  const n = (tabMedia[tab.id] || []).length;
  chrome.action.setBadgeBackgroundColor({ color: "#2a5cc2" });
  chrome.action.setBadgeText({ text: n ? String(n) : "", tabId: tab.id });
}

// ---- download takeover (IDM-style) ----
chrome.downloads.onCreated.addListener((item) => {
  if (!settings.takeover) return;
  if (item.url.indexOf("http://127.0.0.1:") === 0) return; // our own app
  const mime = item.mime || "";
  const isMedia = MEDIA_RE.test(item.url) || mime.indexOf("video/") === 0 || mime.indexOf("audio/") === 0;
  if (!isMedia) return;
  chrome.downloads.cancel(item.id, () => {
    withCookies(item.url, (cookie) => {
      sendToApp({ url: item.url, referer: item.referrer || "", filename: item.filename || "", cookie });
    });
  });
});

// ---- messaging ----
chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (!msg || !msg.type) return;
  if (msg.type === "grab") {
    const referer = msg.referer || (sender.tab && sender.tab.url) || "";
    withCookies(msg.url, (cookie) => {
      sendToApp({ url: msg.url, referer, cookie, filename: msg.filename || "" });
    });
    sendResponse({ ok: true });
  } else if (msg.type === "get-media") {
    const id = msg.tabId != null ? msg.tabId : sender.tab && sender.tab.id;
    sendResponse({ media: tabMedia[id] || [] });
  } else if (msg.type === "get-settings") {
    sendResponse(settings);
  } else if (msg.type === "set-settings") {
    settings.port = msg.port || settings.port;
    settings.takeover = !!msg.takeover;
    chrome.storage.sync.set({ port: settings.port, takeover: settings.takeover });
    sendResponse(settings);
  } else if (msg.type === "ping") {
    fetch(appUrl("/ping"))
      .then((r) => r.json())
      .then((j) => sendResponse(j))
      .catch(() => sendResponse({ ok: false }));
    return true; // async response
  }
  return true;
});
