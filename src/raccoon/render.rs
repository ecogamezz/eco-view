//! Vector rendering for the raccoon.
//!
//! Every body part is a convex shape (an ellipse, or the hull of a few circles)
//! tessellated straight into an `egui::Mesh` with a one-pixel feathered edge,
//! so he is anti-aliased and crisp at any size and on Retina displays. Parts
//! are drawn in groups: each group paints all of its outlines first and then
//! all of its fills, which gives a clean cartoon silhouette without seams
//! between overlapping parts of the same group.

use std::f32::consts::{PI, TAU};

use eframe::egui::{Color32, Mesh, Pos2, Vec2, epaint::Vertex, pos2, vec2};

use super::rig::{Eyes, Pose, Skeleton, Xform};
use super::settings::{Palette, mix};
use super::{Prop, World};

#[derive(Clone, Copy, Debug)]
pub enum Fill {
    Solid(Color32),
    /// Top-to-bottom gradient in screen space.
    Grad(Color32, Color32),
    /// Centre-to-edge gradient.
    Radial(Color32, Color32),
}

/// A convex primitive in model space.
#[derive(Clone, Debug)]
pub enum Prim {
    Circles(Vec<(Vec2, f32)>),
    Ellipse { c: Vec2, rx: f32, ry: f32, ang: f32 },
}

impl Prim {
    fn points(&self, grow: f32) -> Vec<Vec2> {
        match self {
            Prim::Circles(cs) => {
                let mut pts = Vec::with_capacity(cs.len() * 24);
                for &(c, r) in cs {
                    let r = (r + grow).max(0.05);
                    let n = ((r * 2.2) as usize).clamp(10, 28);
                    for i in 0..n {
                        let a = i as f32 / n as f32 * TAU;
                        pts.push(c + vec2(a.cos(), a.sin()) * r);
                    }
                }
                hull(pts)
            }
            Prim::Ellipse { c, rx, ry, ang } => {
                let (rx, ry) = ((rx + grow).max(0.05), (ry + grow).max(0.05));
                let n = (((rx + ry) * 1.6) as usize).clamp(14, 40);
                let (s, co) = ang.sin_cos();
                (0..n)
                    .map(|i| {
                        let a = i as f32 / n as f32 * TAU;
                        let p = vec2(a.cos() * rx, a.sin() * ry);
                        *c + vec2(p.x * co - p.y * s, p.x * s + p.y * co)
                    })
                    .collect()
            }
        }
    }
}

fn circles(cs: &[(Vec2, f32)]) -> Prim {
    Prim::Circles(cs.to_vec())
}

fn capsule(a: Vec2, ra: f32, b: Vec2, rb: f32) -> Prim {
    Prim::Circles(vec![(a, ra), (b, rb)])
}

fn ellipse(c: Vec2, rx: f32, ry: f32, ang: f32) -> Prim {
    Prim::Ellipse { c, rx, ry, ang }
}

