//! Application state and the per-frame loop.
//!
//! The window is split into a sidebar ([`crate::shell`]) and a page area. Each
//! page lives in its own module under [`crate::pages`] as an `impl EcoView`
//! block, so they all share this state.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Local, Timelike};
use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers, Pos2, Rect, Vec2};

use crate::grid::CardId;
use crate::icons::Icon;
use crate::metrics::{self, Metrics};
use crate::notifications::NotifyTx;
use crate::particles::Ambient;
use crate::raccoon::World;
use crate::raccoon::chip::Chip;
use crate::raccoon::overlay::Overlay;
use crate::settings::Settings;
use crate::sound::{self, Cue, SoundTx};
use crate::theme::Palette;
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Performance,
    Processes,
    Storage,
    Devices,
    System,
    Settings,
}

impl Page {
    pub const ALL: [Page; 7] =
        [Page::Overview, Page::Performance, Page::Processes, Page::Storage, Page::Devices, Page::System, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Performance => "Performance",
            Page::Processes => "Processes",
            Page::Storage => "Storage",
            Page::Devices => "Devices",
            Page::System => "System",
            Page::Settings => "Settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Page::Overview => Icon::Overview,
            Page::Performance => Icon::Perf,
            Page::Processes => Icon::Processes,
            Page::Storage => Icon::Storage,
            Page::Devices => Icon::Devices,
            Page::System => Icon::System,
            Page::Settings => Icon::Settings,
        }
    }

    pub fn from_name(s: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.title().eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Crit,
}

impl Level {
    pub fn color(self, pal: &Palette) -> Color32 {
        match self {
            Level::Info => pal.info,
            Level::Ok => pal.ok,
            Level::Warn => pal.warn,
            Level::Crit => pal.crit,
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Level::Info => Icon::Info,
            Level::Ok => Icon::Check,
            Level::Warn | Level::Crit => Icon::Warning,
        }
    }
}

pub struct Toast {
    pub id: u64,
    pub level: Level,
    pub title: String,
    pub body: String,
    pub born: f64,
    pub ttl: f32,
}

pub struct Alert {
    pub when: DateTime<Local>,
    pub level: Level,
    pub title: String,
    pub body: String,
    pub page: Page,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcSort {
    Cpu,
    Memory,
    Name,
    Pid,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsSection {
    General,
    Appearance,
    Dashboard,
    Alerts,
    Sound,
    Discord,
    Raccoon,
    About,
}

impl SettingsSection {
    pub fn from_name(s: &str) -> Option<Self> {
        use SettingsSection::*;
        [General, Appearance, Dashboard, Alerts, Sound, Discord, Raccoon, About]
            .into_iter()
            .find(|x| format!("{x:?}").eq_ignore_ascii_case(s))
    }
}

pub struct KillRequest {
    pub pid: u32,
    pub name: String,
    pub force: bool,
}

#[derive(Clone, Copy)]
pub enum DragKind {
    Move,
    Resize,
}

pub struct EcoView {
    pub(crate) settings: Settings,
    pub(crate) settings_shared: Arc<Mutex<Settings>>,
    pub(crate) metrics_shared: Arc<Mutex<Metrics>>,
    /// This frame's copy of the metrics, refreshed only when a new sample lands.
    pub(crate) m: Metrics,
    m_samples: u64,
    pub(crate) refresh_ms: Arc<Mutex<u64>>,
    pub(crate) sound_gain: Arc<Mutex<(bool, f32)>>,
    pub(crate) notify: NotifyTx,
    pub(crate) sound: SoundTx,
    pub(crate) ambient: Ambient,

    // --- raccoon (unchanged behaviour, just hosted here) ---
    pub(crate) raccoon: Overlay,
    pub(crate) chip: Chip,
    raccoon_shown: bool,
    stage: Option<(Rect, Rect)>,
    stage_checked: Option<Instant>,
    cards_sent: Vec<Rect>,
    pub(crate) platforms: Vec<Rect>,

    // --- navigation ---
    pub(crate) page: Page,
    pub(crate) page_t: f64,
    pub(crate) time: f64,
    pub(crate) pal: Palette,
    style_key: Option<String>,

    // --- dashboard editing ---
    pub(crate) edit_layout: bool,
    pub(crate) drag: Option<(CardId, DragKind, Vec2)>,

    // --- feedback ---
    pub(crate) toasts: Vec<Toast>,
    toast_seq: u64,
    pub(crate) alerts: VecDeque<Alert>,
    pub(crate) unread: usize,
    last_alert: Option<Instant>,
    pub(crate) banner: Option<(Level, String, Page)>,
    pub(crate) alerts_open: bool,

    // --- command palette ---
    pub(crate) palette_open: bool,
    pub(crate) palette_query: String,
    pub(crate) palette_sel: usize,

    // --- pages ---
    pub(crate) proc_query: String,
    pub(crate) proc_sort: ProcSort,
    pub(crate) proc_desc: bool,
    pub(crate) proc_selected: Option<u32>,
    pub(crate) kill: Option<KillRequest>,
    pub(crate) device_query: String,
    pub(crate) device_filter: u8,
    pub(crate) section: SettingsSection,
    /// Headless renders skip the raccoon and anything that touches AppKit.
    pub(crate) headless: bool,
}

impl EcoView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        apply_icon(&cc.egui_ctx, &settings);
        crate::theme::install_fonts(&cc.egui_ctx);
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        Self::build(settings, metrics, false)
    }

