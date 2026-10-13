//! The dashboard: a health summary and a grid of cards the user can arrange.

use eframe::egui::{self, Align, Align2, Color32, Id, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{DragKind, EcoView, Page};
use crate::charts::{Chart, Series};
use crate::devices::DeviceKind;
use crate::format;
use crate::grid::{COLS, CardId};
use crate::icons::{self, Icon};
use crate::metrics;
use crate::theme::{self, Palette};
use crate::widgets::{self, Badge, ButtonKind, Header};

const ROW_H: f32 = 24.0;
const GAP: f32 = 12.0;

impl EcoView {
    pub(crate) fn overview(&mut self, ui: &mut Ui, cards: &mut Vec<Rect>) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.hero(ui);
            ui.add_space(GAP);
            if self.edit_layout {
                self.edit_bar(ui);
            }
            self.grid(ui, cards);
            ui.add_space(8.0);
        });
    }

    fn hero(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let (score, verdict, level) = self.health();
        let snap = self.m.snap.clone();
        let s = &snap;
        let mem = metrics::pct(s.mem_used, s.mem_total);
        let gpu = s.gpu.load;
        let disk = s.disks.first().map(|d| metrics::pct(d.total.saturating_sub(d.available), d.total));
        let width = ui.available_width();
        let h = 168.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        let p = ui.painter();
        widgets::card_surface(p, &pal, rect, false);
        // A wash of the health colour from the left.
        let c = level.color(&pal);
        let wash = Rect::from_min_size(rect.min, vec2(rect.width() * 0.55, rect.height()));
        let mut mesh = egui::Mesh::default();
        let a = pal.tint(c, if pal.dark { 0.07 } else { 0.05 });
        for (pt, col) in [(wash.left_top(), a), (wash.right_top(), Color32::TRANSPARENT), (wash.right_bottom(), Color32::TRANSPARENT), (wash.left_bottom(), a)] {
            mesh.colored_vertex(pt, col);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.with_clip_rect(rect.shrink(1.0)).add(egui::Shape::mesh(mesh));

        // Score.
        let sc = pos2(rect.left() + 86.0, rect.center().y);
        let f = widgets::tween(ui.ctx(), Id::new("hero-score"), score / 100.0, 0.7);
        widgets::ring(p, &pal, sc, 54.0, 10.0, f, c);
        p.text(sc - vec2(0.0, 4.0), Align2::CENTER_CENTER, format!("{:.0}", f * 100.0), theme::bold(34.0), pal.text);
        p.text(sc + vec2(0.0, 24.0), Align2::CENTER_CENTER, "HEALTH", theme::semibold(9.5), pal.muted);

        let tx = rect.left() + 170.0;
        p.text(pos2(tx, rect.top() + 34.0), Align2::LEFT_CENTER, verdict, theme::bold(22.0), pal.text);
        let summary = self.summary_line();
        p.text(pos2(tx, rect.top() + 62.0), Align2::LEFT_CENTER, summary, theme::regular(13.0), pal.text2);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(tx, rect.top() + 90.0), vec2(360.0, 40.0))).layout(Layout::left_to_right(Align::Center)));
        if widgets::button(&mut child, &pal, ButtonKind::Secondary, Some(Icon::Processes), "Top processes").clicked() {
            self.go(Page::Processes);
        }
        if widgets::button(&mut child, &pal, ButtonKind::Ghost, Some(Icon::Perf), "History").clicked() {
            self.go(Page::Performance);
        }

        // Gauges, right-aligned, as many as fit.
        let mut gauges: Vec<(&str, f32, String)> = vec![("CPU", s.cpu_global, format!("{:.0}%", s.cpu_global)), ("MEMORY", mem, format!("{mem:.0}%"))];
        if let Some(g) = gpu {
            gauges.push(("GPU", g, format!("{g:.0}%")));
        }
        if let Some(d) = disk {
            gauges.push(("DISK", d, format!("{d:.0}%")));
        }
        let gs = 104.0;
        let room = rect.right() - (tx + 380.0);
        let fit = ((room / (gs + 14.0)).floor() as usize).min(gauges.len());
        for (i, (label, v, text)) in gauges.iter().take(fit).enumerate() {
            let x = rect.right() - 24.0 - (fit - i) as f32 * (gs + 14.0) + 14.0;
            let gr = Rect::from_min_size(pos2(x, rect.center().y - gs * 0.5), Vec2::splat(gs));
            let mut gu = ui.new_child(egui::UiBuilder::new().max_rect(gr));
            widgets::ring_gauge(&mut gu, &pal, Id::new(("hero-g", *label)), gs, v / 100.0, pal.level(v / 100.0), text, label);
        }
    }

    fn summary_line(&self) -> String {
        let s = &self.m.snap;
        let mut parts = vec![
            format!("CPU {:.0}%", s.cpu_global),
            format!("Memory {:.0}%", metrics::pct(s.mem_used, s.mem_total)),
        ];
        if let Some(t) = &s.temperature {
            parts.push(format!("{:.0}°C", t.celsius));
        }
        if let Some(b) = &s.battery {
            parts.push(format!("Battery {:.0}%{}", b.percent, if b.charging { ", charging" } else { "" }));
        }
        parts.push(format!("up {}", format::uptime(s.uptime_secs)));
        parts.join("  ·  ")
    }

    fn edit_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        egui::Frame::new()
            .fill(pal.tint(pal.accent, 0.10))
            .stroke(Stroke::new(1.0_f32, pal.tint(pal.accent, 0.35)))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    icons::paint_in(ui.painter(), r, Icon::Layout, pal.accent);
                    ui.label(RichText::new("Editing layout").font(theme::semibold(13.0)).color(pal.text));
                    widgets::caption(ui, &pal, "Drag a card's grip to move it, its corner to resize. Cards can't overlap.");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::button(ui, &pal, ButtonKind::Primary, Some(Icon::Check), "Done").clicked() {
                            self.edit_layout = false;
                        }
                        if widgets::button(ui, &pal, ButtonKind::Ghost, Some(Icon::Refresh), "Reset").clicked() {
                            self.settings.layout = crate::grid::GridLayout::default_dashboard();
                            self.persist();
                        }
                    });
                });
            });
        ui.add_space(GAP);
    }

    fn grid(&mut self, ui: &mut Ui, cards: &mut Vec<Rect>) {
        let pal = self.pal;
        let width = ui.available_width();
        let col_w = ((width + GAP) / COLS as f32).max(6.0);
        let max_row = self.settings.layout.max_row() + if self.edit_layout { 6 } else { 0 };
        let (full, _) = ui.allocate_exact_size(vec2(width, max_row as f32 * ROW_H), Sense::hover());
        let origin = full.min;
        if self.edit_layout {
            for r in 0..=max_row {
                for c in (0..=COLS).step_by(2) {
                    ui.painter().circle_filled(pos2(origin.x + c as f32 * col_w - GAP * 0.5, origin.y + r as f32 * ROW_H - GAP * 0.5), 1.1, pal.border_hi);
                }
            }
        }
        let visible = self.settings.visible.clone();
        for item in self.settings.layout.items.clone() {
            if !visible.contains(&item.id) {
                continue;
            }
            let rect = Rect::from_min_size(
                pos2(origin.x + item.col as f32 * col_w, origin.y + item.row as f32 * ROW_H),
                vec2(item.w as f32 * col_w - GAP, item.h as f32 * ROW_H - GAP),
            );
            self.card(ui, item.id, rect);
            cards.push(rect);
            if self.edit_layout {
                self.handles(ui, item.id, rect, col_w, origin);
            }
        }
    }

    fn handles(&mut self, ui: &mut Ui, id: CardId, rect: Rect, col_w: f32, origin: Pos2) {
        let pal = self.pal;
        let p = ui.painter();
        // Dashed outline.
        let mut x = rect.left() + 10.0;
        while x < rect.right() - 10.0 {
            p.line_segment([pos2(x, rect.top()), pos2((x + 5.0).min(rect.right()), rect.top())], Stroke::new(1.5_f32, pal.accent));
            p.line_segment([pos2(x, rect.bottom()), pos2((x + 5.0).min(rect.right()), rect.bottom())], Stroke::new(1.5_f32, pal.accent));
            x += 10.0;
        }
        let grip = Rect::from_center_size(pos2(rect.right() - 20.0, rect.top() + 20.0), Vec2::splat(26.0));
        let corner = Rect::from_min_size(rect.max - Vec2::splat(22.0), Vec2::splat(22.0));
        let gr = ui.interact(grip, Id::new(("mv", id.title())), Sense::drag());
        let cr = ui.interact(corner, Id::new(("rs", id.title())), Sense::drag());
        let p = ui.painter();
        p.circle_filled(grip.center(), 13.0, if gr.hovered() || gr.dragged() { pal.accent } else { pal.tint(pal.accent, 0.8) });
        for dx in [-3.5, 3.5] {
            for dy in [-4.5, 0.0, 4.5] {
                p.circle_filled(grip.center() + vec2(dx, dy), 1.4, pal.on_accent);
            }
        }
        let tri = vec![corner.right_top() + vec2(-2.0, 4.0), corner.right_bottom() + vec2(-2.0, -2.0), corner.left_bottom() + vec2(4.0, -2.0)];
        p.add(egui::Shape::convex_polygon(tri, if cr.hovered() || cr.dragged() { pal.accent } else { pal.tint(pal.accent, 0.7) }, Stroke::NONE));
        if gr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if cr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        if gr.drag_started() {
            let off = ui.ctx().pointer_latest_pos().map(|p| p - rect.min).unwrap_or(Vec2::ZERO);
            self.drag = Some((id, DragKind::Move, off));
        }
        if cr.drag_started() {
            self.drag = Some((id, DragKind::Resize, Vec2::ZERO));
        }
        let Some((_, kind, off)) = self.drag.filter(|(d, _, _)| *d == id) else { return };
        let Some(pointer) = ui.ctx().pointer_latest_pos() else { return };
        let Some(item) = self.settings.layout.items.iter().find(|i| i.id == id).copied() else { return };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let changed = match kind {
            DragKind::Move => {
                let tl = pointer - off;
                let col = ((tl.x - origin.x) / col_w).round() as i32;
                let row = ((tl.y - origin.y) / ROW_H).round() as i32;
                self.settings.layout.try_place(id, col, row, item.w, item.h)
            }
            DragKind::Resize => {
                let x0 = origin.x + item.col as f32 * col_w;
                let y0 = origin.y + item.row as f32 * ROW_H;
                let w = ((pointer.x - x0 + GAP) / col_w).round() as i32;
                let h = ((pointer.y - y0 + GAP) / ROW_H).round() as i32;
                self.settings.layout.try_place(id, item.col, item.row, w, h)
            }
        };
        if changed {
            self.persist();
        }
    }

    fn card(&mut self, ui: &mut Ui, id: CardId, rect: Rect) {
        let pal = self.pal;
        // Borrowed, not cloned: this runs for every card on every frame.
        let s = &self.m.snap;
        let h = &self.m.history;
        let mem = metrics::pct(s.mem_used, s.mem_total);
        let (tint, badge) = match id {
            CardId::Cpu => (pal.accent, Some(Badge::new(format!("{} cores", s.cpu_cores), pal.text2))),
            CardId::Memory => (pal.accent_alt, Some(Badge::new(format::bytes(s.mem_total), pal.text2))),
            CardId::Gpu => (pal.info, s.gpu.load.map(|_| Badge::new("live", pal.ok))),
            CardId::Network => (pal.ok, None),
            CardId::Storage => (pal.warn, Some(Badge::new(format!("{} volumes", s.disks.len()), pal.text2))),
            CardId::Battery => (pal.ok, s.battery.as_ref().map(|b| if b.charging { Badge::new("charging", pal.ok) } else { Badge::new("on battery", pal.text2) })),
            CardId::Audio => (pal.accent_alt, s.audio.muted.then(|| Badge::new("muted", pal.crit))),
            CardId::Thermal => (pal.crit, None),
            CardId::Uptime => (pal.info, None),
            CardId::Processes => (pal.accent, Some(Badge::new(s.process_count.to_string(), pal.text2))),
            CardId::Devices => (pal.info, Some(Badge::new(s.devices.len().to_string(), pal.text2))),
            CardId::System => (pal.text2, None),
        };
        let header = Header::new(id.title(), Icon::for_card(id), tint).badge(badge);
        let step = self.step_secs();
        let edit = self.edit_layout;
        let mut open: Option<Page> = None;
        let resp = widgets::card_at(ui, &pal, rect, &header, |ui| {
            let room = ui.available_height();
            match id {
                CardId::Cpu => {
                    ui.horizontal(|ui| {
                        widgets::big_number(ui, &pal, Id::new("cpu-big"), s.cpu_global, 0, "%", 30.0, pal.text);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            widgets::caption(ui, &pal, format!("load {:.2}", s.load_avg[0]));
                        });
                    });
                    let bars = self.settings.cpu_heatmap && room > 120.0;
                    let chart_h = (room - 46.0 - if bars { 36.0 } else { 0.0 }).max(24.0);
                    Chart::new("cpu-spark", chart_h, &|v| format!("{v:.0}%")).max(100.0).window(90, step).spark().show(ui, &pal, &[Series { name: "CPU", values: &h.cpu, color: pal.accent, fill: true }]);
                    if bars {
                        ui.add_space(6.0);
                        widgets::core_bars(ui, &pal, &s.cpu_per_core, 28.0);
                    }
                }
                CardId::Memory => {
                    ui.horizontal(|ui| {
                        widgets::big_number(ui, &pal, Id::new("mem-big"), mem, 0, "%", 30.0, pal.text);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            widgets::caption(ui, &pal, format!("{} used", format::bytes(s.mem_used)));
                        });
                    });
                    let free = 1.0 - mem / 100.0;
                    widgets::stacked_bar(ui, &pal, &[(mem / 100.0, pal.level(mem / 100.0)), (free.max(0.0), pal.tint(pal.text, 0.06))], 8.0);
                    ui.add_space(4.0);
                    let chart_h = (room - 76.0).max(20.0);
                    Chart::new("mem-spark", chart_h, &|v| format!("{v:.0}%")).max(100.0).window(90, step).spark().show(ui, &pal, &[Series { name: "Memory", values: &h.ram, color: pal.accent_alt, fill: true }]);
                    let sw = metrics::pct(s.swap_used, s.swap_total);
                    widgets::caption(ui, &pal, format!("Swap {} ({sw:.0}%) · {} free", format::bytes(s.swap_used), format::bytes(s.mem_total.saturating_sub(s.mem_used))));
                }
                CardId::Gpu => match s.gpu.load {
                    Some(load) => {
                        widgets::big_number(ui, &pal, Id::new("gpu-big"), load, 0, "%", 30.0, pal.text);
                        if let (Some(u), Some(t)) = (s.gpu.mem_used, s.gpu.mem_total)
                            && t > 0
                        {
                            widgets::bar(ui, &pal, Id::new("vram"), u as f32 / t as f32, pal.info, 8.0);
                            widgets::caption(ui, &pal, format!("VRAM {} of {}", format::bytes(u), format::bytes(t)));
                        }
                        let chart_h = (room - 92.0).max(20.0);
                        Chart::new("gpu-spark", chart_h, &|v| format!("{v:.0}%")).max(100.0).window(90, step).spark().show(ui, &pal, &[Series { name: "GPU", values: &h.gpu, color: pal.info, fill: true }]);
                        widgets::caption(ui, &pal, s.gpu.name.clone().unwrap_or_default());
                    }
                    None => {
                        ui.add_space(6.0);
                        ui.label(RichText::new(s.gpu.name.clone().unwrap_or_else(|| "Graphics".into())).font(theme::semibold(15.0)).color(pal.text));
                        widgets::caption(ui, &pal, "This GPU doesn't publish utilisation through IOKit.");
                    }
                },
                CardId::Network => {
                    ui.horizontal(|ui| {
                        rate(ui, &pal, Icon::ArrowDown, "Download", s.net_down_bps, pal.ok);
                        ui.add_space(18.0);
                        rate(ui, &pal, Icon::ArrowUp, "Upload", s.net_up_bps, pal.accent_alt);
                    });
                    let chart_h = (room - 84.0).max(20.0);
                    Chart::new("net-spark", chart_h, &|v| format::bps(v as f64)).window(90, step).spark().show(
                        ui,
                        &pal,
                        &[
                            Series { name: "Down", values: &h.net_down, color: pal.ok, fill: true },
                            Series { name: "Up", values: &h.net_up, color: pal.accent_alt, fill: false },
                        ],
                    );
                    widgets::caption(ui, &pal, format!("This session: {} down · {} up", format::bytes(s.net_session_down), format::bytes(s.net_session_up)));
                }
                CardId::Storage => {
                    if s.disks.is_empty() {
                        widgets::caption(ui, &pal, "Reading volumes…");
                    }
                    let rows = ((room - 4.0) / 42.0).floor().max(1.0) as usize;
                    for (i, d) in s.disks.iter().take(rows).enumerate() {
                        let used = d.total.saturating_sub(d.available);
                        let pc = metrics::pct(used, d.total);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(crate::app::truncate(if d.name.is_empty() { &d.mount } else { &d.name }, 22)).font(theme::medium(12.5)).color(pal.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::caption(ui, &pal, format!("{} free", format::bytes(d.available)));
                            });
                        });
                        widgets::bar(ui, &pal, Id::new(("disk", i)), pc / 100.0, pal.level(pc / 100.0), 7.0);
                        ui.add_space(4.0);
                    }
                }
                CardId::Battery => match &s.battery {
                    Some(b) => {
                        ui.horizontal(|ui| {
                            let size = (room - 6.0).clamp(60.0, 110.0);
                            let c = if b.percent < 20.0 && !b.charging { pal.crit } else { pal.ok };
                            widgets::ring_gauge(ui, &pal, Id::new("bat-ring"), size, b.percent / 100.0, c, &format!("{:.0}%", b.percent), if b.charging { "CHARGING" } else { "BATTERY" });
                            ui.vertical(|ui| {
                                ui.add_space(8.0);
                                ui.label(RichText::new(if b.charging { "Plugged in" } else { "On battery" }).font(theme::semibold(14.0)).color(pal.text));
                                if let Some(r) = &b.remaining {
                                    widgets::caption(ui, &pal, if b.charging { format!("{r} until full") } else { format!("{r} remaining") });
                                }
                                ui.add_space(4.0);
                                Chart::new("bat-spark", 30.0, &|v| format!("{v:.0}%")).max(100.0).window(300, step).spark().show(ui, &pal, &[Series { name: "Battery", values: &h.battery, color: pal.ok, fill: true }]);
                            });
                        });
                    }
                    None => widgets::empty_state(ui, &pal, Icon::Bolt, "On mains power", "No battery in this Mac."),
                },
                CardId::Audio => match s.audio.volume {
                    Some(v) => {
                        ui.horizontal(|ui| {
                            widgets::big_number(ui, &pal, Id::new("vol-big"), v * 100.0, 0, "%", 24.0, if s.audio.muted { pal.muted } else { pal.text });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let room = (ui.available_width() / 7.0) as usize;
                                widgets::caption(ui, &pal, crate::app::truncate(s.audio.device.as_deref().unwrap_or("Output"), room.max(6)));
                            });
                        });
                        widgets::bar(ui, &pal, Id::new("vol"), v, if s.audio.muted { pal.muted } else { pal.accent_alt }, 7.0);
                    }
                    None => {
                        widgets::caption(ui, &pal, "Output level unavailable");
                    }
                },
                CardId::Thermal => match &s.temperature {
                    Some(t) => {
                        let c = pal.level((t.celsius - 40.0) / 60.0);
                        ui.horizontal(|ui| {
                            widgets::big_number(ui, &pal, Id::new("temp-big"), t.celsius, 0, "°C", 24.0, c);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::caption(ui, &pal, &t.source);
                            });
                        });
                        widgets::bar(ui, &pal, Id::new("temp"), t.celsius / 110.0, c, 7.0);
                    }
                    None => {
                        widgets::caption(ui, &pal, "This Mac's SMC doesn't expose a temperature sensor.");
                    }
                },
                CardId::Uptime => {
                    ui.label(RichText::new(format::uptime(s.uptime_secs)).font(theme::semibold(22.0)).color(pal.text));
                    widgets::caption(ui, &pal, format!("since {}", format::boot_time(s.boot_time)));
                }
                CardId::Processes => {
                    let rows = ((room - 4.0) / 28.0).floor().max(1.0) as usize;
                    let total = s.mem_total.max(1) as f32;
                    for pr in s.processes.iter().take(rows) {
                        ui.horizontal(|ui| {
                            avatar(ui, &pal, &pr.name, 20.0);
                            ui.label(RichText::new(crate::app::truncate(&pr.name, 24)).color(pal.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::cell_bar(ui, &pal, pr.cpu / 100.0, pal.level(pr.cpu / 100.0), &format!("{:.1}%", pr.cpu), 70.0);
                                let _ = total;
                            });
                        });
                    }
                }
                CardId::Devices => {
                    if s.devices.is_empty() {
                        widgets::caption(ui, &pal, "Scanning IOKit…");
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            for d in &s.devices {
                                device_chip(ui, &pal, &d.name, d.kind);
                            }
                        });
                    }
                }
                CardId::System => {
                    widgets::kv(ui, &pal, "Model", if s.model.is_empty() { "Mac".into() } else { s.model.clone() });
                    if room > 40.0 {
                        widgets::kv(ui, &pal, "macOS", s.os.replace("macOS ", ""));
                    }
                    if room > 64.0 {
                        widgets::kv(ui, &pal, "Chip", crate::app::truncate(&s.cpu_brand.replace("(R)", "").replace("(TM)", ""), 28));
                    }
                }
            }
        });
        if resp.hovered() && !edit {
            let target = match id {
                CardId::Cpu | CardId::Memory | CardId::Gpu | CardId::Network | CardId::Thermal => Some(Page::Performance),
                CardId::Processes => Some(Page::Processes),
                CardId::Storage => Some(Page::Storage),
                CardId::Devices => Some(Page::Devices),
                CardId::System | CardId::Uptime => Some(Page::System),
                _ => None,
            };
            if let Some(t) = target {
                // Header click jumps to the detail page.
                let head = Rect::from_min_size(rect.min, vec2(rect.width(), 40.0));
                let hr = ui.interact(head, Id::new(("card-open", id.title())), Sense::click());
                if hr.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    icons::paint_in(ui.painter(), Rect::from_center_size(pos2(rect.right() - 16.0, rect.top() + 24.0), Vec2::splat(12.0)), Icon::ChevronRight, pal.muted);
                }
                if hr.clicked() {
                    open = Some(t);
                }
            }
        }
        if let Some(p) = open {
            self.go(p);
        }
    }
}

