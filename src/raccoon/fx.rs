//! Particles, emotes and speech bubbles.
//!
//! All positions are global; the overlay shifts them into its own window when
//! painting.

use std::f32::consts::{PI, TAU};

use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use super::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dust,
    Heart,
    Star,
    Sweat,
    Confetti,
    Note,
    Zzz,
    Crumb,
    Debris,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub kind: Kind,
    pub pos: Pos2,
    pub vel: Vec2,
    pub life: f32,
    pub max: f32,
    pub size: f32,
    pub rot: f32,
    pub spin: f32,
    pub gravity: f32,
    pub color: Color32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emote {
    Exclaim,
    Question,
    Heart,
    Anger,
    Dots,
    Dizzy,
}

#[derive(Clone, Debug)]
pub struct Bubble {
    pub text: String,
    pub t: f32,
    pub dur: f32,
    pub priority: u8,
}

#[derive(Clone, Debug)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub emote: Option<(Emote, f32)>,
    pub bubble: Option<Bubble>,
    pub anchor: Pos2,
    area: Rect,
    /// Global size multiplier, set from the raccoon's size.
    pub scale: f32,
    pub enabled: bool,
}

impl Default for Fx {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            emote: None,
            bubble: None,
            anchor: Pos2::ZERO,
            area: Rect::NOTHING,
            scale: 1.0,
            enabled: true,
        }
    }
}

const MAX_PARTICLES: usize = 220;