    /// Builds the app around an existing metrics feed. Used by `new` and by
    /// the headless screenshot renderer.
    pub fn build(settings: Settings, metrics: Arc<Mutex<Metrics>>, headless: bool) -> Self {
        let refresh_ms = Arc::new(Mutex::new(settings.refresh_ms));
        if !headless {
            metrics::spawn(metrics.clone(), refresh_ms.clone());
        }
        let settings_shared = Arc::new(Mutex::new(settings.clone()));
        if !headless {
            crate::discord::spawn(metrics.clone(), settings_shared.clone());
        }
        let sound_gain = Arc::new(Mutex::new((settings.sound_enabled && !headless, settings.sound_volume)));
        let notify = crate::notifications::spawn();
        let sound = sound::spawn(sound_gain.clone());
        if !headless && settings.autostart && !crate::autostart::is_enabled() {
            let _ = crate::autostart::set_enabled(true);
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        let world = World::new(&settings.companion, seed);
        let raccoon = Overlay::new(world, metrics.clone(), sound.clone());
        let mut chip = Chip::default();
        if let Some(pos) = settings.companion_chip_pos {
            chip.set_pos(pos);
        }
        let pal = Palette::from_settings(&settings);
        Self {
            m: Metrics::default(),
            m_samples: u64::MAX,
            metrics_shared: metrics,
            raccoon,
            chip,
            raccoon_shown: false,
            stage: None,
            stage_checked: None,
            cards_sent: Vec::new(),
            platforms: Vec::new(),
            settings,
            settings_shared,
            refresh_ms,
            sound_gain,
            notify,
            sound,
            ambient: Ambient::new(),
            page: Page::Overview,
            page_t: -10.0,
            time: 0.0,
            pal,
            style_key: None,
            edit_layout: false,
            drag: None,
            toasts: Vec::new(),
            toast_seq: 0,
            alerts: VecDeque::new(),
            unread: 0,
            last_alert: None,
            banner: None,
            alerts_open: false,
            palette_open: false,
            palette_query: String::new(),
            palette_sel: 0,
            proc_query: String::new(),
            proc_sort: ProcSort::Cpu,
            proc_desc: true,
            proc_selected: None,
            kill: None,
            device_query: String::new(),
            device_filter: 0,
            section: SettingsSection::General,
            headless,
        }
    }

    /// Saves settings and pushes them to everything that keeps a copy.
    pub(crate) fn persist(&mut self) {
        self.settings.sanitize();
        self.settings.companion_chip_pos = self.chip.pos();
        if let Ok(mut w) = self.raccoon.world.lock() {
            w.set_settings(&self.settings.companion);
        }
        if !self.headless {
            self.settings.save();
        }
        if let Ok(mut g) = self.settings_shared.lock() {
            *g = self.settings.clone();
        }
        if let Ok(mut g) = self.refresh_ms.lock() {
            *g = self.settings.refresh_ms;
        }
        if let Ok(mut g) = self.sound_gain.lock() {
            *g = (self.settings.sound_enabled && !self.headless, self.settings.sound_volume);
        }
    }

    pub(crate) fn go(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.page_t = self.time;
            self.sound.play(Cue::Click);
        }
    }