/// Andrew's monotone chain convex hull.
fn hull(mut pts: Vec<Vec2>) -> Vec<Vec2> {
    if pts.len() < 3 {
        return pts;
    }
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lower: Vec<Vec2> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Builds the mesh.
pub struct Canvas {
    pub mesh: Mesh,
    feather: f32,
    alpha: f32,
}

impl Canvas {
    pub fn new(pixels_per_point: f32) -> Self {
        Self { mesh: Mesh::default(), feather: 1.0 / pixels_per_point.max(0.5), alpha: 1.0 }
    }

    pub fn set_alpha(&mut self, a: f32) {
        self.alpha = a.clamp(0.0, 1.0);
    }

    fn col(&self, c: Color32) -> Color32 {
        if self.alpha >= 0.999 { c } else { c.gamma_multiply(self.alpha) }
    }

    /// Tessellates a convex polygon (either winding) with a feathered rim.
    pub fn poly(&mut self, pts: &[Pos2], fill: Fill) {
        let n = pts.len();
        if n < 3 {
            return;
        }
        let mut c = Vec2::ZERO;
        let (mut y0, mut y1) = (f32::MAX, f32::MIN);
        for p in pts {
            c += p.to_vec2();
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
        let c = (c / n as f32).to_pos2();
        let mut rmax: f32 = 0.0;
        for p in pts {
            rmax = rmax.max(p.distance(c));
        }
        let color_at = |p: Pos2| -> Color32 {
            match fill {
                Fill::Solid(c) => c,
                Fill::Grad(top, bottom) => {
                    let t = if y1 > y0 { (p.y - y0) / (y1 - y0) } else { 0.5 };
                    mix(top, bottom, t)
                }
                Fill::Radial(centre, edge) => {
                    let t = if rmax > 0.0 { p.distance(c) / rmax } else { 0.0 };
                    lerp_premul(centre, edge, t)
                }
            }
        };
        let f = self.feather * 0.5;
        let base = self.mesh.vertices.len() as u32;
        self.mesh.vertices.push(Vertex { pos: c, uv: Pos2::ZERO, color: self.col(color_at(c)) });
        for i in 0..n {
            let p = pts[i];
            let prev = pts[(i + n - 1) % n];
            let next = pts[(i + 1) % n];
            let e0 = (p - prev).normalized();
            let e1 = (next - p).normalized();
            let mut nrm = vec2(e0.y, -e0.x) + vec2(e1.y, -e1.x);
            if nrm.length_sq() < 1e-8 {
                nrm = p - c;
            }
            let mut nrm = nrm.normalized();
            if nrm.dot(p - c) < 0.0 {
                nrm = -nrm;
            }
            if !nrm.x.is_finite() || !nrm.y.is_finite() {
                nrm = Vec2::ZERO;
            }
            let col = self.col(color_at(p));
            self.mesh.vertices.push(Vertex { pos: p - nrm * f, uv: Pos2::ZERO, color: col });
            self.mesh.vertices.push(Vertex { pos: p + nrm * f, uv: Pos2::ZERO, color: Color32::TRANSPARENT });
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            let (ii, io) = (base + 1 + i * 2, base + 2 + i * 2);
            let (ji, jo) = (base + 1 + j * 2, base + 2 + j * 2);
            self.mesh.indices.extend_from_slice(&[base, ii, ji, ii, io, jo, ii, jo, ji]);
        }
    }
}

fn lerp_premul(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Draws model-space primitives through a transform.
struct Pen<'a> {
    cv: &'a mut Canvas,
    xf: Xform,
    /// Outline width in model units.
    ow: f32,
    outline: Color32,
}

impl Pen<'_> {
    fn screen(&self, pts: &[Vec2]) -> Vec<Pos2> {
        pts.iter().map(|&p| self.xf.apply(p)).collect()
    }

    fn fill(&mut self, prim: &Prim, fill: Fill) {
        let pts = self.screen(&prim.points(0.0));
        self.cv.poly(&pts, fill);
    }

    /// Outlines first, then fills, so parts in a group merge seamlessly.
    fn group(&mut self, items: &[(Prim, Fill)]) {
        if self.ow > 0.0 {
            for (p, _) in items {
                let pts = self.screen(&p.points(self.ow));
                self.cv.poly(&pts, Fill::Solid(self.outline));
            }
        }
        for (p, f) in items {
            self.fill(p, *f);
        }
    }

    /// A polyline drawn as a chain of capsules.
    fn stroke(&mut self, pts: &[Vec2], width: f32, color: Color32) {
        let r = width * 0.5;
        for w in pts.windows(2) {
            self.fill(&capsule(w[0], r, w[1], r), Fill::Solid(color));
        }
    }
}

/// Everything the drawing needs that isn't part of the pose.
pub struct Look<'a> {
    pub pal: &'a Palette,
    pub outline_width: f32,
    pub alpha: f32,
}

/// Draws the raccoon in global coordinates shifted by `offset`.
pub fn draw_world(w: &World, offset: Vec2, ppp: f32) -> Mesh {
    let mut cv = Canvas::new(ppp);
    let mut xf = w.xform();
    xf.origin += offset;
    if w.cfg.show_shadow {
        draw_shadow(&mut cv, w, offset);
    }
    let look = Look { pal: &w.pal, outline_width: w.cfg.outline_width, alpha: 1.0 };
    draw_pose(&mut cv, &w.pose, xf, &look);
    cv.mesh
}

