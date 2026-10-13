use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

use crate::companion::{Chip, Companion};
use crate::overlay::{self, OverlayEvents, Shared};
use crate::format;
use crate::grid::{CardId, COLS};
use crate::icons::{self, Icon};
use crate::metrics::{self, Metrics};
use crate::notifications::NotifyTx;
use crate::particles::Ambient;
use crate::settings::{Settings, ThemeKind};
use crate::sound::{self, Cue, SoundTx};
use crate::theme::Palette;
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Performance,
    Processes,
    Storage,
    Settings,
}

pub struct EcoView {
    settings: Settings,
    settings_shared: Arc<Mutex<Settings>>,
    metrics: Arc<Mutex<Metrics>>,
    refresh_ms: Arc<Mutex<u64>>,
    sound_gain: Arc<Mutex<(bool, f32)>>,
    notify: NotifyTx,
    sound: SoundTx,
    ambient: Ambient,
    /// Shared with the always-on-top overlay window.
    npc: Shared,
    chip: Chip,
    overlay_live: bool,
    overlay_events: OverlayEvents,
    platforms_cached: Vec<Rect>,
    page: Page,
    last_frame: Instant,
    pulse: f32,
    edit_layout: bool,
    drag: Option<(CardId, DragKind, Vec2)>,
    toast: Option<(String, f32)>,
    banner: Option<String>,
    last_warn: Instant,
    platforms: Vec<Rect>,
}

#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize,
}

impl EcoView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        apply_icon(cc, &settings);
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let refresh_ms = Arc::new(Mutex::new(settings.refresh_ms));
        metrics::spawn(metrics.clone(), refresh_ms.clone());
        let settings_shared = Arc::new(Mutex::new(settings.clone()));
        crate::discord::spawn(metrics.clone(), settings_shared.clone());
        let sound_gain = Arc::new(Mutex::new((settings.sound_enabled, settings.sound_volume)));
        let notify = crate::notifications::spawn();
        let sound = sound::spawn(sound_gain.clone());
        if settings.autostart && !crate::autostart::is_enabled() {
            let _ = crate::autostart::set_enabled(true);
        }
        let mut npc = Companion::new(&settings.companion);
        npc.enabled = settings.companion.enabled;
        let mut chip = Chip::default();
        if let Some(pos) = settings.companion_chip_pos {
            chip.set_pos(pos);
        }
        Self {
            npc: Arc::new(Mutex::new(npc)),
            chip,
            overlay_live: false,
            overlay_events: OverlayEvents::default(),
            platforms_cached: Vec::new(),
            settings,
            settings_shared,
            metrics,
            refresh_ms,
            sound_gain,
            notify,
            sound,
            ambient: Ambient::new(),
            page: Page::Overview,
            last_frame: Instant::now(),
            pulse: 0.0,
            edit_layout: false,
            drag: None,
            toast: None,
            banner: None,
            // `Instant - Duration` panics on underflow; uptime may be shorter than 30s.
            last_warn: Instant::now()
                .checked_sub(std::time::Duration::from_secs(30))
                .unwrap_or_else(Instant::now),
            platforms: Vec::new(),
        }
    }

    fn persist(&mut self) {
        self.settings.companion.enabled = self.npc.lock().map(|n| n.enabled).unwrap_or(false);
        self.settings.companion_chip_pos = self.chip.pos();
        self.settings.save();
        if let Ok(mut g) = self.settings_shared.lock() {
            *g = self.settings.clone();
        }
        if let Ok(mut g) = self.refresh_ms.lock() {
            *g = self.settings.refresh_ms;
        }
        if let Ok(mut g) = self.sound_gain.lock() {
            *g = (self.settings.sound_enabled, self.settings.sound_volume);
        }
    }
}

impl eframe::App for EcoView {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = self.last_frame.elapsed().as_secs_f32().clamp(0.0, 0.05);
        self.last_frame = Instant::now();
        self.pulse = (self.pulse + dt).fract();

        // A card drag can outlive the pointer release if the panel that owns it
        // stops running (page switch, or "Edit layout" toggled off while held),
        // which would otherwise re-apply the stale drag against the new cursor.
        if ctx.input(|i| i.pointer.any_released()) {
            self.drag = None;
        }
        if !self.edit_layout {
            self.drag = None;
        }

        let pal = Palette::from_settings(&self.settings);
        pal.apply(ctx, self.settings.text_scale);

        let metrics = self.metrics.lock().map(|m| m.clone()).unwrap_or_default();
        self.check_warnings(&metrics);

        if let Some((_, ttl)) = self.toast.as_mut() {
            *ttl -= dt;
        }
        if self.toast.as_ref().is_some_and(|(_, t)| *t <= 0.0) {
            self.toast = None;
        }

        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(screen, 0.0, pal.bg);
        self.ambient.paint(
            &painter,
            screen,
            pal,
            self.settings.particles,
            self.settings.particle_intensity,
            dt,
        );

