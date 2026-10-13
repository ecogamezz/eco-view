//! Vector line icons, drawn on a 24-unit grid so they stay crisp at any size.

use std::f32::consts::{PI, TAU};

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Bell,
    Search,
    Sun,
    Moon,
    Layout,
    Check,
    Warning,
    Info,
    Close,
    ChevronRight,
    ChevronDown,
    Bolt,
    Sidebar,
    Copy,
    Leaf,
    Palette,
    Sliders,
    Plug,
    Keyboard,
    ArrowDown,
    ArrowUp,
    Paw,
    Usb,
    Bluetooth,
    Gamepad,
    Refresh,
    Sparkle,
}

impl Icon {
    pub fn for_card(id: crate::grid::CardId) -> Self {
        use crate::grid::CardId;
        match id {
            CardId::Cpu => Icon::Cpu,
            CardId::Memory => Icon::Memory,
            CardId::Gpu => Icon::Gpu,
            CardId::Network => Icon::Network,
            CardId::Storage => Icon::Storage,
            CardId::Battery => Icon::Battery,
            CardId::Audio => Icon::Audio,
            CardId::Thermal => Icon::Thermal,
            CardId::Uptime => Icon::Uptime,
            CardId::Processes => Icon::Processes,
            CardId::Devices => Icon::Devices,
            CardId::System => Icon::System,
        }
    }
}

