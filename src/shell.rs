//! The chrome around the pages: sidebar, header, banner, toasts, alert
//! centre, confirmation dialog and the command palette.

use eframe::egui::{
    self, Align, Align2, Color32, Id, Key, Layout, Order, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2, ViewportCommand, pos2, vec2,
};

use crate::app::{EcoView, KillRequest, Level, Page, SettingsSection};
use crate::format;
use crate::icons::{self, Icon};
use crate::metrics;
use crate::settings::ThemeKind;
use crate::sound::Cue;
use crate::theme::{self, lerp_color};
use crate::widgets::{self, ButtonKind};

const SIDEBAR_W: f32 = 228.0;
const SIDEBAR_MINI: f32 = 70.0;

impl EcoView {
    // ------------------------------------------------------------------
    // Sidebar
    // ------------------------------------------------------------------

    pub(crate) fn sidebar(&mut self, ctx: &egui::Context) {
        let pal = self.pal;
        let target = if self.settings.sidebar_collapsed { SIDEBAR_MINI } else { SIDEBAR_W };
        let w = widgets::tween(ctx, Id::new("sidebar-w"), target, 0.2);
        let mini = w < 150.0;
        egui::SidePanel::left("sidebar")
            .exact_width(w)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(pal.tint(pal.sidebar, if pal.dark { 0.92 } else { 0.96 })).inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 14 }))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.painter().line_segment(
                    [pos2(full.right() + 12.0, full.top()), pos2(full.right() + 12.0, full.bottom() + 14.0)],
                    Stroke::new(1.0_f32, pal.border),
                );
                // Room for the traffic lights, and a handle to drag the window by.
                drag_region(ui, 40.0);

                // Brand.
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                    let x = if mini { (ui.max_rect().width() - 34.0) * 0.5 } else { 0.0 };
                    let r = r.translate(vec2(x, 0.0));
                    let p = ui.painter();
                    p.add(egui::Shadow { offset: [0, 4], blur: 12, spread: 0, color: pal.tint(pal.accent, 0.35) }.as_shape(r, 10));
                    let mut mesh = egui::Mesh::default();
                    for (pt, c) in [(r.left_top(), pal.accent), (r.right_top(), lerp_color(pal.accent, pal.accent_alt, 0.5)), (r.right_bottom(), pal.accent_alt), (r.left_bottom(), lerp_color(pal.accent, pal.accent_alt, 0.5))] {
                        mesh.colored_vertex(pt, c);
                    }
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    // Round the tile by drawing the gradient clipped into a rounded rect.
                    p.rect_filled(r, 10.0, pal.accent);
                    p.with_clip_rect(r.shrink(2.0)).add(Shape::mesh(mesh));
                    icons::paint_in(p, r.shrink(8.0), Icon::Leaf, pal.on_accent);
                    if !mini {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(RichText::new("eco view").font(theme::bold(16.0)).color(pal.text));
                            ui.label(RichText::new("System monitor").size(11.0).color(pal.muted));
                        });
                    }
                });
                ui.add_space(18.0);

                let groups: [(&str, &[Page]); 2] = [
                    ("Monitor", &[Page::Overview, Page::Performance, Page::Processes]),
                    ("Hardware", &[Page::Storage, Page::Devices, Page::System]),
                ];
                for (title, pages) in groups {
                    if !mini {
                        ui.label(RichText::new(title.to_uppercase()).font(theme::semibold(10.0)).color(pal.muted));
                    } else {
                        ui.add_space(6.0);
                    }
                    ui.add_space(2.0);
                    for &p in pages {
                        let badge = match p {
                            Page::Processes => Some(self.m.snap.process_count.to_string()),
                            Page::Devices if !self.m.snap.devices.is_empty() => Some(self.m.snap.devices.len().to_string()),
                            _ => None,
                        };
                        self.nav_item(ui, p, mini, badge);
                    }
                    ui.add_space(14.0);
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    // Collapse toggle.
                    ui.horizontal(|ui| {
                        if mini {
                            ui.add_space((ui.available_width() - 30.0) * 0.5);
                        }
                        let icon = if mini { Icon::ChevronRight } else { Icon::Sidebar };
                        if widgets::icon_button(ui, &pal, icon, if mini { "Expand sidebar  ⌘B" } else { "Collapse sidebar  ⌘B" }, false).clicked() {
                            self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
                            self.persist();
                        }
                        if !mini {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(&self.m.snap.hostname).size(11.0).color(pal.muted));
                            });
                        }
                    });
                    ui.add_space(8.0);
                    self.health_widget(ui, mini);
                    ui.add_space(10.0);
                    self.nav_item(ui, Page::Settings, mini, None);
                });
            });
    }

    fn nav_item(&mut self, ui: &mut Ui, page: Page, mini: bool, badge: Option<String>) {
        let pal = self.pal;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
        let selected = self.page == page;
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let sel = ui.ctx().animate_bool_with_time(resp.id.with("sel"), selected, 0.18);
        let p = ui.painter();
        if sel > 0.0 {
            p.rect_filled(rect, 10.0, pal.tint(pal.accent, 0.15 * sel));
            let bar = Rect::from_min_size(pos2(rect.left() - 12.0, rect.center().y - 9.0 * sel), vec2(3.0, 18.0 * sel));
            p.rect_filled(bar, 2.0, pal.accent);
        } else if hover > 0.0 {
            p.rect_filled(rect, 10.0, pal.tint(pal.text, 0.06 * hover));
        }
        let fg = lerp_color(lerp_color(pal.text2, pal.text, hover), pal.accent, sel);
        let icon_x = if mini { rect.center().x } else { rect.left() + 20.0 };
        icons::paint_in(p, Rect::from_center_size(pos2(icon_x, rect.center().y), Vec2::splat(18.0)), page.icon(), fg);
        if !mini {
            p.text(pos2(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, page.title(), theme::medium(13.5), lerp_color(pal.text, pal.accent, sel));
            let idx = Page::ALL.iter().position(|q| *q == page).unwrap_or(0);
            if hover > 0.5 && idx < 6 {
                p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, format!("⌘{}", idx + 1), theme::regular(11.0), pal.muted);
            } else if page == Page::Settings && hover > 0.5 {
                p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, "⌘,", theme::regular(11.0), pal.muted);
            } else if let Some(b) = badge {
                let g = p.layout_no_wrap(b, theme::semibold(10.5), pal.text2);
                let br = Rect::from_min_size(pos2(rect.right() - 10.0 - g.size().x - 12.0, rect.center().y - 9.0), vec2(g.size().x + 12.0, 18.0));
                p.rect_filled(br, 8.0, if selected { pal.tint(pal.accent, 0.22) } else { pal.tint(pal.text, 0.08) });
                p.galley(br.min + vec2(6.0, 9.0 - g.size().y * 0.5), g, if selected { pal.accent } else { pal.text2 });
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let resp = if mini { resp.on_hover_text(page.title()) } else { resp };
        if resp.clicked() {
            self.go(page);
        }
    }

    fn health_widget(&mut self, ui: &mut Ui, mini: bool) {
        let pal = self.pal;
        let (score, verdict, level) = self.health();
        let color = level.color(&pal);
        let h = if mini { 50.0 } else { 64.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        let p = ui.painter();
        if !mini {
            p.rect_filled(rect, 12.0, pal.surface);
            p.rect_stroke(rect, 12.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
        }
        let c = if mini { rect.center() } else { pos2(rect.left() + 32.0, rect.center().y) };
        let f = widgets::tween(ui.ctx(), resp.id, score / 100.0, 0.6);
        widgets::ring(p, &pal, c, 17.0, 4.5, f, color);
        p.text(c, Align2::CENTER_CENTER, format!("{:.0}", score), theme::semibold(12.0), pal.text);
        if !mini {
            p.text(pos2(rect.left() + 60.0, rect.center().y - 9.0), Align2::LEFT_CENTER, "System health", theme::semibold(12.5), pal.text);
            p.text(pos2(rect.left() + 60.0, rect.center().y + 9.0), Align2::LEFT_CENTER, verdict, theme::regular(11.5), color);
        }
        if resp.clicked() {
            self.go(Page::Overview);
        }
        resp.on_hover_text(format!("Health {score:.0}/100 · {verdict}"));
    }

    // ------------------------------------------------------------------
    // Header
    // ------------------------------------------------------------------

    pub(crate) fn header(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 58.0));
        let bg = ui.interact(rect, Id::new("header-drag"), Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if bg.double_clicked() {
            let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
        }
        let subtitle = self.subtitle();
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(self.page.title()).font(theme::bold(24.0)).color(pal.text));
                ui.label(RichText::new(subtitle).size(12.5).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // Alerts.
                let bell = widgets::icon_button(ui, &pal, Icon::Bell, "Alerts", self.alerts_open);
                if self.unread > 0 {
                    let c = bell.rect.right_top() + vec2(-6.0, 6.0);
                    ui.painter().circle_filled(c, 7.0, pal.crit);
                    ui.painter().text(c, Align2::CENTER_CENTER, self.unread.min(9).to_string(), theme::bold(9.0), Color32::WHITE);
                }
                if bell.clicked() {
                    self.alerts_open = !self.alerts_open;
                    self.unread = 0;
                }
                // Command palette launcher.
                let (r, resp) = ui.allocate_exact_size(vec2(170.0, 30.0), Sense::click());
                let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                let p = ui.painter();
                p.rect_filled(r, 9.0, lerp_color(pal.inset, pal.surface_hi, hov));
                p.rect_stroke(r, 9.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
                icons::paint_in(p, Rect::from_center_size(pos2(r.left() + 15.0, r.center().y), Vec2::splat(14.0)), Icon::Search, pal.muted);
                p.text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, "Search or jump to…", theme::regular(12.0), pal.muted);
                let kr = Rect::from_min_size(pos2(r.right() - 32.0, r.center().y - 9.0), vec2(26.0, 18.0));
                p.rect_filled(kr, 5.0, pal.tint(pal.text, 0.07));
                p.text(kr.center(), Align2::CENTER_CENTER, "⌘K", theme::medium(10.0), pal.text2);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.palette_open = true;
                    self.palette_query.clear();
                    self.palette_sel = 0;
                }

                // Live status pills, as room allows.
                if ui.available_width() > 360.0 {
                    let s = &self.m.snap;
                    if let Some(b) = &s.battery {
                        let c = if b.percent < 20.0 && !b.charging { pal.crit } else { pal.ok };
                        let icon = if b.charging { Icon::Bolt } else { Icon::Battery };
                        widgets::status_pill(ui, &pal, icon, &format!("{:.0}%", b.percent), c, Some(b.percent / 100.0));
                    }
                    if let Some(t) = &s.temperature
                        && ui.available_width() > 260.0
                    {
                        let c = pal.level((t.celsius - 40.0) / 60.0);
                        widgets::status_pill(ui, &pal, Icon::Thermal, &format!("{:.0}°C", t.celsius), c, Some(t.celsius / 110.0));
                    }
                    if ui.available_width() > 200.0 {
                        let mem = metrics::pct(s.mem_used, s.mem_total);
                        widgets::status_pill(ui, &pal, Icon::Memory, &format!("{mem:.0}%"), pal.level(mem / 100.0), Some(mem / 100.0));
                    }
                    if ui.available_width() > 140.0 {
                        widgets::status_pill(ui, &pal, Icon::Cpu, &format!("{:.0}%", s.cpu_global), pal.level(s.cpu_global / 100.0), Some(s.cpu_global / 100.0));
                    }
                }
            });
        });
        ui.add_space(14.0);
    }

    fn subtitle(&self) -> String {
        let s = &self.m.snap;
        let up = format::uptime(s.uptime_secs);
        match self.page {
            Page::Overview => format!("{}. Here's {} right now.", self.greeting(), if s.hostname.is_empty() { "your Mac" } else { &s.hostname }),
            Page::Performance => format!("Live history · sampling every {:.1}s", self.step_secs()),
            Page::Processes => format!("{} running · sorted by {}", s.process_count, match self.proc_sort {
                crate::app::ProcSort::Cpu => "CPU",
                crate::app::ProcSort::Memory => "memory",
                crate::app::ProcSort::Name => "name",
                crate::app::ProcSort::Pid => "PID",
            }),
            Page::Storage => {
                let total: u64 = s.disks.iter().map(|d| d.total).sum();
                format!("{} volumes · {} total", s.disks.len(), format::bytes(total))
            }
            Page::Devices => format!("{} connected · USB, Bluetooth and input devices", s.devices.len()),
            Page::System => format!("{} · {} · up {up}", if s.model.is_empty() { "Mac" } else { &s.model }, s.os),
            Page::Settings => "Make eco view yours. Changes save automatically.".into(),
        }
    }

    pub(crate) fn banner_strip(&mut self, ui: &mut Ui) {
        let Some((level, text, page)) = self.banner.clone() else { return };
        let pal = self.pal;
        let c = level.color(&pal);
        let mut close = false;
        let mut view = false;
        egui::Frame::new()
            .fill(pal.tint(c, 0.12))
            .stroke(Stroke::new(1.0_f32, pal.tint(c, 0.35)))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    icons::paint_in(ui.painter(), r, level.icon(), c);
                    ui.label(RichText::new(text).font(theme::medium(13.0)).color(pal.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, &pal, Icon::Close, "Dismiss", false).clicked() {
                            close = true;
                        }
                        if widgets::button(ui, &pal, ButtonKind::Ghost, None, "View").clicked() {
                            view = true;
                        }
                    });
                });
            });
        ui.add_space(12.0);
        if view {
            self.go(page);
            close = true;
        }
        if close {
            self.banner = None;
        }
    }

    // ------------------------------------------------------------------
    // Toasts
    // ------------------------------------------------------------------

    pub(crate) fn toasts_layer(&mut self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        let pal = self.pal;
        let now = self.time;
        let mut dismiss = None;
        egui::Area::new(Id::new("toasts"))
            .order(Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, [-20.0, -20.0])
            .show(ctx, |ui| {
                ui.set_width(330.0);
                for t in self.toasts.iter().rev() {
                    let age = (now - t.born) as f32;
                    let enter = (age / 0.25).clamp(0.0, 1.0);
                    let leave = ((t.ttl - age) / 0.35).clamp(0.0, 1.0);
                    let a = (1.0 - (1.0 - enter).powi(3)).min(leave);
                    ui.set_opacity(a);
                    let c = t.level.color(&pal);
                    let resp = egui::Frame::new()
                        .fill(pal.surface_hi)
                        .stroke(Stroke::new(1.0_f32, pal.border_hi))
                        .corner_radius(12.0)
                        .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: pal.shadow })
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(306.0);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                                ui.painter().rect_filled(r, 9.0, pal.tint(c, 0.16));
                                icons::paint_in(ui.painter(), r.shrink(7.0), t.level.icon(), c);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(RichText::new(&t.title).font(theme::semibold(13.0)).color(pal.text));
                                    if !t.body.is_empty() {
                                        ui.label(RichText::new(&t.body).size(12.0).color(pal.text2));
                                    }
                                });
                            });
                        })
                        .response;
                    // Time-left line.
                    let left = (1.0 - age / t.ttl).clamp(0.0, 1.0);
                    let r = resp.rect;
                    let line = Rect::from_min_size(pos2(r.left() + 12.0, r.bottom() - 3.0), vec2((r.width() - 24.0) * left, 2.0));
                    ui.painter().rect_filled(line, 1.0, pal.tint(c, 0.6));
                    if ui.interact(r, Id::new(("toast", t.id)), Sense::click()).clicked() {
                        dismiss = Some(t.id);
                    }
                    ui.add_space(8.0);
                }
            });
        if let Some(id) = dismiss {
            self.toasts.retain(|t| t.id != id);
        }
        ctx.request_repaint();
    }

    // ------------------------------------------------------------------
    // Alert centre
    // ------------------------------------------------------------------

    pub(crate) fn alerts_popover(&mut self, ctx: &egui::Context) {
        if !self.alerts_open {
            return;
        }
        let pal = self.pal;
        let screen = ctx.screen_rect();
        let mut go = None;
        let mut clear = false;
        let resp = egui::Area::new(Id::new("alerts-popover"))
            .order(Order::Foreground)
            .fixed_pos(pos2(screen.right() - 24.0 - 380.0, 72.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.surface_hi)
                    .stroke(Stroke::new(1.0_f32, pal.border_hi))
                    .corner_radius(14.0)
                    .shadow(egui::Shadow { offset: [0, 14], blur: 40, spread: 0, color: pal.shadow })
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(352.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Alerts").font(theme::semibold(15.0)).color(pal.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if !self.alerts.is_empty() && widgets::button(ui, &pal, ButtonKind::Ghost, None, "Clear").clicked() {
                                    clear = true;
                                }
                            });
                        });
                        ui.add_space(6.0);
                        if self.alerts.is_empty() {
                            widgets::empty_state(ui, &pal, Icon::Check, "All quiet", "You'll see threshold alerts here.");
                            return;
                        }
                        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            for (i, a) in self.alerts.iter().enumerate() {
                                let c = a.level.color(&pal);
                                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
                                let p = ui.painter();
                                if resp.hovered() {
                                    p.rect_filled(r, 10.0, pal.tint(pal.text, 0.05));
                                }
                                let ir = Rect::from_center_size(pos2(r.left() + 20.0, r.center().y), Vec2::splat(28.0));
                                p.rect_filled(ir, 8.0, pal.tint(c, 0.16));
                                icons::paint_in(p, ir.shrink(7.0), a.level.icon(), c);
                                p.text(pos2(r.left() + 44.0, r.center().y - 9.0), Align2::LEFT_CENTER, &a.title, theme::semibold(12.5), pal.text);
                                p.text(pos2(r.left() + 44.0, r.center().y + 9.0), Align2::LEFT_CENTER, &a.body, theme::regular(11.5), pal.text2);
                                p.text(pos2(r.right() - 6.0, r.center().y - 9.0), Align2::RIGHT_CENTER, a.when.format("%H:%M").to_string(), theme::regular(11.0), pal.muted);
                                if resp.clicked() {
                                    go = Some(a.page);
                                }
                                if i + 1 < self.alerts.len() {
                                    widgets::separator(ui, &pal);
                                }
                            }
                        });
                    });
            })
            .response;
        if clear {
            self.alerts.clear();
        }
        if let Some(p) = go {
            self.go(p);
            self.alerts_open = false;
        }
        // Click outside closes it (the bell itself toggles).
        if ctx.input(|i| i.pointer.any_click())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && !resp.rect.contains(pos)
            && pos.y > 70.0
        {
            self.alerts_open = false;
        }
    }

    // ------------------------------------------------------------------
    // Quit-process confirmation
    // ------------------------------------------------------------------

    pub(crate) fn kill_modal(&mut self, ctx: &egui::Context) {
        let Some(req) = &self.kill else { return };
        let pal = self.pal;
        let (pid, name, force) = (req.pid, req.name.clone(), req.force);
        let mut decision: Option<bool> = None;
        dim_layer(ctx, &pal, "kill-dim");
        egui::Area::new(Id::new("kill-modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, [0.0, -40.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.surface_hi)
                    .stroke(Stroke::new(1.0_f32, pal.border_hi))
                    .corner_radius(16.0)
                    .shadow(egui::Shadow { offset: [0, 20], blur: 50, spread: 0, color: pal.shadow })
                    .inner_margin(egui::Margin::same(22))
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                        ui.painter().rect_filled(r, 12.0, pal.tint(pal.crit, 0.15));
                        icons::paint_in(ui.painter(), r.shrink(10.0), Icon::Warning, pal.crit);
                        ui.add_space(8.0);
                        let verb = if force { "Force quit" } else { "Quit" };
                        ui.label(RichText::new(format!("{verb} “{}”?", crate::app::truncate(&name, 36))).font(theme::semibold(17.0)).color(pal.text));
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(if force {
                                format!("PID {pid} will be stopped immediately. Any unsaved work in it will be lost.")
                            } else {
                                format!("PID {pid} will be asked to quit. Apps may still ask you to save first.")
                            })
                            .size(12.5)
                            .color(pal.text2),
                        );
                        ui.add_space(16.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button(ui, &pal, ButtonKind::Danger, None, verb).clicked() {
                                decision = Some(true);
                            }
                            if widgets::button(ui, &pal, ButtonKind::Secondary, None, "Cancel").clicked() {
                                decision = Some(false);
                            }
                        });
                    });
            });
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            decision = Some(true);
        }
        match decision {
            Some(true) => {
                self.kill = None;
                let ok = pid != std::process::id() && signal(pid, force);
                if ok {
                    self.toast(Level::Ok, format!("Sent {} to {}", if force { "SIGKILL" } else { "SIGTERM" }, name), format!("PID {pid}"));
                } else {
                    self.toast(Level::Crit, "Couldn't quit that process", "It may belong to another user or the system.");
                }
            }
            Some(false) => self.kill = None,
            None => {}
        }
    }

    pub(crate) fn ask_kill(&mut self, pid: u32, name: &str, force: bool) {
        self.kill = Some(KillRequest { pid, name: name.to_string(), force });
    }

    // ------------------------------------------------------------------
    // Command palette
    // ------------------------------------------------------------------

    pub(crate) fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        let pal = self.pal;
        let all = self.commands();
        let q = self.palette_query.to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let hits: Vec<&Command> = all
            .iter()
            .filter(|c| {
                let hay = format!("{} {}", c.title, c.group).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            self.palette_sel = (self.palette_sel + 1).min(hits.len().saturating_sub(1));
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            self.palette_sel = self.palette_sel.saturating_sub(1);
        }
        self.palette_sel = self.palette_sel.min(hits.len().saturating_sub(1));
        let mut run: Option<Action> = None;
        if ctx.input(|i| i.key_pressed(Key::Enter))
            && let Some(c) = hits.get(self.palette_sel)
        {
            run = Some(c.action);
        }

        if dim_layer(ctx, &pal, "palette-dim") {
            self.palette_open = false;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("palette"))
            .order(Order::Foreground)
            .fixed_pos(pos2(screen.center().x - 300.0, screen.top() + screen.height() * 0.14))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.surface_hi)
                    .stroke(Stroke::new(1.0_f32, pal.border_hi))
                    .corner_radius(16.0)
                    .shadow(egui::Shadow { offset: [0, 24], blur: 60, spread: 0, color: pal.shadow })
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(580.0);
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            icons::paint_in(ui.painter(), r, Icon::Search, pal.muted);
                            let te = ui.add(
                                egui::TextEdit::singleline(&mut self.palette_query)
                                    .hint_text(RichText::new("Type a command or a page…").color(pal.muted))
                                    .frame(false)
                                    .font(theme::regular(16.0))
                                    .desired_width(500.0)
                                    .margin(egui::Margin::symmetric(4, 8)),
                            );
                            te.request_focus();
                            if te.changed() {
                                self.palette_sel = 0;
                            }
                        });
                        widgets::separator(ui, &pal);
                        ui.add_space(4.0);
                        if hits.is_empty() {
                            widgets::empty_state(ui, &pal, Icon::Search, "No matches", "Try a page name, a theme, or “raccoon”.");
                            return;
                        }
                        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                            let mut last_group = "";
                            for (i, c) in hits.iter().enumerate() {
                                if c.group != last_group {
                                    last_group = c.group;
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(c.group.to_uppercase()).font(theme::semibold(10.0)).color(pal.muted));
                                }
                                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                                let sel = i == self.palette_sel;
                                if resp.hovered() && ui.ctx().input(|i| i.pointer.delta().length() > 0.0) {
                                    self.palette_sel = i;
                                }
                                let p = ui.painter();
                                if sel {
                                    p.rect_filled(r, 8.0, pal.tint(pal.accent, 0.16));
                                    if !self.settings.reduce_motion {
                                        ui.scroll_to_rect(r, None);
                                    }
                                }
                                icons::paint_in(p, Rect::from_center_size(pos2(r.left() + 18.0, r.center().y), Vec2::splat(16.0)), c.icon, if sel { pal.accent } else { pal.text2 });
                                p.text(pos2(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, &c.title, theme::medium(13.5), pal.text);
                                if !c.hint.is_empty() {
                                    p.text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, &c.hint, theme::regular(11.5), pal.muted);
                                }
                                if resp.clicked() {
                                    run = Some(c.action);
                                }
                            }
                        });
                        ui.add_space(4.0);
                        widgets::separator(ui, &pal);
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            widgets::caption(ui, &pal, "Arrow keys to move · Return to run · Esc to close");
                        });
                    });
            });
        if let Some(a) = run {
            self.palette_open = false;
            self.run(a, ctx);
        }
    }

    fn commands(&self) -> Vec<Command> {
        let mut v = Vec::new();
        for (i, p) in Page::ALL.iter().enumerate() {
            let hint = if i < 6 { format!("⌘{}", i + 1) } else { "⌘,".into() };
            v.push(Command { group: "Go to", title: p.title().into(), hint, icon: p.icon(), action: Action::Go(*p) });
        }
        let raccoon_on = self.settings.companion.enabled;
        v.push(Command { group: "Raccoon", title: if raccoon_on { "Put the raccoon away".into() } else { "Let the raccoon out".into() }, hint: String::new(), icon: Icon::Paw, action: Action::ToggleRaccoon });
        if raccoon_on {
            v.push(Command { group: "Raccoon", title: "Call the raccoon over".into(), hint: String::new(), icon: Icon::Paw, action: Action::CallRaccoon });
        }
        v.push(Command { group: "Raccoon", title: "Raccoon settings".into(), hint: String::new(), icon: Icon::Settings, action: Action::Section(SettingsSection::Raccoon) });
        v.push(Command { group: "Dashboard", title: if self.edit_layout { "Finish editing layout".into() } else { "Edit dashboard layout".into() }, hint: "⌘E".into(), icon: Icon::Layout, action: Action::EditLayout });
        v.push(Command { group: "Dashboard", title: "Reset dashboard layout".into(), hint: String::new(), icon: Icon::Refresh, action: Action::ResetLayout });
        v.push(Command { group: "View", title: if self.settings.sidebar_collapsed { "Expand sidebar".into() } else { "Collapse sidebar".into() }, hint: "⌘B".into(), icon: Icon::Sidebar, action: Action::ToggleSidebar });
        v.push(Command { group: "View", title: if self.settings.compact { "Comfortable spacing".into() } else { "Compact spacing".into() }, hint: String::new(), icon: Icon::Sliders, action: Action::ToggleCompact });
        v.push(Command { group: "View", title: if self.settings.reduce_motion { "Turn animations on".into() } else { "Reduce motion".into() }, hint: String::new(), icon: Icon::Sparkle, action: Action::ToggleMotion });
        for t in ThemeKind::ALL {
            let cur = if t == self.settings.theme { "current" } else { "" };
            v.push(Command { group: "Theme", title: format!("{} theme", t.label()), hint: cur.into(), icon: if t.is_dark() { Icon::Moon } else { Icon::Sun }, action: Action::Theme(t) });
        }
        v.push(Command { group: "Actions", title: if self.settings.sound_enabled { "Mute sounds".into() } else { "Unmute sounds".into() }, hint: String::new(), icon: Icon::Audio, action: Action::ToggleSounds });
        v.push(Command { group: "Actions", title: "Copy system specs".into(), hint: String::new(), icon: Icon::Copy, action: Action::CopySpecs });
        v.push(Command { group: "Actions", title: "Clear alerts".into(), hint: String::new(), icon: Icon::Bell, action: Action::ClearAlerts });
        for (s, name) in [
            (SettingsSection::Appearance, "Appearance"),
            (SettingsSection::Alerts, "Alert thresholds"),
            (SettingsSection::Dashboard, "Dashboard cards"),
            (SettingsSection::Discord, "Discord presence"),
            (SettingsSection::Sound, "Sounds"),
        ] {
            v.push(Command { group: "Settings", title: name.into(), hint: String::new(), icon: Icon::Settings, action: Action::Section(s) });
        }
        v
    }

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::Go(p) => self.go(p),
            Action::Theme(t) => {
                self.settings.theme = t;
                self.persist();
            }
            Action::ToggleSidebar => {
                self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
                self.persist();
            }
            Action::ToggleCompact => {
                self.settings.compact = !self.settings.compact;
                self.persist();
            }
            Action::ToggleMotion => {
                self.settings.reduce_motion = !self.settings.reduce_motion;
                self.persist();
            }
            Action::ToggleSounds => {
                self.settings.sound_enabled = !self.settings.sound_enabled;
                self.persist();
            }
            Action::ToggleRaccoon => {
                self.settings.companion.enabled = !self.settings.companion.enabled;
                self.persist();
            }
            Action::CallRaccoon => {
                if let Ok(mut w) = self.raccoon.world.lock() {
                    let at = ctx.input(|i| i.viewport().inner_rect).map(|r| r.center()).unwrap_or(w.area.center());
                    w.summon(at);
                }
            }
            Action::EditLayout => {
                self.go(Page::Overview);
                self.edit_layout = !self.edit_layout;
            }
            Action::ResetLayout => {
                self.settings.layout = crate::grid::GridLayout::default_dashboard();
                self.persist();
                self.toast(Level::Ok, "Layout reset", "The dashboard is back to its default arrangement.");
            }
            Action::CopySpecs => {
                ctx.copy_text(self.specs_text());
                self.toast(Level::Ok, "Copied", "System specs are on the clipboard.");
            }
            Action::ClearAlerts => {
                self.alerts.clear();
                self.unread = 0;
                self.banner = None;
            }
            Action::Section(s) => {
                self.go(Page::Settings);
                self.section = s;
            }
        }
        self.sound.play(Cue::Click);
    }
}

