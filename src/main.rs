//! OpenDown — IDM-style download manager: GUI (egui) + CLI.
//! Original implementation; no IDM code or assets are used.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;

use engine::{
    category_for, human_eta, human_size, human_speed, plan_for, DownloadTask, SpeedLimiter, Status,
    DEFAULT_CONNECTIONS, DEFAULT_UA,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use egui_extras::{Column, TableBuilder};

// ===========================================================================
// config / persistence
// ===========================================================================

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppCfg {
    pub default_dir: String,
    pub connections: usize,
    pub ua: String,
    pub referer: String,
    pub retries: u32,
    pub monitor_clipboard: bool,
    pub auto_category: bool,
    pub grab_enabled: bool,
    pub grab_port: u16,
    pub grab_auto_start: bool,
    pub minimize_to_tray: bool,
    pub classic_theme: bool,
    /// 0 = unlimited, 1 = 128 KB/s, 2 = 512 KB/s, 3 = 2 MB/s, 4 = custom
    pub speed_preset: usize,
    pub custom_speed_kbps: f64,
}

impl Default for AppCfg {
    fn default() -> Self {
        let dir = std::env::var("USERPROFILE")
            .map(|h| format!("{}\\Downloads", h))
            .unwrap_or_else(|_| ".".to_string());
        Self {
            default_dir: dir,
            connections: DEFAULT_CONNECTIONS,
            ua: DEFAULT_UA.to_string(),
            referer: String::new(),
            retries: 12,
            monitor_clipboard: false,
            auto_category: true,
            grab_enabled: true,
            grab_port: 10081,
            grab_auto_start: true,
            minimize_to_tray: true,
            classic_theme: false,
            speed_preset: 0,
            custom_speed_kbps: 500.0,
        }
    }
}

impl AppCfg {
    fn limit_bytes(&self) -> Option<f64> {
        match self.speed_preset {
            1 => Some(128.0 * 1024.0),
            2 => Some(512.0 * 1024.0),
            3 => Some(2.0 * 1024.0 * 1024.0),
            4 => {
                if self.custom_speed_kbps > 0.0 {
                    Some(self.custom_speed_kbps * 1024.0)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn limit_label(&self) -> String {
        match self.speed_preset {
            1 => "128 KB/s".to_string(),
            2 => "512 KB/s".to_string(),
            3 => "2 MB/s".to_string(),
            4 => format!("{:.0} KB/s", self.custom_speed_kbps),
            _ => "Unlimited".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedTask {
    url: String,
    filename: String,
    save_dir: String,
    connections: usize,
    referer: String,
    #[serde(default)]
    cookie: String,
}

fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let d = PathBuf::from(base).join("OpenDown");
    let _ = std::fs::create_dir_all(&d);
    d
}

// ===========================================================================
// manager
// ===========================================================================

pub struct Manager {
    pub tasks: Vec<Arc<DownloadTask>>,
    pub limiter: Arc<SpeedLimiter>,
    pub cfg: AppCfg,
    pub queue_running: bool,
    cfg_path: PathBuf,
    tasks_path: PathBuf,
}

impl Manager {
    pub fn new() -> Self {
        let dd = data_dir();
        let cfg_path = dd.join("config.json");
        let tasks_path = dd.join("tasks.json");
        let cfg: AppCfg = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let limiter = Arc::new(SpeedLimiter::new(cfg.limit_bytes()));
        let mut tasks = Vec::new();
        if let Ok(s) = std::fs::read_to_string(&tasks_path) {
            if let Ok(saved) = serde_json::from_str::<Vec<SavedTask>>(&s) {
                for st in saved {
                    let fo = if st.filename.is_empty() {
                        None
                    } else {
                        Some(st.filename.clone())
                    };
                    let t = DownloadTask::new(
                        &st.url,
                        PathBuf::from(&st.save_dir),
                        fo,
                        st.connections,
                        &cfg.ua,
                        if st.referer.is_empty() { None } else { Some(st.referer.clone()) },
                        if st.cookie.is_empty() { None } else { Some(st.cookie.clone()) },
                        limiter.clone(),
                        cfg.retries,
                        cfg.auto_category,
                    );
                    *t.category.lock().unwrap() = category_for(&st.filename).to_string();
                    tasks.push(Arc::new(t));
                }
            }
        }
        let queue_running = std::env::var("OPENDOWN_AUTOSTART").is_ok();
        Self {
            tasks,
            limiter,
            cfg,
            queue_running,
            cfg_path,
            tasks_path,
        }
    }

    pub fn save_cfg(&self) {
        if let Ok(j) = serde_json::to_string_pretty(&self.cfg) {
            let _ = std::fs::write(&self.cfg_path, j);
        }
        self.limiter.set_rate(self.cfg.limit_bytes());
    }

    pub fn save_tasks(&self) {
        let saved: Vec<SavedTask> = self
            .tasks
            .iter()
            .filter(|t| {
                let st = *t.status.lock().unwrap();
                st != Status::Cancelled
            })
            .map(|t| {
                let v = t.view();
                SavedTask {
                    url: t.url.clone(),
                    filename: v.filename,
                    save_dir: t.save_dir.display().to_string(),
                    connections: v.connections,
                    referer: t.referer.clone().unwrap_or_default(),
                    cookie: t.cookie.clone().unwrap_or_default(),
                    }
            })
            .collect();
        if let Ok(j) = serde_json::to_string_pretty(&saved) {
            let _ = std::fs::write(&self.tasks_path, j);
        }
    }

    pub fn add_task(
        &mut self,
        url: &str,
        dir: &str,
        filename: Option<String>,
        connections: usize,
        referer: Option<String>,
        cookie: Option<String>,
    ) -> Arc<DownloadTask> {
        let t = Arc::new(DownloadTask::new(
            url,
            PathBuf::from(dir),
            filename.clone(),
            connections,
            &self.cfg.ua,
            referer,
            cookie,
            self.limiter.clone(),
            self.cfg.retries,
            self.cfg.auto_category,
        ));
        if let Some(f) = &filename {
            *t.category.lock().unwrap() = category_for(f).to_string();
        }
        self.tasks.push(t.clone());
        self.save_tasks();
        t
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.tasks.len() {
            let t = self.tasks[idx].clone();
            let st = *t.status.lock().unwrap();
            let del = !matches!(st, Status::Completed);
            t.cancel(del);
            self.tasks.remove(idx);
            self.save_tasks();
        }
    }

    pub fn remove_completed(&mut self) {
        self.tasks.retain(|t| *t.status.lock().unwrap() != Status::Completed);
        self.save_tasks();
    }

    pub fn tick_queue(&mut self) {
        if !self.queue_running {
            return;
        }
        let busy = self.tasks.iter().any(|t| {
            let st = *t.status.lock().unwrap();
            matches!(st, Status::Downloading | Status::Probing)
        });
        if busy {
            return;
        }
        for t in &self.tasks {
            if *t.status.lock().unwrap() == Status::Queued {
                t.start(false);
                break;
            }
        }
    }
}

// ===========================================================================
// UI helpers
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum CatFilter {
    Video,
    Music,
    Documents,
    Programs,
    Compressed,
    Other,
}

impl CatFilter {
    fn label(&self) -> &'static str {
        match self {
            CatFilter::Video => "Video",
            CatFilter::Music => "Music",
            CatFilter::Documents => "Documents",
            CatFilter::Programs => "Programs",
            CatFilter::Compressed => "Compressed",
            CatFilter::Other => "Other",
        }
    }
    fn matches(&self, cat: &str) -> bool {
        self.label() == cat
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SideSel {
    All,
    Cat(CatFilter),
    Unfinished,
    Finished,
    Grabber,
    Queues,
}

fn side_matches(sel: SideSel, cat: &str, st: Status) -> bool {
    match sel {
        SideSel::All => true,
        SideSel::Cat(c) => c.matches(cat),
        SideSel::Unfinished => st != Status::Completed,
        SideSel::Finished => st == Status::Completed,
        SideSel::Queues => matches!(st, Status::Queued | Status::Downloading | Status::Paused),
        SideSel::Grabber => false,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fmt_ago(ts: u64) -> String {
    let d = now_secs().saturating_sub(ts);
    if d < 60 {
        format!("{}s", d)
    } else if d < 3600 {
        format!("{}m", d / 60)
    } else {
        format!("{}h", d / 3600)
    }
}

fn make_icon() -> egui::IconData {
    let s = 32usize;
    let mut rgba = vec![0u8; s * s * 4];
    for y in 0..s {
        for x in 0..s {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let d = (dx * dx + dy * dy).sqrt();
            let mut c = [0u8, 0, 0, 0];
            if d <= 15.5 {
                c = [0, 120, 215, 255];
                let xi = x as i32;
                let yi = y as i32;
                if (14..=18).contains(&xi) && (7..=17).contains(&yi) {
                    c = [255, 255, 255, 255];
                }
                if (16..=25).contains(&yi) {
                    let half = 25 - yi;
                    if (xi - 16).abs() <= half {
                        c = [255, 255, 255, 255];
                    }
                }
            }
            let i = (y * s + x) * 4;
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    egui::IconData {
        rgba,
        width: s as u32,
        height: s as u32,
    }
}

/// Small speed history graph (original rendering).
fn paint_speed_graph(ui: &mut egui::Ui, hist: &[f32], size: egui::Vec2, current: f64) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let track = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(56, 60, 66)
    } else {
        egui::Color32::from_rgb(252, 252, 252)
    };
    p.rect_filled(rect, 2.0, track);
    p.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 170, 170)),
    );
    let n = hist.len();
    if n > 1 {
        let max = hist.iter().cloned().fold(1.0f32, f32::max);
        let pts: Vec<egui::Pos2> = hist
            .iter()
            .enumerate()
            .map(|(i, s)| {
                egui::pos2(
                    rect.left() + rect.width() * i as f32 / (n - 1) as f32,
                    rect.bottom() - rect.height() * (s / max).min(1.0),
                )
            })
            .collect();
        p.add(egui::Shape::line(
            pts,
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 110, 180)),
        ));
    }
    if rect.height() >= 30.0 {
        let peak = hist.iter().cloned().fold(0.0f32, f32::max);
        let txt = if current > 0.0 {
            format!("{}  (peak {})", human_speed(current), human_speed(peak as f64))
        } else {
            format!("peak {}", human_speed(peak as f64))
        };
        p.text(
            rect.left_top() + egui::vec2(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            txt,
            egui::FontId::proportional(10.0),
            egui::Color32::GRAY,
        );
    }
}

const C_GREEN: egui::Color32 = egui::Color32::from_rgb(102, 187, 106);
const C_RED: egui::Color32 = egui::Color32::from_rgb(239, 83, 80);
const C_PINK: egui::Color32 = egui::Color32::from_rgb(244, 143, 177);
const C_PURPLE: egui::Color32 = egui::Color32::from_rgb(179, 136, 255);
const C_YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 213, 79);
const C_SOFT: egui::Color32 = egui::Color32::from_rgb(255, 138, 128);
const C_FOLDER: egui::Color32 = egui::Color32::from_rgb(120, 170, 255);

struct TreeRowResp {
    resp: egui::Response,
    caret: bool,
}

/// Sidebar row: colored icon + label + optional count; whole row is clickable.
fn tree_row(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    count: Option<usize>,
    selected: bool,
    indent: f32,
    icon_color: egui::Color32,
    caret: Option<bool>,
) -> TreeRowResp {
    let w = ui.available_width().max(60.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let weak = ui.visuals().weak_text_color();
    if selected {
        painter.rect_filled(rect, egui::Rounding::same(4.0), ui.visuals().selection.bg_fill);
    } else if resp.hovered() {
        let hb = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(60, 60, 60)
        } else {
            egui::Color32::from_rgb(226, 230, 236)
        };
        painter.rect_filled(rect, egui::Rounding::same(4.0), hb);
    }
    let mut x = rect.left() + 3.0;
    let mut caret_clicked = false;
    if let Some(exp) = caret {
        let glyph = if exp { "▾" } else { "▸" };
        let cg = painter.layout_no_wrap(glyph.to_owned(), egui::FontId::proportional(11.0), weak);
        painter.galley(
            egui::pos2(x, rect.center().y - cg.size().y * 0.5),
            cg.clone(),
            weak,
        );
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if pos.x < x + 14.0 {
                    caret_clicked = true;
                }
            }
        }
        x += 14.0;
    }
    let ig = painter.layout_no_wrap(icon.to_owned(), egui::FontId::proportional(13.0), icon_color);
    painter.galley(
        egui::pos2(x + indent, rect.center().y - ig.size().y * 0.5),
        ig.clone(),
        icon_color,
    );
    let tcol = ui.visuals().text_color();
    let lg = painter.layout_no_wrap(label.to_owned(), egui::FontId::proportional(13.5), tcol);
    painter.galley(
        egui::pos2(x + indent + 19.0, rect.center().y - lg.size().y * 0.5),
        lg.clone(),
        tcol,
    );
    if let Some(n) = count {
        let cg = painter.layout_no_wrap(
            format!("({})", n),
            egui::FontId::proportional(11.0),
            weak,
        );
        painter.galley(
            egui::pos2(rect.right() - cg.size().x - 8.0, rect.center().y - cg.size().y * 0.5),
            cg.clone(),
            weak,
        );
    }
    TreeRowResp {
        resp,
        caret: caret_clicked,
    }
}

/// Large toolbar button: colored icon on top, small label below (IDM style).
fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    icon_color: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    let w = 64.0;
    let h = 54.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let painter = ui.painter_at(rect);
    if resp.hovered() || resp.clicked() {
        let bg = if resp.clicked() {
            ui.visuals().widgets.active.bg_fill
        } else {
            ui.visuals().widgets.hovered.bg_fill
        };
        painter.rect_filled(rect, egui::Rounding::same(5.0), bg);
    }
    let ig = painter.layout_no_wrap(icon.to_owned(), egui::FontId::proportional(21.0), icon_color);
    painter.galley(
        egui::pos2(rect.center().x - ig.size().x * 0.5, rect.top() + 4.0),
        ig.clone(),
        icon_color,
    );
    let text_col = ui.visuals().text_color();
    let small = egui::FontId::proportional(10.5);
    let mut text = label.to_owned();
    if painter
        .layout_no_wrap(text.clone(), small.clone(), text_col)
        .size()
        .x
        > w - 6.0
    {
        while !text.is_empty()
            && painter
                .layout_no_wrap(format!("{}…", text), small.clone(), text_col)
                .size()
                .x
                > w - 6.0
        {
            text.pop();
        }
        text.push('…');
    }
    let lg = painter.layout_no_wrap(text, small, text_col);
    painter.galley(
        egui::pos2(rect.center().x - lg.size().x * 0.5, rect.bottom() - lg.size().y - 3.0),
        lg.clone(),
        text_col,
    );
    resp.on_hover_text(tooltip)
}

fn status_color(st: Status) -> egui::Color32 {
    match st {
        Status::Downloading => egui::Color32::from_rgb(0, 128, 0),
        Status::Completed => egui::Color32::from_rgb(0, 100, 170),
        Status::Paused => egui::Color32::from_rgb(170, 120, 0),
        Status::Error => egui::Color32::from_rgb(190, 30, 30),
        Status::Probing => egui::Color32::from_rgb(70, 70, 190),
        Status::Queued => egui::Color32::from_rgb(110, 110, 110),
        Status::Cancelled => egui::Color32::from_rgb(110, 110, 110),
    }
}

fn bar_color(st: Status) -> egui::Color32 {
    match st {
        Status::Downloading => egui::Color32::from_rgb(0, 120, 215),
        Status::Completed => egui::Color32::from_rgb(0, 150, 90),
        Status::Paused => egui::Color32::from_rgb(215, 160, 30),
        Status::Error => egui::Color32::from_rgb(200, 60, 60),
        _ => egui::Color32::from_rgb(130, 130, 130),
    }
}

// ===========================================================================
// browser grabber — local HTTP endpoint used by the OpenDown browser extension
// ===========================================================================

#[derive(serde::Deserialize)]
struct GrabReq {
    url: String,
    #[serde(default)]
    referer: String,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    cookie: String,
}

struct GrabJob {
    url: String,
    referer: String,
    filename: String,
    cookie: String,
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn start_grab_server(tx: std::sync::mpsc::Sender<GrabJob>, port: u16) -> std::io::Result<()> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = handle_grab(&mut s, &tx);
            });
        }
    });
    Ok(())
}

