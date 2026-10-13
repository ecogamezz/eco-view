//! Connected hardware, grouped and searchable.

use eframe::egui::{Align2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::EcoView;
use crate::devices::DeviceKind;
use crate::icons::{self, Icon};
use crate::pages::overview::device_icon;
use crate::theme;
use crate::widgets;

impl EcoView {
    pub(crate) fn devices(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let all = self.m.snap.devices.clone();
        let count = |k: DeviceKind| all.iter().filter(|d| d.kind == k).count();
        ui.horizontal(|ui| {
            widgets::search_field(ui, &pal, &mut self.device_query, "Find a device", 240.0);
            ui.add_space(6.0);
            let labels = [
                format!("All {}", all.len()),
                format!("USB {}", count(DeviceKind::Usb)),
                format!("Bluetooth {}", count(DeviceKind::Bluetooth)),
                format!("Input {}", count(DeviceKind::Hid)),
                format!("Controllers {}", count(DeviceKind::Controller)),
            ];
            let items: Vec<(u8, &str)> = labels.iter().enumerate().map(|(i, l)| (i as u8, l.as_str())).collect();
            widgets::segmented(ui, &pal, &mut self.device_filter, &items);
        });
        ui.add_space(12.0);
        let q = self.device_query.to_lowercase();
        let kind = match self.device_filter {
            1 => Some(DeviceKind::Usb),
            2 => Some(DeviceKind::Bluetooth),
            3 => Some(DeviceKind::Hid),
            4 => Some(DeviceKind::Controller),
            _ => None,
        };
        let list: Vec<_> = all
            .iter()
            .filter(|d| kind.is_none_or(|k| d.kind == k))
            .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q))
            .collect();
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if all.is_empty() {
                widgets::card(ui, &pal, None, |ui| widgets::empty_state(ui, &pal, Icon::Devices, "Scanning IOKit…", "Devices are inventoried every few seconds."));
                return;
            }
            if list.is_empty() {
                widgets::card(ui, &pal, None, |ui| widgets::empty_state(ui, &pal, Icon::Search, "Nothing matches", "Try another filter or search."));
                return;
            }
            let tile = vec2(250.0, 74.0);
            let per = ((ui.available_width() + 12.0) / (tile.x + 12.0)).floor().max(1.0) as usize;
            let w = (ui.available_width() - (per - 1) as f32 * 12.0) / per as f32;
            for chunk in list.chunks(per) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for d in chunk {
                        let (r, resp) = ui.allocate_exact_size(vec2(w, tile.y), Sense::hover());
                        let p = ui.painter();
                        widgets::card_surface(p, &pal, r, resp.hovered());
                        let c = match d.kind {
                            DeviceKind::Usb => pal.accent,
                            DeviceKind::Bluetooth => pal.info,
                            DeviceKind::Hid => pal.accent_alt,
                            DeviceKind::Controller => pal.warn,
                        };
                        let ir = Rect::from_center_size(pos2(r.left() + 34.0, r.center().y), Vec2::splat(40.0));
                        p.rect_filled(ir, 12.0, pal.tint(c, 0.15));
                        icons::paint_in(p, ir.shrink(10.0), device_icon(d.kind), c);
                        let name = crate::app::truncate(&d.name, ((w - 80.0) / 7.5) as usize);
                        p.text(pos2(r.left() + 64.0, r.center().y - 9.0), Align2::LEFT_CENTER, name, theme::semibold(13.5), pal.text);
                        let kind = match d.kind {
                            DeviceKind::Usb => "USB",
                            DeviceKind::Bluetooth => "Bluetooth",
                            DeviceKind::Hid => "Input device",
                            DeviceKind::Controller => "Game controller",
                        };
                        p.text(pos2(r.left() + 64.0, r.center().y + 10.0), Align2::LEFT_CENTER, kind, theme::regular(11.5), pal.muted);
                        let _ = (Stroke::NONE, StrokeKind::Inside);
                    }
                });
                ui.add_space(12.0);
            }
            let _ = RichText::new("");
        });
    }
}