fn draw_shadow(cv: &mut Canvas, w: &World, offset: Vec2) {
    let s = w.scale();
    let x = w.pos.x;
    // The surface he would land on.
    let ground = if w.support.is_some() {
        Some(w.pos.y)
    } else {
        w.surfaces
            .iter()
            .filter(|srf| srf.contains_x(x) && srf.y >= w.pos.y - 1.0)
            .map(|srf| srf.y)
            .min_by(|a, b| a.total_cmp(b))
    };
    let Some(gy) = ground else { return };
    if w.climbing().is_some() || w.is_grabbed() && gy - w.pos.y > 600.0 {
        return;
    }
    let h = (gy - w.pos.y).max(0.0);
    let k = (1.0 - h / 500.0).clamp(0.25, 1.0);
    let c = pos2(x, gy) + offset;
    let rx = 36.0 * s * k;
    let ry = 6.5 * s * k;
    let n = 32;
    let pts: Vec<Pos2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * TAU;
            c + vec2(a.cos() * rx, a.sin() * ry)
        })
        .collect();
    cv.poly(&pts, Fill::Radial(Color32::from_black_alpha((80.0 * k) as u8), Color32::TRANSPARENT));
}

/// Draws one pose. Shared by the overlay, the chip and the preview sheet.
pub fn draw_pose(cv: &mut Canvas, pose: &Pose, xf: Xform, look: &Look) {
    let pal = look.pal;
    let sk = Skeleton::build(pose, pose.time);
    cv.set_alpha(look.alpha);
    let mut pen = Pen { cv, xf, ow: look.outline_width / xf.s.max(0.1), outline: pal.outline };

    let fur = Fill::Grad(pal.fur_light, pal.fur);
    let dark = |c: Color32| mix(c, pal.mask, 0.28);

    // --- tail (behind, unless curled around the front) ---
    let tail_front = pose.tail_front > 0.5;
    if !tail_front {
        draw_tail(&mut pen, &sk, pal);
    }

    // --- far legs ---
    let mut far = Vec::new();
    for leg in sk.legs.iter().filter(|l| !l.near) {
        leg_prims(leg, &mut far, pal, true, dark);
    }
    pen.group(&far);

    // --- far ear ---
    let sh = (pose.head_turn * 2.5).clamp(-2.5, 2.5);
    let ear_far = ear_prims(&sk, vec2(5.0 + sh * 0.3, 10.5), pose.ear, -0.15, pal, true);
    pen.group(&ear_far[..1]);
    pen.fill(&ear_far[1].0, ear_far[1].1);

    // --- torso, with the neck bridging to the head ---
    let mid = sk.hip + (sk.chest - sk.hip) * 0.5 - vec2(0.0, 0.8);
    let neck = sk.chest + (sk.head - sk.chest) * 0.45;
    let breathe = 1.0 + (pose.time * 2.1).sin() * 0.012;
    let torso = circles(&[
        (sk.hip, sk.hip_r * breathe),
        (mid, 13.4 * breathe),
        (sk.chest, sk.chest_r * breathe),
        (neck, 9.6),
    ]);
    pen.group(&[(torso, fur)]);
    let rs = |v: Vec2| {
        let (s, c) = sk.spine.sin_cos();
        vec2(v.x * c - v.y * s, v.x * s + v.y * c)
    };
    let belly = circles(&[(sk.hip + rs(vec2(3.5, -5.6)), 8.8), (sk.chest + rs(vec2(-1.5, -4.6)), 7.2)]);
    pen.fill(&belly, Fill::Grad(mix(pal.belly, pal.fur, 0.35), mix(pal.belly, pal.fur, 0.12)));

    // --- near legs (the forearm may go in front of the face instead) ---
    let arm_in_front = pose.arms_front > 0.5;
    let mut near = Vec::new();
    for leg in sk.legs.iter().filter(|l| l.near && !(l.front && arm_in_front)) {
        leg_prims(leg, &mut near, pal, false, dark);
    }
    pen.group(&near);

    if tail_front {
        draw_tail(&mut pen, &sk, pal);
    }

    // --- head ---
    draw_head(&mut pen, &sk, pose, pal, sh);

    if arm_in_front {
        let mut arm = Vec::new();
        leg_prims(&sk.legs[3], &mut arm, pal, false, dark);
        pen.group(&arm);
        if pose.prop != Prop::None {
            draw_prop(&mut pen, &sk, pose);
        }
    } else if pose.prop != Prop::None {
        draw_prop(&mut pen, &sk, pose);
    }
}