fn handle_grab(
    s: &mut std::net::TcpStream,
    tx: &std::sync::mpsc::Sender<GrabJob>,
) -> std::io::Result<()> {
    use std::io::{Read, Write};
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    let (head, body) = loop {
        let n = s.read(&mut tmp)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..pos]).to_string();
            let clen = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    if k.eq_ignore_ascii_case("content-length") {
                        v.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
                .min(1_048_576);
            while buf.len() < pos + 4 + clen {
                let n = s.read(&mut tmp)?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let body = buf[pos + 4..(pos + 4 + clen).min(buf.len())].to_vec();
            break (head, body);
        }
        if buf.len() > 262_144 {
            return Ok(());
        }
    };

    let first = head.lines().next().unwrap_or("").to_string();
    let (status, body_out): (&str, String) = if first.starts_with("OPTIONS") {
        ("204 No Content", String::new())
    } else if first.starts_with("GET /ping") {
        ("200 OK", "{\"ok\":true,\"app\":\"opendown\"}".to_string())
    } else if first.starts_with("POST /add") {
        match serde_json::from_slice::<GrabReq>(&body) {
            Ok(req) if req.url.starts_with("http") => {
                let _ = tx.send(GrabJob {
                    url: req.url,
                    referer: req.referer,
                    filename: req.filename,
                    cookie: req.cookie,
                });
                ("200 OK", "{\"ok\":true}".to_string())
            }
            _ => ("400 Bad Request", "{\"ok\":false}".to_string()),
        }
    } else {
        ("404 Not Found", "{\"ok\":false}".to_string())
    };

    let resp = format!(
        "HTTP/1.1 {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body_out.len(),
        body_out
    );
    let _ = s.write_all(resp.as_bytes());
    let _ = s.flush();
    Ok(())
}