    pub(crate) fn toast(&mut self, level: Level, title: impl Into<String>, body: impl Into<String>) {
        self.toast_seq += 1;
        self.toasts.push(Toast {
            id: self.toast_seq,
            level,
            title: title.into(),
            body: body.into(),
            born: self.time,
            ttl: 4.5,
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// Steps of the refresh interval, in seconds.
    pub(crate) fn step_secs(&self) -> f32 {
        self.settings.refresh_ms as f32 / 1000.0
    }

    /// 0..100 summary of how the machine is doing, plus a short verdict.
    pub(crate) fn health(&self) -> (f32, &'static str, Level) {
        let s = &self.m.snap;
        let mut score = 100.0_f32;
        score -= (s.cpu_global - 70.0).max(0.0) * 0.9;
        score -= (metrics::pct(s.mem_used, s.mem_total) - 80.0).max(0.0) * 1.2;
        if let Some(t) = &s.temperature {
            score -= (t.celsius - 80.0).max(0.0) * 1.5;
        }
        if let Some(g) = s.gpu.load {
            score -= (g - 85.0).max(0.0) * 0.6;
        }
        if let Some(d) = s.disks.first() {
            let used = metrics::pct(d.total.saturating_sub(d.available), d.total);
            score -= (used - 90.0).max(0.0) * 2.0;
        }
        if let Some(b) = &s.battery
            && !b.charging
        {
            score -= (20.0 - b.percent).max(0.0);
        }
        let score = score.clamp(0.0, 100.0);
        if score >= 85.0 {
            (score, "Running smoothly", Level::Ok)
        } else if score >= 60.0 {
            (score, "Working hard", Level::Warn)
        } else {
            (score, "Under heavy strain", Level::Crit)
        }
    }

    /// Everything drawn in one frame. `eframe::App::update` and the headless
    /// renderer both call this.
    pub fn frame(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let dt = ((now - self.time) as f32).clamp(0.0, 0.1);
        self.time = now;

        // Style only needs rebuilding when something that affects it changes.
        self.pal = Palette::from_settings(&self.settings);
        let key = format!(
            "{:?}{}{:?}{:?}{}{}",
            self.settings.theme, self.settings.custom_accent, self.settings.accent, self.settings.accent_alt, self.settings.text_scale, self.settings.compact
        );
        if self.style_key.as_deref() != Some(key.as_str()) {
            self.pal.apply(ctx, self.settings.text_scale, self.settings.compact);
            self.style_key = Some(key);
        }
        widgets::set_reduce_motion(ctx, self.settings.reduce_motion);

        if let Ok(m) = self.metrics_shared.lock()
            && m.history.samples != self.m_samples
        {
            self.m_samples = m.history.samples;
            self.m = m.clone();
        }

        if ctx.input(|i| i.pointer.any_released()) || !self.edit_layout {
            self.drag = None;
        }

        self.shortcuts(ctx);
        self.check_alerts();
        self.toasts.retain(|t| (self.time - t.born) < t.ttl as f64 + 0.4);

        // Background: a soft vertical gradient plus the optional ambient effect.
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        let pal = self.pal;
        painter.rect_filled(screen, 0.0, pal.bg);
        widgets::gradient_rect(&painter, screen, pal.bg_top, pal.bg, egui::CornerRadius::ZERO, false);
        self.ambient.paint(&painter, screen, pal, self.settings.particles, self.settings.particle_intensity, dt);

        self.sidebar(ctx);
        self.platforms.clear();
        let mut cards = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 14, bottom: 16 }))
            .show(ctx, |ui| {
                self.header(ui);
                self.banner_strip(ui);
                // Pages fade and rise in.
                let age = (self.time - self.page_t) as f32;
                let t = if self.settings.reduce_motion { 1.0 } else { (age / 0.22).clamp(0.0, 1.0) };
                if t < 1.0 {
                    ctx.request_repaint();
                }
                let ease = 1.0 - (1.0 - t).powi(3);
                ui.set_opacity(ease);
                ui.add_space((1.0 - ease) * 10.0);
                match self.page {
                    Page::Overview => self.overview(ui, &mut cards),
                    Page::Performance => self.performance(ui),
                    Page::Processes => self.processes(ui),
                    Page::Storage => self.storage(ui),
                    Page::Devices => self.devices(ui),
                    Page::System => self.system(ui),
                    Page::Settings => self.settings_page(ui),
                }
            });
        self.platforms = cards;

        self.toasts_layer(ctx);
        self.alerts_popover(ctx);
        self.kill_modal(ctx);
        self.command_palette(ctx);

        if !self.headless {
            self.raccoon_chip(ctx);
            self.drive_raccoon(ctx);
        }

        // Frame pacing: smooth when something animates, otherwise just often
        // enough to show new samples.
        let busy = self.settings.particles != crate::particles::ParticleStyle::Off && self.settings.particle_intensity > 0.01;
        let ms = if busy || self.drag.is_some() { 33 } else { 250 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let pressed = |ctx: &egui::Context, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(cmd, k)));
        if pressed(ctx, Key::K) {
            self.palette_open = !self.palette_open;
            self.palette_query.clear();
            self.palette_sel = 0;
        }
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6];
        for (i, k) in keys.iter().enumerate() {
            if pressed(ctx, *k) {
                self.go(Page::ALL[i]);
            }
        }
        if pressed(ctx, Key::Comma) {
            self.go(Page::Settings);
        }
        if pressed(ctx, Key::E) && self.page == Page::Overview {
            self.edit_layout = !self.edit_layout;
        }
        if pressed(ctx, Key::B) {
            self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
            self.persist();
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.palette_open {
                self.palette_open = false;
            } else if self.kill.is_some() {
                self.kill = None;
            } else if self.alerts_open {
                self.alerts_open = false;
            } else if self.edit_layout {
                self.edit_layout = false;
            }
        }
    }

    /// Raises an alert when a threshold is crossed (with a cooldown).
    fn check_alerts(&mut self) {
        if self.m.history.samples == 0 {
            return;
        }
        let cooldown = self.settings.alert_cooldown_secs as u64;
        if self.last_alert.is_some_and(|t| t.elapsed().as_secs() < cooldown) {
            return;
        }
        let s = &self.m.snap;
        let mem = metrics::pct(s.mem_used, s.mem_total);
        let found: Option<(Level, String, String, Page)> = if s.cpu_global >= self.settings.warn_cpu {
            Some((Level::Crit, format!("CPU at {:.0}%", s.cpu_global), format!("Above your {:.0}% limit", self.settings.warn_cpu), Page::Processes))
        } else if mem >= self.settings.warn_ram {
            Some((Level::Warn, format!("Memory at {mem:.0}%"), format!("Above your {:.0}% limit", self.settings.warn_ram), Page::Performance))
        } else if s.gpu.load.unwrap_or(0.0) >= self.settings.warn_gpu {
            Some((Level::Warn, format!("GPU at {:.0}%", s.gpu.load.unwrap_or(0.0)), format!("Above your {:.0}% limit", self.settings.warn_gpu), Page::Performance))
        } else if let Some(t) = s.temperature.as_ref().filter(|t| t.celsius >= self.settings.warn_temp) {
            Some((Level::Crit, format!("Running hot: {:.0}°C", t.celsius), format!("{} is above {:.0}°C", t.source, self.settings.warn_temp), Page::Performance))
        } else {
            None
        };
        let Some((level, title, body, page)) = found else { return };
        self.last_alert = Some(Instant::now());
        self.alerts.push_front(Alert { when: Local::now(), level, title: title.clone(), body: body.clone(), page });
        self.alerts.truncate(100);
        self.unread += 1;
        if self.settings.show_banner {
            self.banner = Some((level, format!("{title} — {body}"), page));
        }
        if self.settings.show_toasts {
            self.toast(level, title.clone(), body.clone());
        }
        if self.settings.native_alerts && !self.headless {
            self.notify.post(format!("{title}. {body}"));
        }
        self.sound.play(Cue::Warn);
    }

    pub(crate) fn greeting(&self) -> &'static str {
        match Local::now().hour() {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            18..=22 => "Good evening",
            _ => "Burning the midnight oil",
        }
    }

    // ------------------------------------------------------------------
    // Raccoon hosting. The raccoon itself is finished; this only keeps his
    // overlay alive and tells him where the dashboard cards are.
    // ------------------------------------------------------------------

    fn raccoon_chip(&mut self, ctx: &egui::Context) {
        let enabled = self.settings.companion.enabled;
        let status = self
            .raccoon
            .world
            .lock()
            .map(|w| format!("{}: {}", self.settings.companion.name, w.act.label()))
            .unwrap_or_default();
        let raccoon_pal = self.settings.companion.palette();
        let chip = self.chip.paint(ctx, enabled, self.pal.accent, &raccoon_pal, &status);
        if chip.toggled {
            self.sound.play(Cue::Accent);
            self.settings.companion.enabled = !enabled;
        }
        if chip.summon
            && let Ok(mut w) = self.raccoon.world.lock()
        {
            let at = ctx.input(|i| i.viewport().inner_rect).map(|r| r.center()).unwrap_or(w.area.center());
            w.summon(at);
        }
        if chip.toggled || self.chip.settled() {
            self.persist();
        }
    }

    fn drive_raccoon(&mut self, ctx: &egui::Context) {
        if !self.settings.companion.enabled {
            if self.raccoon_shown {
                self.raccoon_shown = false;
                self.raccoon.hidden();
                if let Ok(mut w) = self.raccoon.world.lock() {
                    w.enabled = false;
                }
            }
            return;
        }
        let inner = ctx.input(|i| i.viewport().inner_rect);
        let recheck = self.stage_checked.is_none_or(|t| t.elapsed().as_secs_f32() > 1.0);
        if recheck {
            self.stage_checked = Some(Instant::now());
            let probe = inner.map(|r| r.center()).unwrap_or(Pos2::new(100.0, 100.0));
            if let Some(d) = crate::raccoon::desktop::display_at(probe) {
                self.stage = Some((d.frame, d.visible));
            }
        }
        let Some((stage, visible)) = self.stage else { return };
        let cards: Vec<Rect> = match inner {
            Some(r) if ctx.input(|i| !i.viewport().minimized.unwrap_or(false)) => {
                self.platforms.iter().map(|c| c.translate(r.min.to_vec2())).collect()
            }
            _ => Vec::new(),
        };
        if let Ok(mut w) = self.raccoon.world.lock() {
            if !self.raccoon_shown {
                w.set_settings(&self.settings.companion);
            }
            w.enabled = true;
            w.set_stage(stage, visible);
            if cards != self.cards_sent {
                w.set_cards(cards.clone());
            }
        }
        self.cards_sent = cards;
        self.raccoon_shown = true;
        self.raccoon.show(ctx, stage);
    }
}