fn leg_prims(
    leg: &super::rig::Leg,
    out: &mut Vec<(Prim, Fill)>,
    pal: &Palette,
    far: bool,
    dark: impl Fn(Color32) -> Color32,
) {
    let tone = |c: Color32| if far { dark(c) } else { c };
    let upper = tone(pal.fur);
    let lower = tone(mix(pal.fur, pal.paw, 0.5));
    let paw = tone(pal.paw);
    out.push((capsule(leg.root, leg.r0, leg.knee, leg.r0 * 0.78), Fill::Grad(tone(pal.fur_light), upper)));
    out.push((capsule(leg.knee, leg.r1, leg.foot, leg.r1 * 0.9), Fill::Solid(lower)));
    let grounded = leg.foot.y < 1.5;
    let fwd = if leg.front { 1.0 } else { 0.6 };
    let p = if grounded {
        ellipse(leg.foot + vec2(fwd * leg.paw * 0.35, leg.paw * 0.42), leg.paw * 1.2, leg.paw * 0.72, 0.0)
    } else {
        ellipse(leg.foot, leg.paw * 0.95, leg.paw * 0.85, 0.0)
    };
    out.push((p, Fill::Solid(paw)));
}

fn draw_tail(pen: &mut Pen, sk: &Skeleton, pal: &Palette) {
    let mut items = Vec::new();
    for (i, w) in sk.tail.windows(2).enumerate() {
        let (a, ra) = w[0];
        let (b, rb) = w[1];
        let last = i == sk.tail.len() - 2;
        let col = if i == 0 {
            Fill::Grad(pal.fur_light, pal.fur)
        } else if last || i % 2 == 1 {
            Fill::Solid(pal.ring_dark)
        } else {
            Fill::Grad(mix(pal.ring_light, Color32::WHITE, 0.15), pal.ring_light)
        };
        items.push((capsule(a, ra, b, rb), col));
    }
    pen.group(&items);
}

/// Returns `[ear, inner ear]`.
fn ear_prims(sk: &Skeleton, base: Vec2, perk: f32, extra: f32, pal: &Palette, far: bool) -> [(Prim, Fill); 2] {
    let ang = -0.2 + (1.0 - perk.clamp(0.0, 1.2)) * 0.95 + extra;
    let (s, c) = ang.sin_cos();
    let up = |v: Vec2| vec2(v.x * c - v.y * s, v.x * s + v.y * c);
    let h = 8.0 + perk.clamp(0.0, 1.0) * 1.8;
    let tone = if far { mix(pal.fur, pal.mask, 0.25) } else { pal.fur };
    let ear = circles(&[
        (sk.head_pt(base + up(vec2(-3.8, 0.0))), 3.6),
        (sk.head_pt(base + up(vec2(3.6, 0.0))), 3.3),
        (sk.head_pt(base + up(vec2(-0.5, h))), 2.1),
    ]);
    let inner = circles(&[
        (sk.head_pt(base + up(vec2(-1.6, 2.6))), 1.7),
        (sk.head_pt(base + up(vec2(1.4, 2.6))), 1.5),
        (sk.head_pt(base + up(vec2(-0.4, h - 1.6))), 0.9),
    ]);
    [(ear, Fill::Grad(mix(tone, Color32::WHITE, 0.12), tone)), (inner, Fill::Solid(pal.ear_inner))]
}