struct Command {
    group: &'static str,
    title: String,
    hint: String,
    icon: Icon,
    action: Action,
}

#[derive(Clone, Copy)]
enum Action {
    Go(Page),
    Theme(ThemeKind),
    ToggleSidebar,
    ToggleCompact,
    ToggleMotion,
    ToggleSounds,
    ToggleRaccoon,
    CallRaccoon,
    EditLayout,
    ResetLayout,
    CopySpecs,
    ClearAlerts,
    Section(SettingsSection),
}

/// A full-window dimmer that blocks clicks to the UI below. Returns true when
/// it was clicked (outside whatever sits on top of it).
fn dim_layer(ctx: &egui::Context, pal: &crate::theme::Palette, id: &str) -> bool {
    let screen = ctx.screen_rect();
    let mut clicked = false;
    egui::Area::new(Id::new(id)).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
        let resp = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, 0.0, if pal.dark { Color32::from_black_alpha(140) } else { Color32::from_black_alpha(70) });
        clicked = resp.clicked();
    });
    clicked
}

/// Lets the user drag the window by an empty strip (the title bar is hidden).
fn drag_region(ui: &mut Ui, height: f32) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click_and_drag());
    let _ = r;
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
    }
}

fn signal(pid: u32, force: bool) -> bool {
    let sig = if force { libc::SIGKILL } else { libc::SIGTERM };
    // SAFETY: plain kill(2) on a pid the user picked and confirmed.
    unsafe { libc::kill(pid as libc::pid_t, sig) == 0 }
}