// ===========================================================================
// System tray + autostart
// ===========================================================================

enum TrayMsg {
    Show,
    Exit,
}

fn make_tray(tx: std::sync::mpsc::Sender<TrayMsg>) -> Result<tray_icon::TrayIcon, String> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{TrayIconBuilder, TrayIconEvent};

    let menu = Menu::new();
    let show_item = MenuItem::new("Show OpenDown", true, None);
    let exit_item = MenuItem::new("Exit", true, None);
    menu.append(&show_item).map_err(|e| e.to_string())?;
    menu.append(&exit_item).map_err(|e| e.to_string())?;

    let show_id = show_item.id().clone();
    let exit_id = exit_item.id().clone();
    let tx_menu = tx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if e.id == show_id {
            let _ = tx_menu.send(TrayMsg::Show);
        } else if e.id == exit_id {
            let _ = tx_menu.send(TrayMsg::Exit);
        }
    }));

    let tx_icon = tx;
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = e
        {
            let _ = tx_icon.send(TrayMsg::Show);
        }
    }));

    let ico = make_icon();
    let icon =
        tray_icon::Icon::from_rgba(ico.rgba, ico.width, ico.height).map_err(|e| e.to_string())?;
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("OpenDown — Download Manager")
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())
}

const AUTOSTART_VALUE: &str = "OpenDown";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn autostart_enabled() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(AUTOSTART_VALUE))
        .is_ok()
}

fn set_autostart(enable: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = format!("\"{}\" --minimized", exe.display());
        key.set_value(AUTOSTART_VALUE, &value)
            .map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value(AUTOSTART_VALUE);
    }
    Ok(())
}

#[cfg(windows)]
fn attach_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "rounded".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/VarelaRound-Regular.ttf")).into(),
    );
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.insert(0, "rounded".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Classic light (IDM-like) or dark visuals.
fn apply_theme(ctx: &egui::Context, classic: bool) {
    let mut v = if classic {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    if classic {
        v.panel_fill = egui::Color32::from_rgb(240, 240, 240);
        v.window_fill = egui::Color32::from_rgb(240, 240, 240);
        v.faint_bg_color = egui::Color32::from_rgb(240, 243, 247);
        v.extreme_bg_color = egui::Color32::WHITE;
        v.selection.bg_fill = egui::Color32::from_rgb(0, 120, 215);
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(255, 255, 255));
        v.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 170, 170));
        v.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(25, 25, 25));
    } else {
        // Reference look: dark charcoal + green/blue accents.
        v.panel_fill = egui::Color32::from_rgb(43, 43, 43);
        v.window_fill = egui::Color32::from_rgb(43, 43, 43);
        v.extreme_bg_color = egui::Color32::from_rgb(32, 32, 32);
        v.faint_bg_color = egui::Color32::from_rgb(51, 51, 51);
        v.selection.bg_fill = egui::Color32::from_rgb(40, 92, 190);
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(225, 235, 255));
        v.hyperlink_color = egui::Color32::from_rgb(120, 170, 255);
        v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(58, 58, 58);
        v.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
        v.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(232, 232, 232));
        v.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        v.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(84, 84, 84));
        v.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(228, 228, 228));
        v.widgets.hovered.bg_fill = egui::Color32::from_rgb(74, 74, 74);
        v.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(104, 104, 104));
        v.widgets.active.bg_fill = egui::Color32::from_rgb(88, 88, 88);
        v.widgets.inactive.rounding = egui::Rounding::same(5.0);
        v.widgets.hovered.rounding = egui::Rounding::same(5.0);
        v.widgets.active.rounding = egui::Rounding::same(5.0);
        v.window_rounding = egui::Rounding::same(6.0);
    }
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            egui::FontId::new(19.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            egui::FontId::new(14.5, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            egui::FontId::new(11.5, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            egui::FontId::new(12.5, egui::FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(9.0, 3.0);
    ctx.set_visuals(v);
    ctx.set_style(style);
}

/// IDM-style per-connection progress: one thin vertical bar per segment,
/// filled from the bottom by that segment's progress. Original rendering.
fn paint_segments(
    ui: &mut egui::Ui,
    segs: &[f32],
    status: Status,
    width: f32,
    height: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let track = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(60, 60, 60)
    } else {
        egui::Color32::from_rgb(200, 200, 200)
    };
    if segs.is_empty() {
        painter.rect_filled(rect, 0.0, track);
        return resp;
    }
    let n = segs.len() as f32;
    let gap = 1.0_f32;
    let w = ((rect.width() - gap * (n - 1.0)) / n).max(1.0);
    let fill = bar_color(status);
    for (i, pr) in segs.iter().enumerate() {
        let x = rect.left() + i as f32 * (w + gap);
        let col_rect =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height()));
        painter.rect_filled(col_rect, 0.0, track);
        let h = rect.height() * pr.clamp(0.0, 1.0);
        if h > 0.5 {
            let filled = egui::Rect::from_min_max(
                egui::pos2(x, rect.bottom() - h),
                egui::pos2(x + w, rect.bottom()),
            );
            painter.rect_filled(filled, 0.0, fill);
        }
    }
    resp
}

// ===========================================================================
// app
// ===========================================================================

struct TurboApp {
    mgr: Manager,
    selected: Option<usize>,
    side: SideSel,
    cats_expanded: bool,
    show_side: bool,
    show_add: bool,
    show_options: bool,
    show_log: bool,
    log: Vec<String>,
    prev_states: Vec<Status>,
    add_url: String,
    add_dir: String,
    add_name: String,
    add_conn: i32,
    add_referer: String,
    clip: Option<arboard::Clipboard>,
    last_clip: String,
    clip_check: Instant,
    last_save: Instant,
    speed_hist: Vec<f32>,
    last_hist: Instant,
    total_hist: Vec<f32>,
    sort: Option<(usize, bool)>,
    grab_rx: Option<std::sync::mpsc::Receiver<GrabJob>>,
    tray: Option<tray_icon::TrayIcon>,
    tray_rx: std::sync::mpsc::Receiver<TrayMsg>,
    exit_now: bool,
    start_minimized: bool,
    smoke: Option<(Instant, u64)>,
}