fn draw_head(pen: &mut Pen, sk: &Skeleton, pose: &Pose, pal: &Palette, sh: f32) {
    let h = |v: Vec2| sk.head_pt(v);

    // Near ear goes behind the skull in the same outline group.
    let [ear, inner] = ear_prims(sk, vec2(-6.0 + sh * 0.2, 9.5), pose.ear, pose.ear_twitch * 0.4, pal, false);
    let skull = ellipse(sk.head, 13.8, 12.9, sk.head_angle);
    let cheeks = circles(&[(h(vec2(-5.0, -6.6)), 6.8), (h(vec2(2.0 + sh * 0.5, -8.0)), 6.2), (h(vec2(-10.2, -3.0)), 5.0)]);
    let snout = circles(&[(h(vec2(7.5 + sh, -4.6)), 6.0), (h(vec2(15.6 + sh, -6.3)), 4.2)]);
    let cream = pal.belly;
    pen.group(&[
        ear,
        (skull, Fill::Grad(pal.fur_light, pal.fur)),
        (cheeks, Fill::Grad(mix(cream, pal.fur, 0.1), mix(cream, pal.fur, 0.3))),
        (snout, Fill::Grad(cream, mix(cream, pal.fur, 0.18))),
    ]);
    pen.fill(&inner.0, inner.1);

    // The mask: a dark band across both eyes, sweeping back to the cheek.
    let mask = circles(&[(h(vec2(1.6 + sh, 1.2)), 5.3), (h(vec2(10.0 + sh, 1.8)), 4.1), (h(vec2(-5.2 + sh, -0.8)), 3.4)]);
    pen.fill(&mask, Fill::Grad(mix(pal.mask, pal.fur, 0.25), pal.mask));
    // Bridge of the nose stays light.
    pen.fill(&ellipse(h(vec2(6.2 + sh, 3.5)), 1.4, 3.0, sk.head_angle), Fill::Solid(mix(cream, pal.fur, 0.25)));

    // Brows: white tufts that tilt with the mood.
    let tilt = pose.brow * 1.1;
    let brow_c = mix(cream, Color32::WHITE, 0.3);
    for (x, sz, sign) in [(1.6 + sh, 1.0, 1.0), (10.0 + sh, 0.82, -1.0)] {
        let y = 7.0 + pose.brow.max(0.0) * 0.6;
        let a = h(vec2(x - 2.4 * sz, y - tilt * sign));
        let b = h(vec2(x + 2.4 * sz, y + tilt * sign));
        pen.fill(&capsule(a, 1.6 * sz, b, 1.4 * sz), Fill::Solid(brow_c));
    }

    // Eyes.
    let open = (pose.eye_open * pose.blink).clamp(0.0, 1.2);
    for (x, y, sz) in [(1.8 + sh, 1.5, 1.0), (10.2 + sh, 2.0, 0.8)] {
        draw_eye(pen, sk, pose, pal, vec2(x, y), sz, open);
    }

    // Nose.
    pen.fill(&ellipse(h(vec2(18.2 + sh, -5.4)), 2.9, 2.25, sk.head_angle - 0.1), Fill::Grad(mix(pal.nose, Color32::WHITE, 0.15), pal.nose));
    pen.fill(&ellipse(h(vec2(17.6 + sh, -4.6)), 0.9, 0.55, sk.head_angle), Fill::Solid(Color32::from_white_alpha(150)));

    // Mouth.
    let ink = mix(pal.outline, pal.mask, 0.3);
    let m = vec2(14.2 + sh, -9.7);
    if pose.mouth > 0.12 {
        let o = pose.mouth.min(1.0);
        let inside = mix(pal.mask, Color32::from_rgb(90, 20, 30), 0.35);
        let mouth = circles(&[(h(m + vec2(-0.8, -o * 0.8)), 1.3 + o * 1.5), (h(m + vec2(1.4, -o * 0.4)), 1.0 + o * 1.0)]);
        pen.fill(&mouth, Fill::Solid(inside));
        let tongue_c = mix(pal.blush, Color32::from_rgb(240, 110, 120), 0.6);
        pen.fill(&ellipse(h(m + vec2(-0.3, -o * 1.7)), 1.1 + o * 0.9, 0.7 + o * 0.6, sk.head_angle), Fill::Solid(tongue_c));
    } else {
        let s = pose.smile;
        let pts = [
            h(m + vec2(-2.4, 0.4 + s * 0.7)),
            h(m + vec2(-1.0, -0.5 - s * 0.3)),
            h(m + vec2(0.4, 0.2)),
            h(m + vec2(1.8, -0.4 - s * 0.25)),
            h(m + vec2(2.8, 0.3 + s * 0.4)),
        ];
        pen.stroke(&pts, 0.75, ink);
    }
    if pose.tongue > 0.05 && pose.mouth <= 0.12 {
        let tongue_c = mix(pal.blush, Color32::from_rgb(240, 110, 120), 0.6);
        let t = pose.tongue.min(1.0);
        pen.fill(&ellipse(h(m + vec2(0.5, -1.4 * t - 0.4)), 1.3, 1.0 + t * 0.8, sk.head_angle), Fill::Solid(tongue_c));
    }

    // Whiskers.
    let whisk = Color32::from_white_alpha(150);
    for (dy, len) in [(1.2, 7.5), (-1.2, 7.0)] {
        let a = h(vec2(14.0 + sh, -7.2 + dy * 0.6));
        let b = h(vec2(14.0 + sh + len, -7.0 + dy * 2.2));
        pen.stroke(&[a, b], 0.38, whisk);
    }

    // Blush.
    if pose.blush > 0.02 {
        let c = pal.blush;
        let a = (pose.blush.min(1.0) * 140.0) as u8;
        let col = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
        pen.fill(&ellipse(h(vec2(-2.5 + sh, -4.6)), 3.3, 1.9, sk.head_angle), Fill::Radial(col, Color32::TRANSPARENT));
    }
}

