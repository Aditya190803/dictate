//! Recording pill on the Wayland overlay layer (gtk4-layer-shell + cairo).

use crate::overlay_ipc::{default_socket_path, OverlayState};
use gtk4::cairo::{Context, Operator};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixDatagram;

const W: i32 = 140;
const H: i32 = 40;
const BARS: usize = 16;
const BAR_W: f64 = 3.0;
const BAR_GAP: f64 = 3.0;
const PILL_RADIUS: f64 = 18.0;

struct UiState {
    state: OverlayState,
    levels: VecDeque<f32>,
    preview: String,
    last_packet: Instant,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            state: OverlayState::Idle,
            levels: VecDeque::new(),
            preview: String::new(),
            last_packet: Instant::now(),
        }
    }
}

fn visible(state: &UiState) -> bool {
    !(state.state == OverlayState::Idle && state.last_packet.elapsed() > Duration::from_millis(400))
}

fn accent_rgb(state: OverlayState) -> (f64, f64, f64) {
    match state {
        OverlayState::Listening => (0.29, 0.85, 0.65),
        OverlayState::Processing => (0.96, 0.65, 0.14),
        OverlayState::Idle => (0.42, 0.45, 0.50),
    }
}

fn round_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let degrees = std::f64::consts::PI / 180.0;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90.0 * degrees, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90.0 * degrees);
    cr.arc(x + r, y + h - r, r, 90.0 * degrees, 180.0 * degrees);
    cr.arc(x + r, y + r, r, 180.0 * degrees, 270.0 * degrees);
    cr.close_path();
}

fn paint_pill(cr: &Context, width: i32, height: i32, shared: &Arc<Mutex<UiState>>) {
    let snap = shared
        .lock()
        .ok()
        .map(|g| (visible(&g), g.state, g.levels.clone(), g.preview.clone()))
        .unwrap_or((false, OverlayState::Idle, VecDeque::new(), String::new()));

    let (show, state, levels, preview) = snap;
    if !show {
        return;
    }

    let w = width as f64;
    let h = height as f64;

    cr.set_operator(Operator::Over);
    cr.set_source_rgba(0.10, 0.10, 0.12, 0.88);
    round_rect(cr, 0.0, 0.0, w, h, PILL_RADIUS);
    cr.fill().ok();

    let (r, g, b) = accent_rgb(state);
    cr.set_source_rgb(r, g, b);

    let total_w = BARS as f64 * BAR_W + (BARS - 1) as f64 * BAR_GAP;
    let start_x = (w - total_w) / 2.0;
    let baseline = h - 8.0;
    let max_h = h - 16.0;

    for (i, level) in levels.iter().take(BARS).enumerate() {
        let lv = (*level).clamp(0.08, 1.0) as f64;
        let bh = max_h * lv;
        let x = start_x + i as f64 * (BAR_W + BAR_GAP);
        cr.rectangle(x, baseline - bh, BAR_W, bh);
    }
    cr.fill().ok();

    if !preview.is_empty() && state == OverlayState::Listening {
        cr.set_source_rgba(0.85, 0.88, 0.92, 0.95);
        let _ = cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
        cr.set_font_size(9.0);
        let _ = cr.move_to(8.0, 11.0);
        let _ = cr.show_text(&preview);
    }
}

#[derive(Deserialize)]
struct InState {
    s: OverlayState,
}

#[derive(Deserialize)]
struct InLevel {
    v: f32,
}

#[derive(Deserialize)]
struct InPreview {
    t: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum InMsg {
    State(InState),
    Level(InLevel),
    Preview(InPreview),
}

fn apply_packet(shared: &Arc<Mutex<UiState>>, line: &str) {
    let Ok(msg) = serde_json::from_str::<InMsg>(line) else {
        return;
    };
    let Ok(mut g) = shared.lock() else {
        return;
    };
    g.last_packet = Instant::now();
    match msg {
        InMsg::State(s) => {
            g.state = s.s;
            if s.s == OverlayState::Idle {
                g.levels.clear();
                g.preview.clear();
            }
        }
        InMsg::Preview(p) => {
            g.preview = p.t;
        }
        InMsg::Level(l) => {
            g.levels.push_back(l.v.clamp(0.0, 1.0));
            while g.levels.len() > BARS {
                g.levels.pop_front();
            }
        }
    }
}

fn spawn_socket_listener(
    path: &std::path::Path,
    shared: Arc<Mutex<UiState>>,
) -> anyhow::Result<()> {
    #[cfg(not(unix))]
    anyhow::bail!("dictate-overlay requires Linux/Unix");

    #[cfg(unix)]
    {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let sock = UnixDatagram::bind(path)?;
        sock.set_nonblocking(true)?;
        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            loop {
                match sock.recv(&mut buf) {
                    Ok(n) => {
                        if let Ok(line) = std::str::from_utf8(&buf[..n]) {
                            apply_packet(&shared, line.trim());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(16));
                    }
                    Err(_) => break,
                }
            }
        });
    }
    Ok(())
}

pub fn run() -> anyhow::Result<()> {
    if !gtk4_layer_shell::is_supported() {
        anyhow::bail!(
            "Wayland layer-shell is not available (need a Wayland compositor with wlr-layer-shell / layer-shell)"
        );
    }

    let path = std::env::var("DICTATE_OVERLAY_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| default_socket_path());

    let shared = Arc::new(Mutex::new(UiState {
        last_packet: Instant::now(),
        ..Default::default()
    }));

    spawn_socket_listener(&path, Arc::clone(&shared))?;

    let app = Application::builder()
        .application_id("dev.dictate.overlay")
        .build();

    let shared_ui = Arc::clone(&shared);
    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("dictate")
            .default_width(W)
            .default_height(H)
            .resizable(false)
            .decorated(false)
            .build();

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 72);
        window.set_size_request(W, H);

        let css = gtk4::CssProvider::new();
        css.load_from_data("window { background-color: transparent; }\n");
        gtk4::style_context_add_provider_for_display(
            &gtk4::prelude::RootExt::display(&window),
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let area = DrawingArea::new();
        area.set_content_width(W);
        area.set_content_height(H);
        let draw_shared = Arc::clone(&shared_ui);
        area.set_draw_func(move |_area, cr, width, height| {
            paint_pill(cr, width, height, &draw_shared);
        });

        window.set_child(Some(&area));

        let tick_area = area.clone();
        let tick_shared = Arc::clone(&shared_ui);
        glib::timeout_add_local(Duration::from_millis(33), move || {
            let show = tick_shared
                .lock()
                .ok()
                .map(|g| visible(&g))
                .unwrap_or(false);
            if show {
                tick_area.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        window.present();
    });

    app.run();
    Ok(())
}