impl Fx {
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty() && self.emote.is_none() && self.bubble.is_none()
    }

    pub fn tick(&mut self, dt: f32, _rng: &mut Rng, anchor: Pos2, area: Rect) {
        self.anchor = anchor;
        self.area = area;
        for p in &mut self.particles {
            p.life += dt;
            p.vel.y += p.gravity * dt;
            let drag = match p.kind {
                Kind::Dust | Kind::Zzz | Kind::Note | Kind::Heart => 2.0,
                Kind::Confetti => 1.2,
                _ => 0.2,
            };
            p.vel *= (-dt * drag).exp();
            p.pos += p.vel * dt;
            p.rot += p.spin * dt;
            if matches!(p.kind, Kind::Crumb | Kind::Debris | Kind::Sweat) && p.pos.y > area.bottom() {
                p.pos.y = area.bottom();
                p.vel = Vec2::ZERO;
                p.gravity = 0.0;
            }
        }
        self.particles.retain(|p| p.life < p.max);
        if let Some((_, t)) = &mut self.emote {
            *t += dt;
            if *t > 1.4 {
                self.emote = None;
            }
        }
        if let Some(b) = &mut self.bubble {
            b.t += dt;
            if b.t > b.dur {
                self.bubble = None;
            }
        }
    }

    fn push(&mut self, p: Particle) {
        if !self.enabled {
            return;
        }
        if self.particles.len() >= MAX_PARTICLES {
            self.particles.remove(0);
        }
        self.particles.push(p);
    }

    fn base(kind: Kind, pos: Pos2, vel: Vec2, max: f32, size: f32, color: Color32) -> Particle {
        Particle { kind, pos, vel, life: 0.0, max, size, rot: 0.0, spin: 0.0, gravity: 0.0, color }
    }

    pub fn dust(&mut self, at: Pos2, n: usize, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        for _ in 0..n.min(16) {
            let a = rng.range(PI * 1.05, PI * 1.95);
            let speed = rng.range(40.0, 140.0) * s;
            let mut p = Self::base(
                Kind::Dust,
                at + vec2(rng.range(-10.0, 10.0) * s, -2.0),
                vec2(a.cos() * speed, a.sin() * speed * 0.4),
                rng.range(0.4, 0.8),
                rng.range(4.0, 8.0) * s,
                Color32::from_rgba_unmultiplied(225, 215, 200, 150),
            );
            p.gravity = -20.0;
            self.push(p);
        }
    }

    pub fn heart(&mut self, at: Pos2, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        let mut p = Self::base(
            Kind::Heart,
            at + vec2(rng.range(-14.0, 14.0) * s, 0.0),
            vec2(rng.range(-20.0, 20.0), rng.range(-90.0, -60.0)),
            1.3,
            rng.range(6.0, 9.0) * s,
            Color32::from_rgb(255, 92, 132),
        );
        p.spin = rng.range(-1.0, 1.0);
        self.push(p);
    }

    pub fn star(&mut self, at: Pos2, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        let mut p = Self::base(
            Kind::Star,
            at + vec2(rng.range(-12.0, 12.0) * s, rng.range(-6.0, 4.0)),
            vec2(rng.range(-40.0, 40.0), rng.range(-60.0, -20.0)),
            0.9,
            rng.range(4.0, 6.5) * s,
            Color32::from_rgb(255, 215, 90),
        );
        p.spin = rng.range(-6.0, 6.0);
        self.push(p);
    }

    pub fn sweat(&mut self, at: Pos2, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        let side = rng.sign();
        let mut p = Self::base(
            Kind::Sweat,
            at + vec2(side * 12.0 * s, 6.0 * s),
            vec2(side * rng.range(30.0, 70.0), rng.range(-80.0, -40.0)),
            0.9,
            rng.range(3.0, 4.5) * s,
            Color32::from_rgb(130, 200, 255),
        );
        p.gravity = 500.0;
        self.push(p);
    }

    pub fn confetti(&mut self, at: Pos2, rng: &mut Rng) {
        const COLORS: [Color32; 6] = [
            Color32::from_rgb(255, 99, 132),
            Color32::from_rgb(255, 206, 86),
            Color32::from_rgb(75, 192, 192),
            Color32::from_rgb(54, 162, 235),
            Color32::from_rgb(153, 102, 255),
            Color32::from_rgb(120, 230, 120),
        ];
        let s = self.scale.max(0.4);
        let a = rng.range(-PI * 0.85, -PI * 0.15);
        let speed = rng.range(160.0, 320.0);
        let mut p = Self::base(
            Kind::Confetti,
            at,
            vec2(a.cos() * speed, a.sin() * speed),
            rng.range(1.0, 1.6),
            rng.range(2.5, 4.0) * s,
            *rng.pick(&COLORS),
        );
        p.gravity = 260.0;
        p.spin = rng.range(-12.0, 12.0);
        self.push(p);
    }

    pub fn note(&mut self, at: Pos2, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        let mut p = Self::base(
            Kind::Note,
            at + vec2(rng.range(-16.0, 16.0) * s, 0.0),
            vec2(rng.range(-30.0, 30.0), -70.0),
            1.4,
            7.0 * s,
            Color32::from_rgb(200, 170, 255),
        );
        p.spin = rng.range(-1.5, 1.5);
        self.push(p);
    }

    pub fn zzz(&mut self, at: Pos2, face: f32, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        let mut p = Self::base(
            Kind::Zzz,
            at + vec2(face * 8.0 * s, 0.0),
            vec2(face * rng.range(10.0, 25.0), -28.0),
            2.6,
            rng.range(9.0, 13.0) * s,
            Color32::from_rgb(210, 220, 255),
        );
        p.spin = 0.4 * face;
        self.push(p);
    }

    pub fn crumbs(&mut self, at: Pos2, rng: &mut Rng) {
        let s = self.scale.max(0.4);
        for _ in 0..2 {
            let mut p = Self::base(
                Kind::Crumb,
                at,
                vec2(rng.range(-50.0, 50.0), rng.range(-60.0, 0.0)),
                1.6,
                rng.range(1.2, 2.2) * s,
                Color32::from_rgb(196, 146, 84),
            );
            p.gravity = 700.0;
            self.push(p);
        }
    }

    pub fn debris(&mut self, at: Pos2, dir: f32, rng: &mut Rng) {
        const COLORS: [Color32; 4] = [
            Color32::from_rgb(235, 230, 215),
            Color32::from_rgb(170, 130, 90),
            Color32::from_rgb(120, 160, 90),
            Color32::from_rgb(200, 200, 205),
        ];
        let s = self.scale.max(0.4);
        let mut p = Self::base(
            Kind::Debris,
            at,
            vec2(-dir * rng.range(40.0, 160.0), rng.range(-260.0, -120.0)),
            1.4,
            rng.range(2.0, 4.0) * s,
            *rng.pick(&COLORS),
        );
        p.gravity = 900.0;
        p.spin = rng.range(-10.0, 10.0);
        self.push(p);
    }

    pub fn emote(&mut self, e: Emote) {
        self.emote = Some((e, 0.0));
    }

    pub fn say(&mut self, text: String, priority: u8) {
        if let Some(b) = &self.bubble
            && b.priority > priority
            && b.t < b.dur * 0.6
        {
            return;
        }
        let dur = (1.8 + text.chars().count() as f32 * 0.055).min(5.0);
        self.bubble = Some(Bubble { text, t: 0.0, dur, priority });
    }

    // ------------------------------------------------------------------
    // Painting
    // ------------------------------------------------------------------

    /// Paints everything. `offset` converts global coordinates into the
    /// painter's coordinates.
    pub fn paint(&self, painter: &Painter, offset: Vec2, outline: Color32) {
        for p in &self.particles {
            paint_particle(painter, p, offset);
        }
        let s = self.scale.max(0.5);
        if let Some((e, t)) = self.emote {
            let pop = ease_back((t / 0.25).min(1.0));
            let fade = 1.0 - ((t - 1.1) / 0.3).clamp(0.0, 1.0);
            let at = self.anchor + offset + vec2(0.0, -16.0 * s - t * 6.0);
            paint_emote(painter, e, at, 9.0 * s * pop, fade, t, outline);
        }
        if let Some(b) = &self.bubble {
            self.paint_bubble(painter, b, offset, outline);
        }
    }

    fn paint_bubble(&self, painter: &Painter, b: &Bubble, offset: Vec2, outline: Color32) {
        let fade_in = (b.t / 0.18).min(1.0);
        let fade_out = ((b.dur - b.t) / 0.35).clamp(0.0, 1.0);
        let a = fade_in.min(fade_out);
        if a <= 0.01 {
            return;
        }
        let font = FontId::proportional(13.0);
        let text_col = Color32::from_rgb(34, 30, 40).gamma_multiply(a);
        let galley = painter.layout(b.text.clone(), font, text_col, 220.0);
        let pad = vec2(10.0, 7.0);
        let size = galley.size() + pad * 2.0;
        let lift = 18.0 * self.scale.max(0.5) + (1.0 - fade_in) * 6.0;
        let tip = self.anchor + offset + vec2(0.0, -lift);
        let area = self.area.translate(offset).shrink(4.0);
        let mut rect = Rect::from_min_size(pos2(tip.x - size.x * 0.35, tip.y - 9.0 - size.y), size);
        // Keep it on screen.
        let shift_x = (area.left() - rect.left()).max(0.0) + (area.right() - rect.right()).min(0.0);
        rect = rect.translate(vec2(shift_x, 0.0));
        if rect.top() < area.top() {
            rect = rect.translate(vec2(0.0, area.top() - rect.top()));
        }
        let fill = Color32::from_rgb(252, 250, 246).gamma_multiply(a);
        let stroke = Stroke::new(1.5_f32, outline.gamma_multiply(a * 0.9));
        let shadow = Rect::from_min_size(rect.min + vec2(0.0, 3.0), rect.size());
        painter.rect_filled(shadow, 10.0, Color32::from_black_alpha((50.0 * a) as u8));
        let tail_x = tip.x.clamp(rect.left() + 12.0, rect.right() - 12.0);
        let tail = vec![pos2(tail_x - 6.0, rect.bottom() - 1.0), pos2(tail_x + 6.0, rect.bottom() - 1.0), pos2(tip.x, tip.y)];
        painter.add(Shape::convex_polygon(tail.clone(), fill, stroke));
        painter.rect(rect, 10.0, fill, stroke, egui::StrokeKind::Inside);
        // Cover the tail's stroke where it meets the box.
        painter.line_segment(
            [pos2(tail_x - 4.5, rect.bottom() - 1.5), pos2(tail_x + 4.5, rect.bottom() - 1.5)],
            Stroke::new(2.5_f32, fill),
        );
        painter.galley(rect.min + pad, galley, text_col);
    }
}

