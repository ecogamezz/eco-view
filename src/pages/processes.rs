//! A searchable, sortable process table with a details panel.

use eframe::egui::{self, Align, Align2, Id, Layout, Rect, RichText, ScrollArea, Sense, Ui, Vec2, pos2, vec2};

use crate::app::{EcoView, ProcSort};
use crate::format;
use crate::icons::{self, Icon};
use crate::metrics::ProcInfo;
use crate::pages::overview::avatar;
use crate::theme;
use crate::widgets::{self, ButtonKind};

const ROW: f32 = 34.0;

impl EcoView {
    pub(crate) fn processes(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        ui.horizontal(|ui| {
            widgets::search_field(ui, &pal, &mut self.proc_query, "Filter by name or PID", 260.0);
            ui.add_space(6.0);
            let mut sort = self.proc_sort;
            if widgets::segmented(ui, &pal, &mut sort, &[(ProcSort::Cpu, "CPU"), (ProcSort::Memory, "Memory"), (ProcSort::Name, "Name"), (ProcSort::Pid, "PID")]) {
                self.proc_sort = sort;
                self.proc_desc = matches!(sort, ProcSort::Cpu | ProcSort::Memory);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let total_cpu: f32 = self.m.snap.processes.iter().map(|p| p.cpu).sum();
                widgets::pill(ui, &pal, &format!("Total CPU {:.0}%", total_cpu / self.m.snap.cpu_cores.max(1) as f32), pal.accent);
            });
        });
        ui.add_space(12.0);

        let mut list: Vec<ProcInfo> = self.m.snap.processes.clone();
        let q = self.proc_query.trim().to_lowercase();
        if !q.is_empty() {
            list.retain(|p| p.name.to_lowercase().contains(&q) || p.pid.to_string().starts_with(&q));
        }
        match self.proc_sort {
            ProcSort::Cpu => list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            ProcSort::Memory => list.sort_by(|a, b| b.memory.cmp(&a.memory)),
            ProcSort::Name => list.sort_by_key(|p| p.name.to_lowercase()),
            ProcSort::Pid => list.sort_by_key(|p| p.pid),
        }
        let natural_desc = matches!(self.proc_sort, ProcSort::Cpu | ProcSort::Memory);
        if self.proc_desc != natural_desc {
            list.reverse();
        }