        egui::TopBottomPanel::top("nav")
            .exact_height(56.0)
            .frame(egui::Frame::new().fill(pal.panel).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("eco view")
                            .size(20.0)
                            .color(pal.accent)
                            .strong(),
                    );
                    ui.add_space(12.0);
                    self.nav_btn(ui, pal, Page::Overview, Icon::Overview, "Overview");
                    self.nav_btn(ui, pal, Page::Performance, Icon::Perf, "Performance");
                    self.nav_btn(ui, pal, Page::Processes, Icon::Processes, "Processes");
                    self.nav_btn(ui, pal, Page::Storage, Icon::Storage, "Storage");
                    self.nav_btn(ui, pal, Page::Settings, Icon::Settings, "Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.page == Page::Overview
                            && ui
                                .add(egui::Button::new(if self.edit_layout {
                                    "Lock layout"
                                } else {
                                    "Edit layout"
                                }))
                                .clicked()
                        {
                            self.edit_layout = !self.edit_layout;
                            self.sound.play(Cue::Click);
                        }
                        ui.label(
                            egui::RichText::new(&metrics.snap.hostname)
                                .color(pal.muted)
                                .small(),
                        );
                    });
                });
            });

        if let Some(banner) = self.banner.clone()
            && self.settings.show_banner
        {
            egui::TopBottomPanel::top("banner")
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(pal.warn.r(), pal.warn.g(), pal.warn.b(), 40))
                        .inner_margin(8.0),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(banner).color(pal.warn));
                        if ui.button("Dismiss").clicked() {
                            self.banner = None;
                        }
                    });
                });
        }

        // Card rects are also the raccoon's walkable platforms. Rebuild this every
        // frame so it never lags a page switch and the NPC never walks on
        // cards that are no longer on screen.
        self.platforms.clear();
        let mut collected = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 10)))
            .show(ctx, |ui| {
                match self.page {
                    Page::Overview => self.overview(ui, pal, &metrics, &mut collected),
                    Page::Performance => self.performance(ui, pal, &metrics),
                    Page::Processes => self.processes(ui, pal, &metrics),
                    Page::Storage => self.storage(ui, pal, &metrics),
                    Page::Settings => self.settings_page(ui, pal, ctx),
                }
            });
        self.platforms = collected;

        if let Some((msg, _)) = self.toast.clone()
            && self.settings.show_toasts
        {
            egui::Area::new(egui::Id::new("toast"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -18.0])
                // Purely informational: don't let it steal clicks from
                // whatever card happens to sit underneath.
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(pal.panel)
                        .stroke(egui::Stroke::new(1.0_f32, pal.accent))
                        .corner_radius(10.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.label(msg);
                        });
                });
        }

        let enabled = self.npc.lock().map(|n| n.enabled).unwrap_or(false);
        let toggled = self.chip.paint(ctx, enabled, pal.accent);
        if toggled {
            self.sound.play(Cue::Accent);
            if let Ok(mut n) = self.npc.lock() {
                n.enabled = !n.enabled;
            }
        }
        if toggled || self.chip.settled() {
            self.persist();
        }

        self.drive_overlay(ctx, &metrics, dt);

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

