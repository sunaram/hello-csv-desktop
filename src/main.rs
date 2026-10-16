#![cfg_attr(windows, windows_subsystem = "windows")]

mod csv;

use csv::{json_row, json_rows, json_str};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tao::event::{Event as Ev, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::http::{Response, header::CONTENT_TYPE};
use wry::{DragDropEvent, WebViewBuilder};

const CSS: &str = include_str!("../assets/style.css");
const BODY: &str = include_str!("../assets/body.html");
const JS: &str = include_str!("../assets/app.js");

#[cfg(windows)]
const ORIGIN: &str = "http://hc.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "hc://localhost";

const APP: &str = "Hello CSV";
const SAMPLE_ROWS: usize = 50;

enum User {
    Open(PathBuf),
    Cmd(String),
}

/// What is known about the open file; rebuilt by every `/__open` request.
struct Doc {
    path: PathBuf,
    delim: u8,
    hdr: bool,
    crlf: bool,
    bom: bool,
    ncols: usize,
    idx: Arc<csv::Index>,
}

#[derive(Default)]
struct State {
    file: Option<PathBuf>,
    /// "" = follow the OS, otherwise "light" / "dark"
    theme: &'static str,
    doc: Option<Doc>,
    dirty: bool,
    /// check GitHub for a newer release (never for Microsoft Store installs)
    updates: bool,
}

type Shared = Arc<Mutex<State>>;

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                o.push(v);
                i += 3;
                continue;
            }
        }
        o.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

fn param(q: &str, key: &str) -> String {
    q.split('&').find_map(|kv| kv.strip_prefix(key).and_then(|r| r.strip_prefix('='))).map(unescape).unwrap_or_default()
}

/// Store (MSIX) installs live under WindowsApps and are updated by the Store, so they never check themselves.
fn is_store() -> bool {
    cfg!(windows) && std::env::current_exe().is_ok_and(|p| p.to_string_lossy().to_ascii_lowercase().contains("\\windowsapps\\"))
}

fn canon(p: PathBuf) -> PathBuf {
    let c = p.canonicalize().unwrap_or(p);
    match c.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => c,
    }
}

