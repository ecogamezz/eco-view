//! Procedural pixel-art sprite sheets for the raccoon.
//!
//! The raccoon is rasterised once at startup into a single atlas texture with
//! one cell per animation frame. Each frame is described by a small pose vector
//! (limb phases, body bob, squash, head turn, ear perk, tail wag, expression),
//! so the whole animation set is data rather than hand-drawn art. Playing an
//! animation is then just picking a UV rect out of the atlas.
//!
//! Everything is drawn in "model space": `x` in roughly [-1, 1], `y` up, feet
//! resting at `y = -1`. A pose transform maps that onto the pixel grid.

use eframe::egui::{self, Color32, ColorImage};

/// Size of one atlas cell in pixels.
pub const FRAME_W: usize = 48;
pub const FRAME_H: usize = 48;

/// Model-space half-extent mapped to half the cell, in pixels.
const PX: f32 = 22.0;
/// Centre of the cell in pixels.
const CX: f32 = FRAME_W as f32 * 0.5;
const CY: f32 = FRAME_H as f32 * 0.5;

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// Rasteriser colours. All are tweakable from Settings, which regenerates the
/// atlas when they change.
#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub fur: Color32,
    pub fur_dark: Color32,
    pub fur_light: Color32,
    pub cream: Color32,
    pub mask: Color32,
    pub eye_white: Color32,
    pub iris: Color32,
    pub nose: Color32,
    pub outline: Color32,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            fur: Color32::from_rgb(126, 104, 88),
            fur_dark: Color32::from_rgb(88, 71, 60),
            fur_light: Color32::from_rgb(163, 140, 120),
            cream: Color32::from_rgb(238, 224, 206),
            mask: Color32::from_rgb(34, 29, 26),
            eye_white: Color32::from_rgb(246, 243, 235),
            iris: Color32::from_rgb(214, 150, 44),
            nose: Color32::from_rgb(26, 21, 19),
            outline: Color32::from_rgb(20, 16, 14),
        }
    }
}

