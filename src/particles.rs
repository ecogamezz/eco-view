use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};

use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticleStyle {
    ArcadeSparks,
    EmberDrift,
    NeonSnow,
    MatrixRain,
    OrbitRings,
    AuroraWaves,
    Off,
}

impl ParticleStyle {
    pub fn all() -> &'static [ParticleStyle] {
        &[
            ParticleStyle::ArcadeSparks,
            ParticleStyle::EmberDrift,
            ParticleStyle::NeonSnow,
            ParticleStyle::MatrixRain,
            ParticleStyle::OrbitRings,
            ParticleStyle::AuroraWaves,
            ParticleStyle::Off,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            ParticleStyle::ArcadeSparks => "Arcade Sparks",
            ParticleStyle::EmberDrift => "Ember Drift",
            ParticleStyle::NeonSnow => "Neon Snow",
            ParticleStyle::MatrixRain => "Matrix Rain",
            ParticleStyle::OrbitRings => "Orbit Rings",
            ParticleStyle::AuroraWaves => "Aurora Waves",
            ParticleStyle::Off => "Off",
        }
    }
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    hue: f32,
}

pub struct Ambient {
    particles: Vec<Particle>,
    t: f32,
}

impl Ambient {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            t: 0.0,
        }
    }

    pub fn paint(&mut self, painter: &egui::Painter, rect: Rect, pal: Palette, style: ParticleStyle, intensity: f32, dt: f32) {
        if matches!(style, ParticleStyle::Off) || intensity <= 0.01 {
            return;
        }
        self.t += dt;
        // Clamp defensively: this runs inside the frame loop, so an unbounded
        // intensity would turn into a multi-gigabyte allocation allocation and
        // freeze the UI.
        let count = ((40.0 + intensity.clamp(0.0, 1.0) * 90.0) as usize).min(400);
        while self.particles.len() < count {
            self.particles.push(spawn(rect, style, self.t));
        }
        self.particles.truncate(count);

        match style {
            ParticleStyle::OrbitRings => paint_rings(painter, rect, pal, self.t, intensity),
            ParticleStyle::AuroraWaves => paint_aurora(painter, rect, pal, self.t, intensity),
            _ => {}
        }

        for p in &mut self.particles {
            step(p, rect, style, dt);
            let a = (p.life * intensity * 180.0) as u8;
            let color = match style {
                ParticleStyle::ArcadeSparks => Color32::from_rgba_unmultiplied(pal.accent.r(), pal.accent.g(), pal.accent.b(), a),
                ParticleStyle::EmberDrift => Color32::from_rgba_unmultiplied(255, 120 + (p.hue * 80.0) as u8, 40, a),
                ParticleStyle::NeonSnow => Color32::from_rgba_unmultiplied(180, 230, 255, a),
                ParticleStyle::MatrixRain => Color32::from_rgba_unmultiplied(40, 255, 90, a),
                ParticleStyle::OrbitRings | ParticleStyle::AuroraWaves => {
                    Color32::from_rgba_unmultiplied(pal.accent_alt.r(), pal.accent_alt.g(), pal.accent_alt.b(), a / 2)
                }
                ParticleStyle::Off => Color32::TRANSPARENT,
            };
            let r = match style {
                ParticleStyle::MatrixRain => 1.2,
                ParticleStyle::NeonSnow => 2.4,
                _ => 1.8,
            };
            painter.circle_filled(Pos2::new(p.x, p.y), r, color);
            if matches!(style, ParticleStyle::ArcadeSparks | ParticleStyle::MatrixRain) {
                painter.line_segment(
                    [Pos2::new(p.x, p.y), Pos2::new(p.x - p.vx * 4.0, p.y - p.vy * 4.0)],
                    Stroke::new(1.0_f32, color),
                );
            }
        }
        self.particles.retain(|p| p.life > 0.02);
    }
}

fn spawn(rect: Rect, style: ParticleStyle, t: f32) -> Particle {
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    let seed = (t * 17.0 + w) % 1.0;
    match style {
        ParticleStyle::MatrixRain => Particle {
            x: rect.left() + ((t * 80.0) as u32 as f32 * 47.0) % w,
            y: rect.top() - 10.0,
            vx: 0.0,
            vy: 80.0 + seed * 140.0,
            life: 1.0,
            hue: seed,
        },
        ParticleStyle::NeonSnow => Particle {
            x: rect.left() + seed * w,
            y: rect.top() - 8.0,
            vx: -12.0 + seed * 24.0,
            vy: 18.0 + seed * 30.0,
            life: 1.0,
            hue: seed,
        },
        ParticleStyle::EmberDrift => Particle {
            x: rect.left() + seed * w,
            y: rect.bottom() + 4.0,
            vx: -8.0 + seed * 16.0,
            vy: -22.0 - seed * 30.0,
            life: 1.0,
            hue: seed,
        },
        _ => Particle {
            x: rect.left() + seed * w,
            y: rect.top() + ((seed * 3.1) % 1.0) * h,
            vx: -40.0 + seed * 80.0,
            vy: -20.0 + ((seed * 5.7) % 1.0) * 40.0,
            life: 0.4 + seed * 0.8,
            hue: seed,
        },
    }
}

fn step(p: &mut Particle, rect: Rect, style: ParticleStyle, dt: f32) {
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    p.life -= dt * 0.25;
    if p.x < rect.left() - 20.0 || p.x > rect.right() + 20.0 || p.y > rect.bottom() + 20.0 || p.y < rect.top() - 40.0 || p.life <= 0.0 {
        *p = spawn(rect, style, p.hue + p.x * 0.01);
    }
}

fn paint_rings(painter: &egui::Painter, rect: Rect, pal: Palette, t: f32, intensity: f32) {
    let c = rect.center();
    for i in 0..5 {
        let r = 40.0 + i as f32 * 55.0 + (t * 0.4).sin() * 8.0;
        let a = (28.0 * intensity) as u8;
        painter.circle_stroke(
            c,
            r,
            Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(pal.accent.r(), pal.accent.g(), pal.accent.b(), a)),
        );
    }
}

fn paint_aurora(painter: &egui::Painter, rect: Rect, pal: Palette, t: f32, intensity: f32) {
    for band in 0..4 {
        let mut pts = Vec::new();
        let y0 = rect.top() + 40.0 + band as f32 * 28.0;
        let steps = 24;
        for i in 0..=steps {
            let u = i as f32 / steps as f32;
            let x = rect.left() + u * rect.width();
            let y = y0 + (u * 6.0 + t * 0.8 + band as f32).sin() * 16.0;
            pts.push(Pos2::new(x, y));
        }
        let a = (50.0 * intensity) as u8;
        let color = if band % 2 == 0 { pal.accent } else { pal.accent_alt };
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)),
        ));
    }
}