fn open_external(url: &str) {
    use std::process::Command;
    #[cfg(windows)]
    let _ = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

fn err_json(msg: &str) -> String {
    let mut o = String::from("{\"err\":");
    json_str(msg, &mut o);
    o.push('}');
    o
}

fn page(theme: &str, updates: bool) -> String {
    let upd = if is_store() { String::new() } else { format!("window.__upd={{v:\"{}\",auto:{updates}}};", env!("CARGO_PKG_VERSION")) };
    format!(
        "<!doctype html><html data-theme=\"{theme}\"><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">\
<title>{APP}</title><base href=\"{ORIGIN}/\"><style>{CSS}</style>{BODY}<script>{upd}{JS}</script>"
    )
}

/// `/__open`: sniff the file, send either everything (small files) or the first rows (streaming mode).
fn api_open(st: &Shared, epoch: &Arc<AtomicU64>, q: &str) -> String {
    let mut s = st.lock().unwrap();
    let Some(path) = s.file.clone() else { return err_json("No file open") };
    let hdr = param(q, "hdr") != "0";
    let darg = param(q, "delim");
    let size = match std::fs::metadata(&path) {
        Ok(m) => m.len(),
        Err(e) => return err_json(&format!("Cannot read {}: {e}", path.display())),
    };
    let stream = size > csv::STREAM_OVER;
    let g = epoch.fetch_add(1, Ordering::Relaxed) + 1;
    let idx = csv::Index::new(g);
    let (delim, crlf, bom, rows) = if stream {
        let head = match csv::read_head(&path, 256 * 1024) {
            Ok(h) => h,
            Err(e) => return err_json(&format!("Cannot read {}: {e}", path.display())),
        };
        let (text, bom) = csv::strip_bom(&head);
        let d = if darg.len() == 1 { darg.as_bytes()[0] } else { csv::detect_delim(text) };
        let rows = csv::parse_all(text, d, SAMPLE_ROWS + 1);
        csv::spawn_index(path.clone(), d, idx.clone(), epoch.clone());
        (d, head.windows(2).any(|w| w == b"\r\n"), bom, rows)
    } else {
        let data = match std::fs::read(&path) {
            Ok(h) => h,
            Err(e) => return err_json(&format!("Cannot read {}: {e}", path.display())),
        };
        let (text, bom) = csv::strip_bom(&data);
        let d = if darg.len() == 1 { darg.as_bytes()[0] } else { csv::detect_delim(text) };
        (d, text.windows(2).any(|w| w == b"\r\n"), bom, csv::parse_all(text, d, usize::MAX))
    };
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut rows = rows.into_iter();
    let headers: Vec<String> = if hdr {
        let h = rows.next().unwrap_or_default();
        (0..ncols).map(|i| h.get(i).map(|x| x.trim()).filter(|x| !x.is_empty()).map_or(format!("Column {}", i + 1), String::from)).collect()
    } else {
        (0..ncols).map(|i| format!("Column {}", i + 1)).collect()
    };
    let mut data: Vec<Vec<String>> = rows.take(if stream { SAMPLE_ROWS } else { usize::MAX }).collect();
    for r in &mut data {
        r.resize(ncols, String::new());
    }
    let total = data.len();
    let mut o = String::with_capacity(size as usize * 2 + 256);
    o.push_str("{\"name\":");
    json_str(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), &mut o);
    o.push_str(&format!(",\"size\":{size},\"stream\":{stream},\"hdr\":{hdr},\"cols\":{ncols},\"total\":{total},\"delim\":"));
    json_str(&(delim as char).to_string(), &mut o);
    o.push_str(",\"headers\":");
    json_row(&headers, &mut o);
    o.push_str(",\"rows\":");
    json_rows(&data, &mut o);
    o.push('}');
    s.doc = Some(Doc { path, delim, hdr, crlf, bom, ncols, idx });
    o
}

fn api(st: &Shared, epoch: &Arc<AtomicU64>, path: &str, q: &str) -> String {
    if path == "/__open" {
        return api_open(st, epoch, q);
    }
    let s = st.lock().unwrap();
    let Some(d) = s.doc.as_ref() else { return err_json("No file open") };
    let num = |k: &str, def: u64| param(q, k).parse::<u64>().unwrap_or(def);
    match path {
        "/__status" => {
            let n = d.idx.count.load(Ordering::Relaxed).saturating_sub(d.hdr as u64);
            format!("{{\"total\":{n},\"done\":{}}}", d.idx.done.load(Ordering::Acquire))
        }
        "/__page" => match csv::page(&d.path, &d.idx, d.delim, d.hdr, d.ncols, num("p", 1), num("n", 50).clamp(1, 10_000)) {
            Ok(rows) => {
                let mut o = String::from("{\"rows\":");
                json_rows(&rows, &mut o);
                o.push_str(&format!(",\"total\":{}}}", d.idx.count.load(Ordering::Relaxed).saturating_sub(d.hdr as u64)));
                o
            }
            Err(e) => err_json(&e.to_string()),
        },
        "/__search" => match csv::search(&d.path, d.delim, d.hdr, &param(q, "q"), num("off", 0), num("row", 0), 200) {
            Ok(r) => {
                let size = std::fs::metadata(&d.path).map_or(1, |m| m.len().max(1));
                let mut o = String::from("{\"m\":[");
                for (i, (row, cells)) in r.matches.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    o.push_str(&format!("[{row},"));
                    json_row(cells, &mut o);
                    o.push(']');
                }
                o.push_str(&format!("],\"off\":{},\"row\":{},\"done\":{},\"ratio\":{:.4}}}", r.off, r.row, r.done, r.off as f64 / size as f64));
                o
            }
            Err(e) => err_json(&e.to_string()),
        },
        _ => err_json("unknown request"),
    }
}