impl EcoView {
    fn nav_btn(&mut self, ui: &mut egui::Ui, pal: Palette, page: Page, icon: Icon, label: &str) {
        let selected = self.page == page;
        let fill = if selected {
            pal.accent.gamma_multiply(0.2)
        } else {
            Color32::TRANSPARENT
        };
        let btn = egui::Frame::new()
            .fill(fill)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, p) = ui.allocate_painter(Vec2::splat(16.0), Sense::hover());
                    icons::paint(&p, r.rect.min, 16.0, icon, if selected { pal.accent } else { pal.muted });
                    ui.label(
                        egui::RichText::new(label).color(if selected { pal.accent } else { pal.text }),
                    );
                });
            })
            .response
            .interact(Sense::click());
        if btn.clicked() {
            self.page = page;
            self.sound.play(Cue::Click);
        }
    }

    fn overview(
        &mut self,
        ui: &mut egui::Ui,
        pal: Palette,
        metrics: &Metrics,
        platforms: &mut Vec<Rect>,
    ) {
        let avail = ui.available_size();
        let col_w = (avail.x / COLS as f32).max(8.0);
        let row_h = 22.0;
        let max_row = self.settings.layout.max_row();
        let (full, _) = ui.allocate_exact_size(
            Vec2::new(avail.x, (max_row as f32 * row_h).max(avail.y)),
            Sense::hover(),
        );
        let origin = full.min;
        let visible: Vec<CardId> = self.settings.visible.clone();
        for item in self.settings.layout.items.clone() {
            if !visible.contains(&item.id) {
                continue;
            }
            let rect = Rect::from_min_size(
                Pos2::new(
                    origin.x + item.col as f32 * col_w,
                    origin.y + item.row as f32 * row_h,
                ),
                Vec2::new(item.w as f32 * col_w - 8.0, item.h as f32 * row_h - 8.0),
            );
            self.draw_card(ui, pal, metrics, item.id, rect);
            platforms.push(rect);
            if self.edit_layout {
                self.layout_handles(ui, item.id, rect, col_w, row_h, origin);
            }
        }
    }

    fn layout_handles(
        &mut self,
        ui: &mut egui::Ui,
        id: CardId,
        rect: Rect,
        col_w: f32,
        row_h: f32,
        origin: Pos2,
    ) {
        let move_rect = Rect::from_min_size(rect.min, Vec2::splat(16.0));
        let resize_rect = Rect::from_min_size(rect.max - Vec2::splat(16.0), Vec2::splat(16.0));
        let move_id = ui.make_persistent_id(("mv", id.title()));
        let resize_id = ui.make_persistent_id(("rs", id.title()));
        let move_r = ui.interact(move_rect, move_id, Sense::click_and_drag());
        let resize_r = ui.interact(resize_rect, resize_id, Sense::click_and_drag());
        ui.painter().rect_filled(move_rect, 3.0, Color32::from_rgb(80, 255, 200));
        ui.painter().rect_filled(resize_rect, 3.0, Color32::from_rgb(255, 180, 80));
        if move_r.drag_started() {
            self.drag = Some((id, DragKind::Move, Vec2::ZERO));
        }
        if resize_r.drag_started() {
            self.drag = Some((id, DragKind::Resize, Vec2::ZERO));
        }
        let Some((_, kind, _)) = self.drag.filter(|(drag_id, _, _)| *drag_id == id) else {
            return;
        };
        let Some(pointer) = ui.ctx().pointer_latest_pos() else {
            return;
        };
        let Some(item) = self.settings.layout.items.iter().find(|i| i.id == id).cloned()
        else {
            return;
        };
        // `try_place` reports whether anything actually changed, so this saves
        // once per real edit instead of once per frame of the drag.
        let changed = match kind {
            DragKind::Move => {
                let col = ((pointer.x - origin.x) / col_w).round() as i32;
                let row = ((pointer.y - origin.y) / row_h).round() as i32;
                self.settings.layout.try_place(id, col, row, item.w, item.h)
            }
            DragKind::Resize => {
                // Cards are drawn with an 8px inset, so measure against the
                // card's own top-left rather than the raw grid origin.
                let x0 = origin.x + item.col as f32 * col_w;
                let y0 = origin.y + item.row as f32 * row_h;
                let w = ((pointer.x - x0) / col_w).round() as i32;
                let h = ((pointer.y - y0) / row_h).round() as i32;
                self.settings.layout.try_place(id, item.col, item.row, w, h)
            }
        };
        if changed {
            self.persist();
        }
    }

    fn draw_card(&mut self, ui: &mut egui::Ui, pal: Palette, metrics: &Metrics, id: CardId, rect: Rect) {
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
        ui::card_chrome(ui, pal, id, self.pulse, |ui| match id {
            CardId::Cpu => {
                ui::kpi(ui, pal, format!("{:.0}", metrics.snap.cpu_global), "%");
                ui::meter(ui, metrics.snap.cpu_global / 100.0, pal, ui::warn_value(metrics.snap.cpu_global, self.settings.warn_cpu));
                ui::sparkline(ui, &metrics.history.cpu, 100.0, pal.accent, 36.0);
                if self.settings.cpu_heatmap {
                    ui::heatmap_cores(ui, &metrics.snap.cpu_per_core);
                } else {
                    ui.label(egui::RichText::new(&metrics.snap.cpu_brand).small().color(pal.muted));
                }
            }
            CardId::Memory => {
                let p = metrics::pct(metrics.snap.mem_used, metrics.snap.mem_total);
                ui::kpi(ui, pal, format!("{p:.0}"), "%");
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        format::bytes(metrics.snap.mem_used),
                        format::bytes(metrics.snap.mem_total)
                    ))
                    .small()
                    .color(pal.muted),
                );
                ui::meter(ui, p / 100.0, pal, ui::warn_value(p, self.settings.warn_ram));
                ui::sparkline(ui, &metrics.history.ram, 100.0, pal.accent_alt, 32.0);
                let sw = metrics::pct(metrics.snap.swap_used, metrics.snap.swap_total);
                ui.label(format!("Swap {sw:.0}%"));
            }
            CardId::Gpu => {
                if let Some(load) = metrics.snap.gpu.load {
                    ui::kpi(ui, pal, format!("{load:.0}"), "%");
                    ui::meter(ui, load / 100.0, pal, ui::warn_value(load, self.settings.warn_gpu));
                } else {
                    ui.label(egui::RichText::new("unavailable").color(pal.muted));
                }
                if let Some(name) = &metrics.snap.gpu.name {
                    ui.label(egui::RichText::new(name).small().color(pal.muted));
                }
                match (metrics.snap.gpu.mem_used, metrics.snap.gpu.mem_total) {
                    (Some(u), Some(t)) if t > 0 => {
                        ui.label(format!("{} / {}", format::bytes(u), format::bytes(t)));
                    }
                    _ => {
                        ui.label(egui::RichText::new("GPU memory unavailable").small().color(pal.muted));
                    }
                }
                ui::sparkline(ui, &metrics.history.gpu, 100.0, pal.accent, 28.0);
            }
            CardId::Network => {
                ui.label(format!("↓ {}", format::bps(metrics.snap.net_down_bps)));
                ui.label(format!("↑ {}", format::bps(metrics.snap.net_up_bps)));
                ui::sparkline(ui, &metrics.history.net_down, spark_max(&metrics.history.net_down), pal.ok, 28.0);
                if self.settings.net_heatmap {
                    ui::heatmap_net(ui, metrics.snap.net_down_bps as f32, metrics.snap.net_up_bps as f32, pal);
                }
            }
            CardId::Storage => {
                for disk in metrics.snap.disks.iter().take(3) {
                    let used = disk.total.saturating_sub(disk.available);
                    let p = metrics::pct(used, disk.total);
                    ui.label(format!("{}  {p:.0}%", disk.mount));
                    ui::meter(ui, p / 100.0, pal, p > 90.0);
                }
            }
            CardId::Battery => {
                if let Some(b) = &metrics.snap.battery {
                    ui::kpi(ui, pal, format!("{:.0}", b.percent), "%");
                    ui.label(if b.charging { "Charging" } else { "On battery" });
                    if let Some(r) = &b.remaining {
                        ui.label(egui::RichText::new(r).small().color(pal.muted));
                    }
                    ui::meter(ui, b.percent / 100.0, pal, b.percent < 15.0 && !b.charging);
                } else {
                    ui.label("No battery");
                }
            }
            CardId::Audio => {
                if let Some(v) = metrics.snap.audio.volume {
                    ui::kpi(ui, pal, format!("{:.0}", v * 100.0), "%");
                    ui::meter(ui, v, pal, false);
                    ui::sparkline(ui, &metrics.history.audio, 100.0, pal.accent_alt, 24.0);
                } else {
                    ui.label("Output level unavailable");
                }
                if metrics.snap.audio.muted {
                    ui.label(egui::RichText::new("Muted").color(pal.warn));
                }
                if let Some(d) = &metrics.snap.audio.device {
                    ui.label(egui::RichText::new(d).small().color(pal.muted));
                }
            }
            CardId::Thermal => {
                if let Some(t) = &metrics.snap.temperature {
                    ui::kpi(ui, pal, format!("{:.0}", t.celsius), "°C");
                    ui::meter(ui, (t.celsius / 110.0).clamp(0.0, 1.0), pal, t.celsius >= self.settings.warn_temp);
                    ui.label(egui::RichText::new(&t.source).small().color(pal.muted));
                } else {
                    ui.label("No SMC reading");
                }
            }
            CardId::Uptime => {
                ui::kpi(ui, pal, format::uptime(metrics.snap.uptime_secs), "");
                ui.label(egui::RichText::new(&metrics.snap.os).small().color(pal.muted));
            }
            CardId::Processes => {
                for proc in metrics.snap.processes.iter().take(6) {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{:>5.1}%", proc.cpu)).color(pal.accent).monospace());
                        ui.label(truncate(&proc.name, 22));
                    });
                }
            }
            CardId::Devices => {
                for d in metrics.snap.devices.iter().take(7) {
                    let tag = match d.kind {
                        crate::devices::DeviceKind::Controller => "🎮",
                        crate::devices::DeviceKind::Bluetooth => "bluetooth",
                        crate::devices::DeviceKind::Usb => "usb",
                        crate::devices::DeviceKind::Hid => "hid",
                    };
                    ui.label(format!("{tag}  {}", truncate(&d.name, 28)));
                }
                if metrics.snap.devices.is_empty() {
                    ui.label(egui::RichText::new("Scanning IOKit…").color(pal.muted));
                }
            }
            CardId::System => {
                ui.label(&metrics.snap.hostname);
                ui.label(egui::RichText::new(&metrics.snap.os).small().color(pal.muted));
                ui.label(egui::RichText::new(format!("kernel {}", metrics.snap.kernel)).small().color(pal.muted));
                ui.label(egui::RichText::new(&metrics.snap.cpu_brand).small().color(pal.muted));
                ui.label(format!("{} logical cores · Intel-friendly OpenGL", metrics.snap.cpu_cores));
            }
        });
            },
        );
    }

    fn performance(&mut self, ui: &mut egui::Ui, pal: Palette, metrics: &Metrics) {
        ui.heading("Performance");
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(2, |cols| {
                ui::card_chrome(&mut cols[0], pal, CardId::Cpu, self.pulse, |ui| {
                    ui::kpi(ui, pal, format!("{:.1}", metrics.snap.cpu_global), "%");
                    ui::sparkline(ui, &metrics.history.cpu, 100.0, pal.accent, 70.0);
                    ui::heatmap_cores(ui, &metrics.snap.cpu_per_core);
                });
                ui::card_chrome(&mut cols[1], pal, CardId::Memory, self.pulse, |ui| {
                    let p = metrics::pct(metrics.snap.mem_used, metrics.snap.mem_total);
                    ui::kpi(ui, pal, format!("{p:.1}"), "%");
                    ui::sparkline(ui, &metrics.history.ram, 100.0, pal.accent_alt, 70.0);
                    ui.label(format!(
                        "{} used of {}",
                        format::bytes(metrics.snap.mem_used),
                        format::bytes(metrics.snap.mem_total)
                    ));
                });
            });
            ui.add_space(8.0);
            ui.columns(2, |cols| {
                ui::card_chrome(&mut cols[0], pal, CardId::Gpu, self.pulse, |ui| {
                    ui.label(metrics.snap.gpu.name.clone().unwrap_or_else(|| "GPU".into()));
                    match metrics.snap.gpu.load {
                        Some(_v) => ui::sparkline(ui, &metrics.history.gpu, 100.0, pal.accent, 64.0),
                        None => {
                            ui.label("IOAccelerator did not publish utilization.");
                        }
                    }
                    let _ = metrics.snap.gpu.load;
                });
                ui::card_chrome(&mut cols[1], pal, CardId::Network, self.pulse, |ui| {
                    ui.label(format!("Down {}", format::bps(metrics.snap.net_down_bps)));
                    ui::sparkline(ui, &metrics.history.net_down, spark_max(&metrics.history.net_down), pal.ok, 48.0);
                    ui.label(format!("Up {}", format::bps(metrics.snap.net_up_bps)));
                    ui::sparkline(ui, &metrics.history.net_up, spark_max(&metrics.history.net_up), pal.warn, 48.0);
                });
            });
        });
    }

    fn processes(&mut self, ui: &mut egui::Ui, pal: Palette, metrics: &Metrics) {
        ui.heading("Processes");
        ui.add_space(6.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("procs")
                .striped(true)
                .num_columns(4)
                .min_col_width(80.0)
                .show(ui, |ui| {
                    ui.strong("PID");
                    ui.strong("Name");
                    ui.strong("CPU");
                    ui.strong("Memory");
                    ui.end_row();
                    for proc in &metrics.snap.processes {
                        ui.monospace(proc.pid.to_string());
                        ui.label(&proc.name);
                        ui.colored_label(pal.accent, format!("{:.1}%", proc.cpu));
                        ui.label(format::bytes(proc.memory));
                        ui.end_row();
                    }
                });
        });
    }

    fn storage(&mut self, ui: &mut egui::Ui, pal: Palette, metrics: &Metrics) {
        ui.heading("Storage");
        ui.add_space(8.0);
        for disk in &metrics.snap.disks {
            ui::card_chrome(ui, pal, CardId::Storage, self.pulse, |ui| {
                ui.label(egui::RichText::new(&disk.name).strong());
                ui.label(&disk.mount);
                let used = disk.total.saturating_sub(disk.available);
                ui.label(format!(
                    "{} free of {}",
                    format::bytes(disk.available),
                    format::bytes(disk.total)
                ));
                ui::meter(ui, metrics::pct(used, disk.total) / 100.0, pal, false);
            });
            ui.add_space(8.0);
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui, pal: Palette, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Settings");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Theme");
                if ui.selectable_label(self.settings.theme == ThemeKind::Dark, "Dark").clicked() {
                    self.settings.theme = ThemeKind::Dark;
                    self.persist();
                }
                if ui.selectable_label(self.settings.theme == ThemeKind::Light, "Light").clicked() {
                    self.settings.theme = ThemeKind::Light;
                    self.persist();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Accent");
                if color_edit(ui, &mut self.settings.accent) {
                    self.persist();
                }
                ui.label("Alt");
                if color_edit(ui, &mut self.settings.accent_alt) {
                    self.persist();
                }
            });
            if ui
                .add(egui::Slider::new(&mut self.settings.text_scale, 0.8..=1.4).text("Text scale"))
                .changed()
            {
                self.persist();
            }
            if ui
                .add(egui::Slider::new(&mut self.settings.refresh_ms, 250..=4000).text("Refresh (ms)"))
                .changed()
            {
                self.persist();
            }
            ui.separator();
            ui.label("Warning thresholds");
            let mut thresh_dirty = false;
            thresh_dirty |= ui
                .add(egui::Slider::new(&mut self.settings.warn_cpu, 50.0..=100.0).text("CPU %"))
                .changed();
            thresh_dirty |= ui
                .add(egui::Slider::new(&mut self.settings.warn_ram, 50.0..=100.0).text("RAM %"))
                .changed();
            thresh_dirty |= ui
                .add(egui::Slider::new(&mut self.settings.warn_gpu, 50.0..=100.0).text("GPU %"))
                .changed();
            thresh_dirty |= ui
                .add(egui::Slider::new(&mut self.settings.warn_temp, 50.0..=100.0).text("Temp °C"))
                .changed();
            if thresh_dirty {
                self.persist();
            }
            ui.separator();
            ui.label("Particles");
            ui.horizontal_wrapped(|ui| {
                for style in crate::particles::ParticleStyle::all() {
                    if ui.selectable_label(self.settings.particles == *style, style.label()).clicked() {
                        self.settings.particles = *style;
                        self.persist();
                    }
                }
            });
            if ui
                .add(egui::Slider::new(&mut self.settings.particle_intensity, 0.0..=1.0).text("Intensity"))
                .changed()
            {
                self.persist();
            }
            ui.separator();
            let mut dirty = false;
            dirty |= ui.checkbox(&mut self.settings.show_toasts, "Toasts").changed();
            dirty |= ui.checkbox(&mut self.settings.show_banner, "Warning banner").changed();
            dirty |= ui.checkbox(&mut self.settings.cpu_heatmap, "CPU core heatmap").changed();
            dirty |= ui.checkbox(&mut self.settings.net_heatmap, "Network heatmap").changed();
            dirty |= ui.checkbox(&mut self.settings.sound_enabled, "Sounds").changed();
            if ui
                .add(egui::Slider::new(&mut self.settings.sound_volume, 0.0..=1.0).text("Sound volume"))
                .changed()
            {
                dirty = true;
            }
            if ui.checkbox(&mut self.settings.autostart, "Open at login (LaunchAgent)").changed() {
                match crate::autostart::set_enabled(self.settings.autostart) {
                    Ok(()) => self.toast = Some(("Login item updated".into(), 3.0)),
                    Err(e) => self.toast = Some((e, 4.0)),
                }
                dirty = true;
            }
            ui.separator();
            ui.label("Visible tiles");
            ui.horizontal_wrapped(|ui| {
                for id in CardId::all() {
                    let mut on = self.settings.visible.contains(id);
                    if ui.checkbox(&mut on, id.title()).changed() {
                        if on {
                            if !self.settings.visible.contains(id) {
                                self.settings.visible.push(*id);
                            }
                        } else {
                            self.settings.visible.retain(|v| v != id);
                        }
                        dirty = true;
                    }
                }
            });
            ui.separator();
            ui.label("Discord Rich Presence");
            dirty |= ui.checkbox(&mut self.settings.discord_enabled, "Enable Discord presence").changed();
            dirty |= ui.text_edit_singleline(&mut self.settings.discord_app_id).changed();
            ui.label("Application ID");
            ui.label("Details");
            dirty |= ui.text_edit_singleline(&mut self.settings.discord_details).changed();
            ui.label("State");
            dirty |= ui.text_edit_singleline(&mut self.settings.discord_state).changed();
            ui.label("Large image key");
            dirty |= ui.text_edit_singleline(&mut self.settings.discord_large_image).changed();
            ui.label(egui::RichText::new("{cpu} {ram} {gpu} {temp} {host} {uptime} {network_down} {network_up}").small().color(pal.muted));
            ui.separator();
            ui.label("Custom window icon (PNG, JPG, BMP, ICO)");
            if ui.button("Choose icon…").clicked()
                && let Some(path) = native_open()
            {
                match self.settings.set_icon_from_file(&path) {
                    Ok(()) => {
                        self.toast = Some(("Icon saved. Restart to apply in Dock.".into(), 4.0));
                        dirty = true;
                    }
                    Err(e) => self.toast = Some((e, 4.0)),
                }
            }
            if ui.button("Restore default icon").clicked() {
                self.settings.custom_icon = None;
                dirty = true;
            }
            ui.separator();
            dirty |= self.companion_settings(ui, pal);
            if dirty {
                self.persist();
            }
            let _ = ctx;
        });
    }

    /// The whole companion panel. Returns true when anything changed.
    fn companion_settings(&mut self, ui: &mut egui::Ui, pal: Palette) -> bool {
        use crate::companion::{state_defs, State};
        let mut dirty = false;
        let c = &mut self.settings.companion;

        ui.label(egui::RichText::new("Raccoon").heading());
        ui.label(
            egui::RichText::new(
                "Click the chip on the dashboard to let him out. He floats above every other \
                 app, so you can drag him anywhere and throw him.",
            )
            .small()
            .color(pal.muted),
        );
        dirty |= ui.checkbox(&mut c.enabled, "Companion enabled").changed();

        // ---- live state readout -------------------------------------------
        if let Ok(mut npc) = self.npc.lock() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "now: {}",
                        crate::companion::def_for(npc.state).label
                    ))
                    .small()
                    .color(pal.accent),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "state {:.1}s · {}",
                        npc.state_t,
                        if npc.on_ground { "grounded" } else { "airborne" }
                    ))
                    .small()
                    .color(pal.muted),
                );
            });
            ui.horizontal(|ui| {
                if ui.button("Flip").clicked() {
                    let t = npc.time;
                    npc.enter(State::Flip, t);
                }
                if ui.button("Dance").clicked() {
                    let t = npc.time;
                    npc.enter(State::Dance, t);
                }
                if ui.button("Panic").clicked() {
                    let t = npc.time;
                    npc.enter(State::Panic, t);
                }
                if ui.button("Cheer").clicked() {
                    let t = npc.time;
                    npc.enter(State::Cheer, t);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Recentre on screen").clicked() {
                    let r = npc.bounds;
                    npc.pos = eframe::egui::Pos2::new(r.center().x, r.bottom() - 60.0);
                    npc.vel = eframe::egui::Vec2::ZERO;
                }
                if ui.button("Toss him").clicked() {
                    npc.vel = eframe::egui::Vec2::new(420.0, -520.0);
                    npc.spin_vel = 1.4;
                    let t = npc.time;
                    npc.enter(State::Thrown, t);
                }
            });
        }

        // ---- presentation -------------------------------------------------
        ui.add_space(6.0);
        if section(ui, pal, "Presentation") {
            dirty |= draggable(ui, &mut c.sprite_scale, 0.4..=5.0, "Sprite scale");
            dirty |= draggable(ui, &mut c.animation_speed, 0.1..=6.0, "Animation speed");
            dirty |= ui.checkbox(&mut c.show_shadow, "Ground shadow").changed();
            dirty |= ui.checkbox(&mut c.show_name_tag, "Name tag").changed();
            dirty |= ui.checkbox(&mut c.flip_sprite_with_facing, "Mirror when turning").changed();

            ui.separator();
            ui.label(egui::RichText::new("Colours").small().color(pal.muted));
            fn pick(ui: &mut egui::Ui, v: &mut [u8; 3]) -> bool {
                ui.color_edit_button_srgb(v).changed()
            }
            egui::Grid::new("raccoon_palette").num_columns(4).show(ui, |ui| {
                let slots: [(&str, &mut [u8; 3]); 7] = [
                    ("Fur", &mut c.fur),
                    ("Dark", &mut c.fur_dark),
                    ("Light", &mut c.fur_light),
                    ("Cream", &mut c.cream),
                    ("Mask", &mut c.mask),
                    ("Iris", &mut c.iris),
                    ("Outline", &mut c.outline),
                ];
                for (i, (label, slot)) in slots.into_iter().enumerate() {
                    if i % 4 == 0 && i > 0 {
                        ui.end_row();
                    }
                    ui.vertical(|ui| {
                        ui.label(label);
                        dirty |= pick(ui, slot);
                    });
                }
            });
        }

        // ---- window -------------------------------------------------------
        if section(ui, pal, "Window") {
            dirty |= ui.checkbox(&mut c.always_on_top, "Float above all apps").changed();
            dirty |= ui
                .checkbox(&mut c.click_through, "Ignore mouse clicks (decorative)")
                .changed();
            dirty |= ui.checkbox(&mut c.follow_cursor, "Follow the cursor").changed();
            ui.label(
                egui::RichText::new(
                    "His window is small and follows him, so clicks beside it still reach \
                     whatever is underneath.",
                )
                .small()
                .color(pal.muted),
            );
        }

        // ---- movement -----------------------------------------------------
        if section(ui, pal, "Movement") {
            dirty |= draggable(ui, &mut c.walk_speed, 0.0..=600.0, "Walk speed");
            dirty |= draggable(ui, &mut c.run_speed, 0.0..=1200.0, "Run speed");
            dirty |= draggable(ui, &mut c.sprint_speed, 0.0..=1800.0, "Sprint speed");
        }

        // ---- physics ------------------------------------------------------
        if section(ui, pal, "Physics") {
            dirty |= draggable(ui, &mut c.gravity, 0.0..=4000.0, "Gravity");
            dirty |= draggable(ui, &mut c.air_drag, 0.0..=8.0, "Air drag");
            dirty |= draggable(ui, &mut c.restitution, 0.0..=1.0, "Bounciness");
            dirty |= draggable(ui, &mut c.ground_friction, 0.0..=1.0, "Ground friction");
            dirty |= draggable(ui, &mut c.wall_restitution, 0.0..=1.0, "Wall bounce");
            dirty |= draggable(ui, &mut c.throw_power, 0.0..=5.0, "Throw power");
            dirty |= draggable(ui, &mut c.toss_strength, 0.0..=1200.0, "Gentle toss");
            dirty |= draggable(ui, &mut c.spin_per_speed, 0.0..=0.02, "Spin per speed");
            dirty |= draggable(ui, &mut c.hard_impact_speed, 50.0..=2000.0, "Hard landing");
            dirty |= ui.checkbox(&mut c.throw_dizzy, "Get dizzy after a bad throw").changed();
        }

        // ---- reactions ----------------------------------------------------
        if section(ui, pal, "Reactions") {
            dirty |= draggable(ui, &mut c.sleep_below_cpu, 0.0..=60.0, "Sleep below CPU %");
            dirty |= draggable(ui, &mut c.sleep_below_gpu, 0.0..=100.0, "Sleep below GPU %");
            dirty |= draggable(ui, &mut c.panic_above_cpu, 0.0..=100.0, "Panic above CPU %");
            dirty |= draggable(ui, &mut c.panic_above_gpu, 0.0..=100.0, "Panic above GPU %");
            ui.horizontal_wrapped(|ui| {
                dirty |= ui.checkbox(&mut c.react_to_cpu, "CPU").changed();
                dirty |= ui.checkbox(&mut c.react_to_gpu, "GPU").changed();
                dirty |= ui.checkbox(&mut c.react_to_memory, "Memory").changed();
                dirty |= ui.checkbox(&mut c.react_to_battery, "Battery").changed();
                dirty |= ui.checkbox(&mut c.react_to_network, "Network").changed();
            });
        }

        // ---- timing -------------------------------------------------------
        if section(ui, pal, "Timing") {
            dirty |= draggable(ui, &mut c.state_time_scale, 0.1..=5.0, "Global time scale");
            dirty |= draggable(ui, &mut c.idle_boredom, 0.0..=4.0, "Boredom");
        }

        // ---- per-state ----------------------------------------------------
        if section(ui, pal, "Per-state tuning") {
            ui.label(
                egui::RichText::new("How long he lasts, how often he picks it, and how long he waits.")
                    .small()
                    .color(pal.muted),
            );
            for d in state_defs() {
                let key = d.label.to_string();
                let mut tune = c.states.get(&key).copied().unwrap_or_default();
                egui::Grid::new(("st", d.state as u32)).num_columns(4).show(ui, |ui| {
                    ui.label(d.label);
                    let r = ui.add(
                        egui::Slider::new(&mut tune.duration_mul, 0.05..=6.0)
                            .suffix("x time")
                            .text(""),
                    );
                    let r2 = ui.add(
                        egui::Slider::new(&mut tune.weight_mul, 0.0..=20.0)
                            .suffix("x chance")
                            .text(""),
                    );
                    let r3 = ui.add(
                        egui::Slider::new(&mut tune.cooldown_mul, 0.0..=8.0)
                            .suffix("x wait")
                            .text(""),
                    );
                    ui.end_row();
                    if r.changed() || r2.changed() || r3.changed() {
                        c.states.insert(key.clone(), tune);
                        dirty = true;
                    }
                });
            }
        }

        if ui.button("Reset companion settings").clicked() {
            self.settings.companion = Default::default();
            dirty = true;
        }
        dirty
    }

    fn check_warnings(&mut self, metrics: &Metrics) {
        if self.last_warn.elapsed().as_secs() < 20 {
            return;
        }
        let snap = &metrics.snap;
        let mut reason = None;
        if snap.cpu_global >= self.settings.warn_cpu {
            reason = Some(format!("CPU at {:.0}%", snap.cpu_global));
        } else if metrics::pct(snap.mem_used, snap.mem_total) >= self.settings.warn_ram {
            reason = Some(format!("Memory at {:.0}%", metrics::pct(snap.mem_used, snap.mem_total)));
        } else if snap.gpu.load.unwrap_or(0.0) >= self.settings.warn_gpu {
            reason = Some(format!("GPU at {:.0}%", snap.gpu.load.unwrap_or(0.0)));
        } else if snap
            .temperature
            .as_ref()
            .is_some_and(|t| t.celsius >= self.settings.warn_temp)
        {
            reason = Some(format!(
                "Thermal {:.0}°C",
                snap.temperature.as_ref().map(|t| t.celsius).unwrap_or(0.0)
            ));
        }
        if let Some(reason) = reason {
            self.banner = Some(reason.clone());
            self.toast = Some((reason.clone(), 5.0));
            self.notify.post(reason);
            self.sound.play(Cue::Warn);
            self.last_warn = Instant::now();
        }
    }
}

