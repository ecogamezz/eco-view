//! Large live charts with a selectable time window.

use eframe::egui::{Align, Color32, Id, Layout, RichText, Ui};

use crate::app::EcoView;
use crate::charts::{self, Chart, Series};
use crate::format;
use crate::icons::Icon;
use crate::metrics;
use crate::theme::{self, Palette};
use crate::widgets::{self, Badge, Header};

impl EcoView {
    pub(crate) fn performance(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        // Toolbar.
        ui.horizontal(|ui| {
            let mut w = self.settings.chart_window_secs;
            if widgets::segmented(ui, &pal, &mut w, &[(60, "1 min"), (300, "5 min"), (900, "15 min")]) {
                self.settings.chart_window_secs = w;
                self.persist();
            }
            ui.add_space(8.0);
            widgets::caption(ui, &pal, "Hover a chart to read exact values.");
        });
        ui.add_space(12.0);

        let step = self.step_secs();
        let n = ((self.settings.chart_window_secs as f32 / step).round() as usize).clamp(10, metrics::HISTORY);
        let h = &self.m.history;
        let s = &self.m.snap;
        let wide = ui.available_width() > 860.0;

        egui_scroll(ui, |ui| {
            // CPU, full width.
            let (avg, peak) = charts::stats(&h.cpu, n);
            let head = Header::new("Processor", Icon::Cpu, pal.accent).badge(Some(Badge::new(format!("{} threads", s.cpu_cores), pal.text2)));
            widgets::card(ui, &pal, Some(&head), |ui| {
                stat_row(ui, &pal, &[("Now", format!("{:.0}%", s.cpu_global)), ("Average", format!("{avg:.0}%")), ("Peak", format!("{peak:.0}%")), ("Load", format!("{:.2} · {:.2} · {:.2}", s.load_avg[0], s.load_avg[1], s.load_avg[2]))]);
                ui.add_space(8.0);
                Chart::new("perf-cpu", 190.0, &|v| format!("{v:.0}%")).max(100.0).window(n, step).show(ui, &pal, &[Series { name: "CPU", values: &h.cpu, color: pal.accent, fill: true }]);
                ui.add_space(10.0);
                ui.label(RichText::new("PER CORE").font(theme::semibold(10.0)).color(pal.muted));
                widgets::core_bars(ui, &pal, &s.cpu_per_core, 46.0);
            });
            ui.add_space(14.0);

            let mem = metrics::pct(s.mem_used, s.mem_total);
            let mem_card = |ui: &mut Ui| {
                let (avg, peak) = charts::stats(&h.ram, n);
                let head = Header::new("Memory", Icon::Memory, pal.accent_alt).badge(Some(Badge::new(format::bytes(s.mem_total), pal.text2)));
                widgets::card(ui, &pal, Some(&head), |ui| {
                    stat_row(ui, &pal, &[("Used", format::bytes(s.mem_used)), ("Now", format!("{mem:.0}%")), ("Avg", format!("{avg:.0}%")), ("Peak", format!("{peak:.0}%"))]);
                    ui.add_space(8.0);
                    Chart::new("perf-mem", 150.0, &|v| format!("{v:.0}%")).max(100.0).window(n, step).show(ui, &pal, &[Series { name: "Memory", values: &h.ram, color: pal.accent_alt, fill: true }]);
                    widgets::caption(ui, &pal, format!("Swap {} of {}", format::bytes(s.swap_used), format::bytes(s.swap_total)));
                });
            };
            let gpu_card = |ui: &mut Ui| {
                let head = Header::new("Graphics", Icon::Gpu, pal.info).badge(s.gpu.name.as_ref().map(|n| Badge::new(crate::app::truncate(n, 26), pal.text2)));
                widgets::card(ui, &pal, Some(&head), |ui| match s.gpu.load {
                    Some(load) => {
                        let (avg, peak) = charts::stats(&h.gpu, n);
                        let vram = match (s.gpu.mem_used, s.gpu.mem_total) {
                            (Some(u), Some(t)) if t > 0 => format!("{} / {}", format::bytes(u), format::bytes(t)),
                            _ => "—".into(),
                        };
                        stat_row(ui, &pal, &[("Now", format!("{load:.0}%")), ("Avg", format!("{avg:.0}%")), ("Peak", format!("{peak:.0}%")), ("VRAM", vram)]);
                        ui.add_space(8.0);
                        Chart::new("perf-gpu", 150.0, &|v| format!("{v:.0}%")).max(100.0).window(n, step).show(ui, &pal, &[Series { name: "GPU", values: &h.gpu, color: pal.info, fill: true }]);
                        widgets::caption(ui, &pal, "Device utilisation reported by IOAccelerator");
                    }
                    None => widgets::empty_state(ui, &pal, Icon::Gpu, "No utilisation data", "This GPU doesn't publish its load through IOKit."),
                });
            };
            let net_card = |ui: &mut Ui| {
                let (avg_d, peak_d) = charts::stats(&h.net_down, n);
                let head = Header::new("Network", Icon::Network, pal.ok);
                widgets::card(ui, &pal, Some(&head), |ui| {
                    stat_row(ui, &pal, &[("Down", format::bps(s.net_down_bps)), ("Up", format::bps(s.net_up_bps)), ("Avg down", format::bps(avg_d as f64)), ("Peak down", format::bps(peak_d as f64))]);
                    ui.add_space(8.0);
                    Chart::new("perf-net", 150.0, &|v| format::bps(v as f64)).window(n, step).show(
                        ui,
                        &pal,
                        &[
                            Series { name: "Down", values: &h.net_down, color: pal.ok, fill: true },
                            Series { name: "Up", values: &h.net_up, color: pal.accent_alt, fill: true },
                        ],
                    );
                    widgets::caption(ui, &pal, format!("Since launch: {} down · {} up", format::bytes(s.net_session_down), format::bytes(s.net_session_up)));
                });
            };
            let temp_card = |ui: &mut Ui| {
                let head = Header::new("Temperature", Icon::Thermal, pal.crit).badge(s.temperature.as_ref().map(|t| Badge::new(t.source.clone(), pal.text2)));
                widgets::card(ui, &pal, Some(&head), |ui| match &s.temperature {
                    Some(t) => {
                        let (avg, peak) = charts::stats(&h.temp, n);
                        stat_row(ui, &pal, &[("Now", format!("{:.0}°C", t.celsius)), ("Avg", format!("{avg:.0}°C")), ("Peak", format!("{peak:.0}°C")), ("Limit", format!("{:.0}°C", self.settings.warn_temp))]);
                        ui.add_space(8.0);
                        Chart::new("perf-temp", 150.0, &|v| format!("{v:.0}°")).max(110.0).window(n, step).show(ui, &pal, &[Series { name: "Temp", values: &h.temp, color: pal.crit, fill: true }]);
                    }
                    None => widgets::empty_state(ui, &pal, Icon::Thermal, "No sensor", "This Mac's SMC doesn't expose a readable temperature."),
                });
            };

            if wide {
                ui.columns(2, |c| {
                    mem_card(&mut c[0]);
                    gpu_card(&mut c[1]);
                });
                ui.add_space(14.0);
                ui.columns(2, |c| {
                    net_card(&mut c[0]);
                    temp_card(&mut c[1]);
                });
            } else {
                mem_card(ui);
                ui.add_space(14.0);
                gpu_card(ui);
                ui.add_space(14.0);
                net_card(ui);
                ui.add_space(14.0);
                temp_card(ui);
            }
            ui.add_space(8.0);
        });
        let _ = Id::NULL;
    }
}

fn egui_scroll(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    eframe::egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, add);
}

/// A row of labelled figures, evenly spaced.
pub(crate) fn stat_row(ui: &mut Ui, pal: &Palette, items: &[(&str, String)]) {
    ui.horizontal(|ui| {
        let w = ui.available_width() / items.len().max(1) as f32;
        for (label, value) in items {
            ui.allocate_ui_with_layout(eframe::egui::vec2(w, 36.0), Layout::top_down(Align::Min), |ui| {
                ui.set_min_width(w - ui.spacing().item_spacing.x);
                widgets::stat(ui, pal, label, value.clone());
            });
        }
    });
    let _ = Color32::TRANSPARENT;
}