/// Write the file back. `msg` is `save<RS><csv text>` (small files) or
/// `delta<RS><header or empty><RS><row>{1}<record><RS>...` (RS = U+001E: a NUL would truncate the IPC string)
fn save(st: &Shared, msg: &str) -> Result<(), String> {
    let s = st.lock().unwrap();
    let d = s.doc.as_ref().ok_or("No file open")?;
    let mut parts = msg.split('\u{1e}');
    match parts.next() {
        Some("save") => {
            let text = parts.next().unwrap_or("");
            let mut out = Vec::with_capacity(text.len() + 3);
            if d.bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            out.extend_from_slice(if d.crlf { text.replace('\n', "\r\n") } else { text.to_string() }.as_bytes());
            let tmp = PathBuf::from(format!("{}.hcsv.tmp", d.path.display()));
            std::fs::write(&tmp, out).and_then(|_| std::fs::rename(&tmp, &d.path)).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                e.to_string()
            })
        }
        Some("delta") => {
            let header = parts.next().filter(|h| !h.is_empty());
            let mods: HashMap<u64, String> = parts
                .filter_map(|p| p.split_once('\u{1}'))
                .filter_map(|(i, rec)| Some((i.parse().ok()?, rec.to_string())))
                .collect();
            csv::save_delta(&d.path, d.delim, d.hdr, header, &mods, d.crlf).map_err(|e| e.to_string())
        }
        _ => Err("bad request".into()),
    }
}

const MAX_RECENT: usize = 10;

struct Settings {
    theme: &'static str,
    zoom: f64,
    recent: Vec<PathBuf>,
    updates: bool,
}

fn settings_path() -> Option<PathBuf> {
    use std::env::var_os;
    #[cfg(windows)]
    let base = var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.map(|b| b.join("hellocsv").join("settings.txt"))
}

fn load_settings() -> Settings {
    let mut s = Settings { theme: "", zoom: 1.0, recent: vec![], updates: true };
    let Some(text) = settings_path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
    for line in text.lines() {
        match line.split_once('=') {
            Some(("theme", "light")) => s.theme = "light",
            Some(("theme", "dark")) => s.theme = "dark",
            Some(("updates", v)) => s.updates = v != "off",
            Some(("zoom", v)) => s.zoom = v.parse().map_or(1.0, |z: f64| z.clamp(0.3, 5.0)),
            Some(("recent", p)) if s.recent.len() < MAX_RECENT => s.recent.push(PathBuf::from(p)),
            _ => {}
        }
    }
    s
}

fn save_settings(s: &Settings) {
    let Some(path) = settings_path() else { return };
    let mut out = format!("theme={}\nzoom={}\nupdates={}\n", s.theme, s.zoom, if s.updates { "on" } else { "off" });
    for p in &s.recent {
        out.push_str(&format!("recent={}\n", p.display()));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, out);
}

/// Rebuild the "Open Recent" submenu from `recent`.
fn fill_recent(sub: &muda::Submenu, old: &mut Vec<muda::MenuItem>, recent: &[PathBuf]) {
    use muda::MenuItem;
    for i in old.drain(..) {
        let _ = sub.remove(&i);
    }
    let mut add = |id: String, label: String, enabled: bool| {
        let m = MenuItem::with_id(id, label, enabled, None);
        let _ = sub.append(&m);
        old.push(m);
    };
    if recent.is_empty() {
        add("none".into(), "(empty)".into(), false);
        return;
    }
    for (i, p) in recent.iter().enumerate() {
        add(format!("recent:{i}"), p.to_string_lossy().replace('&', "&&"), true);
    }
    add("recent_clear".into(), "Clear Recent".into(), true);
}