fn rate(ui: &mut Ui, pal: &Palette, icon: Icon, label: &str, bps: f64, c: Color32) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
        ui.painter().rect_filled(r, 8.0, pal.tint(c, 0.15));
        icons::paint_in(ui.painter(), r.shrink(6.0), icon, c);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(format::bps(bps)).font(theme::semibold(17.0)).color(pal.text));
            ui.label(RichText::new(label).size(10.5).color(pal.muted));
        });
    });
}

/// A coloured initial, so process lists are easy to scan.
pub(crate) fn avatar(ui: &mut Ui, pal: &Palette, name: &str, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let hue = (hash % 360) as f32 / 360.0;
    let c: Color32 = egui::ecolor::Hsva::new(hue, if pal.dark { 0.55 } else { 0.6 }, if pal.dark { 0.85 } else { 0.75 }, 1.0).into();
    ui.painter().rect_filled(r, size * 0.3, pal.tint(c, 0.2));
    let ch = name.chars().find(|c| c.is_alphanumeric()).unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(r.center(), Align2::CENTER_CENTER, ch, theme::bold(size * 0.52), c);
}

pub(crate) fn device_icon(kind: DeviceKind) -> Icon {
    match kind {
        DeviceKind::Usb => Icon::Usb,
        DeviceKind::Bluetooth => Icon::Bluetooth,
        DeviceKind::Hid => Icon::Keyboard,
        DeviceKind::Controller => Icon::Gamepad,
    }
}

fn device_chip(ui: &mut Ui, pal: &Palette, name: &str, kind: DeviceKind) {
    let text = crate::app::truncate(name, 26);
    let g = ui.painter().layout_no_wrap(text, theme::medium(12.0), pal.text);
    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 40.0, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 14.0, pal.inset);
    p.rect_stroke(r, 14.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
    icons::paint_in(p, Rect::from_center_size(pos2(r.left() + 16.0, r.center().y), Vec2::splat(14.0)), device_icon(kind), pal.info);
    p.galley(pos2(r.left() + 30.0, r.center().y - g.size().y * 0.5), g, pal.text);
}