// ---------------------------------------------------------------------------
// Pose
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Eye {
    Open,
    Half,
    Closed,
    /// Upturned arcs, for cheering.
    Happy,
    Wide,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mouth {
    Neutral,
    Smile,
    Frown,
    Open,
    Oh,
    Grin,
}

/// One animation frame. Defaults describe a relaxed, standing raccoon.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    /// Leg cycle phase, -1..1. Arms counter-phase.
    pub leg: f32,
    /// Vertical body offset in model units (positive = higher).
    pub bob: f32,
    /// Body rotation, in half-turns (0.5 = upside down).
    pub tilt: f32,
    /// -1 stretches tall, +1 squashes flat.
    pub squash: f32,
    /// How far the body is lowered, 0..1. Bends the legs.
    pub crouch: f32,
    /// Forward lean of the torso.
    pub lean: f32,
    /// Head turn and tilt, in model units / half-turns.
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub head_bob: f32,
    /// Ear perk, 0 = flat, 1 = fully up.
    pub ear: f32,
    /// Tail wag, -1..1.
    pub tail: f32,
    /// How far the tail curls up.
    pub tail_lift: f32,
    pub eye: Eye,
    pub mouth: Mouth,
    /// Raise the arms overhead (cheer, wave, stretch).
    pub arm_high: bool,
    /// Extend one arm forward, pointing.
    pub pointing: bool,
    /// 0..1 phase used to swing a raised hand.
    pub wave_phase: f32,
    /// 0 = airborne (legs tucked, ears up), 1 = fully planted.
    pub planted: f32,
    /// Facing: -1 looks left, 1 looks right.
    pub facing: f32,
    /// Little sweat drop, for panicking.
    pub sweat: bool,
    /// Dizzy stars orbiting the head.
    pub stars: bool,
    /// Heart, for the affectionate states.
    pub heart: bool,
    /// Zzz glyph, drawn to the upper right.
    pub sleep_marks: bool,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            leg: 0.0,
            bob: 0.0,
            tilt: 0.0,
            squash: 0.0,
            crouch: 0.0,
            lean: 0.0,
            head_yaw: 0.0,
            head_pitch: 0.0,
            head_bob: 0.0,
            ear: 0.55,
            tail: 0.0,
            tail_lift: 0.15,
            eye: Eye::Open,
            mouth: Mouth::Neutral,
            arm_high: false,
            pointing: false,
            wave_phase: 0.0,
            planted: 1.0,
            facing: 1.0,
            sweat: false,
            stars: false,
            heart: false,
            sleep_marks: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Rasteriser
// ---------------------------------------------------------------------------

struct Canvas {
    px: Vec<Color32>,
}

impl Canvas {
    fn new() -> Self {
        Self { px: vec![Color32::TRANSPARENT; FRAME_W * FRAME_H] }
    }

    /// Blits a pixel, alpha-blended so overlaps do not blow out.
    fn put(&mut self, x: i32, y: i32, c: Color32) {
        if x < 0 || y < 0 || x >= FRAME_W as i32 || y >= FRAME_H as i32 {
            return;
        }
        let a = c.a() as u32;
        if a == 0 {
            return;
        }
        let i = (y as usize) * FRAME_W + x as usize;
        if a == 255 {
            self.px[i] = c;
            return;
        }
        let dst = self.px[i];
        let sa = a as f32 / 255.0;
        let mix = |d: u8, s: u8| {
            (d as f32 * (1.0 - sa) + s as f32 * sa).round().clamp(0.0, 255.0) as u8
        };
        self.px[i] = Color32::from_rgba_premultiplied(
            mix(dst.r(), c.r()),
            mix(dst.g(), c.g()),
            mix(dst.b(), c.b()),
            255,
        );
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color32) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let x0 = ((cx - rx).floor() as i32).max(0);
        let x1 = ((cx + rx).ceil() as i32).min(FRAME_W as i32 - 1);
        let y0 = ((cy - ry).floor() as i32).max(0);
        let y1 = ((cy + ry).ceil() as i32).min(FRAME_H as i32 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// Filled triangle, used for ears and tail tips.
    fn fill_tri(&mut self, p: [(f32, f32); 3], c: Color32) {
        let minx = p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor() as i32;
        let maxx = p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as i32;
        let miny = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i32;
        let maxy = p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i32;
        let (ax, ay) = p[0];
        let (bx, by) = p[1];
        let (cx, cy) = p[2];
        let det = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
        if det.abs() < 1e-6 {
            return;
        }
        for y in miny.max(0)..=maxy.min(FRAME_H as i32 - 1) {
            for x in minx.max(0)..=maxx.min(FRAME_W as i32 - 1) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w1 = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / det;
                let w2 = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / det;
                let w3 = 1.0 - w1 - w2;
                if w1 >= -0.002 && w2 >= -0.002 && w3 >= -0.002 {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// Adds a 1px outline around every opaque region, which is what makes the
    /// sprite read cleanly at small sizes.
    fn outline(&mut self, color: Color32) {
        let mut edges = Vec::new();
        for y in 0..FRAME_H as i32 {
            for x in 0..FRAME_W as i32 {
                let at = |x: i32, y: i32| -> Color32 {
                    if x < 0 || y < 0 || x >= FRAME_W as i32 || y >= FRAME_H as i32 {
                        Color32::TRANSPARENT
                    } else {
                        self.px[(y as usize) * FRAME_W + x as usize]
                    }
                };
                if at(x, y).a() > 0 {
                    continue;
                }
                if at(x - 1, y).a() > 0
                    || at(x + 1, y).a() > 0
                    || at(x, y - 1).a() > 0
                    || at(x, y + 1).a() > 0
                {
                    edges.push((x, y));
                }
            }
        }
        for (x, y) in edges {
            self.put(x, y, color);
        }
    }
}

/// Model-space to pixel transform for a pose.
struct Xform {
    cx: f32,
    cy: f32,
    /// cos/sin of the whole-body rotation.
    cos: f32,
    sin: f32,
    sx: f32,
    sy: f32,
}

impl Xform {
    fn from_pose(pose: &Pose) -> Self {
        let angle = pose.tilt * std::f32::consts::PI;
        let sy = 1.0 - pose.squash * 0.22;
        let sx = 1.0 + pose.squash * 0.18;
        Self {
            cx: CX,
            cy: CY - pose.bob * PX * 0.5,
            cos: angle.cos(),
            sin: angle.sin(),
            sx,
            sy,
        }
    }

    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        let x = x * self.sx;
        let y = y * self.sy;
        (
            self.cx + (x * self.cos - y * self.sin) * PX,
            self.cy - (x * self.sin + y * self.cos) * PX,
        )
    }

    fn e(&self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color32, out: &mut Canvas) {
        let (p0x, p0y) = self.p(cx, cy);
        // Radii arrive in model units; the rasteriser wants pixels. The extra
        // factor covers rotation, which can widen an axis.
        let rx = rx * self.sx.max(self.sy) * 1.25 * PX;
        let ry = ry * self.sy.max(self.sx) * 1.25 * PX;
        out.fill_ellipse(p0x, p0y, rx, ry, c);
    }

    fn tri(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Color32, out: &mut Canvas) {
        let pa = self.p(a.0, a.1);
        let pb = self.p(b.0, b.1);
        let pc = self.p(c.0, c.1);
        out.fill_tri([pa, pb, pc], col);
    }
}

// ---------------------------------------------------------------------------
// The raccoon, drawn part by part
// ---------------------------------------------------------------------------

/// Rasterises one frame into a fresh cell.
/// Rasterises one frame into a fresh cell.
///
/// Model-space proportions, paws resting on `y = -1.0`:
/// ```text
///  0.82  ear tips
///  0.48  head centre
/// -0.02  torso centre
/// -0.60  hind leg centre
/// -1.00  paws (ground line)
/// ```
fn draw_pose(pose: &Pose, pal: &Palette, c: &mut Canvas) {
    let x = Xform::from_pose(pose);
    let f = pose.facing;

    let planted = pose.planted.clamp(0.0, 1.0);
    let leg = pose.leg * planted;
    let crouch = pose.crouch;
    let lean = pose.lean * f;

    // Torso rides high when airborne, sinks when crouching.
    let body_y = -0.02 - crouch * 0.22 + pose.bob * 0.05;

    // --- tail: an arc sweeping back and up, ringed like a real raccoon ---
    for i in 0..6 {
        let t = i as f32 / 5.0;
        let tx = -f * (0.24 + t * 0.52);
        let ty = body_y + 0.04 + t * t * 0.34 + pose.tail * t * 0.18;
        // Alternate light and dark so the rings stay legible at 1x.
        let ring = if i % 2 == 0 { pal.cream } else { pal.mask };
        let r = 0.15 - t * 0.05;
        x.e(tx, ty, r, r, ring, c);
    }

    // --- hind legs, reaching the ground ---
    for i in [0usize, 1] {
        let phase = if i == 0 { leg } else { -leg };
        let lx = (if i == 0 { -0.17 } else { 0.17 }) + phase * 0.11 * f;
        let ly = body_y - 0.30 - crouch * 0.06;
        x.e(lx, ly, 0.115, 0.30 - crouch * 0.10, pal.fur_dark, c);
        x.e(lx + phase * 0.06 * f, ly - 0.32, 0.125, 0.085, pal.fur_dark, c);
    }

    // --- torso ---
    x.e(lean * 0.08, body_y, 0.33 - crouch * 0.02, 0.35, pal.fur, c);
    x.e(lean * 0.08 + f * 0.06, body_y - 0.06, 0.22, 0.26, pal.fur_light, c);

    // --- forelegs / arms ---
    let arm = -leg * 0.18;
    for side in [-1.0f32, 1.0] {
        let s = if side < 0.0 { arm } else { -arm };
        let (ax, ay) = if pose.arm_high {
            let wob = if side > 0.0 {
                (pose.wave_phase * std::f32::consts::TAU).sin() * 0.09
            } else {
                0.0
            };
            (side * 0.28 + wob * f, body_y + 0.62)
        } else if pose.pointing && side > 0.0 {
            (0.44 * f, body_y + 0.26)
        } else {
            (side * 0.28 + s * f + lean * 0.10, body_y + 0.14)
        };
        let (paw_dx, paw_dy) = if pose.arm_high {
            (0.0, 0.14)
        } else if pose.pointing && side > 0.0 {
            (0.08 * f, 0.0)
        } else {
            (s * 0.5 * f, -0.30)
        };
        x.e(ax, ay, 0.095, if pose.arm_high { 0.15 } else { 0.24 }, pal.fur, c);
        x.e(ax + paw_dx, ay + paw_dy, 0.10, 0.08, pal.fur_dark, c);
    }

    // --- head ---
    let hx = f * (0.05 + lean * 0.16 + pose.head_yaw * 0.26);
    let hy = body_y + 0.50 + pose.head_bob * 0.05 + pose.head_pitch * 0.06;
    let head_roll = pose.head_pitch * 0.25 + pose.tilt;
    let hxr = {
        let a = head_roll * std::f32::consts::PI;
        (hx * a.cos() - hy * a.sin(), hx * a.sin() + hy * a.cos())
    };

    // Ears first, so the skull overlaps their base.
    for side in [-1.0f32, 1.0] {
        let ex = hxr.0 + side * 0.21;
        let ey = hxr.1 + 0.20;
        x.tri(
            (ex - side * 0.12, ey - 0.03),
            (ex + side * 0.12, ey - 0.03),
            (ex + side * 0.03 * f, ey + 0.16 + pose.ear * 0.12),
            pal.fur,
            c,
        );
        x.tri(
            (ex - side * 0.06, ey + 0.01),
            (ex + side * 0.06, ey + 0.01),
            (ex + side * 0.02 * f, ey + 0.11 + pose.ear * 0.09),
            pal.mask,
            c,
        );
    }

    // skull + cheek ruffs
    x.e(hxr.0, hxr.1, 0.30, 0.29, pal.fur, c);
    for side in [-1.0f32, 1.0] {
        x.e(hxr.0 + side * 0.23, hxr.1 - 0.09, 0.13, 0.12, pal.cream, c);
    }

    // the raccoon mask: a dark band across both eyes
    x.e(hxr.0, hxr.1 + 0.02, 0.27, 0.14, pal.mask, c);
    x.e(hxr.0, hxr.1 + 0.12, 0.23, 0.05, pal.fur_dark, c);

    // muzzle
    let snx = hxr.0 + f * 0.15;
    let sny = hxr.1 - 0.11;
    x.e(snx, sny, 0.18, 0.13, pal.cream, c);
    x.e(snx + f * 0.13, sny + 0.015, 0.06, 0.048, pal.nose, c);

    // eyes
    for side in [-1.0f32, 1.0] {
        let ex = hxr.0 + side * 0.125 + f * 0.05;
        let ey = hxr.1 + 0.02;
        match pose.eye {
            Eye::Open | Eye::Wide => {
                let r = if pose.eye == Eye::Wide { 0.085 } else { 0.068 };
                x.e(ex, ey, r, r * 1.15, pal.eye_white, c);
                x.e(ex + f * 0.012, ey, r * 0.55, r * 0.62, pal.iris, c);
                x.e(ex + f * 0.02, ey, r * 0.24, r * 0.3, pal.nose, c);
            }
            Eye::Half => {
                x.e(ex, ey + 0.03, 0.068, 0.026, pal.eye_white, c);
                x.e(ex + f * 0.012, ey + 0.03, 0.036, 0.028, pal.iris, c);
            }
            Eye::Closed => {
                let (ax, ay) = x.p(ex, ey - 0.01);
                c.fill_ellipse(ax, ay, 0.078 * PX, 0.016 * PX, pal.mask);
            }
            Eye::Happy => {
                let (ax, ay) = x.p(ex, ey - 0.04);
                x.tri(
                    (ax - 0.06 * PX, ay - 0.03 * PX),
                    (ax + 0.06 * PX, ay - 0.03 * PX),
                    (ax, ay + 0.035 * PX),
                    pal.mask,
                    c,
                );
            }
        }
    }

    // mouth
    match pose.mouth {
        Mouth::Neutral | Mouth::Frown => {}
        Mouth::Smile => {
            let (ax, ay) = x.p(snx + f * 0.10, sny - 0.07);
            x.tri(
                (ax - 0.045 * PX, ay - 0.02 * PX),
                (ax + 0.045 * PX, ay - 0.02 * PX),
                (ax, ay + 0.032 * PX),
                pal.mask,
                c,
            );
        }
        Mouth::Grin => {
            let (ax, ay) = x.p(snx + f * 0.10, sny - 0.06);
            x.e(ax, ay, 0.08, 0.05, pal.mask, c);
        }
        Mouth::Open | Mouth::Oh => {
            let r = if pose.mouth == Mouth::Oh { 0.065 } else { 0.05 };
            x.e(snx + f * 0.10, sny - 0.07, r, r * 1.2, pal.mask, c);
        }
    }

    // --- overlays ---
    if pose.sweat {
        let (ax, ay) = x.p(hxr.0 - 0.34, hxr.1 + 0.24);
        c.fill_ellipse(ax, ay, 2.4, 3.2, Color32::from_rgb(150, 205, 255));
        c.fill_ellipse(ax, ay - 2.8, 1.5, 1.7, Color32::from_rgb(150, 205, 255));
    }
    if pose.stars {
        for i in 0..3 {
            let a = pose.head_bob * 6.0 + i as f32 * 2.094;
            let (ax, ay) = x.p(
                hxr.0 + 0.32 + a.cos() * 0.10,
                hxr.1 + 0.28 + a.sin() * 0.07,
            );
            c.fill_ellipse(ax, ay, 2.1, 2.1, Color32::from_rgb(255, 214, 92));
        }
    }
    if pose.heart {
        let (ax, ay) = x.p(hxr.0 + 0.32, hxr.1 + 0.32);
        c.fill_ellipse(ax - 1.4, ay, 2.0, 2.2, Color32::from_rgb(236, 92, 130));
        c.fill_ellipse(ax + 1.4, ay, 2.0, 2.2, Color32::from_rgb(236, 92, 130));
        c.fill_tri(
            [(ax - 3.0, ay - 0.4), (ax + 3.0, ay - 0.4), (ax, ay + 4.0)],
            Color32::from_rgb(236, 92, 130),
        );
    }
    if pose.sleep_marks {
        let (ax, ay) = x.p(hxr.0 + 0.36, hxr.1 + 0.36);
        if ax >= 0.0 && ay >= 0.0 {
            let ix = (ax as usize).min(FRAME_W - 1);
            let iy = (ay as usize).min(FRAME_H - 1);
            c.px[iy * FRAME_W + ix] = Color32::from_rgb(206, 214, 236);
        }
    }

    c.outline(pal.outline);
}
// ---------------------------------------------------------------------------
// Clips
// ---------------------------------------------------------------------------

/// A named animation: a run of poses plus playback metadata.
pub struct Clip {
    pub name: &'static str,
    pub frames: Vec<Pose>,
    pub fps: f32,
    /// Loop forever, or hold the last frame when the clip ends.
    pub looping: bool,
}


/// Builds a pose from a tweak closure, for one-off frames.
fn pose(f: impl FnOnce(&mut Pose)) -> Pose {
    let mut p = Pose::default();
    f(&mut p);
    p
}

/// Sine helper for cyclic motion.
fn wave(phase: f32, amplitude: f32, offset: f32) -> f32 {
    (phase * std::f32::consts::TAU).sin() * amplitude + offset
}

fn walk_like(frames: usize, _speed: f32, lean: f32, bob: f32) -> Vec<Pose> {
    (0..frames)
        .map(|i| {
            let t = i as f32 / frames as f32;
            pose(|p| {
                p.leg = (t * std::f32::consts::TAU).sin();
                p.lean = lean;
                p.bob = wave(t, bob, 0.0);
                p.ear = 0.7;
                p.tail = wave(t * 0.5, 0.35, 0.0);
                p.tail_lift = 0.35;
                p.eye = Eye::Open;
            })
        })
        .collect()
}

/// Every animation the raccoon can play.
pub fn build_clips() -> Vec<Clip> {
    let mut clips: Vec<Clip> = Vec::new();

    // --- resting / awareness -------------------------------------------------
    clips.push(Clip {
        name: "idle",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 8.0;
                pose(|p| {
                    p.bob = wave(t, 0.035, 0.0);
                    p.head_pitch = wave(t, 0.05, 0.0);
                    p.tail = wave(t * 0.5, 0.22, 0.0);
                    p.ear = 0.5;
                })
            })
            .collect(),
        fps: 6.0,
        looping: true,
    });

    clips.push(Clip {
        name: "look_around",
        frames: (0..12)
            .map(|i| {
                let t = i as f32 / 12.0;
                pose(|p| {
                    // Sweep the head left, then right, then back to centre.
                    let sweep = (t * std::f32::consts::TAU).sin();
                    p.head_yaw = sweep * 0.75;
                    p.head_pitch = (t * std::f32::consts::TAU * 2.0).sin() * 0.12;
                    p.head_bob = sweep * 0.05;
                    p.bob = wave(t, 0.03, 0.0);
                    p.ear = 0.65;
                    p.tail = sweep * 0.2;
                    p.eye = Eye::Wide;
                })
            })
            .collect(),
        fps: 8.0,
        looping: true,
    });

    clips.push(Clip {
        name: "peek",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 8.0;
                pose(|p| {
                    p.crouch = 0.55;
                    p.lean = 0.45;
                    p.head_yaw = wave(t, 0.35, 0.15);
                    p.ear = 0.85;
                    p.tail = wave(t, 0.5, -0.2);
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Neutral;
                })
            })
            .collect(),
        fps: 6.0,
        looping: true,
    });

    clips.push(Clip {
        name: "sleep",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 8.0;
                pose(|p| {
                    // Breathing, head tilted, eyes shut.
                    p.crouch = 0.35;
                    p.bob = wave(t, 0.06, -0.08);
                    p.head_pitch = 0.5 + wave(t, 0.08, 0.0);
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Smile;
                    p.ear = 0.2;
                    p.tail = 0.0;
                    p.tail_lift = 0.1;
                    p.sleep_marks = true;
                })
            })
            .collect(),
        fps: 3.0,
        looping: true,
    });

    clips.push(Clip {
        name: "wake",
        frames: (0..5)
            .map(|i| {
                let t = i as f32 / 4.0;
                pose(|p| {
                    p.squash = t * 0.7;
                    p.crouch = 0.4 - t * 0.3;
                    p.ear = 0.4 + t * 0.6;
                    p.eye = if i < 3 { Eye::Closed } else { Eye::Open };
                    p.tail = (1.0 - t) * 0.5;
                })
            })
            .collect(),
        fps: 10.0,
        looping: false,
    });

    // --- locomotion ----------------------------------------------------------
    clips.push(Clip {
        name: "walk",
        frames: walk_like(8, 1.0, 0.10, 0.05),
        fps: 9.0,
        looping: true,
    });

    clips.push(Clip {
        name: "run",
        frames: walk_like(8, 1.0, 0.55, 0.14),
        fps: 16.0,
        looping: true,
    });

    clips.push(Clip {
        name: "sprint",
        frames: walk_like(8, 1.0, 0.95, 0.20),
        fps: 22.0,
        looping: true,
    });

    clips.push(Clip {
        name: "jump_rise",
        frames: (0..4)
            .map(|i| {
                let t = i as f32 / 3.0;
                pose(|p| {
                    p.squash = -0.6 * (1.0 - t);
                    p.planted = 1.0 - t;
                    p.ear = 1.0;
                    p.leg = 0.5 * t;
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Oh;
                    p.tail_lift = 0.7;
                })
            })
            .collect(),
        fps: 14.0,
        looping: false,
    });

    clips.push(Clip {
        name: "jump_fall",
        frames: (0..4)
            .map(|i| {
                let t = i as f32 / 3.0;
                pose(|p| {
                    p.planted = 0.1;
                    p.ear = 1.0;
                    p.leg = -0.5;
                    p.leg = 0.4;
                    p.tail_lift = 0.5;
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Oh;
                    p.squash = t * 0.2;
                })
            })
            .collect(),
        fps: 12.0,
        looping: false,
    });

    clips.push(Clip {
        name: "land",
        frames: (0..4)
            .map(|i| {
                let t = i as f32 / 3.0;
                pose(|p| {
                    p.squash = 0.9 - t * 0.9;
                    p.crouch = 0.5 - t * 0.4;
                    p.ear = 0.8;
                    p.tail = 0.6;
                    p.eye = if i == 0 { Eye::Closed } else { Eye::Open };
                })
            })
            .collect(),
        fps: 14.0,
        looping: false,
    });

    clips.push(Clip {
        name: "flip",
        frames: (0..12)
            .map(|i| {
                let t = i as f32 / 11.0;
                pose(|p| {
                    p.tilt = t; // half a turn over the clip
                    p.planted = 0.25;
                    p.ear = 1.0;
                    p.leg = 0.6;
                    p.tail_lift = 0.8;
                    p.eye = Eye::Happy;
                    p.mouth = Mouth::Open;
                })
            })
            .collect(),
        fps: 14.0,
        looping: false,
    });

    clips.push(Clip {
        name: "climb",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 8.0;
                pose(|p| {
                    p.lean = 0.7;
                    p.leg = (t * std::f32::consts::TAU).sin() * 0.9;
                    p.arm_high = true;
                    p.ear = 0.8;
                    p.tail = wave(t, 0.4, 0.0);
                    p.eye = Eye::Open;
                })
            })
            .collect(),
        fps: 11.0,
        looping: true,
    });

    // --- stationary set pieces ------------------------------------------------
    clips.push(Clip {
        name: "sit",
        frames: (0..5)
            .map(|i| {
                let t = i as f32 / 4.0;
                pose(|p| {
                    p.crouch = 0.75;
                    p.squash = 0.25;
                    p.bob = wave(t, 0.03, -0.05);
                    p.head_pitch = 0.12;
                    p.ear = 0.6;
                    p.tail = wave(t * 2.0, 0.3, 0.3);
                    p.tail_lift = 0.0;
                    p.eye = Eye::Open;
                })
            })
            .collect(),
        fps: 5.0,
        looping: true,
    });

    clips.push(Clip {
        name: "stretch",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    // Reach up and forward, then arch back.
                    let s = (t * std::f32::consts::PI).sin();
                    p.arm_high = t < 0.6;
                    p.squash = -0.7 * s;
                    p.lean = -0.4 * s;
                    p.ear = 0.8;
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Open;
                    p.tail_lift = 0.8 * s;
                })
            })
            .collect(),
        fps: 8.0,
        looping: false,
    });

    clips.push(Clip {
        name: "groom",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    p.crouch = 0.3;
                    p.head_pitch = 0.55 + (t * std::f32::consts::TAU).sin() * 0.15;
                    p.head_yaw = wave(t, 0.25, 0.0);
                    p.ear = 0.5;
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Smile;
                    p.tail = wave(t * 0.5, 0.25, 0.0);
                })
            })
            .collect(),
        fps: 10.0,
        looping: true,
    });

    clips.push(Clip {
        name: "scratch",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.crouch = 0.2;
                    p.lean = 0.3;
                    p.head_pitch = 0.3 + (t * std::f32::consts::TAU).sin() * 0.4;
                    p.ear = 0.45;
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Smile;
                    p.tail = (t * std::f32::consts::TAU).sin() * 0.5;
                })
            })
            .collect(),
        fps: 11.0,
        looping: true,
    });

    clips.push(Clip {
        name: "wave",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.arm_high = i > 3;
                    p.wave_phase = t;
                    p.ear = 0.8;
                    p.eye = Eye::Happy;
                    p.mouth = Mouth::Smile;
                    p.heart = i >= 6;
                })
            })
            .collect(),
        fps: 12.0,
        looping: false,
    });

    clips.push(Clip {
        name: "dance",
        frames: (0..12)
            .map(|i| {
                let t = i as f32 / 11.0;
                pose(|p| {
                    p.leg = (t * std::f32::consts::TAU).sin();
                    p.bob = wave(t, 0.16, 0.0);
                    p.lean = wave(t * 0.5, 0.3, 0.0);
                    p.squash = wave(t, 0.18, 0.0);
                    p.arm_high = i % 2 == 0;
                    p.ear = 1.0;
                    p.tail = wave(t, 0.8, 0.0);
                    p.tail_lift = 0.6;
                    p.eye = Eye::Happy;
                    p.mouth = Mouth::Open;
                })
            })
            .collect(),
        fps: 14.0,
        looping: true,
    });

    clips.push(Clip {
        name: "salute",
        frames: (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                pose(|p| {
                    p.arm_high = true;
                    p.squash = -0.3 * t;
                    p.ear = 0.9;
                    p.eye = Eye::Open;
                    p.mouth = Mouth::Smile;
                    p.tail = t * 0.5;
                })
            })
            .collect(),
        fps: 10.0,
        looping: false,
    });

    clips.push(Clip {
        name: "shrug",
        frames: (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                pose(|p| {
                    p.arm_high = true;
                    p.head_pitch = 0.2 * t;
                    p.head_yaw = wave(t, 0.15, 0.0);
                    p.ear = 0.55;
                    p.eye = Eye::Half;
                    p.mouth = Mouth::Neutral;
                    p.tail = wave(t, 0.2, 0.0);
                })
            })
            .collect(),
        fps: 9.0,
        looping: false,
    });

    clips.push(Clip {
        name: "point",
        frames: (0..6)
            .map(|_i| {
                pose(|p| {
                    p.pointing = true;
                    p.lean = 0.25;
                    p.head_yaw = 0.3;
                    p.ear = 0.85;
                    p.eye = Eye::Open;
                    p.mouth = Mouth::Open;
                })
            })
            .collect(),
        fps: 9.0,
        looping: false,
    });

    clips.push(Clip {
        name: "sneeze",
        frames: (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                pose(|p| {
                    p.squash = 0.6 - t * 0.6;
                    p.crouch = 0.3 * (1.0 - t);
                    p.head_pitch = -0.4 * (1.0 - t);
                    p.ear = 0.9;
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Oh;
                    p.bob = -0.2 * (1.0 - t) * (1.0 - t);
                })
            })
            .collect(),
        fps: 12.0,
        looping: false,
    });

    clips.push(Clip {
        name: "eat",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.crouch = 0.45;
                    p.head_pitch = 0.45 + (t * std::f32::consts::TAU * 2.0).sin() * 0.22;
                    p.ear = 0.55;
                    p.eye = Eye::Closed;
                    p.mouth = if i % 4 < 2 { Mouth::Smile } else { Mouth::Neutral };
                    p.tail = wave(t, 0.4, 0.2);
                })
            })
            .collect(),
        fps: 9.0,
        looping: true,
    });

    clips.push(Clip {
        name: "play",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    p.crouch = 0.3 + (t * std::f32::consts::TAU).sin() * 0.2;
                    p.arm_high = i % 3 == 0;
                    p.leg = (t * std::f32::consts::TAU).sin() * 0.7;
                    p.ear = 1.0;
                    p.tail = wave(t, 0.7, 0.0);
                    p.eye = Eye::Happy;
                    p.mouth = Mouth::Smile;
                    p.heart = i >= 7;
                })
            })
            .collect(),
        fps: 12.0,
        looping: true,
    });

    // --- reactions to the machine ------------------------------------------
    clips.push(Clip {
        name: "alert",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.squash = -0.5 * (1.0 - t);
                    p.ear = 1.0;
                    p.head_pitch = -0.2;
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Oh;
                    p.tail_lift = 0.8;
                    p.planted = 1.0 - t * 0.3;
                })
            })
            .collect(),
        fps: 14.0,
        looping: false,
    });

    clips.push(Clip {
        name: "panic",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.leg = (t * std::f32::consts::TAU * 2.0).sin();
                    p.bob = wave(t, 0.12, 0.0);
                    p.lean = wave(t, 0.2, 0.0);
                    p.ear = 1.0;
                    p.sweat = true;
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Oh;
                    p.tail = wave(t, 0.6, 0.0);
                })
            })
            .collect(),
        fps: 18.0,
        looping: true,
    });

    clips.push(Clip {
        name: "cheer",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    let hop = ((t * std::f32::consts::PI * 2.0).sin()).abs();
                    p.planted = 1.0 - hop * 0.7;
                    p.bob = hop * 0.5;
                    p.arm_high = true;
                    p.squash = -hop * 0.4;
                    p.ear = 1.0;
                    p.eye = Eye::Happy;
                    p.mouth = Mouth::Open;
                    p.tail = wave(t, 0.8, 0.0);
                    p.tail_lift = 0.6;
                })
            })
            .collect(),
        fps: 14.0,
        looping: false,
    });

    // --- physical states ------------------------------------------------------
    clips.push(Clip {
        name: "drag",
        frames: (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                pose(|p| {
                    p.planted = 0.0;
                    p.leg = wave(t, 0.5, -0.5);
                    p.arm_high = true;
                    p.ear = 0.9;
                    p.tail = wave(t, 0.6, 0.0);
                    p.tail_lift = 0.5;
                    p.eye = Eye::Open;
                    p.mouth = Mouth::Oh;
                })
            })
            .collect(),
        fps: 8.0,
        looping: true,
    });

    clips.push(Clip {
        name: "tumble",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    p.tilt = t * 0.25; // continuous rotation handled by physics
                    p.planted = 0.0;
                    p.leg = 0.5;
                    p.ear = 1.0;
                    p.eye = if i % 2 == 0 { Eye::Wide } else { Eye::Open };
                    p.mouth = Mouth::Oh;
                    p.tail_lift = 0.7;
                })
            })
            .collect(),
        fps: 16.0,
        looping: true,
    });

    clips.push(Clip {
        name: "bounce",
        frames: (0..5)
            .map(|i| {
                let t = i as f32 / 4.0;
                pose(|p| {
                    p.squash = 0.8 * (1.0 - t);
                    p.crouch = 0.4 * (1.0 - t);
                    p.eye = Eye::Closed;
                    p.mouth = if t < 0.4 { Mouth::Oh } else { Mouth::Grin };
                    p.stars = t > 0.5;
                })
            })
            .collect(),
        fps: 13.0,
        looping: false,
    });

    clips.push(Clip {
        name: "dizzy",
        frames: (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                pose(|p| {
                    p.bob = wave(t, 0.07, -0.05);
                    p.squash = wave(t, 0.15, 0.0);
                    p.head_pitch = wave(t, 0.25, 0.1);
                    p.ear = 0.3;
                    p.stars = true;
                    p.eye = Eye::Closed;
                    p.mouth = Mouth::Neutral;
                    p.tail = wave(t, 0.3, -0.1);
                })
            })
            .collect(),
        fps: 7.0,
        looping: true,
    });

    clips.push(Clip {
        name: "trip",
        frames: (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                pose(|p| {
                    p.tilt = t * 0.4;
                    p.planted = 1.0 - t;
                    p.leg = 1.0 - t * 1.6;
                    p.ear = 1.0;
                    p.sweat = true;
                    p.eye = Eye::Wide;
                    p.mouth = Mouth::Frown;
                })
            })
            .collect(),
        fps: 13.0,
        looping: false,
    });

    clips.push(Clip {
        name: "idle_talk",
        frames: (0..10)
            .map(|i| {
                let t = i as f32 / 9.0;
                pose(|p| {
                    p.bob = wave(t, 0.04, 0.0);
                    p.mouth = if i % 5 < 2 { Mouth::Open } else { Mouth::Smile };
                    p.ear = 0.7;
                    p.head_yaw = wave(t, 0.12, 0.0);
                    p.tail = wave(t * 0.5, 0.3, 0.0);
                })
            })
            .collect(),
        fps: 9.0,
        looping: true,
    });

    clips
}