        let selected = self.proc_selected.and_then(|pid| self.m.snap.processes.iter().find(|p| p.pid == pid).cloned());
        let show_details = selected.is_some() && ui.available_width() > 820.0;
        let avail = ui.available_size();
        let table_w = if show_details { avail.x - 300.0 } else { avail.x };

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(table_w, avail.y), Layout::top_down(Align::Min), |ui| {
                self.table(ui, &list);
            });
            if show_details && let Some(p) = &selected {
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(vec2(286.0, avail.y), Layout::top_down(Align::Min), |ui| {
                    self.details(ui, p);
                });
            }
        });
    }

    fn table(&mut self, ui: &mut Ui, list: &[ProcInfo]) {
        let pal = self.pal;
        let total_mem = self.m.snap.mem_total.max(1) as f32;
        let rect = ui.available_rect_before_wrap();
        let bg = ui.painter().add(egui::Shape::Noop);
        let w = rect.width();
        // Column layout: avatar+name | pid | cpu | memory
        let cols = [w - 392.0, 80.0, 140.0, 140.0];
        let x = |i: usize| rect.left() + 16.0 + cols[..i].iter().sum::<f32>();

        // Header.
        let (hr, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
        let heads = [("NAME", ProcSort::Name), ("PID", ProcSort::Pid), ("CPU", ProcSort::Cpu), ("MEMORY", ProcSort::Memory)];
        for (i, (label, sort)) in heads.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(x(i), hr.top()), vec2(cols[i] - 16.0, hr.height()));
            let r = ui.interact(cell, Id::new(("ph", i)), Sense::click());
            let active = self.proc_sort == *sort;
            let color = if active { pal.accent } else if r.hovered() { pal.text } else { pal.muted };
            let align = if i == 0 { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
            let at = if i == 0 { cell.left_center() + vec2(i as f32, 0.0) } else { cell.right_center() };
            let tr = ui.painter().text(at, align, *label, theme::semibold(10.5), color);
            if active {
                let arrow = if self.proc_desc { Icon::ChevronDown } else { Icon::ArrowUp };
                let ax = if i == 0 { tr.right() + 10.0 } else { tr.left() - 10.0 };
                icons::paint_in(ui.painter(), Rect::from_center_size(pos2(ax, cell.center().y), Vec2::splat(10.0)), arrow, color);
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                if active {
                    self.proc_desc = !self.proc_desc;
                } else {
                    self.proc_sort = *sort;
                    self.proc_desc = matches!(sort, ProcSort::Cpu | ProcSort::Memory);
                }
            }
        }
        widgets::separator(ui, &pal);

        if list.is_empty() {
            widgets::empty_state(ui, &pal, Icon::Search, "No matching processes", "Try a different name or PID.");
        }
        let mut action: Option<(u32, String, u8)> = None;
        ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW, list.len(), |ui, range| {
            for i in range {
                let p = &list[i];
                let (r, resp) = ui.allocate_exact_size(vec2(w, ROW), Sense::click());
                let sel = self.proc_selected == Some(p.pid);
                let painter = ui.painter();
                if sel {
                    painter.rect_filled(r.shrink2(vec2(6.0, 1.0)), 8.0, pal.tint(pal.accent, 0.15));
                } else if resp.hovered() {
                    painter.rect_filled(r.shrink2(vec2(6.0, 1.0)), 8.0, pal.tint(pal.text, 0.05));
                }
                let mut name_ui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x(0), r.top()), vec2(cols[0] - 16.0, ROW))).layout(Layout::left_to_right(Align::Center)));
                avatar(&mut name_ui, &pal, &p.name, 22.0);
                name_ui.label(RichText::new(crate::app::truncate(&p.name, 48)).font(theme::medium(13.0)).color(pal.text));
                let painter = ui.painter();
                painter.text(pos2(x(1) + cols[1] - 16.0, r.center().y), Align2::RIGHT_CENTER, p.pid.to_string(), theme::mono(12.0), pal.text2);
                let mut cpu_ui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x(2) + 20.0, r.top() + 8.0), vec2(cols[2] - 36.0, 18.0))));
                widgets::cell_bar(&mut cpu_ui, &pal, p.cpu / 100.0, pal.level(p.cpu / 100.0), &format!("{:.1}%", p.cpu), cols[2] - 36.0);
                let mut mem_ui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x(3) + 20.0, r.top() + 8.0), vec2(cols[3] - 36.0, 18.0))));
                widgets::cell_bar(&mut mem_ui, &pal, p.memory as f32 / total_mem * 4.0, pal.accent_alt, &format::bytes(p.memory), cols[3] - 36.0);

                if resp.clicked() {
                    self.proc_selected = if sel { None } else { Some(p.pid) };
                }
                resp.context_menu(|ui| {
                    ui.label(RichText::new(&p.name).strong());
                    ui.separator();
                    if ui.button("Copy name").clicked() {
                        ui.ctx().copy_text(p.name.clone());
                        ui.close_menu();
                    }
                    if ui.button("Copy PID").clicked() {
                        ui.ctx().copy_text(p.pid.to_string());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit…").clicked() {
                        action = Some((p.pid, p.name.clone(), 0));
                        ui.close_menu();
                    }
                    if ui.button("Force quit…").clicked() {
                        action = Some((p.pid, p.name.clone(), 1));
                        ui.close_menu();
                    }
                });
            }
        });
        let used = ui.min_rect().union(rect.with_max_y(ui.cursor().top()));
        ui.painter().set(bg, egui::Shape::Rect(egui::epaint::RectShape::new(used, 14, pal.surface, egui::Stroke::new(1.0_f32, pal.border), egui::StrokeKind::Inside)));
        if let Some((pid, name, force)) = action {
            self.ask_kill(pid, &name, force == 1);
        }
    }

    fn details(&mut self, ui: &mut Ui, p: &ProcInfo) {
        let pal = self.pal;
        let total = self.m.snap.mem_total.max(1);
        widgets::card(ui, &pal, None, |ui| {
            ui.horizontal(|ui| {
                avatar(ui, &pal, &p.name, 42.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(crate::app::truncate(&p.name, 26)).font(theme::semibold(16.0)).color(pal.text));
                    widgets::caption(ui, &pal, format!("PID {}", p.pid));
                });
            });
            ui.add_space(12.0);
            widgets::stat(ui, &pal, "CPU", format!("{:.1}%", p.cpu));
            widgets::bar(ui, &pal, Id::new(("pd-cpu", p.pid)), p.cpu / 100.0, pal.level(p.cpu / 100.0), 6.0);
            ui.add_space(8.0);
            widgets::stat(ui, &pal, "Memory", format!("{} · {:.1}% of RAM", format::bytes(p.memory), p.memory as f64 / total as f64 * 100.0));
            widgets::bar(ui, &pal, Id::new(("pd-mem", p.pid)), p.memory as f32 / total as f32, pal.accent_alt, 6.0);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, &pal, ButtonKind::Secondary, Some(Icon::Close), "Quit").clicked() {
                    self.ask_kill(p.pid, &p.name, false);
                }
                if widgets::button(ui, &pal, ButtonKind::Danger, None, "Force quit").clicked() {
                    self.ask_kill(p.pid, &p.name, true);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, &pal, ButtonKind::Ghost, Some(Icon::Copy), "Copy PID").clicked() {
                    ui.ctx().copy_text(p.pid.to_string());
                }
                if widgets::button(ui, &pal, ButtonKind::Ghost, None, "Deselect").clicked() {
                    self.proc_selected = None;
                }
            });
        });
    }
}
