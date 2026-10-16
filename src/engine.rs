//! OpenDown — segmented, resumable HTTP/HTTPS download engine.
//!
//! Original implementation (no IDM code/assets). Core techniques:
//! server probe (HEAD -> Range 0-0), multi-connection Range segments,
//! pause/resume via sidecar meta, retries with backoff, global speed limiter,
//! single-stream fallback.
#![allow(clippy::too_many_arguments)]

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_CONNECTIONS: usize = 8;
pub const MAX_CONNECTIONS: usize = 32;
pub const MIN_SEGMENT: u64 = 1024 * 1024;
pub const READ_CHUNK: usize = 64 * 1024;
pub const FLUSH_EVERY: u64 = 4 * 1024 * 1024;

pub const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub fn human_size(n: Option<u64>) -> String {
    match n {
        None => "?".to_string(),
        Some(n) => {
            let mut v = n as f64;
            for unit in ["B", "KB", "MB", "GB", "TB"] {
                if v < 1024.0 || unit == "TB" {
                    if unit == "B" {
                        return format!("{:.0} B", v);
                    }
                    return format!("{} {}", trim_num(v), unit);
                }
                v /= 1024.0;
            }
            format!("{:.1} TB", v)
        }
    }
}

fn trim_num(v: f64) -> String {
    let s = format!("{:.1}", v);
    s.strip_suffix(".0").map(|x| x.to_string()).unwrap_or(s)
}

pub fn human_speed(bps: f64) -> String {
    if bps <= 0.0 {
        "0 B/s".to_string()
    } else {
        format!("{}/s", human_size(Some(bps as u64)))
    }
}

pub fn human_eta(sec: Option<f64>) -> String {
    match sec {
        None => "--:--".to_string(),
        Some(s) if s <= 0.0 || s > 30.0 * 86400.0 => "--:--".to_string(),
        Some(s) => {
            let s = s as u64;
            let (h, rem) = (s / 3600, s % 3600);
            let (m, sec) = (rem / 60, rem % 60);
            if h > 0 {
                format!("{}:{:02}:{:02}", h, m, sec)
            } else {
                format!("{:02}:{:02}", m, sec)
            }
        }
    }
}