fn build_menu(updates: bool) -> (muda::Menu, muda::Submenu) {
    use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem as P, Submenu};
    // macOS gets real accelerators; elsewhere shortcuts are handled in page JS
    // (the focused webview swallows menu accelerators) and shown as a label hint.
    let it = |id: &str, label: &str, key: &str| -> MenuItem {
        #[cfg(target_os = "macos")]
        {
            MenuItem::with_id(id, label, true, format!("CmdOrCtrl+{key}").parse().ok())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let l = if key.is_empty() { label.to_string() } else { format!("{label}\tCtrl+{key}") };
            MenuItem::with_id(id, l, true, None)
        }
    };
    let sep = P::separator();
    let recent = Submenu::new("Open &Recent", true);
    let file = Submenu::with_items(
        "&File",
        true,
        &[
            &it("open", "&Open…", "O"),
            &recent,
            &it("save", "&Save", "S"),
            &it("reload", "&Reload from Disk", "R"),
            &it("editor", "Open in Default &App", "E"),
            &sep,
            &it("quit", "&Quit", "W"),
        ],
    )
    .unwrap();
    let view = Submenu::with_items(
        "&View",
        true,
        &[
            &it("find", "&Search…", "F"),
            &sep,
            &it("zin", "Zoom &In", "="),
            &it("zout", "Zoom &Out", "-"),
            &it("zreset", "&Actual Size", "0"),
            &sep,
            &it("theme_auto", "Theme: &System", ""),
            &it("theme_light", "Theme: &Light", ""),
            &it("theme_dark", "Theme: &Dark", ""),
        ],
    )
    .unwrap();
    let help = Submenu::with_items("&Help", true, &[&it("about", "&About", "")]).unwrap();
    if !is_store() {
        let _ = help.prepend(&sep);
        let _ = help.prepend(&CheckMenuItem::with_id("update_auto", "Check for updates at &startup", true, updates, None));
        let _ = help.prepend(&it("update", "Check for &Updates…", ""));
    }
    (Menu::with_items(&[&file, &view, &help]).unwrap(), recent)
}

