use std::collections::VecDeque;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::grid::CardId;
use crate::icons::{self, Icon};
use crate::theme::{self, Palette};

pub fn sparkline(ui: &mut Ui, data: &VecDeque<f32>, max: f32, color: Color32, height: f32) {
    let desired = Vec2::new(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    if data.len() < 2 {
        return;
    }
    let max = max.max(1.0);
    let painter = ui.painter_at(rect);
    let n = data.len().saturating_sub(1) as f32;
    let pts: Vec<Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.left() + i as f32 / n * rect.width();
            let y = rect.bottom() - (*v / max).clamp(0.0, 1.0) * rect.height();
            Pos2::new(x, y)
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.6_f32, color)));
}

pub fn meter(ui: &mut Ui, frac: f32, pal: Palette, warn: bool) {
    let desired = Vec2::new(ui.available_width(), 10.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, pal.track);
    let mut fill = rect;
    fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
    let color = if warn { pal.warn } else { theme::heat(frac) };
    painter.rect_filled(fill, 5.0, color);
}

pub fn card_chrome(
    ui: &mut Ui,
    pal: Palette,
    id: CardId,
    pulse: f32,
    inner: impl FnOnce(&mut Ui),
) -> Rect {
    let fill = pal.card;
    let stroke = Stroke::new(
        1.0_f32,
        Color32::from_rgba_unmultiplied(
            pal.accent.r(),
            pal.accent.g(),
            pal.accent.b(),
            50 + (pulse * 40.0) as u8,
        ),
    );
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(14.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let icon_sz = 18.0;
                let (resp, painter) = ui.allocate_painter(Vec2::splat(icon_sz), Sense::hover());
                icons::paint(&painter, resp.rect.min, icon_sz, Icon::for_card(id), pal.accent);
                ui.label(egui::RichText::new(id.title()).strong().color(pal.text));
            });
            inner(ui);
        })
        .response
        .rect
}

pub fn kpi(ui: &mut Ui, pal: Palette, value: impl Into<String>, unit: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(value.into())
                .size(26.0)
                .color(pal.text)
                .strong(),
        );
        ui.label(egui::RichText::new(unit).color(pal.muted).size(13.0));
    });
}

pub fn heatmap_cores(ui: &mut Ui, cores: &[f32]) {
    let cols = cores.len().clamp(1, 8);
    let cell = ((ui.available_width() - (cols as f32 - 1.0) * 4.0) / cols as f32).clamp(10.0, 28.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for (i, v) in cores.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
            ui.painter().rect_filled(rect, 3.0, theme::heat(*v / 100.0));
            resp.on_hover_text(format!("Core {i}: {v:.0}%"));
            let _ = cols;
        }
    });
}

pub fn heatmap_net(ui: &mut Ui, down: f32, up: f32, pal: Palette) {
    let t = ((down + up) / (8.0 * 1024.0 * 1024.0)).clamp(0.0, 1.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 4.0, pal.track);
    let mut fill = rect;
    fill.set_width(rect.width() * t);
    ui.painter().rect_filled(fill, 4.0, theme::heat(t));
}

pub fn warn_value(value: f32, limit: f32) -> bool {
    value >= limit
}