impl eframe::App for EcoView {
    /// Fully transparent, so the raccoon's overlay window (which shares this
    /// GL setup) shows only what it paints. The dashboard paints its own
    /// opaque background over every pixel.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

pub(crate) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

pub(crate) fn native_open() -> Option<std::path::PathBuf> {
    let script = r#"POSIX path of (choose file of type {"public.png","public.jpeg","com.microsoft.bmp","com.apple.icns","public.image"} with prompt "Choose an icon")"#;
    let output = std::process::Command::new("osascript").args(["-e", script]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() { None } else { Some(std::path::PathBuf::from(path)) }
}

/// Applies the custom app icon, off the main thread.
///
/// Reading a file in a privacy-protected folder (Downloads, Desktop, ...) can
/// wait on a macOS permission prompt; doing that on the main thread would
/// keep the window from ever appearing.
pub(crate) fn apply_icon(ctx: &egui::Context, settings: &Settings) {
    let Some(path) = settings.custom_icon.clone() else { return };
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("eco-icon".into())
        .spawn(move || {
            let Ok(bytes) = std::fs::read(&path) else { return };
            let Ok(img) = image::load_from_memory(&bytes) else { return };
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let icon = egui::IconData { rgba: rgba.into_raw(), width: w, height: h };
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(icon))));
            ctx.request_repaint();
        })
        .ok();
}