fn draw_eye(pen: &mut Pen, sk: &Skeleton, pose: &Pose, pal: &Palette, at: Vec2, sz: f32, open: f32) {
    let h = |v: Vec2| sk.head_pt(v);
    let light = mix(pal.belly, Color32::WHITE, 0.4);
    match pose.eyes {
        Eyes::Normal | Eyes::Wide if open > 0.15 => {
            let wide = if pose.eyes == Eyes::Wide { 1.16 } else { 1.0 };
            let rx = 3.2 * sz * wide;
            let ry = 3.8 * sz * wide * open.min(1.0);
            pen.fill(&ellipse(h(at), rx, ry, sk.head_angle), Fill::Grad(Color32::WHITE, mix(Color32::WHITE, pal.belly, 0.6)));
            let pupil = pose.pupil * if pose.eyes == Eyes::Wide { 0.72 } else { 1.0 };
            let lk = vec2(pose.look.x.clamp(-1.0, 1.0), pose.look.y.clamp(-1.0, 1.0));
            let px = rx * 0.78 * pupil.min(1.2);
            let py = ry * 0.82 * pupil.min(1.2);
            let off = vec2(lk.x * (rx - px * 0.85).max(0.3), lk.y * (ry - py * 0.85).max(0.2));
            let c = at + off;
            pen.fill(&ellipse(h(c), px, py, sk.head_angle), Fill::Grad(mix(pal.eye, Color32::WHITE, 0.12), pal.eye));
            pen.fill(&ellipse(h(c + vec2(-px * 0.32, py * 0.38)), px * 0.34, (py * 0.3).max(0.2), 0.0), Fill::Solid(Color32::WHITE));
            pen.fill(&ellipse(h(c + vec2(px * 0.38, -py * 0.36)), px * 0.16, (py * 0.14).max(0.1), 0.0), Fill::Solid(Color32::from_white_alpha(200)));
        }
        Eyes::Hearts => {
            let c = Color32::from_rgb(255, 92, 132);
            let r = 1.7 * sz;
            pen.fill(&circles(&[(h(at + vec2(-r * 0.55, r * 0.35)), r * 0.7)]), Fill::Solid(c));
            pen.fill(&circles(&[(h(at + vec2(r * 0.55, r * 0.35)), r * 0.7)]), Fill::Solid(c));
            pen.fill(
                &circles(&[(h(at + vec2(-r * 1.15, r * 0.15)), 0.1), (h(at + vec2(r * 1.15, r * 0.15)), 0.1), (h(at + vec2(0.0, -r * 1.25)), 0.1)]),
                Fill::Solid(c),
            );
        }
        Eyes::Happy => {
            let w = 2.8 * sz;
            pen.stroke(&[h(at + vec2(-w, -0.7)), h(at + vec2(-w * 0.45, 0.6)), h(at + vec2(0.0, 1.0)), h(at + vec2(w * 0.45, 0.6)), h(at + vec2(w, -0.7))], 1.0 * sz, light);
        }
        Eyes::Swirl => {
            let mut pts = Vec::new();
            let spin = pose.time * 6.0;
            for i in 0..18 {
                let k = i as f32 / 17.0;
                let a = spin + k * TAU * 1.6;
                pts.push(h(at + vec2(a.cos(), a.sin()) * (0.4 + k * 2.8 * sz)));
            }
            pen.stroke(&pts, 0.7 * sz, light);
        }
        _ => {
            // Closed (or a blink): a soft downward curve.
            let w = 2.8 * sz;
            pen.stroke(&[h(at + vec2(-w, 0.4)), h(at + vec2(-w * 0.45, -0.5)), h(at + vec2(0.0, -0.7)), h(at + vec2(w * 0.45, -0.5)), h(at + vec2(w, 0.4))], 0.95 * sz, light);
        }
    }
}