impl TurboApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mgr = Manager::new();
        install_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, mgr.cfg.classic_theme);
        let add_dir = mgr.cfg.default_dir.clone();
        let add_conn = mgr.cfg.connections as i32;
        let clip = arboard::Clipboard::new().ok();
        let smoke = std::env::var("OPENDOWN_SMOKE_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(|ms| (Instant::now(), ms));
        let (grab_tx, grab_rx_all) = std::sync::mpsc::channel::<GrabJob>();
        let mut grab_rx = None;
        let grab_status = if mgr.cfg.grab_enabled {
            match start_grab_server(grab_tx, mgr.cfg.grab_port) {
                Ok(()) => {
                    grab_rx = Some(grab_rx_all);
                    format!("Grabber listening on http://127.0.0.1:{}", mgr.cfg.grab_port)
                }
                Err(e) => format!("Grabber failed to start: {}", e),
            }
        } else {
            "Grabber disabled".to_string()
        };
        let (tray_tx, tray_rx) = std::sync::mpsc::channel::<TrayMsg>();
        let tray_result = make_tray(tray_tx);
        let tray_status = match &tray_result {
            Ok(_) => "System tray icon active".to_string(),
            Err(e) => format!("System tray unavailable: {}", e),
        };
        let start_minimized = std::env::args().any(|a| a == "--minimized");
        let mut app = Self {
            mgr,
            selected: None,
            side: SideSel::All,
            cats_expanded: true,
            show_side: true,
            show_add: false,
            show_options: false,
            show_log: true,
            log: vec![
                "OpenDown ready — click \"Add URL…\" to start.".to_string(),
                grab_status,
                tray_status,
            ],
            prev_states: Vec::new(),
            add_url: String::new(),
            add_dir,
            add_name: String::new(),
            add_conn,
            add_referer: String::new(),
            clip,
            last_clip: String::new(),
            clip_check: Instant::now(),
            last_save: Instant::now(),
            speed_hist: Vec::new(),
            last_hist: Instant::now(),
            total_hist: Vec::new(),
            sort: None,
            grab_rx,
            tray: tray_result.ok(),
            tray_rx,
            exit_now: false,
            start_minimized,
            smoke,
        };
        for _ in 0..app.mgr.tasks.len() {
            app.prev_states.push(Status::Queued);
        }
        if std::env::var("OPENDOWN_AUTOSEL").is_ok() && !app.mgr.tasks.is_empty() {
            app.selected = Some(0);
        }
        app
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 400 {
            let over = self.log.len() - 400;
            self.log.drain(0..over);
        }
    }

    fn poll_clipboard(&mut self) {
        if !self.mgr.cfg.monitor_clipboard || self.show_add {
            return;
        }
        if self.clip_check.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.clip_check = Instant::now();
        let text = match &mut self.clip {
            Some(c) => match c.get_text() {
                Ok(t) => t.trim().to_string(),
                Err(_) => return,
            },
            None => return,
        };
        if text.is_empty() || text == self.last_clip {
            return;
        }
        if text.starts_with("http://") || text.starts_with("https://") {
            if text.len() < 2048 && !text.contains(char::is_whitespace) {
                self.last_clip = text.clone();
                self.add_url = text;
                self.show_add = true;
                self.log("Clipboard: URL detected — Add dialog opened.");
            }
        }
    }

    fn track_transitions(&mut self) {
        while self.prev_states.len() < self.mgr.tasks.len() {
            self.prev_states.push(Status::Queued);
        }
        for i in 0..self.mgr.tasks.len() {
            let st = *self.mgr.tasks[i].status.lock().unwrap();
            if self.prev_states[i] != st {
                let name = self.mgr.tasks[i].view().filename;
                self.log(format!("{}: {}", name, st.label()));
                self.prev_states[i] = st;
            }
        }
    }

    fn selected_task(&self) -> Option<Arc<DownloadTask>> {
        self.selected.and_then(|i| self.mgr.tasks.get(i).cloned())
    }

    fn add_grab(&mut self, job: GrabJob) {
        let filename = if job.filename.trim().is_empty() {
            None
        } else {
            Some(job.filename.clone())
        };
        let referer = if job.referer.trim().is_empty() {
            None
        } else {
            Some(job.referer.clone())
        };
        let cookie = if job.cookie.trim().is_empty() {
            None
        } else {
            Some(job.cookie.clone())
        };
        let dir = self.mgr.cfg.default_dir.clone();
        let conns = self.mgr.cfg.connections;
        let t = self
            .mgr
            .add_task(&job.url, &dir, filename, conns, referer, cookie);
        if self.mgr.cfg.grab_auto_start {
            t.start(false);
            self.log(format!("Grabbed (auto-start): {}", job.url));
        } else {
            self.log(format!("Grabbed: {}", job.url));
        }
    }

    fn menubar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Tasks", |ui| {
                    if ui.button("➕  Add URL…").clicked() {
                        self.show_add = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("▶  Start / Resume").clicked() {
                        if let Some(t) = self.selected_task() {
                            let st = *t.status.lock().unwrap();
                            if st == Status::Paused {
                                t.resume();
                            } else if matches!(st, Status::Queued | Status::Error | Status::Cancelled)
                            {
                                t.start(false);
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("⏸  Stop").clicked() {
                        if let Some(t) = self.selected_task() {
                            t.pause();
                        }
                        ui.close_menu();
                    }
                    if ui.button("⏹  Stop All").clicked() {
                        for t in &self.mgr.tasks {
                            t.pause();
                        }
                        self.mgr.queue_running = false;
                        self.log("Stop All — all downloads paused.");
                        ui.close_menu();
                    }
                });
                ui.menu_button("File", |ui| {
                    if ui.button("📂  Open Folder").clicked() {
                        if let Some(t) = self.selected_task() {
                            let v = t.view();
                            if let Some(p) = v.path {
                                if let Some(dir) = p.parent() {
                                    self.open_path(dir, false);
                                }
                            } else if matches!(v.status, Status::Downloading | Status::Paused) {
                                self.open_path(&t.save_dir, false);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🗑  Remove").clicked() {
                        if let Some(i) = self.selected {
                            self.mgr.remove(i);
                            self.selected = None;
                        }
                        ui.close_menu();
                    }
                    if ui.button("✓  Remove Completed").clicked() {
                        self.mgr.remove_completed();
                        self.selected = None;
                        self.log("Completed tasks removed.");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Downloads", |ui| {
                    let qlabel = if self.mgr.queue_running {
                        "⏸  Stop queue"
                    } else {
                        "▶  Start queue"
                    };
                    if ui.button(qlabel).clicked() {
                        self.mgr.queue_running = !self.mgr.queue_running;
                        ui.close_menu();
                    }
                    let mut preset = self.mgr.cfg.speed_preset;
                    ui.menu_button("Speed limiter", |ui| {
                        for (val, name) in [
                            (0usize, "Unlimited"),
                            (1usize, "128 KB/s"),
                            (2usize, "512 KB/s"),
                            (3usize, "2 MB/s"),
                            (4usize, "Custom…"),
                        ] {
                            if ui.selectable_label(preset == val, name).clicked() {
                                preset = val;
                                ui.close_menu();
                            }
                        }
                    });
                    if preset != self.mgr.cfg.speed_preset {
                        self.mgr.cfg.speed_preset = preset;
                        self.mgr.save_cfg();
                        let l = self.mgr.cfg.limit_label();
                        self.log(format!("Speed limit: {}", l));
                    }
                    ui.separator();
                    if ui.button("⚙  Options…").clicked() {
                        self.show_options = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_log, "Show log");
                    ui.checkbox(&mut self.show_side, "Categories panel");
                });
                ui.menu_button("Registration", |ui| {
                    if ui.button("Status registrasi").clicked() {
                        self.log("OpenDown is free forever — no registration needed 🎉");
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About OpenDown").clicked() {
                        self.log("OpenDown — download manager (original implementation, IDM-style GUI).");
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn open_path(&self, path: &std::path::Path, select: bool) {
        let arg = if select {
            format!("/select,\"{}\"", path.display())
        } else {
            path.display().to_string()
        };
        let _ = std::process::Command::new("explorer").arg(arg).spawn();
    }

    // ------------------------------------------------------------------ UI

    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                if tool_button(ui, "➕", "Add URL", C_GREEN, "Add a new download").clicked() {
                    self.show_add = true;
                }
                ui.separator();
                if tool_button(ui, "▶", "Resume", C_GREEN, "Start / resume the selected task")
                    .clicked()
                {
                    if let Some(t) = self.selected_task() {
                        let st = *t.status.lock().unwrap();
                        if st == Status::Paused {
                            t.resume();
                        } else if matches!(st, Status::Queued | Status::Error | Status::Cancelled)
                        {
                            t.start(false);
                        }
                    }
                }
                if tool_button(ui, "■", "Stop", C_RED, "Pause the selected task").clicked() {
                    if let Some(t) = self.selected_task() {
                        t.pause();
                    }
                }
                if tool_button(ui, "✋", "Stop All", C_SOFT, "Pause all tasks and stop the queue")
                    .clicked()
                {
                    for t in &self.mgr.tasks {
                        t.pause();
                    }
                    self.mgr.queue_running = false;
                    self.log("Stop All — all downloads paused.");
                }
                if tool_button(ui, "🗑", "Delete", C_RED, "Delete the selected task").clicked() {
                    if let Some(i) = self.selected {
                        self.mgr.remove(i);
                        self.selected = None;
                        self.speed_hist.clear();
                    }
                }
                if tool_button(ui, "🗑", "Delete Completed", C_PINK, "Delete all completed tasks")
                    .clicked()
                {
                    self.mgr.remove_completed();
                    self.selected = None;
                    self.log("Completed tasks deleted.");
                }
                ui.separator();
                if tool_button(ui, "⚙", "Options", C_PURPLE, "Settings").clicked() {
                    self.show_options = true;
                }
                if tool_button(ui, "🕐", "Scheduler", C_YELLOW, "Scheduler — not available yet")
                    .clicked()
                {
                    self.log("Scheduler is not available yet.");
                }
                ui.separator();
                if tool_button(ui, "⬇", "Start Queue", C_GREEN, "Jalankan antrean").clicked() {
                    if !self.mgr.queue_running {
                        self.mgr.queue_running = true;
                        self.log("Queue: ON");
                    }
                }
                if tool_button(ui, "⏸", "Stop Queue", C_PINK, "Hentikan antrean").clicked() {
                    if self.mgr.queue_running {
                        self.mgr.queue_running = false;
                        self.log("Queue: off");
                    }
                }
                if tool_button(ui, "👍", "Tell a Friend", C_YELLOW, "Thanks for using OpenDown!")
                    .clicked()
                {
                    self.log("Thanks! Tell a friend about OpenDown 👍");
                }
            });
            ui.add_space(4.0);
        });
    }

    fn categories(&mut self, ctx: &egui::Context) {
        if !self.show_side {
            return;
        }
        egui::SidePanel::left("categories")
            .resizable(false)
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Categories").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(2.0);
                        if ui
                            .small_button("✕")
                            .on_hover_text("Hide (View ▸ Categories panel)")
                            .clicked()
                        {
                            self.show_side = false;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(3.0);

                let total = self.mgr.tasks.len();
                let unfinished = self
                    .mgr
                    .tasks
                    .iter()
                    .filter(|t| t.view().status != Status::Completed)
                    .count();
                let finished = self
                    .mgr
                    .tasks
                    .iter()
                    .filter(|t| t.view().status == Status::Completed)
                    .count();
                let queued = self
                    .mgr
                    .tasks
                    .iter()
                    .filter(|t| {
                        matches!(
                            t.view().status,
                            Status::Queued | Status::Downloading | Status::Paused
                        )
                    })
                    .count();

                let out = tree_row(
                    ui,
                    "📂",
                    "All Downloads",
                    Some(total),
                    self.side == SideSel::All,
                    0.0,
                    C_FOLDER,
                    Some(self.cats_expanded),
                );
                if out.resp.clicked() {
                    if out.caret {
                        self.cats_expanded = !self.cats_expanded;
                    } else {
                        self.side = SideSel::All;
                    }
                }

                if self.cats_expanded {
                    let cats = [
                        CatFilter::Compressed,
                        CatFilter::Documents,
                        CatFilter::Music,
                        CatFilter::Other,
                        CatFilter::Programs,
                        CatFilter::Video,
                    ];
                    for cat in cats {
                        let n = self
                            .mgr
                            .tasks
                            .iter()
                            .filter(|t| cat.matches(&t.view().category))
                            .count();
                        let out = tree_row(
                            ui,
                            "📁",
                            cat.label(),
                            Some(n),
                            self.side == SideSel::Cat(cat),
                            18.0,
                            C_FOLDER,
                            None,
                        );
                        if out.resp.clicked() {
                            self.side = SideSel::Cat(cat);
                        }
                    }
                }

                ui.add_space(8.0);
                for (icon, label, n, sel) in [
                    ("📁", "Unfinished", Some(unfinished), SideSel::Unfinished),
                    ("📁", "Finished", Some(finished), SideSel::Finished),
                    ("📁", "Grabber projects", None, SideSel::Grabber),
                    ("📁", "Queues", Some(queued), SideSel::Queues),
                ] {
                    let out = tree_row(ui, icon, label, n, self.side == sel, 0.0, C_FOLDER, None);
                    if out.resp.clicked() {
                        self.side = sel;
                    }
                }
            });
    }

    fn table(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // ---- details of the selected download (IDM-style: inside the right area, bottom)
            if let Some(t) = self.selected_task() {
                let v = t.view();
                egui::TopBottomPanel::bottom("details_inner")
                    .exact_height(80.0)
                    .show_inside(ui, |ui| {
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&v.filename).strong());
                                ui.label(format!(
                                    "Size: {}   Downloaded: {}   Speed: {}   Time left: {}",
                                    human_size(v.size),
                                    human_size(Some(v.done)),
                                    human_speed(v.speed),
                                    human_eta(v.eta)
                                ));
                                ui.label(format!(
                                    "Connections: {}{}   Segments: {}   Status: {}",
                                    v.connections,
                                    if v.single_stream { " (single)" } else { "" },
                                    v.segs.len().max(1),
                                    v.status.label()
                                ));
                                if let Some(p) = &v.path {
                                    ui.label(
                                        egui::RichText::new(p.display().to_string()).small().weak(),
                                    );
                                }
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                paint_speed_graph(
                                    ui,
                                    &self.speed_hist,
                                    egui::vec2(260.0, 44.0),
                                    v.speed,
                                );
                                ui.add_space(2.0);
                                paint_segments(ui, &v.segs, v.status, 260.0, 10.0);
                            });
                        });
                        ui.add_space(2.0);
                    });
            }

            // ---- list
            let mut snapshot: Vec<(usize, Arc<DownloadTask>)> = self
                .mgr
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    let v = t.view();
                    side_matches(self.side, &v.category, v.status)
                })
                .map(|(i, t)| (i, t.clone()))
                .collect();

            if let Some((col, asc)) = self.sort {
                snapshot.sort_by(|a, b| {
                    let va = a.1.view();
                    let vb = b.1.view();
                    let ord = match col {
                        0 => va.filename.to_lowercase().cmp(&vb.filename.to_lowercase()),
                        1 => {
                            let ka = if matches!(va.status, Status::Queued) {
                                a.0
                            } else {
                                usize::MAX
                            };
                            let kb = if matches!(vb.status, Status::Queued) {
                                b.0
                            } else {
                                usize::MAX
                            };
                            ka.cmp(&kb)
                        }
                        2 => va.size.unwrap_or(0).cmp(&vb.size.unwrap_or(0)),
                        3 => va.status.label().cmp(vb.status.label()),
                        4 => va
                            .progress
                            .partial_cmp(&vb.progress)
                            .unwrap_or(std::cmp::Ordering::Equal),
                        5 => va
                            .eta
                            .unwrap_or(f64::MAX)
                            .partial_cmp(&vb.eta.unwrap_or(f64::MAX))
                            .unwrap_or(std::cmp::Ordering::Equal),
                        6 => va
                            .speed
                            .partial_cmp(&vb.speed)
                            .unwrap_or(std::cmp::Ordering::Equal),
                        7 => va.last_try.unwrap_or(0).cmp(&vb.last_try.unwrap_or(0)),
                        _ => va.url.cmp(&vb.url),
                    };
                    if asc {
                        ord
                    } else {
                        ord.reverse()
                    }
                });
            }

            let mut qseq = 0usize;
            let qmap: Vec<Option<usize>> = self
                .mgr
                .tasks
                .iter()
                .map(|t| {
                    if *t.status.lock().unwrap() == Status::Queued {
                        qseq += 1;
                        Some(qseq)
                    } else {
                        None
                    }
                })
                .collect();

            if snapshot.is_empty() {
                ui.horizontal(|ui| {
                    let cols: [(&str, f32); 9] = [
                        ("File Name", 200.0),
                        ("Q", 30.0),
                        ("Size", 75.0),
                        ("Status", 95.0),
                        ("Downloaded", 165.0),
                        ("Time left", 70.0),
                        ("Transfer rate", 95.0),
                        ("Last Try", 80.0),
                        ("Description", 0.0),
                    ];
                    for (name, w) in cols {
                        let lbl = egui::Label::new(egui::RichText::new(name).strong());
                        if w > 0.0 {
                            ui.add_sized([w, 20.0], lbl);
                        } else {
                            ui.add(lbl);
                        }
                    }
                });
                ui.separator();
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "No downloads yet. Click \"➕ Add URL…\" or enable the clipboard monitor in Options.",
                        )
                        .weak(),
                    );
                });
                return;
            }

            enum RowAct {
                OpenFile,
                Resume,
                Pause,
                Remove,
                OpenFolder,
                CopyUrl,
            }
            let mut act: Option<(RowAct, usize)> = None;
            let mut hdr_click: Option<usize> = None;

            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(200.0).at_least(120.0).clip(true))
                .column(Column::initial(30.0).at_least(24.0).clip(true))
                .column(Column::initial(75.0).at_least(56.0).clip(true))
                .column(Column::initial(95.0).at_least(80.0).clip(true))
                .column(Column::initial(165.0).at_least(120.0).clip(true))
                .column(Column::initial(70.0).at_least(58.0).clip(true))
                .column(Column::initial(95.0).at_least(70.0).clip(true))
                .column(Column::initial(80.0).at_least(64.0).clip(true))
                .column(Column::remainder().at_least(100.0).clip(true))
                .min_scrolled_height(0.0)
                .sense(egui::Sense::click())
                .header(22.0, |mut header| {
                    let labels = [
                        "File Name",
                        "Q",
                        "Size",
                        "Status",
                        "Downloaded",
                        "Time left",
                        "Transfer rate",
                        "Last Try",
                        "Description",
                    ];
                    for (ci, name) in labels.iter().enumerate() {
                        header.col(|ui| {
                            let arrow = match self.sort {
                                Some((c, asc)) if c == ci => {
                                    if asc {
                                        " ↑"
                                    } else {
                                        " ↓"
                                    }
                                }
                                _ => "",
                            };
                            let r = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}{}", name, arrow))
                                        .strong()
                                        .size(12.5),
                                )
                                .frame(false),
                            );
                            if r.clicked() {
                                hdr_click = Some(ci);
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(24.0, snapshot.len(), |mut row| {
                        let idx = row.index();
                        let (i, task) = &snapshot[idx];
                        let i = *i;
                        let v = task.view();
                        row.set_selected(self.selected == Some(i));

                        row.col(|ui| {
                            ui.add(egui::Label::new(egui::RichText::new(&v.filename)).truncate());
                        });
                        row.col(|ui| {
                            let qs = qmap
                                .get(i)
                                .and_then(|q| *q)
                                .map(|n| n.to_string())
                                .unwrap_or_default();
                            ui.add(egui::Label::new(egui::RichText::new(qs).small()).truncate());
                        });
                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(human_size(v.size)))
                                    .truncate(),
                            );
                        });
                        row.col(|ui| {
                            let st = if v.status == Status::Error && !v.error.is_empty() {
                                format!("⚠ {}", v.status.label())
                            } else {
                                v.status.label().to_string()
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(st).color(status_color(v.status)),
                                )
                                .truncate(),
                            );
                        });
                        row.col(|ui| {
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "{:.1}%  {}/{}",
                                            v.progress * 100.0,
                                            human_size(Some(v.done)),
                                            human_size(v.size)
                                        ))
                                        .small(),
                                    )
                                    .truncate(),
                                );
                                paint_segments(
                                    ui,
                                    &v.segs,
                                    v.status,
                                    (ui.available_width() - 6.0).max(24.0),
                                    6.0,
                                );
                            });
                        });
                        row.col(|ui| {
                            let s = if v.status == Status::Downloading {
                                human_eta(v.eta)
                            } else {
                                String::new()
                            };
                            ui.add(egui::Label::new(egui::RichText::new(s)).truncate());
                        });
                        row.col(|ui| {
                            let s = if v.status == Status::Downloading {
                                human_speed(v.speed)
                            } else {
                                String::new()
                            };
                            ui.add(egui::Label::new(egui::RichText::new(s)).truncate());
                        });
                        row.col(|ui| {
                            let s = v.last_try.map(fmt_ago).unwrap_or_default();
                            ui.add(egui::Label::new(egui::RichText::new(s)).truncate());
                        });
                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(&v.url).small().weak())
                                    .truncate(),
                            );
                        });

                        let resp = row.response();
                        if resp.clicked() {
                            if self.selected != Some(i) {
                                self.speed_hist.clear();
                            }
                            self.selected = Some(i);
                        }
                        if resp.double_clicked() && v.status == Status::Completed {
                            act = Some((RowAct::OpenFile, i));
                        }
                        resp.context_menu(|ui| {
                            if ui.button("▶  Start / Resume").clicked() {
                                act = Some((RowAct::Resume, i));
                                ui.close_menu();
                            }
                            if ui.button("⏸  Stop").clicked() {
                                act = Some((RowAct::Pause, i));
                                ui.close_menu();
                            }
                            if v.status == Status::Completed
                                && ui.button("▶  Open File").clicked()
                            {
                                act = Some((RowAct::OpenFile, i));
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("📂  Open Folder").clicked() {
                                act = Some((RowAct::OpenFolder, i));
                                ui.close_menu();
                            }
                            if ui.button("🔗  Copy URL").clicked() {
                                act = Some((RowAct::CopyUrl, i));
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🗑  Remove").clicked() {
                                act = Some((RowAct::Remove, i));
                                ui.close_menu();
                            }
                        });
                    });
                });

            if let Some(ci) = hdr_click {
                self.sort = match self.sort {
                    Some((c, asc)) if c == ci => Some((ci, !asc)),
                    _ => Some((ci, true)),
                };
            }

            if let Some((a, i)) = act {
                match a {
                    RowAct::OpenFile => {
                        if let Some(t) = self.mgr.tasks.get(i) {
                            let v = t.view();
                            if v.status == Status::Completed {
                                if let Some(p) = v.path {
                                    self.open_path(&p, true);
                                }
                            }
                        }
                    }
                    RowAct::Resume => {
                        if let Some(t) = self.mgr.tasks.get(i) {
                            let st = *t.status.lock().unwrap();
                            if st == Status::Paused {
                                t.resume();
                            } else if matches!(
                                st,
                                Status::Queued | Status::Error | Status::Cancelled
                            ) {
                                t.start(false);
                            }
                        }
                    }
                    RowAct::Pause => {
                        if let Some(t) = self.mgr.tasks.get(i) {
                            t.pause();
                        }
                    }
                    RowAct::Remove => {
                        self.mgr.remove(i);
                        self.selected = None;
                        self.speed_hist.clear();
                    }
                    RowAct::OpenFolder => {
                        if let Some(t) = self.mgr.tasks.get(i) {
                            let v = t.view();
                            if let Some(p) = &v.path {
                                if let Some(dir) = p.parent() {
                                    let dir = dir.to_path_buf();
                                    self.open_path(&dir, false);
                                }
                            }
                        }
                    }
                    RowAct::CopyUrl => {
                        if let Some(t) = self.mgr.tasks.get(i) {
                            let v = t.view();
                            if let Some(c) = &mut self.clip {
                                let _ = c.set_text(v.url.clone());
                                self.log("URL copied to clipboard.");
                            }
                        }
                    }
                }
            }
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let mut total_speed = 0.0f64;
                let mut active = 0usize;
                let mut done_total = 0u64;
                for t in &self.mgr.tasks {
                    let v = t.view();
                    done_total += v.done;
                    if v.status == Status::Downloading {
                        total_speed += v.speed;
                        active += 1;
                    }
                }
                ui.label(format!("Total: {}", human_speed(total_speed)));
                ui.separator();
                ui.label(format!("Active: {} / {}", active, self.mgr.tasks.len()));
                ui.separator();
                ui.label(format!(
                    "Queue: {}",
                    if self.mgr.queue_running { "ON" } else { "off" }
                ));
                ui.separator();
                ui.checkbox(&mut self.show_log, "Log");
                ui.separator();
                ui.label("Speed limiter:");
                let cur = self.mgr.cfg.speed_preset;
                let names = ["Unlimited", "128 KB/s", "512 KB/s", "2 MB/s", "Custom…"];
                let mut sel = cur;
                egui::ComboBox::from_id_salt("speedlim_statusbar")
                    .selected_text(names[cur.min(4)])
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut sel, i, *n);
                        }
                    });
                if sel != cur {
                    self.mgr.cfg.speed_preset = sel;
                    self.mgr.save_cfg();
                    let l = self.mgr.cfg.limit_label();
                    self.log(format!("Speed limit: {}", l));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    paint_speed_graph(ui, &self.total_hist, egui::vec2(180.0, 16.0), total_speed);
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "Downloaded: {}",
                            human_size(Some(done_total))
                        ))
                        .small()
                        .weak(),
                    );
                });
                let busy = self
                    .mgr
                    .tasks
                    .iter()
                    .any(|t| *t.status.lock().unwrap() == Status::Downloading);
                if busy {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
            });
            if self.show_log {
                let h = if self.log.len() > 6 { 110.0 } else { 70.0 };
                egui::ScrollArea::vertical()
                    .max_height(h)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.log {
                            ui.label(egui::RichText::new(line).monospace().small());
                        }
                    });
            }
            ui.add_space(2.0);
        });
    }

    fn add_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_add;
        egui::Window::new("Add Download")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                egui::Grid::new("add_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_url)
                                .desired_width(420.0)
                                .hint_text("https://example.com/file.zip"),
                        );
                        ui.end_row();

                        ui.label("Save to:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.add_dir)
                                    .desired_width(330.0),
                            );
                            if ui.button("Browse…").clicked() {
                                if let Some(d) = rfd::FileDialog::new()
                                    .set_directory(&self.add_dir)
                                    .pick_folder()
                                {
                                    self.add_dir = d.display().to_string();
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("File name:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_name)
                                .desired_width(300.0)
                                .hint_text("(empty = from server/URL)"),
                        );
                        ui.end_row();

                        ui.label("Connections:");
                        ui.add(egui::DragValue::new(&mut self.add_conn).range(1..=32));
                        ui.end_row();

                        ui.label("Referer:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_referer)
                                .desired_width(300.0)
                                .hint_text("(optional)"),
                        );
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let url = self.add_url.trim().to_string();
                    let valid = url.starts_with("http://") || url.starts_with("https://");
                    if ui
                        .add_enabled(valid, egui::Button::new("⬇ Download Now"))
                        .clicked()
                    {
                        let conn = self.add_conn.max(1) as usize;
                        let fname = if self.add_name.trim().is_empty() {
                            None
                        } else {
                            Some(self.add_name.trim().to_string())
                        };
                        let refr = if self.add_referer.trim().is_empty() {
                            None
                        } else {
                            Some(self.add_referer.trim().to_string())
                        };
                        let t = self.mgr.add_task(
                            &url,
                            self.add_dir.trim(),
                            fname,
                            conn,
                            refr,
                            None,
                        );
                        self.log(format!("Added: {}", url));
                        t.start(false);
                        self.show_add = false;
                    }
                    if ui
                        .add_enabled(valid, egui::Button::new("Add to Queue"))
                        .clicked()
                    {
                        let conn = self.add_conn.max(1) as usize;
                        let fname = if self.add_name.trim().is_empty() {
                            None
                        } else {
                            Some(self.add_name.trim().to_string())
                        };
                        let refr = if self.add_referer.trim().is_empty() {
                            None
                        } else {
                            Some(self.add_referer.trim().to_string())
                        };
                        let t = self.mgr.add_task(
                            &url,
                            self.add_dir.trim(),
                            fname,
                            conn,
                            refr,
                            None,
                        );
                        self.log(format!("Queued: {}", url));
                        let _ = t;
                        self.show_add = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_add = false;
                    }
                    if !valid && !self.add_url.trim().is_empty() {
                        ui.label(
                            egui::RichText::new("URL must start with http:// or https://")
                                .color(status_color(Status::Error)),
                        );
                    }
                });
            });
        self.show_add = open;
    }

    fn options_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_options;
        egui::Window::new("Options")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                egui::Grid::new("opt_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Default folder:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.mgr.cfg.default_dir)
                                    .desired_width(300.0),
                            );
                            if ui.button("Browse…").clicked() {
                                if let Some(d) = rfd::FileDialog::new()
                                    .set_directory(&self.mgr.cfg.default_dir)
                                    .pick_folder()
                                {
                                    self.mgr.cfg.default_dir = d.display().to_string();
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Default connections:");
                        ui.add(
                            egui::DragValue::new(&mut self.mgr.cfg.connections)
                                .range(1..=32),
                        );
                        ui.end_row();

                        ui.label("Retries per segment:");
                        ui.add(egui::DragValue::new(&mut self.mgr.cfg.retries).range(1..=100));
                        ui.end_row();

                        ui.label("User-Agent:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.mgr.cfg.ua)
                                .desired_width(360.0),
                        );
                        ui.end_row();

                        ui.label("Referer (default):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.mgr.cfg.referer)
                                .desired_width(300.0),
                        );
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.mgr.cfg.monitor_clipboard,
                    "Monitor clipboard for URLs (IDM-style)",
                );
                ui.checkbox(
                    &mut self.mgr.cfg.auto_category,
                    "Auto-sort into category subfolders (Video/Music/…)",
                );
                let mut classic = self.mgr.cfg.classic_theme;
                if ui
                    .checkbox(&mut classic, "Classic light theme (IDM-style)")
                    .changed()
                {
                    self.mgr.cfg.classic_theme = classic;
                    apply_theme(ctx, classic);
                    self.mgr.save_cfg();
                }
                ui.add_space(6.0);
                ui.separator();
                ui.label(egui::RichText::new("Browser grabber (OpenDown extension)").strong());
                ui.checkbox(
                    &mut self.mgr.cfg.grab_enabled,
                    "Enable grabber listener (restart to apply)",
                );
                ui.horizontal(|ui| {
                    ui.label("Port:");
                    ui.add(egui::DragValue::new(&mut self.mgr.cfg.grab_port).range(1024..=65535));
                });
                ui.checkbox(
                    &mut self.mgr.cfg.grab_auto_start,
                    "Auto-start grabbed downloads",
                );
                if self.grab_rx.is_some() {
                    ui.label(format!(
                        "Status: listening on http://127.0.0.1:{}",
                        self.mgr.cfg.grab_port
                    ));
                } else {
                    ui.label("Status: not listening");
                }
                ui.add_space(6.0);
                ui.separator();
                ui.label(egui::RichText::new("System").strong());
                ui.checkbox(
                    &mut self.mgr.cfg.minimize_to_tray,
                    "Close / minimize to the system tray (keeps downloads running)",
                );
                let mut autostart = autostart_enabled();
                if ui
                    .checkbox(&mut autostart, "Start with Windows (minimized to tray)")
                    .changed()
                {
                    match set_autostart(autostart) {
                        Ok(()) => {
                            if autostart {
                                self.log("Autostart enabled (OpenDown will start minimized).");
                            } else {
                                self.log("Autostart disabled.");
                            }
                        }
                        Err(e) => self.log(format!("Autostart change failed: {}", e)),
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.mgr.save_cfg();
                        self.mgr.save_tasks();
                        self.log("Options saved.");
                        self.show_options = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_options = false;
                    }
                });
            });
        self.show_options = open;
    }
}

