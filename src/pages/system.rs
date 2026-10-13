//! "About this Mac", in more detail.

use eframe::egui::{Align, Align2, Layout, Rect, RichText, ScrollArea, Sense, Ui, Vec2, pos2, vec2};

use crate::app::EcoView;
use crate::format;
use crate::icons::{self, Icon};
use crate::metrics;
use crate::theme;
use crate::widgets::{self, ButtonKind, Header};

impl EcoView {
    pub(crate) fn system(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let s = self.m.snap.clone();
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Hero.
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
            let p = ui.painter();
            widgets::card_surface(p, &pal, rect, false);
            let ir = Rect::from_center_size(pos2(rect.left() + 80.0, rect.center().y), Vec2::splat(88.0));
            p.rect_filled(ir, 24.0, pal.tint(pal.accent, 0.14));
            icons::paint_in(p, ir.shrink(20.0), Icon::System, pal.accent);
            let tx = rect.left() + 150.0;
            p.text(pos2(tx, rect.top() + 42.0), Align2::LEFT_CENTER, if s.hostname.is_empty() { "This Mac" } else { &s.hostname }, theme::bold(26.0), pal.text);
            p.text(pos2(tx, rect.top() + 72.0), Align2::LEFT_CENTER, format!("{}  ·  {}", if s.model.is_empty() { "Mac" } else { &s.model }, s.os), theme::regular(13.5), pal.text2);
            p.text(pos2(tx, rect.top() + 96.0), Align2::LEFT_CENTER, format!("Up {} · booted {}", format::uptime(s.uptime_secs), format::boot_time(s.boot_time)), theme::regular(12.5), pal.muted);
            let mut bu = ui.new_child(eframe::egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(rect.right() - 170.0, rect.top() + 18.0), vec2(150.0, 32.0))).layout(Layout::right_to_left(Align::Center)));
            if widgets::button(&mut bu, &pal, ButtonKind::Secondary, Some(Icon::Copy), "Copy specs").clicked() {
                ui.ctx().copy_text(self.specs_text());
                self.toast(crate::app::Level::Ok, "Copied", "System specs are on the clipboard.");
            }
            ui.add_space(14.0);

            let mem = metrics::pct(s.mem_used, s.mem_total);
            let cards: Vec<(Header, Vec<(&str, String)>)> = vec![
                (
                    Header::new("Processor", Icon::Cpu, pal.accent),
                    vec![
                        ("Chip", s.cpu_brand.replace("(R)", "").replace("(TM)", "")),
                        ("Cores", match s.physical_cores { Some(p) => format!("{p} physical · {} logical", s.cpu_cores), None => format!("{} logical", s.cpu_cores) }),
                        ("Load average", format!("{:.2} · {:.2} · {:.2}", s.load_avg[0], s.load_avg[1], s.load_avg[2])),
                    ],
                ),
                (
                    Header::new("Memory", Icon::Memory, pal.accent_alt),
                    vec![("Installed", format::bytes(s.mem_total)), ("In use", format!("{} ({mem:.0}%)", format::bytes(s.mem_used))), ("Swap", format!("{} of {}", format::bytes(s.swap_used), format::bytes(s.swap_total)))],
                ),
                (
                    Header::new("Graphics", Icon::Gpu, pal.info),
                    vec![
                        ("GPU", s.gpu.name.clone().unwrap_or_else(|| "—".into())),
                        ("VRAM", s.gpu.mem_total.map(format::bytes).unwrap_or_else(|| "shared / unknown".into())),
                        ("Renderer", "OpenGL (glow)".into()),
                    ],
                ),
                (
                    Header::new("Software", Icon::Leaf, pal.ok),
                    vec![("macOS", s.os.clone()), ("Kernel", s.kernel.clone()), ("Processes", s.process_count.to_string())],
                ),
                (
                    Header::new("Power", Icon::Battery, pal.ok),
                    match &s.battery {
                        Some(b) => vec![
                            ("Charge", format!("{:.0}%", b.percent)),
                            ("Source", if b.charging { "Power adapter".into() } else { "Battery".into() }),
                            ("Time", b.remaining.clone().unwrap_or_else(|| "calculating".into())),
                        ],
                        None => vec![("Source", "Power adapter".into())],
                    },
                ),
                (
                    Header::new("Sensors", Icon::Thermal, pal.crit),
                    vec![
                        ("Temperature", s.temperature.as_ref().map(|t| format!("{:.0}°C", t.celsius)).unwrap_or_else(|| "—".into())),
                        ("Sensor", s.temperature.as_ref().map(|t| t.source.clone()).unwrap_or_else(|| "not exposed".into())),
                        ("Audio out", s.audio.device.clone().unwrap_or_else(|| "—".into())),
                    ],
                ),
            ];
            let cols = if ui.available_width() > 900.0 { 3 } else { 2 };
            for chunk in cards.chunks(cols) {
                ui.columns(cols, |c| {
                    for (i, (h, rows)) in chunk.iter().enumerate() {
                        widgets::card(&mut c[i], &pal, Some(h), |ui| {
                            for (k, v) in rows {
                                widgets::kv(ui, &pal, k, crate::app::truncate(v, 34));
                            }
                        });
                    }
                });
                ui.add_space(14.0);
            }
            let _ = RichText::new("");
        });
    }

    /// Plain-text summary for the clipboard.
    pub(crate) fn specs_text(&self) -> String {
        let s = &self.m.snap;
        let mut out = vec![
            format!("{} ({})", s.hostname, s.model),
            s.os.clone(),
            format!("CPU: {} · {} threads", s.cpu_brand, s.cpu_cores),
            format!("Memory: {}", format::bytes(s.mem_total)),
        ];
        if let Some(g) = &s.gpu.name {
            out.push(format!("GPU: {g}"));
        }
        for d in &s.disks {
            out.push(format!("Disk {}: {} free of {}", d.mount, format::bytes(d.available), format::bytes(d.total)));
        }
        out.push(format!("Uptime: {}", format::uptime(s.uptime_secs)));
        out.join("\n")
    }
}