fn spark_max(data: &std::collections::VecDeque<f32>) -> f32 {
    data.iter().copied().fold(1.0, f32::max) * 1.15
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn color_edit(ui: &mut egui::Ui, rgb: &mut [u8; 3]) -> bool {
    let mut c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    let changed = ui.color_edit_button_srgba(&mut c).changed();
    if changed {
        *rgb = [c.r(), c.g(), c.b()];
    }
    changed
}

fn native_open() -> Option<std::path::PathBuf> {
    let script = r#"POSIX path of (choose file of type {"public.png","public.jpeg","com.microsoft.bmp","com.apple.icns","public.image"} with prompt "Choose an icon")"#;
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(path))
    }
}

fn apply_icon(cc: &eframe::CreationContext<'_>, settings: &Settings) {
    let Some(path) = &settings.custom_icon else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(img) = image::load_from_memory(&bytes) else {
        return;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let icon = egui::IconData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    };
    cc.egui_ctx
        .send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(icon))));
}

impl EcoView {
    /// Advances the raccoon and keeps its always-on-top window in place.
    ///
    /// The NPC lives in *global screen coordinates* so it can be dragged and
    /// thrown anywhere on the desktop. The dashboard's card rects are in
    /// window-local coordinates, so they are shifted by this window's screen
    /// position to become platforms it can stand on.
    fn drive_overlay(&mut self, ctx: &eframe::egui::Context, metrics: &Metrics, dt: f32) {
        let enabled = self.settings.companion.enabled;

        if !enabled {
            // Nothing to draw. Tear the window down once and get out of the
            // way, rather than doing per-frame bookkeeping for an overlay
            // that is not there.
            if self.overlay_live {
                self.overlay_live = false;
                overlay::close(ctx);
            }
            if let Ok(mut npc) = self.npc.lock() {
                npc.enabled = false;
            }
            return;
        }

        // This window's own offset within the desktop.
        let screen_rect = ctx.input(|i| i.screen_rect());
        let screen_origin = screen_rect.min.to_vec2();

        // Dashboard cards become walkable platforms. They are in window-local
        // coordinates, so shift them into global screen space.
        if self.platforms_changed() {
            let platforms: Vec<Rect> = self
                .platforms
                .iter()
                .map(|r| r.translate(screen_origin))
                .collect();
            if let Ok(mut npc) = self.npc.lock() {
                npc.platforms = platforms;
            }
        }

        let cfg = self.settings.companion.clone();
        if let Ok(mut npc) = self.npc.lock() {
            npc.enabled = true;
            npc.scale = cfg.sprite_scale;
            npc.bounds = screen_rect;
            // Always give him the bottom of the display to stand on. Without
            // it, pages with no cards leave him with nothing to land on, so he
            // could never come to rest or walk anywhere.
            npc.floor = Some(screen_rect.bottom());
            // Place him on the first run, otherwise he starts at the origin.
            if !npc.placed {
                npc.placed = true;
                npc.pos = Pos2::new(screen_rect.center().x, screen_rect.bottom() - 40.0);
            }
            npc.apply_view(&cfg);
            npc.repaint(&cfg);
            npc.tick(&metrics.snap, &cfg, dt);
        }

        self.overlay_live = true;
        let npc = self.npc.clone();
        let events = &mut self.overlay_events;
        ctx.show_viewport_immediate(
            overlay::viewport_id(),
            overlay::builder(),
            move |vctx, _class| {
                overlay::ui(vctx, &npc, events);
                overlay::sync(vctx, &npc);
            },
        );
        if self.overlay_events.grabbed {
            self.overlay_events.grabbed = false;
            self.sound.play(Cue::Click);
        }
        if self.overlay_events.released {
            self.overlay_events.released = false;
            if self.overlay_events.throw.length() > 300.0 {
                self.sound.play(Cue::Accent);
            }
        }
    }

    /// Cheap check so the platform Vec is only rebuilt when cards actually move.
    fn platforms_changed(&mut self) -> bool {
        if self.platforms != self.platforms_cached {
            self.platforms_cached = self.platforms.clone();
            return true;
        }
        false
    }
}


/// A collapsible sub-panel header. Returns true while open.
fn section(ui: &mut egui::Ui, pal: Palette, title: &str) -> bool {
    ui.collapsing(egui::RichText::new(title).strong().color(pal.accent), |ui| {
        ui.add_space(4.0);
    })
    .body_response
    .is_some()
}

/// A labelled slider that marks the settings dirty when dragged.
fn draggable(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) -> bool {
    let r = ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(value, range).text(""))
    });
    r.inner.changed()
}