fn draw_prop(pen: &mut Pen, sk: &Skeleton, pose: &Pose) {
    let at = sk.paw() + vec2(2.0, 2.5);
    match pose.prop {
        Prop::None => {}
        Prop::Cookie => {
            pen.group(&[(ellipse(at, 4.6, 4.3, 0.0), Fill::Grad(Color32::from_rgb(222, 170, 98), Color32::from_rgb(186, 128, 62)))]);
            for (dx, dy) in [(-1.6, 1.2), (1.5, 0.4), (-0.2, -1.8), (1.9, -1.7)] {
                pen.fill(&ellipse(at + vec2(dx, dy), 0.75, 0.65, 0.0), Fill::Solid(Color32::from_rgb(80, 46, 30)));
            }
            // A bite taken out of it, once he's been at it a while.
            if (pose.time * 0.5).fract() > 0.4 {
                pen.fill(&ellipse(at + vec2(3.8, 2.6), 1.9, 1.9, 0.0), Fill::Solid(Color32::TRANSPARENT));
            }
        }
        Prop::Apple => {
            pen.group(&[
                (ellipse(at, 4.8, 4.5, 0.0), Fill::Grad(Color32::from_rgb(240, 80, 70), Color32::from_rgb(170, 30, 40))),
                (ellipse(at + vec2(1.6, 5.0), 2.2, 1.0, 0.5), Fill::Solid(Color32::from_rgb(90, 170, 70))),
            ]);
            pen.stroke(&[at + vec2(0.0, 3.6), at + vec2(0.4, 6.0)], 0.7, Color32::from_rgb(90, 60, 40));
            pen.fill(&ellipse(at + vec2(-1.6, 1.6), 1.2, 0.8, 0.6), Fill::Solid(Color32::from_white_alpha(120)));
        }
        Prop::Leaf => {
            let a = 1.1 + (pose.time * 14.0).sin() * 0.25;
            let tip = at + vec2(a.cos(), a.sin()) * 9.0;
            pen.group(&[(ellipse(at + (tip - at) * 0.55, 6.4, 3.2, a), Fill::Grad(Color32::from_rgb(140, 210, 90), Color32::from_rgb(70, 150, 60)))]);
            pen.stroke(&[at, tip], 0.6, Color32::from_rgb(60, 110, 50));
        }
    }
}

/// A head-and-shoulders portrait, used for the dashboard chip.
pub fn draw_portrait(cv: &mut Canvas, centre: Pos2, size: f32, pal: &Palette, time: f32) {
    let mut pose = Pose { rear: 0.75, sit: 1.0, arms: 1.0, time, ear: 0.9, smile: 0.6, ..Pose::default() };
    pose.look = vec2(0.3, 0.1);
    pose.blink = if (time * 0.35).fract() < 0.04 { 0.1 } else { 1.0 };
    pose.tail_lift = 0.6;
    let s = size / 62.0;
    let sk = Skeleton::build(&pose, time);
    let head = sk.head;
    // Centre the head in the chip.
    let mut xf = Xform::simple(centre, s, 1.0);
    let probe = xf.apply(head + vec2(2.0, 0.0));
    xf.origin += centre - probe;
    let look = Look { pal, outline_width: 1.2, alpha: 1.0 };
    draw_pose(cv, &pose, xf, &look);
}

#[allow(dead_code)]
fn _unused() -> f32 {
    PI
}