fn clean_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().trim_matches('"').trim_matches('\'').chars() {
        if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let out = out.trim_matches(|c| c == ' ' || c == '.').to_string();
    out.chars().take(180).collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_disposition(cd: &str) -> Option<String> {
    let lower = cd.to_ascii_lowercase();
    if let Some(idx) = lower.find("filename*=") {
        let rest = cd[idx + 10..].trim_start();
        if let Some(q1) = rest.find('\'') {
            if let Some(q2) = rest[q1 + 1..].find('\'') {
                let name = &rest[q1 + 1 + q2 + 1..];
                let name = name.split(';').next().unwrap_or("").trim();
                let decoded = percent_decode(name);
                let s = clean_name(&decoded);
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    if let Some(idx) = lower.find("filename=") {
        let rest = cd[idx + 9..].trim();
        let name = if let Some(stripped) = rest.strip_prefix('"') {
            stripped.split('"').next().unwrap_or("").to_string()
        } else {
            rest.split(';').next().unwrap_or("").trim().to_string()
        };
        let s = clean_name(&name);
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

pub fn filename_from_url(url: &str) -> String {
    let no_q = url.split(['?', '#']).next().unwrap_or(url);
    let last = no_q.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    let s = clean_name(&decoded);
    if s.is_empty() {
        "download.bin".to_string()
    } else {
        s
    }
}

/// IDM-style category by extension.
pub fn category_for(filename: &str) -> &'static str {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    const VIDEO: &[&str] = &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "3gp", "rmvb"];
    const MUSIC: &[&str] = &["mp3", "wav", "flac", "m4a", "aac", "ogg", "wma", "opus", "m4b"];
    const DOCS: &[&str] = &["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "epub", "rtf", "odt", "csv", "md"];
    const PROGRAMS: &[&str] = &["exe", "msi", "apk", "dmg", "bin", "iso", "img", "deb", "rpm", "appx"];
    const COMPRESSED: &[&str] = &["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "cab"];
    for (list, name) in [
        (VIDEO, "Video"),
        (MUSIC, "Music"),
        (DOCS, "Documents"),
        (PROGRAMS, "Programs"),
        (COMPRESSED, "Compressed"),
    ] {
        if list.contains(&ext.as_str()) {
            return name;
        }
    }
    "Other"
}

// ---------------------------------------------------------------------------
// control flags / speed limiter
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Ctrl {
    pub pause: AtomicBool,
    pub cancel: AtomicBool,
}

pub enum StopErr {
    Pause,
    Cancel,
    Io(String),
}

impl Ctrl {
    pub fn check(&self) -> Result<(), StopErr> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(StopErr::Cancel);
        }
        if self.pause.load(Ordering::Relaxed) {
            return Err(StopErr::Pause);
        }
        Ok(())
    }
}

pub struct SpeedLimiter {
    inner: Mutex<(Option<f64>, Instant)>,
}

impl SpeedLimiter {
    pub fn new(rate: Option<f64>) -> Self {
        Self {
            inner: Mutex::new((rate, Instant::now())),
        }
    }
    pub fn set_rate(&self, rate: Option<f64>) {
        let mut g = self.inner.lock().unwrap();
        *g = (rate, Instant::now());
    }
    pub fn rate(&self) -> Option<f64> {
        self.inner.lock().unwrap().0
    }
    /// Sleep so the global rate stays <= rate. `ctrl.check()` may abort.
    pub fn consume(&self, n: usize, ctrl: &Ctrl) -> Result<(), StopErr> {
        let wait = {
            let mut g = self.inner.lock().unwrap();
            let rate = g.0;
            match rate {
                None => return ctrl.check(),
                Some(r) => {
                    let now = Instant::now();
                    let next = if g.1 < now { now } else { g.1 };
                    let wait = next.saturating_duration_since(now);
                    g.1 = next + Duration::from_secs_f64(n as f64 / r);
                    wait
                }
            }
        };
        let deadline = Instant::now() + wait;
        loop {
            ctrl.check()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            let remain = deadline - now;
            thread::sleep(if remain > Duration::from_millis(200) {
                Duration::from_millis(200)
            } else {
                remain
            });
        }
    }
}

struct SpeedCalc {
    last: Instant,
    last_bytes: u64,
    speed: f64,
}

impl SpeedCalc {
    fn new() -> Self {
        Self {
            last: Instant::now(),
            last_bytes: 0,
            speed: 0.0,
        }
    }
    fn update(&mut self, total: u64) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        if dt >= 0.5 {
            let inst = total.saturating_sub(self.last_bytes) as f64 / dt;
            self.speed = if self.speed <= 0.0 {
                inst
            } else {
                0.65 * self.speed + 0.35 * inst
            };
            self.last = now;
            self.last_bytes = total;
        }
    }
}

// ---------------------------------------------------------------------------
// status / segment / meta
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    Queued,
    Probing,
    Downloading,
    Paused,
    Completed,
    Error,
    Cancelled,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Queued => "Queued",
            Status::Probing => "Connecting...",
            Status::Downloading => "Downloading",
            Status::Paused => "Paused",
            Status::Completed => "Complete",
            Status::Error => "Error",
            Status::Cancelled => "Cancelled",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start: u64,
    pub end: u64,
    pub done: u64,
}

impl Segment {
    pub fn len(&self) -> u64 {
        self.end - self.start + 1
    }
    pub fn pos(&self) -> u64 {
        self.start + self.done
    }
}

#[derive(Serialize, Deserialize)]
pub struct Meta {
    pub version: u32,
    pub url: String,
    pub final_url: Option<String>,
    pub filename: String,
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub single_stream: bool,
    pub stream_pos: u64,
    pub connections: usize,
    pub segments: Vec<Segment>,
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

pub struct Probe {
    pub final_url: String,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub accept_ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

pub fn make_agent() -> ureq::Agent {
    // ureq 2.12 with only the `native-tls` feature needs the connector passed
    // explicitly (TLS backend is not auto-selected without the `tls` feature).
    let tls = Arc::new(ureq::native_tls::TlsConnector::new().expect("native-tls init failed"));
    ureq::AgentBuilder::new()
        .tls_connector(tls)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .redirects(10)
        .build()
}

fn absorb(resp: &ureq::Response, status: u16, p: &mut Probe) {
    let u = resp.get_url();
    if !u.is_empty() {
        p.final_url = u.to_string();
    }
    if let Some(v) = resp.header("ETag") {
        if p.etag.is_none() {
            p.etag = Some(v.to_string());
        }
    }
    if let Some(v) = resp.header("Last-Modified") {
        if p.last_modified.is_none() {
            p.last_modified = Some(v.to_string());
        }
    }
    if let Some(v) = resp.header("Content-Type") {
        if p.content_type.is_none() {
            p.content_type = Some(v.to_string());
        }
    }
    if let Some(cd) = resp.header("Content-Disposition") {
        if p.filename.is_none() {
            p.filename = parse_disposition(cd);
        }
    }
    if let Some(cr) = resp.header("Content-Range") {
        if let Some(total) = cr.rsplit('/').next() {
            if let Ok(v) = total.trim().parse::<u64>() {
                if v > 0 {
                    p.size = Some(v);
                }
            }
        }
        if status == 206 {
            p.accept_ranges = true;
        }
    }
    if p.size.is_none() && status == 200 {
        if let Some(cl) = resp.header("Content-Length") {
            if let Ok(v) = cl.trim().parse::<u64>() {
                if v > 0 {
                    p.size = Some(v);
                }
            }
        }
    }
    if let Some(ar) = resp.header("Accept-Ranges") {
        if ar.to_ascii_lowercase().contains("bytes") {
            p.accept_ranges = true;
        }
    }
}

pub fn probe(url: &str, ua: &str, referer: Option<&str>, cookie: Option<&str>) -> Result<Probe, String> {
    let agent = make_agent();
    let mut p = Probe {
        final_url: url.to_string(),
        filename: None,
        size: None,
        accept_ranges: false,
        etag: None,
        last_modified: None,
        content_type: None,
    };

    // 1) HEAD
    let mut head = agent
        .head(url)
        .set("User-Agent", ua)
        .set("Accept", "*/*")
        .set("Accept-Encoding", "identity");
    if let Some(r) = referer {
        head = head.set("Referer", r);
    }
    if let Some(c) = cookie {
        head = head.set("Cookie", c);
    }
    match head.call() {
        Ok(resp) => {
            let st = resp.status();
            absorb(&resp, st, &mut p);
        }
        Err(ureq::Error::Status(code, resp)) => absorb(&resp, code, &mut p),
        Err(_) => {}
    }

    // 2) definitive Range probe if needed
    if !(p.accept_ranges && p.size.is_some() && p.filename.is_some()) {
        let mut req = agent
            .get(url)
            .set("User-Agent", ua)
            .set("Accept", "*/*")
            .set("Accept-Encoding", "identity")
            .set("Range", "bytes=0-0");
        if let Some(r) = referer {
            req = req.set("Referer", r);
        }
        if let Some(c) = cookie {
            req = req.set("Cookie", c);
        }
        match req.call() {
            Ok(resp) => {
                let st = resp.status();
                absorb(&resp, st, &mut p);
                if st == 200 {
                    p.accept_ranges = false;
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                absorb(&resp, code, &mut p);
                if code != 200 && code != 206 {
                    return Err(format!("server answered HTTP {}", code));
                }
            }
            Err(e) => return Err(format!("cannot reach server: {}", e)),
        }
    }

    if p.filename.is_none() {
        p.filename = Some(filename_from_url(&p.final_url));
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// task
// ---------------------------------------------------------------------------

pub struct TaskView {
    pub url: String,
    pub filename: String,
    pub size: Option<u64>,
    pub done: u64,
    pub progress: f32,
    pub speed: f64,
    pub eta: Option<f64>,
    pub status: Status,
    pub error: String,
    pub path: Option<PathBuf>,
    pub category: String,
    pub connections: usize,
    pub single_stream: bool,
    pub segs: Vec<f32>,
    pub last_try: Option<u64>,
}

pub struct DownloadTask {
    pub url: String,
    pub save_dir: PathBuf,
    pub filename_override: Option<String>,
    pub connections: Mutex<usize>,
    pub ua: String,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub retries: u32,

    pub filename: Mutex<String>,
    pub path: Mutex<Option<PathBuf>>,
    pub tmp_path: Mutex<Option<PathBuf>>,
    pub meta_path: Mutex<Option<PathBuf>>,

    pub size: Mutex<Option<u64>>,
    pub final_url: Mutex<Option<String>>,
    pub etag: Mutex<Option<String>>,
    pub range_ok: AtomicBool,
    pub single_stream: AtomicBool,
    pub stream_pos: AtomicU64,

    pub status: Mutex<Status>,
    pub error: Mutex<String>,

    pub segments: Mutex<Vec<Segment>>,
    pub done: AtomicU64,
    speed: Mutex<SpeedCalc>,
    last_meta_save: Mutex<Instant>,

    pub ctrl: Arc<Ctrl>,
    pub limiter: Arc<SpeedLimiter>,
    pub category: Mutex<String>,
    pub delete_partial: AtomicBool,
    pub last_try: Mutex<Option<u64>>,
    pub auto_category: bool,
    running: AtomicBool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl DownloadTask {
    pub fn new(
        url: &str,
        save_dir: PathBuf,
        filename_override: Option<String>,
        connections: usize,
        ua: &str,
        referer: Option<String>,
        cookie: Option<String>,
        limiter: Arc<SpeedLimiter>,
        retries: u32,
        auto_category: bool,
    ) -> Self {
        let filename = filename_override.clone().unwrap_or_default();
        Self {
            url: url.to_string(),
            save_dir,
            filename_override,
            connections: Mutex::new(connections.clamp(1, MAX_CONNECTIONS)),
            ua: ua.to_string(),
            referer,
            cookie,
            retries,
            filename: Mutex::new(filename),
            path: Mutex::new(None),
            tmp_path: Mutex::new(None),
            meta_path: Mutex::new(None),
            size: Mutex::new(None),
            final_url: Mutex::new(None),
            etag: Mutex::new(None),
            range_ok: AtomicBool::new(false),
            single_stream: AtomicBool::new(false),
            stream_pos: AtomicU64::new(0),
            status: Mutex::new(Status::Queued),
            error: Mutex::new(String::new()),
            segments: Mutex::new(Vec::new()),
            done: AtomicU64::new(0),
            speed: Mutex::new(SpeedCalc::new()),
            last_meta_save: Mutex::new(Instant::now() - Duration::from_secs(5)),
            ctrl: Arc::new(Ctrl::default()),
            limiter,
            category: Mutex::new("Other".to_string()),
            delete_partial: AtomicBool::new(false),
            last_try: Mutex::new(None),
            auto_category,
            running: AtomicBool::new(false),
        }
    }

    // ---------------- probing ----------------

    pub fn ensure_probed(&self) -> Result<(), String> {
        let have = self.size.lock().unwrap().is_some() && !self.filename.lock().unwrap().is_empty();
        if have {
            return Ok(());
        }
        let p = probe(&self.url, &self.ua, self.referer.as_deref(), self.cookie.as_deref())?;
        let fname = self
            .filename_override
            .clone()
            .filter(|s| !s.is_empty())
            .or(p.filename)
            .unwrap_or_else(|| filename_from_url(&p.final_url));
        let cat = category_for(&fname).to_string();
        if self.category.lock().unwrap().is_empty() {
            *self.category.lock().unwrap() = cat;
        }
        *self.filename.lock().unwrap() = fname.clone();
        let dir = if self.auto_category {
            self.save_dir.join(category_for(&fname))
        } else {
            self.save_dir.clone()
        };
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(&fname);
        *self.tmp_path.lock().unwrap() = Some(path.with_file_name(format!("{}.tgpart", fname)));
        *self.meta_path.lock().unwrap() = Some(path.with_file_name(format!("{}.tgmeta", fname)));
        *self.path.lock().unwrap() = Some(path);
        *self.size.lock().unwrap() = p.size;
        *self.final_url.lock().unwrap() = Some(p.final_url);
        *self.etag.lock().unwrap() = p.etag;
        self.range_ok.store(p.accept_ranges, Ordering::Relaxed);
        Ok(())
    }

    // ---------------- meta ----------------

    pub fn save_meta(&self, force: bool) {
        {
            let mut last = self.last_meta_save.lock().unwrap();
            if !force && last.elapsed() < Duration::from_millis(1500) {
                return;
            }
            *last = Instant::now();
        }
        let meta_path = match self.meta_path.lock().unwrap().clone() {
            Some(p) => p,
            None => return,
        };
        let meta = Meta {
            version: 1,
            url: self.url.clone(),
            final_url: self.final_url.lock().unwrap().clone(),
            filename: self.filename.lock().unwrap().clone(),
            size: *self.size.lock().unwrap(),
            etag: self.etag.lock().unwrap().clone(),
            single_stream: self.single_stream.load(Ordering::Relaxed),
            stream_pos: self.stream_pos.load(Ordering::Relaxed),
            connections: *self.connections.lock().unwrap(),
            segments: self.segments.lock().unwrap().clone(),
        };
        if let Ok(json) = serde_json::to_string(&meta) {
            let tmp = meta_path.with_extension("tgmeta.new");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &meta_path);
            }
        }
    }

    fn load_meta(&self) -> Option<Meta> {
        let meta_path = self.meta_path.lock().unwrap().clone()?;
        let data = std::fs::read_to_string(meta_path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // ---------------- run ----------------

    pub fn start(self: &Arc<Self>, resume: bool) {
        {
            let st = *self.status.lock().unwrap();
            if matches!(st, Status::Downloading | Status::Probing | Status::Completed) {
                return;
            }
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.ctrl.pause.store(false, Ordering::Relaxed);
        self.ctrl.cancel.store(false, Ordering::Relaxed);
        *self.last_try.lock().unwrap() = Some(now_secs());
        *self.status.lock().unwrap() = Status::Queued;
        let me = self.clone();
        thread::spawn(move || me.run_supervisor(resume));
    }

    fn run_supervisor(self: Arc<Self>, resume: bool) {
        let result = self.do_run(resume);
        self.running.store(false, Ordering::SeqCst);
        match result {
            Ok(()) => {}
            Err(e) => {
                let was_pause = self.ctrl.pause.load(Ordering::Relaxed);
                let was_cancel = self.ctrl.cancel.load(Ordering::Relaxed);
                if was_cancel {
                    self.finish_cancelled();
                } else if was_pause {
                    *self.status.lock().unwrap() = Status::Paused;
                    self.save_meta(true);
                } else {
                    *self.status.lock().unwrap() = Status::Error;
                    *self.error.lock().unwrap() = e;
                    self.save_meta(true);
                }
            }
        }
    }

    fn do_run(self: &Arc<Self>, resume: bool) -> Result<(), String> {
        *self.status.lock().unwrap() = Status::Probing;
        self.ensure_probed()?;
        if self.ctrl.cancel.load(Ordering::Relaxed) {
            self.finish_cancelled();
            return Ok(());
        }
        if self.ctrl.pause.load(Ordering::Relaxed) {
            *self.status.lock().unwrap() = Status::Paused;
            return Ok(());
        }

        let size = *self.size.lock().unwrap();
        let range_ok = self.range_ok.load(Ordering::Relaxed);
        let tmp_path = self.tmp_path.lock().unwrap().clone().unwrap();
        let meta_path = self.meta_path.lock().unwrap().clone().unwrap();

        // ---- resume or fresh plan
        let mut have_partial = resume && tmp_path.exists();
        let mut plan_single = !range_ok || size.is_none();
        let mut plan_stream_pos: u64 = 0;
        let mut plan_segments: Vec<Segment> = Vec::new();

        if have_partial {
            if let Some(meta) = self.load_meta() {
                let same = meta.size == size && meta.url == self.url && meta.filename == *self.filename.lock().unwrap();
                if same {
                    plan_single = meta.single_stream;
                    plan_stream_pos = meta.stream_pos;
                    plan_segments = meta.segments;
                    for s in plan_segments.iter_mut() {
                        if s.done > s.len() {
                            s.done = s.len();
                        }
                    }
                } else {
                    have_partial = false;
                }
            } else {
                have_partial = false;
            }
        }

        if !have_partial {
            let _ = std::fs::remove_file(&tmp_path);
            let f = File::create(&tmp_path).map_err(|e| format!("cannot create {}: {}", tmp_path.display(), e))?;
            if let Some(sz) = size {
                if !plan_single && sz > 0 {
                    let _ = f.set_len(sz);
                }
            }
            drop(f);
            if plan_single {
                plan_stream_pos = 0;
                plan_segments = Vec::new();
            } else {
                plan_segments = plan_for(size.unwrap_or(0), *self.connections.lock().unwrap());
            }
        } else {
            // never shrink an existing partial away; only extend if needed
            if let Some(sz) = size {
                if !plan_single {
                    if let Ok(f) = OpenOptions::new().read(true).write(true).open(&tmp_path) {
                        let cur = f.metadata().map(|m| m.len()).unwrap_or(0);
                        if cur < sz {
                            let _ = f.set_len(sz);
                        }
                    }
                }
            }
        }

        self.single_stream.store(plan_single, Ordering::Relaxed);
        self.stream_pos.store(plan_stream_pos, Ordering::Relaxed);
        {
            let mut segs = self.segments.lock().unwrap();
            *segs = plan_segments;
        }
        self.done.store(self.compute_done(), Ordering::Relaxed);
        let _ = meta_path;

        *self.status.lock().unwrap() = Status::Downloading;
        self.spawn_meta_keeper();

        // ---- spawn workers
        let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();
        if plan_single {
            let me = self.clone();
            handles.push(thread::spawn(move || me.run_stream_worker()));
        } else {
            let n = self.segments.lock().unwrap().len();
            for idx in 0..n {
                let remaining = {
                    let segs = self.segments.lock().unwrap();
                    segs[idx].len() - segs[idx].done
                };
                if remaining == 0 {
                    continue;
                }
                let me = self.clone();
                handles.push(thread::spawn(move || me.run_segment_worker(idx)));
            }
        }
        for h in handles {
            let _ = h.join();
        }

        // ---- outcome
        if self.ctrl.cancel.load(Ordering::Relaxed) {
            self.finish_cancelled();
            return Ok(());
        }
        if self.ctrl.pause.load(Ordering::Relaxed) {
            *self.status.lock().unwrap() = Status::Paused;
            self.save_meta(true);
            return Ok(());
        }
        let err = self.error.lock().unwrap().clone();
        if !err.is_empty() {
            *self.status.lock().unwrap() = Status::Error;
            self.save_meta(true);
            return Ok(());
        }
        self.finalize()
    }

    fn finish_cancelled(&self) {
        *self.status.lock().unwrap() = Status::Cancelled;
        if self.delete_partial.load(Ordering::Relaxed) {
            if let Some(p) = self.tmp_path.lock().unwrap().clone() {
                let _ = std::fs::remove_file(p);
            }
            if let Some(p) = self.meta_path.lock().unwrap().clone() {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    fn finalize(&self) -> Result<(), String> {
        let done = self.compute_done();
        if let Some(sz) = *self.size.lock().unwrap() {
            if done < sz {
                *self.status.lock().unwrap() = Status::Error;
                *self.error.lock().unwrap() = format!("incomplete: {}/{} bytes", done, sz);
                self.save_meta(true);
                return Ok(());
            }
        }
        let tmp = self.tmp_path.lock().unwrap().clone().unwrap();
        let path = self.path.lock().unwrap().clone().unwrap();
        if let Some(meta) = self.meta_path.lock().unwrap().clone() {
            let _ = std::fs::remove_file(meta);
        }
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        std::fs::rename(&tmp, &path).map_err(|e| format!("cannot finalize: {}", e))?;
        *self.status.lock().unwrap() = Status::Completed;
        Ok(())
    }

    fn spawn_meta_keeper(self: &Arc<Self>) {
        let me = self.clone();
        thread::spawn(move || loop {
            if *me.status.lock().unwrap() != Status::Downloading {
                break;
            }
            if me.ctrl.cancel.load(Ordering::Relaxed) {
                break;
            }
            me.save_meta(true);
            thread::sleep(Duration::from_millis(1000));
        });
    }

    fn compute_done(&self) -> u64 {
        if self.single_stream.load(Ordering::Relaxed) {
            self.stream_pos.load(Ordering::Relaxed)
        } else {
            self.segments.lock().unwrap().iter().map(|s| s.done).sum()
        }
    }

    fn add_progress(&self, idx: usize, n: u64) {
        let total = {
            let mut segs = self.segments.lock().unwrap();
            if let Some(s) = segs.get_mut(idx) {
                s.done += n;
            }
            segs.iter().map(|s| s.done).sum::<u64>()
        };
        self.done.store(total, Ordering::Relaxed);
        self.speed.lock().unwrap().update(total);
    }

    fn add_stream_progress(&self, n: u64) {
        let total = self.stream_pos.fetch_add(n, Ordering::Relaxed) + n;
        self.done.store(total, Ordering::Relaxed);
        self.speed.lock().unwrap().update(total);
    }

    // ---------------- workers ----------------

    fn run_segment_worker(self: Arc<Self>, idx: usize) {
        let mut attempt: u32 = 0;
        loop {
            if self.ctrl.check().is_err() {
                return;
            }
            let (pos, end, remaining) = {
                let segs = self.segments.lock().unwrap();
                let s = &segs[idx];
                (s.pos(), s.end, s.len() - s.done)
            };
            if remaining == 0 {
                return;
            }
            match self.segment_session(idx, pos, end) {
                Ok(()) => return,
                Err(StopErr::Pause) | Err(StopErr::Cancel) => return,
                Err(StopErr::Io(e)) => {
                    attempt += 1;
                    if attempt >= self.retries {
                        *self.error.lock().unwrap() =
                            format!("segment {} failed after {} tries: {}", idx, attempt, e);
                        self.ctrl.cancel.store(true, Ordering::Relaxed);
                        return;
                    }
                    self.save_meta(true);
                    let backoff = (1.5_f64.powi(attempt.min(5) as i32) * 1000.0) as u64;
                    thread::sleep(Duration::from_millis(backoff.min(30_000)));
                }
            }
        }
    }

    fn segment_session(&self, idx: usize, pos: u64, end: u64) -> Result<(), StopErr> {
        let url = self
            .final_url
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.url.clone());
        let agent = make_agent();
        let mut req = agent
            .get(&url)
            .set("User-Agent", &self.ua)
            .set("Accept", "*/*")
            .set("Accept-Encoding", "identity")
            .set("Range", &format!("bytes={}-{}", pos, end));
        if let Some(r) = &self.referer {
            req = req.set("Referer", r);
        }
        if let Some(c) = &self.cookie {
            req = req.set("Cookie", c);
        }
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                return Err(StopErr::Io(format!("HTTP {}", code)))
            }
            Err(e) => return Err(StopErr::Io(format!("{}", e))),
        };
        let st = resp.status();
        if st == 200 && pos > 0 {
            return Err(StopErr::Io("server stopped honoring Range".to_string()));
        }
        if st != 200 && st != 206 {
            return Err(StopErr::Io(format!("HTTP {}", st)));
        }
        let tmp = self
            .tmp_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| StopErr::Io("no tmp path".to_string()))?;
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| StopErr::Io(e.to_string()))?;
        f.seek(SeekFrom::Start(pos))
            .map_err(|e| StopErr::Io(e.to_string()))?;
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; READ_CHUNK];
        let mut remaining = end - pos + 1;
        let mut written_since_flush: u64 = 0;
        while remaining > 0 {
            self.ctrl.check()?;
            let want = buf.len().min(remaining as usize);
            let n = reader
                .read(&mut buf[..want])
                .map_err(|e| StopErr::Io(e.to_string()))?;
            if n == 0 {
                return Err(StopErr::Io("connection closed early".to_string()));
            }
            self.limiter.consume(n, &self.ctrl)?;
            f.write_all(&buf[..n]).map_err(|e| StopErr::Io(e.to_string()))?;
            written_since_flush += n as u64;
            remaining -= n as u64;
            self.add_progress(idx, n as u64);
            if written_since_flush >= FLUSH_EVERY {
                let _ = f.sync_data();
                written_since_flush = 0;
                self.save_meta(false);
            }
        }
        let _ = f.sync_data();
        Ok(())
    }

    fn run_stream_worker(self: Arc<Self>) {
        let mut attempt: u32 = 0;
        loop {
            if self.ctrl.check().is_err() {
                return;
            }
            match self.stream_session() {
                Ok(()) => return,
                Err(StopErr::Pause) | Err(StopErr::Cancel) => return,
                Err(StopErr::Io(e)) => {
                    attempt += 1;
                    if attempt >= self.retries {
                        *self.error.lock().unwrap() =
                            format!("stream failed after {} tries: {}", attempt, e);
                        self.ctrl.cancel.store(true, Ordering::Relaxed);
                        return;
                    }
                    self.save_meta(true);
                    let backoff = (1.5_f64.powi(attempt.min(5) as i32) * 1000.0) as u64;
                    thread::sleep(Duration::from_millis(backoff.min(30_000)));
                }
            }
        }
    }

    fn stream_session(&self) -> Result<(), StopErr> {
        let url = self
            .final_url
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.url.clone());
        let agent = make_agent();
        let pos = self.stream_pos.load(Ordering::Relaxed);
        let range_ok = self.range_ok.load(Ordering::Relaxed);
        let mut req = agent
            .get(&url)
            .set("User-Agent", &self.ua)
            .set("Accept", "*/*")
            .set("Accept-Encoding", "identity");
        if pos > 0 && range_ok {
            req = req.set("Range", &format!("bytes={}-", pos));
        }
        if let Some(r) = &self.referer {
            req = req.set("Referer", r);
        }
        if let Some(c) = &self.cookie {
            req = req.set("Cookie", c);
        }
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                return Err(StopErr::Io(format!("HTTP {}", code)))
            }
            Err(e) => return Err(StopErr::Io(format!("{}", e))),
        };
        let st = resp.status();
        if st != 200 && st != 206 {
            return Err(StopErr::Io(format!("HTTP {}", st)));
        }
        let mut start_pos = pos;
        if pos > 0 && st != 206 {
            start_pos = 0;
            self.stream_pos.store(0, Ordering::Relaxed);
            self.done.store(0, Ordering::Relaxed);
        }
        if let Some(cr) = resp.header("Content-Range") {
            if let Some(total) = cr.rsplit('/').next() {
                if let Ok(v) = total.trim().parse::<u64>() {
                    if v > 0 {
                        *self.size.lock().unwrap() = Some(v);
                    }
                }
            }
        }
        let tmp = self
            .tmp_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| StopErr::Io("no tmp path".to_string()))?;
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| StopErr::Io(e.to_string()))?;
        f.seek(SeekFrom::Start(start_pos))
            .map_err(|e| StopErr::Io(e.to_string()))?;
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; READ_CHUNK];
        let mut written_since_flush: u64 = 0;
        loop {
            self.ctrl.check()?;
            let n = reader
                .read(&mut buf)
                .map_err(|e| StopErr::Io(e.to_string()))?;
            if n == 0 {
                break;
            }
            self.limiter.consume(n, &self.ctrl)?;
            f.write_all(&buf[..n]).map_err(|e| StopErr::Io(e.to_string()))?;
            written_since_flush += n as u64;
            self.add_stream_progress(n as u64);
            if written_since_flush >= FLUSH_EVERY {
                let _ = f.sync_data();
                written_since_flush = 0;
                self.save_meta(false);
            }
        }
        let _ = f.sync_data();
        Ok(())
    }

    // ---------------- control ----------------

    pub fn pause(&self) {
        let st = *self.status.lock().unwrap();
        if matches!(st, Status::Downloading | Status::Probing) {
            self.ctrl.pause.store(true, Ordering::Relaxed);
        }
    }

    pub fn cancel(&self, delete_partial: bool) {
        self.delete_partial.store(delete_partial, Ordering::Relaxed);
        self.ctrl.cancel.store(true, Ordering::Relaxed);
        self.ctrl.pause.store(false, Ordering::Relaxed);
        let st = *self.status.lock().unwrap();
        if matches!(st, Status::Queued | Status::Paused | Status::Error | Status::Cancelled) {
            // not running: clean up immediately
            self.finish_cancelled();
        }
    }

    pub fn resume(self: &Arc<Self>) {
        if *self.status.lock().unwrap() == Status::Paused {
            self.start(true);
        }
    }

    // ---------------- view ----------------

    pub fn view(&self) -> TaskView {
        let size = *self.size.lock().unwrap();
        let done = self.compute_done();
        let status = *self.status.lock().unwrap();
        let speed = self.speed.lock().unwrap().speed;
        let progress = match size {
            Some(sz) if sz > 0 => (done as f32 / sz as f32).min(1.0),
            _ => 0.0,
        };
        let eta = if status == Status::Downloading && speed > 1.0 {
            size.map(|sz| (sz.saturating_sub(done)) as f64 / speed)
        } else {
            None
        };
        let single = self.single_stream.load(Ordering::Relaxed);
        let segs: Vec<f32> = {
            let list = self.segments.lock().unwrap();
            if list.is_empty() {
                vec![if single { progress } else { 0.0 }]
            } else {
                list.iter()
                    .map(|s| {
                        let l = s.len();
                        if l == 0 {
                            0.0
                        } else {
                            (s.done as f32 / l as f32).clamp(0.0, 1.0)
                        }
                    })
                    .collect()
            }
        };
        TaskView {
            url: self.url.clone(),
            filename: self.filename.lock().unwrap().clone(),
            size,
            done,
            progress,
            speed,
            eta,
            status,
            error: self.error.lock().unwrap().clone(),
            path: self.path.lock().unwrap().clone(),
            category: self.category.lock().unwrap().clone(),
            connections: *self.connections.lock().unwrap(),
            single_stream: self.single_stream.load(Ordering::Relaxed),
            segs,
            last_try: *self.last_try.lock().unwrap(),
        }
    }
}

pub fn plan_for(size: u64, connections: usize) -> Vec<Segment> {
    let mut n = connections.clamp(1, MAX_CONNECTIONS);
    if size < MIN_SEGMENT {
        n = 1;
    } else {
        let cap = (size / MIN_SEGMENT) as usize;
        if cap < n {
            n = cap.max(1);
        }
    }
    let base = size / n as u64;
    let mut segs = Vec::with_capacity(n);
    let mut start = 0u64;
    for i in 0..n {
        let end = if i < n - 1 { start + base - 1 } else { size - 1 };
        segs.push(Segment {
            start,
            end,
            done: 0,
        });
        start = end + 1;
    }
    segs
}