impl eframe::App for TurboApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // smoke auto-close for automated testing
        if let Some((start, ms)) = self.smoke {
            if start.elapsed().as_millis() as u64 >= ms {
                self.exit_now = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // system tray events (show / exit)
        while let Ok(msg) = self.tray_rx.try_recv() {
            match msg {
                TrayMsg::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                }
                TrayMsg::Exit => {
                    self.exit_now = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        // start-at-boot launches hidden ("--minimized")
        if self.start_minimized {
            self.start_minimized = false;
            if self.smoke.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        // close / minimize to tray
        if ctx.input(|i| i.viewport().close_requested())
            && !self.exit_now
            && self.smoke.is_none()
            && self.tray.is_some()
            && self.mgr.cfg.minimize_to_tray
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if ctx.input(|i| i.viewport().minimized) == Some(true)
            && !self.exit_now
            && self.smoke.is_none()
            && self.tray.is_some()
            && self.mgr.cfg.minimize_to_tray
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.poll_clipboard();
        self.mgr.tick_queue();
        self.track_transitions();
        let mut jobs: Vec<GrabJob> = Vec::new();
        if let Some(rx) = &self.grab_rx {
            while let Ok(job) = rx.try_recv() {
                jobs.push(job);
            }
        }
        for job in jobs {
            self.add_grab(job);
        }
        if self.last_hist.elapsed() > Duration::from_millis(500) {
            self.last_hist = Instant::now();
            if let Some(t) = self.selected_task() {
                let s = t.view().speed as f32;
                self.speed_hist.push(s);
                if self.speed_hist.len() > 150 {
                    self.speed_hist.remove(0);
                }
            } else if !self.speed_hist.is_empty() {
                self.speed_hist.clear();
            }
            let mut tot = 0.0f32;
            for t in &self.mgr.tasks {
                let v = t.view();
                if v.status == Status::Downloading {
                    tot += v.speed as f32;
                }
            }
            self.total_hist.push(tot);
            if self.total_hist.len() > 150 {
                self.total_hist.remove(0);
            }
        }
        if self.last_save.elapsed() > Duration::from_secs(5) {
            self.last_save = Instant::now();
            self.mgr.save_tasks();
        }

        self.menubar(ctx);
        self.toolbar(ctx);
        self.status_bar(ctx);
        self.categories(ctx);
        self.table(ctx);

        if self.show_add {
            self.add_window(ctx);
        }
        if self.show_options {
            self.options_window(ctx);
        }

        ctx.request_repaint_after(Duration::from_millis(300));
    }
}

// ===========================================================================
// CLI mode
// ===========================================================================

fn run_cli(args: &[String]) -> i32 {
    attach_console();
    let mut url: Option<String> = None;
    let mut out = String::new();
    let mut filename: Option<String> = None;
    let mut conns: usize = DEFAULT_CONNECTIONS;
    let mut limit_kbps: Option<f64> = None;
    let mut resume = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                i += 1;
                if i < args.len() {
                    out = args[i].clone();
                }
            }
            "-f" | "--name" => {
                i += 1;
                if i < args.len() {
                    filename = Some(args[i].clone());
                }
            }
            "-n" | "--connections" => {
                i += 1;
                if i < args.len() {
                    conns = args[i].parse().unwrap_or(DEFAULT_CONNECTIONS);
                }
            }
            "--limit" => {
                i += 1;
                if i < args.len() {
                    limit_kbps = args[i].parse::<f64>().ok();
                }
            }
            "--resume" => resume = true,
            _ => {
                if url.is_none() && !args[i].starts_with('-') {
                    url = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let url = match url {
        Some(u) => u,
        None => {
            eprintln!("usage: opendown --cli <URL> [-o DIR] [-f NAME] [-n CONNS] [--limit KB] [--resume]");
            return 2;
        }
    };
    if out.is_empty() {
        out = std::env::var("USERPROFILE")
            .map(|h| format!("{}\\Downloads", h))
            .unwrap_or_else(|_| ".".to_string());
    }
    let limiter = Arc::new(SpeedLimiter::new(limit_kbps.map(|k| k * 1024.0)));
    let task = Arc::new(DownloadTask::new(
        &url,
        PathBuf::from(&out),
        filename,
        conns,
        DEFAULT_UA,
        None,
        None,
        limiter,
        12,
        false,
    ));

    println!("OpenDown CLI — {}", url);
    println!("probing...");
    match task.ensure_probed() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("ERROR: {}", e);
            return 2;
        }
    }
    {
        let v = task.view();
        println!("  file : {}", v.filename);
        println!("  size : {}", human_size(v.size));
        println!(
            "  mode : {}",
            if v.single_stream {
                "single stream".to_string()
            } else {
                format!("segmented, {} conns", v.connections)
            }
        );
        if let Some(p) = &v.path {
            println!("  save : {}", p.display());
        }
    }
    task.start(resume);
    let test_pause = std::env::var("OPENDOWN_TEST_PAUSE").is_ok();
    let mut paused_once = false;

    let mut last_line = String::new();
    loop {
        let v = task.view();
        if test_pause {
            if !paused_once && v.status == Status::Downloading && v.progress > 0.3 {
                task.pause();
                paused_once = true;
            } else if paused_once && v.status == Status::Paused {
                println!("  [test] paused at {:.1}% -> resuming", v.progress * 100.0);
                std::thread::sleep(Duration::from_millis(600));
                task.resume();
                continue;
            }
        }
        let line = format!(
            "  {:>5.1}%  {}/{}  {}  ETA {}   ",
            v.progress * 100.0,
            human_size(Some(v.done)),
            human_size(v.size),
            human_speed(v.speed),
            human_eta(v.eta)
        );
        if line != last_line {
            print!("\r{}", line);
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
            last_line = line;
        }
        match v.status {
            Status::Completed => {
                println!();
                println!("  ✔ done: {}", v.path.map(|p| p.display().to_string()).unwrap_or_default());
                return 0;
            }
            Status::Paused => {
                println!();
                println!("  ⏸ paused (use --resume to continue)");
                return 3;
            }
            Status::Error => {
                println!();
                println!("  ✖ error: {}", v.error);
                return 1;
            }
            Status::Cancelled => {
                println!();
                return 1;
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

// ===========================================================================
// main
// ===========================================================================

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--cli" {
        std::process::exit(run_cli(&args[2..]));
    }
    if args.iter().any(|a| a == "--autostart-on") {
        attach_console();
        match set_autostart(true) {
            Ok(()) => {
                println!("autostart: enabled");
                return Ok(());
            }
            Err(e) => {
                eprintln!("autostart: {}", e);
                std::process::exit(1);
            }
        }
    }
    if args.iter().any(|a| a == "--autostart-off") {
        attach_console();
        match set_autostart(false) {
            Ok(()) => {
                println!("autostart: disabled");
                return Ok(());
            }
            Err(e) => {
                eprintln!("autostart: {}", e);
                std::process::exit(1);
            }
        }
    }

    eprintln!("[gui] starting eframe...");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 680.0])
            .with_min_inner_size([900.0, 480.0])
            .with_title("OpenDown — Download Manager")
            .with_icon(std::sync::Arc::new(make_icon())),
        ..Default::default()
    };
    let res = eframe::run_native(
        "OpenDown — Download Manager",
        options,
        Box::new(|cc| Ok(Box::new(TurboApp::new(cc)))),
    );
    match &res {
        Ok(()) => eprintln!("[gui] run_native returned Ok"),
        Err(e) => eprintln!("[gui] run_native returned Err: {e}"),
    }
    res
}

// keep planner symbol referenced for potential future UI (segment preview)
#[allow(dead_code)]
fn _plan_preview(size: u64, conns: usize) -> usize {
    plan_for(size, conns).len()
}
