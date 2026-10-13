use eframe::egui::{self, Color32, Pos2, Stroke};

pub fn paint(painter: &egui::Painter, origin: Pos2, size: f32, kind: Icon, color: Color32) {
    let s = size;
    let x = origin.x;
    let y = origin.y;
    let st = Stroke::new((s * 0.08).max(1.2), color);
    match kind {
        Icon::Cpu => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.22, y + s * 0.22), egui::vec2(s * 0.56, s * 0.56)),
                3.0,
                st,
                egui::StrokeKind::Middle,
            );
            for i in 0..4 {
                let t = 0.28 + i as f32 * 0.15;
                painter.line_segment([Pos2::new(x + s * t, y + s * 0.12), Pos2::new(x + s * t, y + s * 0.22)], st);
                painter.line_segment([Pos2::new(x + s * t, y + s * 0.78), Pos2::new(x + s * t, y + s * 0.88)], st);
            }
        }
        Icon::Memory => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.18, y + s * 0.32), egui::vec2(s * 0.64, s * 0.36)),
                4.0,
                st,
                egui::StrokeKind::Middle,
            );
            for i in 0..3 {
                painter.circle_filled(Pos2::new(x + s * (0.32 + i as f32 * 0.18), y + s * 0.5), s * 0.06, color);
            }
        }
        Icon::Gpu => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.16, y + s * 0.28), egui::vec2(s * 0.68, s * 0.44)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.28, y + s * 0.38), egui::vec2(s * 0.28, s * 0.24)),
                2.0,
                color.gamma_multiply(0.8),
            );
        }
        Icon::Network => {
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.72), s * 0.08, st);
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.72), s * 0.22, st);
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.72), s * 0.36, st);
        }
        Icon::Storage => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.2, y + s * 0.28), egui::vec2(s * 0.6, s * 0.44)),
                6.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.line_segment([Pos2::new(x + s * 0.3, y + s * 0.5), Pos2::new(x + s * 0.7, y + s * 0.5)], st);
        }
        Icon::Battery => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.18, y + s * 0.34), egui::vec2(s * 0.56, s * 0.32)),
                3.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.74, y + s * 0.42), egui::vec2(s * 0.08, s * 0.16)),
                1.0,
                color,
            );
        }
        Icon::Audio => {
            painter.line_segment([Pos2::new(x + s * 0.28, y + s * 0.42), Pos2::new(x + s * 0.42, y + s * 0.42)], st);
            painter.line_segment([Pos2::new(x + s * 0.42, y + s * 0.42), Pos2::new(x + s * 0.58, y + s * 0.28)], st);
            painter.line_segment([Pos2::new(x + s * 0.42, y + s * 0.58), Pos2::new(x + s * 0.58, y + s * 0.72)], st);
            painter.circle_stroke(Pos2::new(x + s * 0.62, y + s * 0.5), s * 0.14, st);
        }
        Icon::Thermal => {
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.62), s * 0.14, st);
            painter.line_segment([Pos2::new(x + s * 0.5, y + s * 0.2), Pos2::new(x + s * 0.5, y + s * 0.5)], st);
        }
        Icon::Uptime => {
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.5), s * 0.32, st);
            painter.line_segment([Pos2::new(x + s * 0.5, y + s * 0.5), Pos2::new(x + s * 0.5, y + s * 0.3)], st);
            painter.line_segment([Pos2::new(x + s * 0.5, y + s * 0.5), Pos2::new(x + s * 0.68, y + s * 0.5)], st);
        }
        Icon::Processes => {
            for i in 0..3 {
                let yy = y + s * (0.28 + i as f32 * 0.18);
                painter.line_segment([Pos2::new(x + s * 0.22, yy), Pos2::new(x + s * 0.78, yy)], st);
            }
        }
        Icon::Devices => {
            painter.rect_stroke(
                egui::Rect::from_min_size(Pos2::new(x + s * 0.28, y + s * 0.22), egui::vec2(s * 0.44, s * 0.56)),
                4.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_filled(Pos2::new(x + s * 0.5, y + s * 0.68), s * 0.04, color);
        }
        Icon::System => {
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.5), s * 0.28, st);
            painter.circle_filled(Pos2::new(x + s * 0.5, y + s * 0.5), s * 0.08, color);
        }
        Icon::Settings => {
            painter.circle_stroke(Pos2::new(x + s * 0.5, y + s * 0.5), s * 0.16, st);
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                let inner = Pos2::new(x + s * 0.5 + a.cos() * s * 0.22, y + s * 0.5 + a.sin() * s * 0.22);
                let outer = Pos2::new(x + s * 0.5 + a.cos() * s * 0.36, y + s * 0.5 + a.sin() * s * 0.36);
                painter.line_segment([inner, outer], st);
            }
        }
        Icon::Overview => {
            painter.rect_stroke(egui::Rect::from_min_size(Pos2::new(x + s * 0.2, y + s * 0.2), egui::vec2(s * 0.26, s * 0.26)), 3.0, st, egui::StrokeKind::Middle);
            painter.rect_stroke(egui::Rect::from_min_size(Pos2::new(x + s * 0.54, y + s * 0.2), egui::vec2(s * 0.26, s * 0.26)), 3.0, st, egui::StrokeKind::Middle);
            painter.rect_stroke(egui::Rect::from_min_size(Pos2::new(x + s * 0.2, y + s * 0.54), egui::vec2(s * 0.6, s * 0.26)), 3.0, st, egui::StrokeKind::Middle);
        }
        Icon::Perf => {
            let pts = [
                Pos2::new(x + s * 0.18, y + s * 0.7),
                Pos2::new(x + s * 0.38, y + s * 0.45),
                Pos2::new(x + s * 0.52, y + s * 0.55),
                Pos2::new(x + s * 0.8, y + s * 0.22),
            ];
            painter.add(egui::Shape::line(pts.to_vec(), st));
        }
    }
}

#[derive(Clone, Copy)]
pub enum Icon {
    Cpu,
    Memory,
    Gpu,
    Network,
    Storage,
    Battery,
    Audio,
    Thermal,
    Uptime,
    Processes,
    Devices,
    System,
    Settings,
    Overview,
    Perf,
}

impl Icon {
    pub fn for_card(id: crate::grid::CardId) -> Self {
        match id {
            crate::grid::CardId::Cpu => Icon::Cpu,
            crate::grid::CardId::Memory => Icon::Memory,
            crate::grid::CardId::Gpu => Icon::Gpu,
            crate::grid::CardId::Network => Icon::Network,
            crate::grid::CardId::Storage => Icon::Storage,
            crate::grid::CardId::Battery => Icon::Battery,
            crate::grid::CardId::Audio => Icon::Audio,
            crate::grid::CardId::Thermal => Icon::Thermal,
            crate::grid::CardId::Uptime => Icon::Uptime,
            crate::grid::CardId::Processes => Icon::Processes,
            crate::grid::CardId::Devices => Icon::Devices,
            crate::grid::CardId::System => Icon::System,
        }
    }
}
