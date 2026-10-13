//! The small round toggle on the dashboard.
//!
//! It is a foreground `egui::Area` kept deliberately small and outside the
//! grid, so it can never displace a card or swallow clicks meant for the
//! dashboard (see `tests/hit_test.rs`).

use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Vec2};

use super::render::{Canvas, draw_portrait};
use super::settings::Palette;

#[derive(Default)]
pub struct Chip {
    pos: Option<Pos2>,
    settled: bool,
}

pub struct ChipResult {
    pub toggled: bool,
    /// Right-clicked: call him over to the cursor.
    pub summon: bool,
}

impl Chip {
    pub fn pos(&self) -> Option<[f32; 2]> {
        self.pos.map(|p| [p.x, p.y])
    }

    pub fn set_pos(&mut self, p: [f32; 2]) {
        self.pos = Some(Pos2::new(p[0], p[1]));
    }

    /// True on the frame a drag of the chip finished.
    pub fn settled(&self) -> bool {
        self.settled
    }

    pub fn paint(&mut self, ctx: &egui::Context, enabled: bool, accent: Color32, pal: &Palette, status: &str) -> ChipResult {
        let mut out = ChipResult { toggled: false, summon: false };
        let time = ctx.input(|i| i.time) as f32;
        egui::Area::new(egui::Id::new("eco-companion-chip"))
            .order(egui::Order::Foreground)
            .movable(true)
            .constrain(true)
            .default_pos(self.pos.unwrap_or(Pos2::new(24.0, 72.0)))
            .show(ctx, |ui| {
                ui.set_width(52.0);
                let (resp, painter) = ui.allocate_painter(Vec2::splat(52.0), egui::Sense::click());
                // `movable(true)` makes the Area the drag target, so latch the
                // new position on pointer release instead.
                let released = ui.input(|i| i.pointer.any_released());
                let prev = self.pos;
                let now = resp.rect.min;
                self.settled = released && prev.is_some_and(|p| (p - now).length() > 0.5);
                self.pos = Some(now);

                let centre = resp.rect.center();
                let ring = if enabled { accent } else { Color32::from_gray(110) };
                let glow = if resp.hovered() { 1.0_f32 } else { 0.0 };
                if enabled {
                    let pulse = (time * 2.0).sin() * 0.5 + 0.5;
                    painter.circle_filled(centre, 25.0 + pulse * 2.0, ring.gamma_multiply(0.15 + 0.1 * pulse));
                }
                painter.circle_filled(centre, 24.0, Color32::from_rgb(22, 24, 34));
                let mut cv = Canvas::new(ctx.pixels_per_point());
                let alpha = if enabled { 1.0 } else { 0.55 };
                cv.set_alpha(alpha);
                draw_portrait(&mut cv, centre + Vec2::new(0.0, 3.0), 42.0, pal, time);
                // Keep the portrait inside the round chip.
                let mesh = cv.mesh;
                painter.with_clip_rect(egui::Rect::from_center_size(centre, Vec2::splat(46.0)))
                    .add(Shape::mesh(mesh));
                painter.circle_stroke(centre, 24.0, Stroke::new(2.0 + glow, ring));

                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.secondary_clicked() && enabled {
                    out.summon = true;
                } else if resp.clicked() {
                    out.toggled = true;
                }
                resp.on_hover_text(if enabled {
                    format!("{status}\nClick to put him away · right-click to call him over · drag to move")
                } else {
                    "Raccoon is away - click to let him out".to_string()
                });
            });
        out
    }
}