// ---------------------------------------------------------------------------
// Atlas
// ---------------------------------------------------------------------------

/// Every frame of every clip, packed into one image, plus the lookup tables the
/// animator needs. Built once at startup and rebuilt only when the palette
/// changes.
pub struct Atlas {
    pub image: ColorImage,
    /// Clip name -> index into `starts`/`lens`.
    pub clips: Vec<Clip>,
    pub starts: Vec<usize>,
    pub lens: Vec<usize>,
    /// Cells per row in the atlas grid.
    pub cols: usize,
    pub total_frames: usize,
}

impl Atlas {
    pub fn build(pal: &Palette) -> Self {
        let clips = build_clips();
        let total_frames: usize = clips.iter().map(|c| c.frames.len()).sum();
        let cols = 16usize.min(total_frames.max(1));
        let rows = total_frames.div_ceil(cols);
        let mut image = ColorImage::new([FRAME_W * cols, FRAME_H * rows], Color32::TRANSPARENT);

        let mut starts = Vec::with_capacity(clips.len());
        let mut lens = Vec::with_capacity(clips.len());
        let mut index = 0usize;
        for clip in &clips {
            starts.push(index);
            lens.push(clip.frames.len());
            for f in &clip.frames {
                let mut canvas = Canvas::new();
                draw_pose(f, pal, &mut canvas);
                let cx = (index % cols) * FRAME_W;
                let cy = (index / cols) * FRAME_H;
                for y in 0..FRAME_H {
                    for x in 0..FRAME_W {
                        image.pixels[(cy + y) * FRAME_W * cols + cx + x] = canvas.px[y * FRAME_W + x];
                    }
                }
                index += 1;
            }
        }

        Self { image, clips, starts, lens, cols, total_frames }
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }

