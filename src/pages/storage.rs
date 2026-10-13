//! Volumes with capacity rings and details.

use eframe::egui::{Id, Layout, RichText, ScrollArea, Ui, vec2};

use crate::app::EcoView;
use crate::format;
use crate::icons::Icon;
use crate::metrics::{self, DiskInfo};
use crate::theme;
use crate::widgets::{self, Badge, Header};

impl EcoView {
    pub(crate) fn storage(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let disks = self.m.snap.disks.clone();
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if disks.is_empty() {
                widgets::card(ui, &pal, None, |ui| widgets::empty_state(ui, &pal, Icon::Storage, "Reading volumes…", "Disk information arrives with the first refresh."));
                return;
            }
            // Summary across all volumes.
            let total: u64 = disks.iter().map(|d| d.total).sum();
            let free: u64 = disks.iter().map(|d| d.available).sum();
            let used = total.saturating_sub(free);
            widgets::card(ui, &pal, None, |ui| {
                ui.horizontal(|ui| {
                    let f = metrics::pct(used, total) / 100.0;
                    widgets::ring_gauge(ui, &pal, Id::new("st-total"), 120.0, f, pal.level(f), &format!("{:.0}%", f * 100.0), "USED");
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new(format!("{} free", format::bytes(free))).font(theme::bold(26.0)).color(pal.text));
                        widgets::caption(ui, &pal, format!("{} used of {} across {} volume{}", format::bytes(used), format::bytes(total), disks.len(), if disks.len() == 1 { "" } else { "s" }));
                        ui.add_space(10.0);
                        let segs: Vec<_> = disks
                            .iter()
                            .enumerate()
                            .map(|(i, d)| {
                                let c = [pal.accent, pal.accent_alt, pal.info, pal.warn, pal.ok][i % 5];
                                (d.total.saturating_sub(d.available) as f32 / total.max(1) as f32, c)
                            })
                            .collect();
                        ui.set_width(ui.available_width().min(520.0));
                        widgets::stacked_bar(ui, &pal, &segs, 10.0);
                    });
                });
            });
            ui.add_space(14.0);
            let cols = if ui.available_width() > 900.0 { 3 } else if ui.available_width() > 560.0 { 2 } else { 1 };
            for chunk in disks.chunks(cols) {
                ui.columns(cols, |c| {
                    for (i, d) in chunk.iter().enumerate() {
                        volume_card(&mut c[i], &pal, d);
                    }
                });
                ui.add_space(14.0);
            }
        });
    }
}

fn volume_card(ui: &mut Ui, pal: &crate::theme::Palette, d: &DiskInfo) {
    let used = d.total.saturating_sub(d.available);
    let f = metrics::pct(used, d.total) / 100.0;
    let title = if d.name.is_empty() { d.mount.clone() } else { d.name.clone() };
    let badge = if d.removable { Badge::new("external", pal.info) } else { Badge::new(d.kind.clone(), pal.text2) };
    let head = Header::new(crate::app::truncate(&title, 28), Icon::Storage, pal.level(f)).badge(Some(badge));
    widgets::card(ui, pal, Some(&head), |ui| {
        ui.horizontal(|ui| {
            widgets::ring_gauge(ui, pal, Id::new(("vol", &d.mount)), 84.0, f, pal.level(f), &format!("{:.0}%", f * 100.0), "");
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{} free", format::bytes(d.available))).font(theme::semibold(16.0)).color(pal.text));
                widgets::caption(ui, pal, format!("of {}", format::bytes(d.total)));
            });
        });
        ui.add_space(6.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::top_down(eframe::egui::Align::Min), |ui| {
            widgets::kv(ui, pal, "Mount point", crate::app::truncate(&d.mount, 30));
            widgets::kv(ui, pal, "File system", if d.file_system.is_empty() { "—".into() } else { d.file_system.to_uppercase() });
            widgets::kv(ui, pal, "Used", format::bytes(used));
        });
    });
}