fn ease_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn paint_particle(painter: &Painter, p: &Particle, offset: Vec2) {
    let k = p.life / p.max;
    let fade = (1.0 - k).clamp(0.0, 1.0);
    let at = p.pos + offset;
    match p.kind {
        Kind::Dust => {
            let r = p.size * (0.6 + k * 1.2);
            painter.circle_filled(at, r, p.color.gamma_multiply(fade * 0.8));
        }
        Kind::Heart => heart(painter, at, p.size * (0.6 + 0.4 * (k * 6.0).min(1.0)), p.rot, p.color.gamma_multiply(fade)),
        Kind::Star => star(painter, at, p.size, p.rot, p.color.gamma_multiply(fade)),
        Kind::Sweat => {
            let c = p.color.gamma_multiply(fade);
            painter.circle_filled(at, p.size, c);
            let dir = if p.vel.length() > 1.0 { -p.vel.normalized() } else { vec2(0.0, -1.0) };
            let tip = at + dir * p.size * 2.0;
            let side = vec2(-dir.y, dir.x) * p.size * 0.9;
            painter.add(Shape::convex_polygon(vec![at + side, tip, at - side], c, Stroke::NONE));
        }
        Kind::Confetti => {
            let c = p.color.gamma_multiply(fade);
            let w = p.size * (p.rot * 2.0).cos().abs().max(0.2);
            let (s, co) = p.rot.sin_cos();
            let ax = vec2(co, s) * w;
            let ay = vec2(-s, co) * p.size * 0.6;
            painter.add(Shape::convex_polygon(vec![at - ax - ay, at + ax - ay, at + ax + ay, at - ax + ay], c, Stroke::NONE));
        }
        Kind::Note => {
            let c = p.color.gamma_multiply(fade);
            let r = p.size * 0.45;
            painter.circle_filled(at, r, c);
            painter.line_segment([at + vec2(r * 0.9, 0.0), at + vec2(r * 0.9, -p.size * 1.6)], Stroke::new(1.6_f32, c));
            painter.line_segment(
                [at + vec2(r * 0.9, -p.size * 1.6), at + vec2(r * 2.2, -p.size * 1.2)],
                Stroke::new(1.6_f32, c),
            );
        }
        Kind::Zzz => {
            let c = p.color.gamma_multiply(fade * (k * 5.0).min(1.0));
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                "z",
                FontId::proportional(p.size * (0.7 + k * 0.6)),
                c,
            );
        }
        Kind::Crumb => {
            painter.circle_filled(at, p.size, p.color.gamma_multiply(fade));
        }
        Kind::Debris => {
            let c = p.color.gamma_multiply(fade);
            let (s, co) = p.rot.sin_cos();
            let ax = vec2(co, s) * p.size;
            let ay = vec2(-s, co) * p.size * 0.7;
            painter.add(Shape::convex_polygon(vec![at - ax - ay, at + ax - ay, at + ax + ay, at - ax + ay], c, Stroke::NONE));
        }
    }
}