/// Paints `kind` in the square at `origin` with side `size`.
pub fn paint(painter: &egui::Painter, origin: Pos2, size: f32, kind: Icon, color: Color32) {
    let k = size / 24.0;
    let p = |x: f32, y: f32| pos2(origin.x + x * k, origin.y + y * k);
    let w = (1.75 * k).max(1.1);
    let st = Stroke::new(w, color);
    let line = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        painter.add(Shape::line(v, st));
        // Round the joints and caps.
        for &(x, y) in pts {
            painter.circle_filled(p(x, y), w * 0.5, color);
        }
    };
    let rrect = |x: f32, y: f32, ww: f32, hh: f32, r: f32| {
        painter.rect_stroke(
            Rect::from_min_size(p(x, y), vec2(ww * k, hh * k)),
            r * k,
            st,
            egui::StrokeKind::Middle,
        );
    };
    let fill_rect = |x: f32, y: f32, ww: f32, hh: f32, r: f32| {
        painter.rect_filled(Rect::from_min_size(p(x, y), vec2(ww * k, hh * k)), r * k, color);
    };
    let circle = |x: f32, y: f32, r: f32| painter.circle_stroke(p(x, y), r * k, st);
    let dot = |x: f32, y: f32, r: f32| painter.circle_filled(p(x, y), r * k, color);
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let n = 20;
        let v: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                p(cx + a.cos() * r, cy + a.sin() * r)
            })
            .collect();
        painter.add(Shape::line(v, st));
    };

    match kind {
        Icon::Cpu => {
            rrect(6.0, 6.0, 12.0, 12.0, 2.5);
            fill_rect(9.5, 9.5, 5.0, 5.0, 1.0);
            for t in [9.0, 12.0, 15.0] {
                line(&[(t, 3.0), (t, 6.0)]);
                line(&[(t, 18.0), (t, 21.0)]);
                line(&[(3.0, t), (6.0, t)]);
                line(&[(18.0, t), (21.0, t)]);
            }
        }
        Icon::Memory => {
            rrect(3.0, 7.0, 18.0, 10.0, 2.0);
            for x in [7.0, 11.0, 15.0] {
                fill_rect(x - 0.5, 10.0, 3.0, 4.0, 0.6);
            }
            for x in [6.0, 9.0, 12.0, 15.0, 18.0] {
                line(&[(x, 17.0), (x, 20.0)]);
            }
        }
        Icon::Gpu => {
            rrect(2.5, 6.0, 19.0, 11.0, 2.0);
            circle(9.0, 11.5, 3.0);
            dot(9.0, 11.5, 1.0);
            line(&[(15.0, 9.5), (18.5, 9.5)]);
            line(&[(15.0, 13.5), (18.5, 13.5)]);
            line(&[(5.0, 17.0), (5.0, 20.0)]);
        }
        Icon::Network => {
            arc(12.0, 18.0, 13.0, PI * 1.25, PI * 1.75);
            arc(12.0, 18.0, 8.5, PI * 1.25, PI * 1.75);
            arc(12.0, 18.0, 4.0, PI * 1.25, PI * 1.75);
            dot(12.0, 18.0, 1.6);
        }
        Icon::Storage => {
            rrect(3.0, 5.0, 18.0, 6.0, 2.0);
            rrect(3.0, 13.0, 18.0, 6.0, 2.0);
            dot(7.0, 8.0, 1.1);
            dot(7.0, 16.0, 1.1);
            line(&[(11.0, 8.0), (17.0, 8.0)]);
            line(&[(11.0, 16.0), (17.0, 16.0)]);
        }
        Icon::Battery => {
            rrect(2.5, 7.0, 17.0, 10.0, 2.5);
            fill_rect(20.0, 10.0, 2.0, 4.0, 0.8);
            fill_rect(5.0, 9.5, 8.0, 5.0, 1.0);
        }
        Icon::Audio => {
            line(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5), (4.0, 9.5)]);
            arc(13.0, 12.0, 4.0, -PI * 0.3, PI * 0.3);
            arc(13.0, 12.0, 7.5, -PI * 0.32, PI * 0.32);
        }
        Icon::Thermal => {
            line(&[(10.0, 15.0), (10.0, 5.0)]);
            arc(12.0, 5.0, 2.0, PI, TAU);
            line(&[(14.0, 5.0), (14.0, 15.0)]);
            circle(12.0, 17.5, 3.6);
            dot(12.0, 17.5, 1.8);
            line(&[(12.0, 9.0), (12.0, 16.0)]);
            line(&[(17.0, 7.0), (19.0, 7.0)]);
            line(&[(17.0, 10.0), (19.0, 10.0)]);
        }
        Icon::Uptime => {
            circle(12.0, 12.0, 9.0);
            line(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
        }
        Icon::Processes => {
            for (i, y) in [6.0, 12.0, 18.0].iter().enumerate() {
                dot(5.0, *y, 1.4);
                line(&[(9.0, *y), (20.0 - i as f32 * 3.0, *y)]);
            }
        }
        Icon::Devices => {
            rrect(2.5, 5.0, 13.0, 10.0, 1.5);
            line(&[(6.0, 18.0), (12.0, 18.0)]);
            line(&[(9.0, 15.0), (9.0, 18.0)]);
            rrect(15.5, 8.0, 6.0, 11.0, 1.5);
            dot(18.5, 16.5, 0.8);
        }
        Icon::System => {
            rrect(3.0, 4.5, 18.0, 12.0, 2.0);
            line(&[(8.0, 20.0), (16.0, 20.0)]);
            line(&[(12.0, 16.5), (12.0, 20.0)]);
        }
        Icon::Settings => {
            circle(12.0, 12.0, 3.2);
            let n = 8;
            for i in 0..n {
                let a = i as f32 / n as f32 * TAU;
                let (s, c) = a.sin_cos();
                line(&[(12.0 + c * 6.5, 12.0 + s * 6.5), (12.0 + c * 9.0, 12.0 + s * 9.0)]);
            }
            circle(12.0, 12.0, 6.5);
        }
        Icon::Overview => {
            rrect(3.5, 3.5, 7.5, 7.5, 2.0);
            rrect(13.0, 3.5, 7.5, 7.5, 2.0);
            rrect(3.5, 13.0, 7.5, 7.5, 2.0);
            rrect(13.0, 13.0, 7.5, 7.5, 2.0);
        }
        Icon::Perf => {
            line(&[(3.0, 17.0), (8.0, 11.0), (12.0, 14.0), (20.0, 5.0)]);
            line(&[(15.0, 5.0), (20.0, 5.0), (20.0, 10.0)]);
            line(&[(3.0, 21.0), (21.0, 21.0)]);
        }
        Icon::Bell => {
            line(&[(5.0, 17.0), (19.0, 17.0)]);
            line(&[(6.5, 17.0), (6.5, 11.0)]);
            line(&[(17.5, 17.0), (17.5, 11.0)]);
            arc(12.0, 11.0, 5.5, PI, TAU);
            arc(12.0, 18.5, 2.0, 0.0, PI);
        }
        Icon::Search => {
            circle(10.5, 10.5, 6.0);
            line(&[(15.0, 15.0), (20.0, 20.0)]);
        }
        Icon::Sun => {
            circle(12.0, 12.0, 4.0);
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                let (s, c) = a.sin_cos();
                line(&[(12.0 + c * 7.0, 12.0 + s * 7.0), (12.0 + c * 9.0, 12.0 + s * 9.0)]);
            }
        }
        Icon::Moon => {
            arc(12.0, 12.0, 8.0, PI * 0.35, PI * 1.85);
            arc(15.5, 9.0, 6.0, PI * 0.55, PI * 1.25);
        }
        Icon::Layout => {
            rrect(3.0, 3.0, 18.0, 18.0, 3.0);
            line(&[(3.0, 9.0), (21.0, 9.0)]);
            line(&[(10.0, 9.0), (10.0, 21.0)]);
        }
        Icon::Check => line(&[(5.0, 12.5), (10.0, 17.0), (19.0, 7.0)]),
        Icon::Warning => {
            line(&[(12.0, 3.5), (21.5, 19.5), (2.5, 19.5), (12.0, 3.5)]);
            line(&[(12.0, 9.5), (12.0, 13.5)]);
            dot(12.0, 16.5, 1.2);
        }
        Icon::Info => {
            circle(12.0, 12.0, 9.0);
            line(&[(12.0, 11.0), (12.0, 16.5)]);
            dot(12.0, 7.8, 1.2);
        }
        Icon::Close => {
            line(&[(6.0, 6.0), (18.0, 18.0)]);
            line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::ChevronRight => line(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)]),
        Icon::ChevronDown => line(&[(5.0, 9.0), (12.0, 16.0), (19.0, 9.0)]),
        Icon::Bolt => {
            let v = vec![p(13.5, 2.5), p(5.0, 13.5), p(11.0, 13.5), p(10.0, 21.5), p(19.0, 10.0), p(13.0, 10.0)];
            painter.add(Shape::convex_polygon(vec![v[0], v[1], v[2], v[5]], color, Stroke::NONE));
            painter.add(Shape::convex_polygon(vec![v[5], v[2], v[3], v[4]], color, Stroke::NONE));
        }
        Icon::Sidebar => {
            rrect(3.0, 4.5, 18.0, 15.0, 2.5);
            line(&[(9.0, 4.5), (9.0, 19.5)]);
            line(&[(5.0, 8.5), (7.0, 8.5)]);
            line(&[(5.0, 11.5), (7.0, 11.5)]);
        }
        Icon::Copy => {
            rrect(8.0, 8.0, 12.0, 12.0, 2.0);
            line(&[(16.0, 8.0), (16.0, 5.5), (14.0, 4.0), (5.5, 4.0), (4.0, 5.5), (4.0, 14.0), (5.5, 16.0), (8.0, 16.0)]);
        }
        Icon::Leaf => {
            let n = 16;
            let mut a: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    p(4.0 + t * 16.0, 20.0 - t * 16.0 - (t * PI).sin() * 6.5)
                })
                .collect();
            let b: Vec<Pos2> = (0..=n)
                .rev()
                .map(|i| {
                    let t = i as f32 / n as f32;
                    p(4.0 + t * 16.0 + (t * PI).sin() * 5.0, 20.0 - t * 16.0 + (t * PI).sin() * 1.5)
                })
                .collect();
            a.extend(b);
            painter.add(Shape::closed_line(a, st));
            line(&[(4.0, 20.0), (14.0, 10.0)]);
        }
        Icon::Palette => {
            arc(12.0, 12.0, 9.0, PI * 0.15, PI * 1.95);
            line(&[(20.6, 13.5), (16.5, 13.5), (15.0, 15.0), (15.5, 17.5), (14.5, 20.6)]);
            dot(8.0, 9.0, 1.4);
            dot(12.0, 7.0, 1.4);
            dot(16.0, 9.0, 1.4);
            dot(7.5, 13.5, 1.4);
        }
        Icon::Sliders => {
            for (y, x) in [(6.0, 15.0), (12.0, 8.0), (18.0, 13.0)] {
                line(&[(3.0, y), (21.0, y)]);
                painter.circle(p(x, y), 2.4 * k, Color32::TRANSPARENT, st);
            }
        }
        Icon::Plug => {
            line(&[(9.0, 3.0), (9.0, 8.0)]);
            line(&[(15.0, 3.0), (15.0, 8.0)]);
            line(&[(6.0, 8.0), (18.0, 8.0), (18.0, 11.0)]);
            arc(12.0, 11.0, 6.0, 0.0, PI);
            line(&[(6.0, 11.0), (6.0, 8.0)]);
            line(&[(12.0, 17.0), (12.0, 21.0)]);
        }
        Icon::Keyboard => {
            rrect(2.5, 6.0, 19.0, 12.0, 2.0);
            for x in [6.0, 9.5, 13.0, 16.5] {
                dot(x, 10.0, 0.9);
            }
            line(&[(7.0, 14.5), (17.0, 14.5)]);
        }
        Icon::ArrowDown => {
            line(&[(12.0, 4.0), (12.0, 20.0)]);
            line(&[(6.0, 14.0), (12.0, 20.0), (18.0, 14.0)]);
        }
        Icon::ArrowUp => {
            line(&[(12.0, 20.0), (12.0, 4.0)]);
            line(&[(6.0, 10.0), (12.0, 4.0), (18.0, 10.0)]);
        }
        Icon::Paw => {
            painter.circle_filled(p(12.0, 15.5), 4.5 * k, color);
            for (x, y) in [(6.0, 10.5), (9.5, 6.0), (14.5, 6.0), (18.0, 10.5)] {
                painter.circle_filled(p(x, y), 2.2 * k, color);
            }
        }
        Icon::Usb => {
            line(&[(12.0, 21.0), (12.0, 4.0)]);
            line(&[(9.5, 6.5), (12.0, 3.0), (14.5, 6.5)]);
            line(&[(12.0, 16.0), (7.0, 12.5), (7.0, 10.0)]);
            line(&[(12.0, 13.0), (17.0, 10.5), (17.0, 8.0)]);
            dot(7.0, 9.0, 1.5);
            fill_rect(15.6, 6.0, 2.8, 2.8, 0.3);
            dot(12.0, 20.0, 1.8);
        }
        Icon::Bluetooth => line(&[(7.0, 7.5), (17.0, 16.0), (12.0, 20.5), (12.0, 3.5), (17.0, 8.0), (7.0, 16.5)]),
        Icon::Gamepad => {
            rrect(2.5, 7.0, 19.0, 11.0, 5.5);
            line(&[(6.0, 12.5), (10.0, 12.5)]);
            line(&[(8.0, 10.5), (8.0, 14.5)]);
            dot(15.0, 11.0, 1.2);
            dot(17.5, 13.5, 1.2);
        }
        Icon::Refresh => {
            arc(12.0, 12.0, 7.5, -PI * 0.1, PI * 1.5);
            line(&[(19.5, 5.5), (19.3, 10.7), (14.5, 9.5)]);
        }
        Icon::Sparkle => {
            let star = |cx: f32, cy: f32, r: f32| {
                let pts: Vec<Pos2> = (0..8)
                    .map(|i| {
                        let a = i as f32 * PI / 4.0 - PI / 2.0;
                        let rr = if i % 2 == 0 { r } else { r * 0.32 };
                        p(cx + a.cos() * rr, cy + a.sin() * rr)
                    })
                    .collect();
                // Two convex halves of a four-point star.
                let c = p(cx, cy);
                for i in 0..4 {
                    painter.add(Shape::convex_polygon(
                        vec![c, pts[i * 2 + 1], pts[(i * 2 + 2) % 8], pts[(i * 2 + 3) % 8]],
                        color,
                        Stroke::NONE,
                    ));
                    painter.add(Shape::convex_polygon(vec![c, pts[(i * 2 + 7) % 8], pts[i * 2], pts[i * 2 + 1]], color, Stroke::NONE));
                }
            };
            star(10.0, 13.0, 8.0);
            star(18.5, 5.5, 3.5);
        }
    }
}

/// Paints an icon centred in `rect`.
pub fn paint_in(painter: &egui::Painter, rect: Rect, kind: Icon, color: Color32) {
    let s = rect.width().min(rect.height());
    paint(painter, rect.center() - Vec2::splat(s * 0.5), s, kind, color);
}