fn confirm_discard() -> bool {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title(APP)
        .set_description("You have unsaved changes. Discard them?")
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn main() {
    let mut cfg = load_settings();
    cfg.recent.retain(|p| p.exists());
    let arg = std::env::args_os().nth(1).map(|a| canon(PathBuf::from(a)));
    if let Some(p) = &arg {
        cfg.recent.retain(|r| r != p);
        cfg.recent.insert(0, p.clone());
        cfg.recent.truncate(MAX_RECENT);
        save_settings(&cfg);
    }
    let state: Shared = Arc::new(Mutex::new(State {
        // no argument: reopen the last file
        file: arg.or_else(|| cfg.recent.first().cloned()),
        theme: cfg.theme,
        updates: cfg.updates,
        ..Default::default()
    }));
    let epoch = Arc::new(AtomicU64::new(0));

    let event_loop = EventLoopBuilder::<User>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title(APP)
        .with_inner_size(tao::dpi::LogicalSize::new(1100.0, 760.0))
        .build(&event_loop)
        .unwrap();

    // title-bar / taskbar icon: the one embedded in the exe by build.rs (resource id 1)
    #[cfg(windows)]
    {
        use tao::platform::windows::IconExtWindows;
        if let Ok(i) = tao::window::Icon::from_resource(1, None) {
            window.set_window_icon(Some(i));
        }
    }

    // The menu must be attached before the webview on Linux so it sits above it in the GTK box.
    let (menu, recent_menu) = build_menu(cfg.updates);
    let mut recent_items = Vec::new();
    fill_recent(&recent_menu, &mut recent_items, &cfg.recent);
    {
        let proxy = Mutex::new(proxy.clone());
        muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
            let _ = proxy.lock().unwrap().send_event(User::Cmd(e.id.0.clone()));
        }));
    }
    #[cfg(windows)]
    unsafe {
        use tao::platform::windows::WindowExtWindows;
        menu.init_for_hwnd(window.hwnd() as isize).unwrap();
    }
    #[cfg(target_os = "linux")]
    {
        use tao::platform::unix::WindowExtUnix;
        menu.init_for_gtk_window(window.gtk_window(), window.default_vbox()).unwrap();
    }
    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();

    let st = state.clone();
    let g = epoch.clone();
    // keep WebView2's profile out of the exe's folder (may be read-only, e.g. Program Files)
    let mut web_ctx = wry::WebContext::new(settings_path().and_then(|p| p.parent().map(|d| d.join("webview"))));
    let builder = WebViewBuilder::new_with_web_context(&mut web_ctx)
        .with_url(format!("{ORIGIN}/"))
        .with_custom_protocol("hc".into(), move |_, req| {
            let path = req.uri().path().to_string();
            let ok = |ct: &str, body: Vec<u8>| Response::builder().header(CONTENT_TYPE, ct).body(Cow::Owned(body)).unwrap();
            if path == "/" {
                return ok("text/html; charset=utf-8", { let s = st.lock().unwrap(); page(s.theme, s.updates) }.into_bytes());
            }
            if path.starts_with("/__") {
                return ok("application/json", api(&st, &g, &path, req.uri().query().unwrap_or("")).into_bytes());
            }
            Response::builder().status(404).body(Cow::Borrowed(&b""[..])).unwrap()
        })
        // the app is a single page: never navigate away
        .with_navigation_handler(|url| url.strip_prefix(ORIGIN).is_some_and(|r| r.starts_with('/') && !r.starts_with("/__")))
        .with_drag_drop_handler({
            let proxy = proxy.clone();
            move |ev| {
                if let DragDropEvent::Drop { paths, .. } = ev {
                    if let Some(p) = paths.into_iter().next() {
                        let _ = proxy.send_event(User::Open(p));
                    }
                }
                true
            }
        })
        .with_ipc_handler({
            let proxy = proxy.clone();
            move |req| {
                let _ = proxy.send_event(User::Cmd(req.body().clone()));
            }
        });

    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window).unwrap();
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().unwrap()).unwrap()
    };

    let title = |s: &State| {
        let dot = if s.dirty { "● " } else { "" };
        match s.file.as_ref().and_then(|p| p.file_name()) {
            Some(n) => format!("{dot}{} - {APP}", n.to_string_lossy()),
            None => APP.into(),
        }
    };
    let mtime = |f: &Option<PathBuf>| f.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    window.set_title(&title(&state.lock().unwrap()));
    let mut last: Option<SystemTime> = mtime(&state.lock().unwrap().file);
    let mut zoom = cfg.zoom;
    if zoom != 1.0 {
        let _ = webview.zoom(zoom);
    }

    event_loop.run(move |event, _, flow| {
        let _keep = &menu;
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(700));
        match event {
            Ev::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if !state.lock().unwrap().dirty || confirm_discard() {
                    *flow = ControlFlow::Exit;
                }
            }
            // Finder "Open With" / double-click delivers files as an Apple Event, not argv
            #[cfg(target_os = "macos")]
            Ev::Opened { urls } => {
                if let Some(p) = urls.iter().find_map(|u| u.to_file_path().ok()) {
                    let _ = proxy.send_event(User::Open(p));
                }
            }
            #[cfg(windows)]
            Ev::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                let _ = webview.set_bounds(wry::Rect {
                    position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                    size: wry::dpi::PhysicalSize::new(size.width, size.height).into(),
                });
            }
            Ev::UserEvent(u) => {
                let mut pick = None;
                match u {
                    User::Open(p) => pick = Some(p),
                    User::Cmd(c) => match c.as_str() {
                        "open" => {
                            let mut d = rfd::FileDialog::new()
                                .add_filter("CSV / TSV", &["csv", "tsv", "tab", "txt"])
                                .add_filter("All files", &["*"]);
                            if let Some(dir) = state.lock().unwrap().file.as_ref().and_then(|f| f.parent()) {
                                d = d.set_directory(dir);
                            }
                            if !state.lock().unwrap().dirty || confirm_discard() {
                                pick = d.pick_file();
                            }
                        }
                        "reload" => {
                            if !state.lock().unwrap().dirty || confirm_discard() {
                                let _ = webview.evaluate_script("__load()");
                            }
                        }
                        "save" => {
                            let _ = webview.evaluate_script("__save()");
                        }
                        "editor" => {
                            if let Some(f) = &state.lock().unwrap().file {
                                open_external(&f.to_string_lossy());
                            }
                        }
                        "find" => {
                            let _ = webview.evaluate_script("__find()");
                        }
                        "zin" | "zout" | "zreset" => {
                            zoom = match c.as_str() {
                                "zin" => zoom + 0.1,
                                "zout" => zoom - 0.1,
                                _ => 1.0,
                            }
                            .clamp(0.3, 5.0);
                            let _ = webview.zoom(zoom);
                            cfg.zoom = zoom;
                            save_settings(&cfg);
                        }
                        "recent_clear" => {
                            cfg.recent.clear();
                            fill_recent(&recent_menu, &mut recent_items, &cfg.recent);
                            save_settings(&cfg);
                        }
                        r if r.starts_with("recent:") => {
                            if !state.lock().unwrap().dirty || confirm_discard() {
                                pick = r[7..].parse::<usize>().ok().and_then(|i| cfg.recent.get(i).cloned());
                            }
                        }
                        "theme_auto" | "theme_light" | "theme_dark" => {
                            cfg.theme = match c.as_str() {
                                "theme_light" => "light",
                                "theme_dark" => "dark",
                                _ => "",
                            };
                            state.lock().unwrap().theme = cfg.theme;
                            save_settings(&cfg);
                            let _ = webview.evaluate_script(&format!("__theme('{}')", cfg.theme));
                        }
                        "update" => {
                            let _ = webview.evaluate_script("__checkUpdate&&__checkUpdate(true)");
                        }
                        "update_auto" => {
                            cfg.updates = !cfg.updates;
                            state.lock().unwrap().updates = cfg.updates;
                            save_settings(&cfg);
                        }
                        "about" => {
                            rfd::MessageDialog::new()
                                .set_title("About")
                                .set_description(format!(
                                    "{APP} {}\nFast CSV / TSV viewer and editor for files of any size.",
                                    env!("CARGO_PKG_VERSION")
                                ))
                                .show();
                        }
                        "quit" => {
                            if !state.lock().unwrap().dirty || confirm_discard() {
                                *flow = ControlFlow::Exit;
                            }
                        }
                        "dirty:1" | "dirty:0" => {
                            let mut s = state.lock().unwrap();
                            s.dirty = c.ends_with('1');
                            window.set_title(&title(&s));
                        }
                        m if m.starts_with("save\u{1e}") || m.starts_with("delta\u{1e}") => {
                            let r = save(&state, m);
                            last = mtime(&state.lock().unwrap().file);
                            let js = match r {
                                Ok(()) => "__saved(true,'')".to_string(),
                                Err(e) => {
                                    let mut o = String::new();
                                    json_str(&e, &mut o);
                                    format!("__saved(false,{o})")
                                }
                            };
                            let _ = webview.evaluate_script(&js);
                        }
                        _ => {}
                    },
                }
                if let Some(p) = pick {
                    let mut s = state.lock().unwrap();
                    s.file = Some(canon(p));
                    s.dirty = false;
                    window.set_title(&title(&s));
                    last = mtime(&s.file);
                    let _ = webview.evaluate_script("__load(true)");
                    if let Some(f) = s.file.clone() {
                        cfg.recent.retain(|r| *r != f);
                        cfg.recent.insert(0, f);
                        cfg.recent.truncate(MAX_RECENT);
                        fill_recent(&recent_menu, &mut recent_items, &cfg.recent);
                        save_settings(&cfg);
                    }
                }
            }
            // tell the page when the file changed on disk (it reloads itself if there are no unsaved edits)
            Ev::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                let m = mtime(&state.lock().unwrap().file);
                if m != last {
                    last = m;
                    let _ = webview.evaluate_script("__ext()");
                }
            }
            _ => {}
        }
    });
}