pub fn heart(painter: &Painter, at: Pos2, r: f32, rot: f32, c: Color32) {
    let (s, co) = (rot * 0.3).sin_cos();
    let rotp = |v: Vec2| at + vec2(v.x * co - v.y * s, v.x * s + v.y * co);
    painter.circle_filled(rotp(vec2(-r * 0.5, -r * 0.2)), r * 0.55, c);
    painter.circle_filled(rotp(vec2(r * 0.5, -r * 0.2)), r * 0.55, c);
    painter.add(Shape::convex_polygon(
        vec![rotp(vec2(-r * 1.02, 0.0)), rotp(vec2(r * 1.02, 0.0)), rotp(vec2(0.0, r * 1.05))],
        c,
        Stroke::NONE,
    ));
}

pub fn star(painter: &Painter, at: Pos2, r: f32, rot: f32, c: Color32) {
    let pt = |i: usize, rad: f32| {
        let a = rot + i as f32 * TAU / 10.0 - PI / 2.0;
        at + vec2(a.cos(), a.sin()) * rad
    };
    // A star isn't convex: draw a pentagon core plus five convex points.
    let core: Vec<Pos2> = (0..5).map(|i| pt(i * 2 + 1, r * 0.45)).collect();
    painter.add(Shape::convex_polygon(core, c, Stroke::NONE));
    for i in 0..5 {
        let tip = pt(i * 2, r);
        let a = pt((i * 2 + 9) % 10, r * 0.45);
        let b = pt(i * 2 + 1, r * 0.45);
        painter.add(Shape::convex_polygon(vec![a, tip, b], c, Stroke::NONE));
    }
}

fn paint_emote(painter: &Painter, e: Emote, at: Pos2, r: f32, alpha: f32, t: f32, outline: Color32) {
    if r <= 0.5 || alpha <= 0.0 {
        return;
    }
    let ink = outline.gamma_multiply(alpha);
    match e {
        Emote::Exclaim | Emote::Question => {
            let fill = if e == Emote::Exclaim { Color32::from_rgb(255, 214, 70) } else { Color32::from_rgb(140, 210, 255) };
            painter.circle(at, r * 1.1, fill.gamma_multiply(alpha), Stroke::new(1.5_f32, ink));
            painter.text(
                at + vec2(0.0, 0.5),
                egui::Align2::CENTER_CENTER,
                if e == Emote::Exclaim { "!" } else { "?" },
                FontId::proportional(r * 1.6),
                ink,
            );
        }
        Emote::Heart => heart(painter, at, r, 0.0, Color32::from_rgb(255, 92, 132).gamma_multiply(alpha)),
        Emote::Anger => {
            // The classic "cross-popping vein" mark: four bent strokes.
            let c = Color32::from_rgb(235, 60, 60).gamma_multiply(alpha);
            let w = Stroke::new(r * 0.32, c);
            for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let o = at + vec2(sx * r * 0.35, sy * r * 0.35);
                painter.add(Shape::line(
                    vec![o + vec2(sx * r * 0.55, 0.0), o, o + vec2(0.0, sy * r * 0.55)],
                    w,
                ));
            }
        }
        Emote::Dots => {
            for i in 0..3 {
                let on = ((t * 3.0) as i32) % 4 > i;
                if on {
                    painter.circle_filled(at + vec2((i as f32 - 1.0) * r * 0.7, 0.0), r * 0.22, ink);
                }
            }
        }
        Emote::Dizzy => {
            for i in 0..3 {
                let a = t * 5.0 + i as f32 * TAU / 3.0;
                star(painter, at + vec2(a.cos() * r * 1.4, a.sin() * r * 0.45 + r * 0.6), r * 0.45, a, Color32::from_rgb(255, 215, 90).gamma_multiply(alpha));
            }
        }
    }
}