    /// UV rect of one frame within the atlas, in normalised units.
    pub fn uv_rect(&self, clip: usize, frame: usize) -> egui::epaint::Rect {
        let len = self.lens[clip].max(1);
        let frame = frame.min(len - 1);
        let cell = self.starts[clip] + frame;
        let cols = self.cols.max(1);
        let w = (FRAME_W * cols) as f32;
        let h = (FRAME_H * self.total_frames.div_ceil(cols)) as f32;
        let fw = FRAME_W as f32 / w;
        let fh = FRAME_H as f32 / h;
        let x0 = (cell % cols) as f32 * fw;
        let y0 = (cell / cols) as f32 * fh;
        // Half-texel inset keeps neighbouring cells from bleeding in.
        let ix = 0.5 / w;
        let iy = 0.5 / h;
        egui::epaint::Rect::from_min_max(
            egui::epaint::Pos2::new(x0 + ix, y0 + iy),
            egui::epaint::Pos2::new(x0 + fw - ix, y0 + fh - iy),
        )
    }
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

/// Which clip is playing, and how far through it we are.
#[derive(Clone, Debug)]
pub struct Anim {
    pub clip: Option<usize>,
    pub time: f32,
    pub finished: bool,
}

impl Default for Anim {
    fn default() -> Self {
        Self { clip: None, time: 0.0, finished: false }
    }
}

impl Anim {
    /// Switches clips. Restarting the same clip is a no-op unless `force`.
    pub fn play(&mut self, atlas: &Atlas, name: &str, force: bool) {
        let Some(idx) = atlas.index_of(name) else { return };
        if !force && self.clip == Some(idx) {
            return;
        }
        self.clip = Some(idx);
        self.time = 0.0;
        self.finished = false;
    }

