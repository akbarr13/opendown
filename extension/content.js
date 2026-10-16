// OpenDown Integration Module — content script: floating grab panel.
(() => {
  if (window.__opendownInjected) return;
  window.__opendownInjected = true;

  const MEDIA_RE = /\.(mp4|m4v|mkv|webm|mov|avi|flv|wmv|ts|m3u8|mpd|mp3|m4a|aac|flac|wav|ogg|opus|zip|rar|7z|tar|gz|bz2|xz|iso|img|exe|msi|dmg|apk|pdf|epub)(\?|#|$)/i;
  let panel = null;
  let count = 0;

  function grab(url, referer) {
    chrome.runtime.sendMessage({ type: "grab", url, referer });
  }

  function collectMedia() {
    const urls = new Set();
    document.querySelectorAll("video, audio").forEach((el) => {
      if (el.src) urls.add(el.src);
      el.querySelectorAll("source").forEach((s) => s.src && urls.add(s.src));
    });
    document.querySelectorAll("a[href]").forEach((a) => {
      try { if (MEDIA_RE.test(a.href)) urls.add(a.href); } catch (e) {}
    });
    return [...urls];
  }

  function setHint(t) {
    const h = panel && panel.querySelector("#oidm-hint");
    if (h) h.textContent = t;
  }

  function ensurePanel() {
    if (panel) return panel;
    panel = document.createElement("div");
    panel.id = "opendown-panel";
    panel.innerHTML =
      '<div class="oidm-title"><span>⬇ OpenDown</span><button class="oidm-close" id="oidm-close">✕</button></div>' +
      '<button class="oidm-btn" id="oidm-grab-page">Download this page’s media</button>' +
      '<button class="oidm-btn oidm-ghost" id="oidm-open-popup">Show list…</button>' +
      '<div class="oidm-hint" id="oidm-hint"></div>';
    document.documentElement.appendChild(panel);
    panel.querySelector("#oidm-grab-page").addEventListener("click", () => {
      chrome.runtime.sendMessage({ type: "get-media", tabId: null }, (res) => {
        const list = (res && res.media) || [];
        const urls = new Set(list.map((m) => m.url));
        collectMedia().forEach((u) => urls.add(u));
        if (urls.size === 0) { setHint("No media detected on this page."); return; }
        let n = 0;
        urls.forEach((u) => { grab(u, location.href); n++; });
        setHint("Sent " + n + " URL(s) to OpenDown");
      });
    });
    panel.querySelector("#oidm-open-popup").addEventListener("click", () => {
      setHint("Click the OpenDown toolbar icon for the full list.");
    });
    panel.querySelector("#oidm-close").addEventListener("click", () => {
      panel.style.display = "none";
    });
    return panel;
  }

  function refresh() {
    if (panel && panel.style.display === "none") return;
    const n = collectMedia().length + count;
    if (n > 0) {
      ensurePanel();
      setHint(count > 0 ? count + " media request(s) sniffed" : "");
    }
  }

  chrome.runtime.onMessage.addListener((msg) => {
    if (msg && msg.type === "media-updated") {
      count = msg.count || 0;
      if (count > 0) {
        ensurePanel();
        panel.style.display = "";
        setHint(count + " media request(s) sniffed");
      }
    }
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", refresh);
  } else {
    refresh();
  }
})();