// ----------------------------------------------------------------------
// Lines
// ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quip {
    Hello,
    Summoned,
    HighLoad,
    Hot,
    Calm,
    LowBattery,
    Charging,
    MemoryFull,
    Download,
    Sleepy,
    Woken,
    Petted,
    Booped,
    Annoyed,
    PickedUp,
    Thrown,
    Splat,
    Dizzy,
    OnWindow,
    Snack,
    Dig,
    Treasure,
    Dance,
    Caught,
    Idle,
    Bored,
    Content,
    Night,
}

impl Quip {
    pub fn priority(self) -> u8 {
        match self {
            Quip::HighLoad | Quip::Hot | Quip::LowBattery | Quip::MemoryFull => 3,
            Quip::Thrown | Quip::Splat | Quip::Dizzy | Quip::PickedUp | Quip::Annoyed => 2,
            Quip::Idle | Quip::Bored | Quip::Content | Quip::Night | Quip::OnWindow => 0,
            _ => 1,
        }
    }

    pub fn lines(self) -> &'static [&'static str] {
        match self {
            Quip::Hello => &["Hi! I'm {name}.", "{name}, reporting for duty.", "Ooh, a fresh desktop."],
            Quip::Summoned => &["Coming!", "Wheee!", "You rang?"],
            Quip::HighLoad => &["CPU at {cpu}%!", "Everything is on fire!", "Too many processes!", "Is it supposed to be this loud?"],
            Quip::Hot => &["It's {temp}°C in here...", "Phew. Toasty.", "Somebody open a window."],
            Quip::Calm => &["Phew, it calmed down.", "Crisis averted.", "Back to normal."],
            Quip::LowBattery => &["Battery at {bat}%... sleepy.", "Plug me in?", "Running on fumes."],
            Quip::Charging => &["Ooh, power!", "Charging! Yay.", "Snack time for the battery."],
            Quip::MemoryFull => &["RAM's at {mem}%. Stuffed.", "No room left in here.", "So full... like after a bin raid."],
            Quip::Download => &["Something big is downloading.", "{net} Mbps! Zoom.", "Packets incoming!"],
            Quip::Sleepy => &["*yawn*", "Nap time.", "Just resting my eyes."],
            Quip::Woken => &["Huh? I'm up!", "Five more minutes...", "Wha-?"],
            Quip::Petted => &["Purrr...", "More please.", "Right there!", "Hehe."],
            Quip::Booped => &["Boop!", "Hey!", "Hi to you too.", "Eep!"],
            Quip::Annoyed => &["Stop that!", "Grr.", "Okay, that's enough poking."],
            Quip::PickedUp => &["Whoa!", "Put me down!", "Hey, hands off the fur!"],
            Quip::Thrown => &["Wheeee!", "AAAAA!", "I can fly!"],
            Quip::Splat => &["Ow.", "Splat.", "Who put a wall there?"],
            Quip::Dizzy => &["The room is spinning...", "@_@", "Which way is up?"],
            Quip::OnWindow => &["Nice view from up here.", "King of the window!", "I live here now."],
            Quip::Snack => &["Found a snack!", "Nom.", "Don't tell anyone."],
            Quip::Dig => &["Ooh, trash.", "Let's see what's in here...", "Rummage rummage."],
            Quip::Treasure => &["Treasure!", "A bottle cap!", "Shiny!"],
            Quip::Dance => &["Dance break!", "♪ ♪", "Feel the beat."],
            Quip::Caught => &["Got it!", "Gotcha!", "Mine!"],
            Quip::Idle => &["Quiet day.", "What're we working on?", "I could go for a grape."],
            Quip::Bored => &["Bored...", "Play with me?", "Wanna throw me?"],
            Quip::Content => &["Life is good.", "I like it here.", "Best desktop ever."],
            Quip::Night => &["It's late... bed?", "Night owl, huh?", "Raccoon hours!"],
        }
    }
}