    pub fn advance(&mut self, atlas: &Atlas, dt: f32, speed: f32) {
        let Some(idx) = self.clip else { return };
        let len = atlas.lens[idx].max(1);
        let looping = atlas.clips[idx].looping;
        self.time += dt * atlas.clips[idx].fps * speed.max(0.0);
        if self.time >= len as f32 {
            if looping {
                self.time %= len as f32;
            } else {
                self.time = len as f32 - 0.001;
                self.finished = true;
            }
        }
    }

    /// Current frame index within the playing clip.
    pub fn frame(&self, atlas: &Atlas) -> usize {
        let Some(idx) = self.clip else { return 0 };
        let len = atlas.lens[idx].max(1);
        (self.time.floor() as usize) % len
    }

    /// Normalised progress through the clip, 0..1.
    #[allow(dead_code)]
    pub fn progress(&self, atlas: &Atlas) -> f32 {
        let Some(idx) = self.clip else { return 0.0 };
        let len = atlas.lens[idx].max(1);
        (self.time / len as f32).clamp(0.0, 1.0)
    }

    /// Duration of the clip in seconds, at normal speed.
    #[allow(dead_code)]
    pub fn duration(&self, atlas: &Atlas) -> f32 {
        let Some(idx) = self.clip else { return 0.0 };
        atlas.lens[idx].max(1) as f32 / atlas.clips[idx].fps.max(0.01)
    }

    pub fn uv(&self, atlas: &Atlas) -> egui::epaint::Rect {
        let Some(idx) = self.clip else {
            return egui::epaint::Rect::from_min_max(
                egui::epaint::Pos2::ZERO,
                egui::epaint::Pos2::new(1.0, 1.0),
            );
        };
        atlas.uv_rect(idx, self.frame(atlas))
    }
}
