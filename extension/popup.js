const $ = (s) => document.querySelector(s);
let media = [];

function fmtSize(n) {
  if (!n) return "";
  const u = ["B", "KB", "MB", "GB"];
  let i = 0;
  while (n >= 1024 && i < 3) { n /= 1024; i++; }
  return n.toFixed(i ? 1 : 0) + " " + u[i];
}

function render() {
  const list = $("#list");
  list.innerHTML = "";
  if (!media.length) {
    list.innerHTML = '<div class="item"><div class="meta">No media detected yet. Play the video/audio on the page, then reopen this popup.</div></div>';
    return;
  }
  media.forEach((m) => {
    const el = document.createElement("div");
    el.className = "item";
    const name = decodeURIComponent((m.url.split("/").pop() || m.url).split("?")[0]) || m.url;
    el.innerHTML = '<div class="meta"><div class="name"></div><div class="sub"></div></div>';
    el.querySelector(".name").textContent = name.slice(0, 60);
    el.querySelector(".sub").textContent = (m.type || "file") + (m.size ? " · " + fmtSize(m.size) : "");
    const b = document.createElement("button");
    b.textContent = "Get";
    b.onclick = () => send(m.url, b);
    el.appendChild(b);
    list.appendChild(el);
  });
}

function send(url, btn) {
  chrome.runtime.sendMessage({ type: "grab", url, referer: "" }, () => {
    if (btn) {
      btn.textContent = "✓";
      setTimeout(() => (btn.textContent = "Get"), 1500);
    }
  });
}

function refresh() {
  chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    const id = tabs[0] && tabs[0].id;
    chrome.runtime.sendMessage({ type: "get-media", tabId: id }, (res) => {
      media = (res && res.media) || [];
      render();
    });
  });
  chrome.runtime.sendMessage({ type: "ping" }, (res) => {
    const ok = res && res.ok;
    $("#status").textContent = ok ? "app online" : "app offline";
    $("#status").style.color = ok ? "#7fd87f" : "#ff8a80";
  });
}

chrome.runtime.sendMessage({ type: "get-settings" }, (s) => {
  if (!s) return;
  $("#port").value = s.port;
  $("#takeover").checked = !!s.takeover;
});

$("#takeover").addEventListener("change", () => {
  chrome.runtime.sendMessage({ type: "get-settings" }, (s) => {
    chrome.runtime.sendMessage({ type: "set-settings", port: Number($("#port").value) || s.port, takeover: $("#takeover").checked });
  });
});

$("#port").addEventListener("change", () => {
  const port = Number($("#port").value);
  if (port >= 1024 && port <= 65535) {
    chrome.runtime.sendMessage({ type: "get-settings" }, (s) => {
      chrome.runtime.sendMessage({ type: "set-settings", port, takeover: s.takeover }, () => {
        $("#portsave").textContent = "saved";
        refresh();
        setTimeout(() => ($("#portsave").textContent = ""), 1500);
      });
    });
  }
});

$("#grab-all").addEventListener("click", () => media.forEach((m) => send(m.url, null)));
$("#refresh").addEventListener("click", refresh);

refresh();
