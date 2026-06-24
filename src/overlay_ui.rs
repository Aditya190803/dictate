//! Floating recording pill (winit + softbuffer).

use crate::overlay_ipc::{default_socket_path, OverlayState};
use serde::Deserialize;
use softbuffer::{Context, Surface};
use std::collections::VecDeque;

use std::sync::{Arc, Mutex};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowAttributes};

const W: u32 = 140;
const H: u32 = 40;
const BARS: usize = 16;

struct UiState {
    state: OverlayState,
    levels: VecDeque<f32>,
    last_packet: Instant,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            state: OverlayState::Idle,
            levels: VecDeque::new(),
            last_packet: Instant::now(),
        }
    }
}

struct PillApp {
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    shared: Arc<Mutex<UiState>>,
}

impl PillApp {
    fn new(shared: Arc<Mutex<UiState>>) -> Self {
        Self {
            window: None,
            surface: None,
            shared,
        }
    }

    fn paint(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let Some(surface) = self.surface.as_mut() else {
            return;
        };

        let state = self.shared.lock().ok();
        let (visible, state, levels) = state.map(|g| {
            let idle_hide = g.state == OverlayState::Idle
                && g.last_packet.elapsed() > Duration::from_millis(400);
            (!idle_hide, g.state, g.levels.clone())
        }).unwrap_or((false, OverlayState::Idle, VecDeque::new()));

        let w = NonZeroU32::new(size.width).unwrap_or(NonZeroU32::MIN);
        let h = NonZeroU32::new(size.height).unwrap_or(NonZeroU32::MIN);
        surface.resize(w, h).ok();
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };

        let w = size.width as usize;
        let h = size.height as usize;
        let bg = if visible { 0xE0_1A_1A_1E } else { 0x00_00_00_00 };

        for pixel in buffer.iter_mut() {
            *pixel = bg;
        }

        if !visible {
            let _ = buffer.present();
            return;
        }

        let accent = match state {
            OverlayState::Listening => 0xFF_4A_D9_A5,
            OverlayState::Processing => 0xFF_F5_A6_23,
            OverlayState::Idle => 0xFF_6B_72_80,
        };

        let bar_w = 4;
        let gap = 3;
        let total_w = BARS * bar_w + (BARS - 1) * gap;
        let start_x = (w.saturating_sub(total_w)) / 2;
        let max_h = h.saturating_sub(12);

        for (i, level) in levels.iter().take(BARS).enumerate() {
            let bh = ((max_h as f32) * level.clamp(0.08, 1.0)) as usize;
            let x0 = start_x + i * (bar_w + gap);
            let y0 = h - 6 - bh;
            for y in y0..(h - 6).min(h) {
                for x in x0..(x0 + bar_w).min(w) {
                    if let Some(p) = buffer.get_mut(y * w + x) {
                        *p = accent;
                    }
                }
            }
        }

        let _ = buffer.present();
    }
}

impl ApplicationHandler for PillApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("dictate")
            .with_inner_size(LogicalSize::new(W, H))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false);
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create overlay window"),
        );
        let ctx = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&ctx, window.clone()).expect("softbuffer surface");
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.paint(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
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
#[serde(tag = "type", rename_all = "lowercase")]
enum InMsg {
    State(InState),
    Level(InLevel),
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
            }
        }
        InMsg::Level(l) => {
            g.levels.push_back(l.v.clamp(0.0, 1.0));
            while g.levels.len() > BARS {
                g.levels.pop_front();
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let path = std::env::var("DICTATE_OVERLAY_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| default_socket_path());
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    #[cfg(not(unix))]
    anyhow::bail!("dictate-overlay requires Linux/Unix");

    let shared = Arc::new(Mutex::new(UiState {
        last_packet: Instant::now(),
        ..Default::default()
    }));

    #[cfg(unix)]
    {
        let sock = UnixDatagram::bind(&path)?;
        sock.set_nonblocking(true)?;
        let shared_recv = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            loop {
                match sock.recv(&mut buf) {
                Ok(n) => {
                    if let Ok(line) = std::str::from_utf8(&buf[..n]) {
                        apply_packet(&shared_recv, line.trim());
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

    // Bottom-center pill
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = PillApp::new(shared);
    event_loop.run_app(&mut app)?;
    Ok(())
}