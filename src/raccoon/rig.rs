//! Procedural animation: a pose per activity, smoothly blended, turned into a
//! skeleton (hips, chest, head, IK legs, a spring tail) that the renderer
//! draws.
//!
//! Model space: units are roughly "raccoon centimetres", `y` points up, the
//! ground is `y = 0` and he faces `+x`. Standing on all fours he is about 60
//! units tall and 110 long including the tail.

use std::f32::consts::{PI, TAU};

use eframe::egui::{Pos2, Vec2, vec2};

use super::acts::Act;
use super::body::CENTRE_H;
use super::{Prop, World, approach};

pub const HIP_R: f32 = 15.5;
pub const CHEST_R: f32 = 12.5;
pub const HEAD_R: f32 = 13.5;
const SPINE: f32 = 25.0;
const STAND_HIP: f32 = 26.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Eyes {
    #[default]
    Normal,
    Wide,
    Happy,
    Closed,
    Swirl,
    Hearts,
}

/// Everything that describes one instant of animation. Continuous fields are
/// blended between activities; `eyes` switches halfway through a blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub rear: f32,
    pub sit: f32,
    pub crouch: f32,
    pub lie: f32,
    pub pitch: f32,
    pub bob: f32,
    pub lean: f32,
    pub gait: f32,
    pub gallop: f32,
    pub tuck: f32,
    pub dangle: f32,
    pub kick: f32,
    pub head_pitch: f32,
    pub head_roll: f32,
    pub head_turn: f32,
    pub head_drop: f32,
    pub eye_open: f32,
    pub pupil: f32,
    pub brow: f32,
    pub mouth: f32,
    pub smile: f32,
    pub tongue: f32,
    pub blush: f32,
    pub ear: f32,
    pub tail_lift: f32,
    pub tail_curl: f32,
    pub tail_wag: f32,
    pub tail_speed: f32,
    pub tail_front: f32,
    pub arms: f32,
    pub arm_near: Vec2,
    pub arm_far: Vec2,
    pub arms_front: f32,
    pub roll: f32,
    pub legs_up: f32,
    pub prop_hold: f32,
    pub squash: f32,
    // --- not blended: copied in every frame ---
    pub eyes: Eyes,
    pub prop: Prop,
    pub phase: f32,
    pub time: f32,
    pub tail_lag: f32,
    /// World "down" in model space, for dangling legs and tail.
    pub down: Vec2,
    pub look: Vec2,
    pub ear_twitch: f32,
    /// 1 = eyes open, 0 = mid-blink.
    pub blink: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            rear: 0.0,
            sit: 0.0,
            crouch: 0.0,
            lie: 0.0,
            pitch: 0.0,
            bob: 0.0,
            lean: 0.0,
            gait: 0.0,
            gallop: 0.0,
            tuck: 0.0,
            dangle: 0.0,
            kick: 0.0,
            head_pitch: 0.0,
            head_roll: 0.0,
            head_turn: 0.0,
            head_drop: 0.0,
            eye_open: 1.0,
            pupil: 1.0,
            brow: 0.0,
            mouth: 0.0,
            smile: 0.2,
            tongue: 0.0,
            blush: 0.0,
            ear: 0.75,
            tail_lift: 0.3,
            tail_curl: 0.0,
            tail_wag: 0.25,
            tail_speed: 2.2,
            tail_front: 0.0,
            arms: 0.0,
            arm_near: vec2(4.0, -13.0),
            arm_far: vec2(5.0, -12.0),
            arms_front: 0.0,
            roll: 0.0,
            legs_up: 0.0,
            prop_hold: 0.0,
            squash: 0.0,
            eyes: Eyes::Normal,
            prop: Prop::None,
            phase: 0.0,
            time: 0.0,
            tail_lag: 0.0,
            down: vec2(0.0, -1.0),
            look: Vec2::ZERO,
            ear_twitch: 0.0,
            blink: 1.0,
        }
    }
}

macro_rules! blend_fields {
    ($self:ident, $t:ident, $k:ident; $($f:ident),*) => {
        $( $self.$f += ($t.$f - $self.$f) * $k; )*
    };
}

impl Pose {
    /// Moves this pose a fraction `k` (0..1) of the way to `t`.
    pub fn blend(&mut self, t: &Pose, k: f32) {
        blend_fields!(self, t, k;
            rear, sit, crouch, lie, pitch, bob, lean, gait, gallop, tuck, dangle, kick,
            head_pitch, head_roll, head_turn, head_drop, eye_open, pupil, brow, mouth, smile,
            tongue, blush, ear, tail_lift, tail_curl, tail_wag, tail_speed, tail_front, arms,
            arms_front, roll, legs_up, prop_hold, squash);
        self.arm_near += (t.arm_near - self.arm_near) * k;
        self.arm_far += (t.arm_far - self.arm_far) * k;
        self.eyes = t.eyes;
        self.prop = t.prop;
    }
}

// ----------------------------------------------------------------------
// Skeleton
// ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Leg {
    pub root: Vec2,
    pub knee: Vec2,
    pub foot: Vec2,
    pub front: bool,
    pub near: bool,
    pub r0: f32,
    pub r1: f32,
    pub paw: f32,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub hip: Vec2,
    pub hip_r: f32,
    pub chest: Vec2,
    pub chest_r: f32,
    pub spine: f32,
    pub head: Vec2,
    pub head_r: f32,
    pub head_angle: f32,
    /// far hind, far front, near hind, near front
    pub legs: [Leg; 4],
    pub tail: Vec<(Vec2, f32)>,
}

fn rot(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn dir(a: f32) -> Vec2 {
    vec2(a.cos(), a.sin())
}

/// Two-bone IK. `bend` picks which way the joint points (+1: forward).
fn ik(root: Vec2, target: Vec2, a: f32, b: f32, bend: f32) -> (Vec2, Vec2) {
    let mut d = target - root;
    let len = d.length().clamp((a - b).abs() + 0.5, a + b - 0.05);
    if d.length() < 1e-4 {
        d = vec2(0.0, -1.0);
    }
    let dn = d.normalized();
    let foot = root + dn * len;
    let cos_a = ((a * a + len * len - b * b) / (2.0 * a * len)).clamp(-1.0, 1.0);
    let ang = cos_a.acos() * bend;
    let knee = root + rot(dn, ang) * a;
    (knee, foot)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Skeleton {
    pub fn build(p: &Pose, _time: f32) -> Self {
        let t = p.time;

        // --- hips and spine ---
        let lie_h = HIP_R * 0.82;
        let sit_h = HIP_R * 0.9;
        let mut hip_y = STAND_HIP - p.crouch * 8.0;
        hip_y += (lie_h - hip_y) * p.lie.clamp(0.0, 1.0);
        hip_y += (sit_h - hip_y) * p.sit.clamp(0.0, 1.0);
        hip_y += p.bob;
        // Running bounces the whole body.
        hip_y += -((p.phase * 2.0).sin().abs()) * 1.6 * p.gait * (0.4 + p.gallop);
        let hip_x = -13.0 + p.rear * 12.0 + p.lean * 0.3;
        let hip = vec2(hip_x, hip_y.max(HIP_R * 0.6));

        let gallop_flex = (p.phase).sin() * 0.14 * p.gallop * p.gait;
        let spine = p.rear * 1.22 + p.pitch + gallop_flex - p.lie * 0.04;
        let mut chest = hip + dir(spine) * SPINE;
        chest.x += p.lean;
        // Lying flat brings the chest down onto the ground too.
        chest.y += (CHEST_R * 0.8 - chest.y) * (p.lie * (1.0 - p.rear)).clamp(0.0, 1.0);

        // --- head ---
        let head_dir = 0.72 + p.rear * 0.35 - p.head_drop * 1.0 + p.pitch * 0.4;
        let mut head = chest + dir(head_dir) * 16.5;
        head.y -= p.head_drop * 3.0;
        head.y = head.y.max(HEAD_R * 0.7);
        let head_angle = spine * 0.2 + p.head_pitch + p.head_roll - p.head_drop * 0.25;

        // --- legs ---
        let ground = 0.0;
        let s_len = 7.0 + 5.0 * p.gallop;
        let lift = 4.5 + 3.5 * p.gallop;
        let trot = [PI, 0.0, 0.0, PI];
        let gal = [0.45, PI * 1.1 + 0.45, 0.0, PI * 1.1];
        let arm_w = smooth(0.35, 0.85, p.rear).max(p.arms).clamp(0.0, 1.0);

        let mut legs = [Leg {
            root: Vec2::ZERO,
            knee: Vec2::ZERO,
            foot: Vec2::ZERO,
            front: false,
            near: false,
            r0: 0.0,
            r1: 0.0,
            paw: 0.0,
        }; 4];
        for (i, leg) in legs.iter_mut().enumerate() {
            let front = i % 2 == 1;
            let near = i >= 2;
            let depth = if near { Vec2::ZERO } else { vec2(3.5, 1.2) };
            let (root, a, b, r0, r1, paw) = if front {
                (chest + rot(vec2(2.0, -5.5), spine) + depth, 10.2, 11.0, 5.4, 4.2, 4.3)
            } else {
                (hip + rot(vec2(-1.0, -6.0), spine) + depth, 10.0, 10.8, 7.0, 4.6, 4.7)
            };

            // Where the foot wants to be on the ground.
            let off = trot[i] + (gal[i] - trot[i]) * p.gallop;
            let ph = p.phase + off;
            let mut foot = vec2(
                root.x + if front { 1.5 } else { -1.0 } - s_len * p.gait * ph.cos(),
                ground + lift * p.gait * ph.sin().max(0.0),
            );
            // Sitting: hind feet come forward under the belly.
            if !front {
                foot.x += p.sit * 8.0;
            }
            // Crouching spreads the stance a little.
            foot.x += if front { 1.0 } else { -1.0 } * p.crouch * 2.5;
            // Lying: legs folded under, front paws stretched out ahead.
            let fold = vec2(if front { 7.0 } else { -4.0 }, ground + 0.6);
            foot = foot + (vec2(root.x, 0.0) + fold - foot) * p.lie.clamp(0.0, 1.0);

            // Airborne: tuck in or reach out.
            if p.tuck > 0.0 {
                let tucked = root + vec2(if front { 5.0 } else { -4.0 }, -9.0);
                foot = foot + (tucked - foot) * p.tuck.min(1.0);
            } else if p.tuck < 0.0 {
                let reach = root + vec2(if front { 11.0 } else { -11.0 }, -17.0);
                foot = foot + (reach - foot) * (-p.tuck).min(1.0);
            }

            // Hanging from the cursor: legs follow gravity and kick a bit.
            if p.dangle > 0.0 {
                let wiggle = (t * 17.0 + i as f32 * 1.7).sin() * 3.0;
                let hang = root + p.down * (a + b) * 0.9 + vec2(wiggle, 0.0);
                foot = foot + (hang - foot) * p.dangle.min(1.0);
            }

            // On his back with legs in the air.
            if p.legs_up > 0.0 {
                let wiggle = (t * 9.0 + i as f32 * 2.1).sin() * 3.0;
                let up = root + rot(vec2(if front { 4.0 } else { -3.0 } + wiggle, -16.0), spine);
                foot = foot + (up - foot) * p.legs_up.min(1.0);
            }

            // Front legs become arms when he rears up.
            if front {
                let target = root + if near { p.arm_near } else { p.arm_far };
                foot = foot + (target - foot) * arm_w;
            }

            // Scratching: near hind leg reaches up to the ear.
            if !front && near && p.kick > 0.0 {
                let scratch = root + vec2(7.0 + (t * 26.0).sin() * 2.5, 11.0 + (t * 26.0).cos() * 1.5);
                foot = foot + (scratch - foot) * p.kick.min(1.0);
            }

            let bend = if front { -1.0 } else { 1.0 };
            let (knee, foot) = ik(root, foot, a, b, bend);
            *leg = Leg { root, knee, foot, front, near, r0, r1, paw };
        }

        // --- tail ---
        let n = 7;
        let radii = [5.6, 7.2, 8.2, 8.4, 7.8, 6.6, 4.8, 2.8];
        let base = hip + rot(vec2(-HIP_R * 0.8, 3.5), spine);
        let mut tail = Vec::with_capacity(n + 1);
        tail.push((base, radii[0]));
        let mut ang = PI - 0.25 - p.tail_lift * 0.85 + spine * 0.8;
        // Curled: start down and wrap forward around the body.
        ang += p.tail_curl * 0.55;
        let mut at = base;
        let down_ang = p.down.y.atan2(p.down.x);
        for i in 0..n {
            let k = (i + 1) as f32 / n as f32;
            let wag = p.tail_wag * (t * p.tail_speed * TAU * 0.5 - i as f32 * 0.7).sin() * 0.16;
            let curve = -0.12 * (1.0 - p.tail_curl) + p.tail_curl * 0.42;
            ang += curve + wag + p.tail_lag * 0.12;
            if p.dangle > 0.0 {
                // Sag toward the ground while carried.
                let mut d = down_ang - ang;
                d = (d + PI).rem_euclid(TAU) - PI;
                ang += d * 0.16 * p.dangle;
            }
            at += dir(ang) * 5.6;
            let r = radii[i + 1] * (1.0 - 0.15 * p.tail_curl * k);
            tail.push((at, r));
        }

        Skeleton {
            hip,
            hip_r: HIP_R,
            chest,
            chest_r: CHEST_R,
            spine,
            head,
            head_r: HEAD_R,
            head_angle,
            legs,
            tail,
        }
    }

    /// Converts a point in head-local space (facing +x, y up) to model space.
    pub fn head_pt(&self, local: Vec2) -> Vec2 {
        self.head + rot(local, self.head_angle)
    }

    /// Where held props go: just in front of the near forepaw.
    pub fn paw(&self) -> Vec2 {
        self.legs[3].foot
    }
}

// ----------------------------------------------------------------------
// Model -> screen
// ----------------------------------------------------------------------

/// Maps model space onto the screen for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Xform {
    pub origin: Pos2,
    pub s: f32,
    pub fx: f32,
    pub sx: f32,
    pub sy: f32,
    pub pose_roll: f32,
    pub pose_pivot: Vec2,
    pub roll: f32,
}

impl Xform {
    pub fn new(w: &World) -> Self {
        let face = w.face;
        let fx = if face.abs() < 0.12 { 0.12 * if face < 0.0 { -1.0 } else { 1.0 } } else { face };
        let sq = (w.squash + w.pose.squash).clamp(-0.7, 0.8);
        Self {
            origin: w.pos,
            s: w.scale(),
            fx,
            sx: 1.0 + sq * 0.22,
            sy: 1.0 - sq * 0.26,
            pose_roll: w.pose.roll,
            pose_pivot: vec2(0.0, 26.0),
            roll: w.roll,
        }
    }

    /// For previews: a plain transform with no world attached.
    pub fn simple(origin: Pos2, s: f32, face: f32) -> Self {
        Self { origin, s, fx: face, sx: 1.0, sy: 1.0, pose_roll: 0.0, pose_pivot: vec2(0.0, 26.0), roll: 0.0 }
    }

    pub fn apply(&self, m: Vec2) -> Pos2 {
        let p = rot(m - self.pose_pivot, self.pose_roll) + self.pose_pivot;
        let p = vec2(p.x * self.sx * self.fx, p.y * self.sy);
        let v = vec2(p.x * self.s, -(p.y - CENTRE_H) * self.s);
        let (sn, cs) = self.roll.sin_cos();
        let r = vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs);
        self.origin + vec2(0.0, -CENTRE_H * self.s) + r
    }

    /// A screen-space direction expressed in model space (ignores squash and
    /// the pose's own roll).
    pub fn dir_to_model(&self, d: Vec2) -> Vec2 {
        let (sn, cs) = self.roll.sin_cos();
        let v = vec2(d.x * cs + d.y * sn, -d.x * sn + d.y * cs);
        let m = vec2(v.x / self.fx.signum(), -v.y);
        let m = rot(m, -self.pose_roll);
        if m.length() > 1e-5 { m.normalized() } else { vec2(0.0, -1.0) }
    }
}

// ----------------------------------------------------------------------
// Activity -> pose
// ----------------------------------------------------------------------

fn bell(x: f32) -> f32 {
    // 0 -> 1 -> 0 over 0..1
    (x.clamp(0.0, 1.0) * PI).sin()
}

impl World {
    /// The pose this activity wants right now.
    pub fn target_pose(&self) -> Pose {
        let mut p = Pose::default();
        let t = self.act_t;
        let tt = self.time;
        let dur = self.act_dur.max(0.01);
        let prog = (t / dur).clamp(0.0, 1.0);
        let speed = if self.support.is_some() { self.vel.x.abs() } else { 0.0 };
        p.gait = (speed / 28.0).clamp(0.0, 1.0);
        p.gallop = ((speed - 110.0) / 90.0).clamp(0.0, 1.0);
        // Breathing.
        p.bob = (tt * 2.1).sin() * 0.35;

        match self.act {
            Act::Idle => {
                p.head_pitch = (tt * 0.7).sin() * 0.06;
                p.tail_wag = 0.3;
            }
            Act::LookAround => {
                p.head_turn = (t * 1.9).sin() * 0.9;
                p.head_pitch = 0.08 + (t * 1.1).sin() * 0.12;
                p.ear = 0.95;
                p.rear = 0.2;
            }
            Act::Sit => {
                p.sit = 1.0;
                p.rear = 0.72;
                p.arms = 1.0;
                p.arm_near = vec2(5.0, -9.0);
                p.arm_far = vec2(6.0, -8.5);
                p.head_pitch = 0.08;
                p.tail_lift = -0.45;
                p.smile = 0.4;
            }
            Act::Loaf => {
                p.lie = 1.0;
                p.head_drop = 0.35;
                p.eye_open = 0.62;
                p.tail_lift = -0.6;
                p.ear = 0.55;
                p.tail_wag = 0.1;
            }
            Act::Sleep => {
                p.lie = 1.0;
                p.head_drop = 1.0;
                p.head_pitch = -0.1;
                p.eyes = Eyes::Closed;
                p.eye_open = 0.0;
                p.tail_curl = 1.0;
                p.tail_front = 1.0;
                p.tail_wag = 0.04;
                p.bob = (tt * 1.2).sin() * 0.7;
                p.ear = 0.35;
                p.smile = 0.35;
                p.blush = 0.2;
            }
            Act::Wake => {
                p.rear = 0.25 * prog;
                p.eye_open = smooth(0.2, 0.7, prog);
                p.head_roll = (t * 18.0).sin() * 0.14 * (1.0 - prog);
                p.ear = 0.4 + 0.5 * prog;
                p.lie = 1.0 - prog;
            }
            Act::Yawn => {
                let b = bell(prog);
                p.sit = 0.7;
                p.rear = 0.45;
                p.mouth = b;
                p.eye_open = 1.0 - b * 0.85;
                p.head_pitch = 0.4 * b;
                p.ear = 0.5;
                p.arms = 0.6;
            }
            Act::Stretch => {
                let b = bell(prog);
                p.pitch = -0.38 * b;
                p.lean = 5.0 * b;
                p.tail_lift = 0.3 + 0.7 * b;
                p.eyes = if b > 0.5 { Eyes::Happy } else { Eyes::Normal };
                p.mouth = 0.4 * b;
                p.head_pitch = 0.25 * b;
            }
            Act::Groom => {
                let lick = ((t * 3.0).sin() * 0.5 + 0.5).powi(2);
                p.sit = 1.0;
                p.rear = 0.75;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(10.5 + lick * 1.5, 9.0 + lick * 2.0);
                p.arm_far = vec2(6.0, -8.0);
                p.head_pitch = -0.12 - lick * 0.12;
                p.head_roll = (t * 1.3).sin() * 0.08;
                p.tongue = lick;
                p.mouth = lick * 0.3;
                p.eye_open = 0.35;
                p.eyes = Eyes::Happy;
            }
            Act::WashPaws => {
                let rub = (t * 13.0).sin() * 2.6;
                p.sit = 1.0;
                p.rear = 0.6;
                p.crouch = 0.2;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(9.0 + rub, -6.0);
                p.arm_far = vec2(10.0 - rub, -5.5);
                p.head_pitch = -0.3;
                p.head_drop = 0.15;
                p.ear = 0.9;
                p.tail_wag = 0.45;
            }
            Act::Scratch => {
                p.sit = 1.0;
                p.rear = 0.45;
                p.kick = 1.0;
                p.head_roll = 0.28;
                p.eye_open = 0.3;
                p.eyes = Eyes::Happy;
                p.tongue = 0.5;
                p.arms = 0.5;
                p.arm_near = vec2(3.0, -12.0);
            }
            Act::Sniff => {
                p.crouch = 0.35;
                p.pitch = -0.14;
                p.head_drop = 0.75;
                p.head_pitch = -0.25 + (tt * 17.0).sin() * 0.03;
                p.ear = 0.95;
                p.tail_wag = 0.45;
                p.tail_speed = 3.5;
            }
            Act::Dig => {
                let a = (t * 15.0).sin();
                let b = (t * 15.0 + PI).sin();
                p.crouch = 0.4;
                p.pitch = -0.28;
                p.head_drop = 0.55;
                p.arms = 1.0;
                p.arm_near = vec2(9.0 + a * 4.5, -15.0 + a.max(0.0) * 6.0);
                p.arm_far = vec2(9.0 + b * 4.5, -15.0 + b.max(0.0) * 6.0);
                p.tail_lift = 0.85;
                p.tail_wag = 0.7;
                p.tail_speed = 4.0;
                p.ear = 0.9;
            }
            Act::Snack => {
                let chew = (t * 10.0).sin() * 0.5 + 0.5;
                p.sit = 1.0;
                p.rear = 0.82;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(10.5, 7.5);
                p.arm_far = vec2(11.0, 8.0);
                p.prop_hold = if t > 0.3 { 1.0 } else { 0.0 };
                p.mouth = chew * 0.55;
                p.eyes = Eyes::Happy;
                p.eye_open = 0.5;
                p.blush = 0.35;
                p.smile = 0.6;
                p.head_pitch = -0.05;
            }
            Act::Wave => {
                let w = (t * 11.0).sin();
                p.rear = 1.0;
                p.sit = 0.25;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(18.0, -2.0 + w * 4.5);
                p.arm_far = vec2(5.0, -9.0);
                p.smile = 1.0;
                p.mouth = 0.35;
                p.eyes = if t < 0.5 { Eyes::Happy } else { Eyes::Normal };
                p.tail_wag = 0.8;
                p.tail_speed = 4.0;
            }
            Act::Dance => {
                let beat = t * 6.0;
                p.rear = 1.0;
                p.sit = 0.1;
                p.bob = beat.sin().abs() * 3.0;
                p.lean = (beat * 0.5).sin() * 4.0;
                p.head_roll = beat.sin() * 0.2;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(16.0, -3.0 + 5.0 * beat.sin());
                p.arm_far = vec2(16.0, -3.0 - 5.0 * beat.sin());
                p.smile = 1.0;
                p.mouth = 0.4;
                p.eyes = Eyes::Happy;
                p.tail_wag = 1.0;
                p.tail_speed = 6.0;
            }
            Act::TailChase => {
                p.crouch = 0.25;
                p.head_turn = -0.7;
                p.gait = 0.8;
                p.tail_wag = 1.0;
                p.tail_speed = 6.0;
                p.smile = 0.8;
                p.mouth = 0.4;
                p.tongue = 0.6;
            }
            Act::RollOver => {
                let up = smooth(0.0, 0.25, prog) * (1.0 - smooth(0.8, 1.0, prog));
                p.lie = 1.0;
                p.roll = PI * up;
                p.legs_up = up;
                p.bob = -3.0 * up;
                p.smile = 1.0;
                p.eyes = Eyes::Happy;
                p.tongue = 0.6 * up;
                p.tail_wag = 0.9;
                p.tail_speed = 5.0;
            }
            Act::Sneeze => {
                let wind = smooth(0.0, 0.7, t);
                let snap = smooth(0.7, 0.8, t) * (1.0 - smooth(0.85, 1.1, t));
                p.rear = 0.3;
                p.head_pitch = 0.35 * wind * (1.0 - snap) - 0.45 * snap;
                p.eye_open = 1.0 - wind * 0.7;
                p.eyes = if snap > 0.2 { Eyes::Closed } else { Eyes::Normal };
                p.mouth = wind * 0.5 * (1.0 - snap) + snap;
                p.ear = 0.4 + snap * 0.6;
            }
            Act::Hop | Act::Jump | Act::Pounce => {
                let vy = self.vel.y;
                p.tuck = if vy < 0.0 { 0.65 } else { -0.7 };
                p.pitch = (-vy / 2400.0).clamp(-0.35, 0.35);
                p.ear = 0.4;
                p.mouth = 0.25;
                p.smile = 0.8;
                if self.act == Act::Pounce {
                    p.eyes = Eyes::Wide;
                    p.pupil = 1.4;
                    p.mouth = 0.7;
                }
            }
            Act::Flip => {
                let k = smooth(0.0, 1.0, (t / 0.7).min(1.0));
                p.roll = TAU * k;
                p.tuck = 1.0;
                p.eyes = Eyes::Happy;
                p.smile = 1.0;
                p.mouth = 0.5;
            }
            Act::Curious => {
                p.head_roll = 0.32 * (t * 1.4).sin().signum();
                p.ear = 1.0;
                p.pupil = 1.25;
                p.rear = 0.35;
                p.tail_wag = 0.5;
                p.smile = 0.3;
            }
            Act::Wander | Act::Explore | Act::FollowCursor => {
                p.head_pitch = (self.gait_phase * 2.0).sin() * 0.04;
                p.ear = 0.8;
                p.tail_wag = 0.3;
                p.tail_lift = 0.35;
            }
            Act::Run | Act::ChaseCursor => {
                p.ear = 0.35;
                p.tail_lift = 0.55;
                p.tail_wag = 0.15;
                p.mouth = 0.35;
                p.tongue = 0.3;
                p.smile = 0.7;
                if self.act == Act::ChaseCursor {
                    p.eyes = Eyes::Wide;
                    p.pupil = 1.3;
                }
            }
            Act::Stalk => {
                p.crouch = 0.9;
                p.pitch = -0.04;
                p.head_drop = 0.15;
                p.bob = (tt * 15.0).sin() * 0.6;
                p.lean = (tt * 15.0).sin() * 1.5;
                p.eyes = Eyes::Wide;
                p.pupil = 1.5;
                p.ear = 1.0;
                p.tail_lift = -0.1;
                p.tail_wag = 0.8;
                p.tail_speed = 7.0;
            }
            Act::Climb => {
                let c = (self.gait_phase).sin();
                p.rear = 1.0;
                p.arms = 1.0;
                p.arm_near = vec2(7.0, 8.0 + c * 4.0);
                p.arm_far = vec2(8.0, 4.0 - c * 4.0);
                p.head_pitch = 0.35;
                p.ear = 0.9;
                p.tail_lift = -0.8;
                p.tail_wag = 0.5;
            }
            Act::Mantle => {
                p.crouch = 0.5;
                p.pitch = -0.1;
                p.ear = 0.9;
            }
            Act::Fall => {
                p.tuck = -0.8;
                p.eyes = Eyes::Wide;
                p.pupil = 0.8;
                p.mouth = 0.55;
                p.smile = -0.2;
                p.ear = 1.0;
                p.tail_lift = 0.9;
                p.brow = 0.7;
            }
            Act::Land => {
                p.crouch = 0.6 * bell(prog);
                p.ear = 0.55;
            }
            Act::Alert => {
                p.rear = 0.65;
                p.eyes = Eyes::Wide;
                p.pupil = 0.65;
                p.mouth = 0.4;
                p.smile = -0.3;
                p.ear = 1.0;
                p.brow = 1.0;
                p.tail_lift = 1.0;
                p.tail_wag = 0.0;
                p.arms = 0.6;
                p.arm_near = vec2(7.0, 2.0);
                p.arm_far = vec2(8.0, 3.0);
            }
            Act::Panic => {
                p.eyes = Eyes::Wide;
                p.pupil = 0.6;
                p.mouth = 0.85;
                p.smile = -0.6;
                p.brow = 1.0;
                p.ear = 0.2;
                p.tail_lift = 1.0;
                p.tail_wag = 0.9;
                p.tail_speed = 8.0;
            }
            Act::Hot => {
                let fan = (t * 14.0).sin();
                p.sit = 1.0;
                p.rear = 0.72;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(9.0, 13.0 + fan * 3.5);
                p.arm_far = vec2(6.0, -8.0);
                p.prop_hold = 1.0;
                p.tongue = 1.0;
                p.mouth = 0.55;
                p.eye_open = 0.55;
                p.brow = 0.7;
                p.blush = 0.8;
                p.ear = 0.35;
            }
            Act::Tired => {
                p.lie = 0.5;
                p.crouch = 0.6;
                p.head_drop = 0.55;
                p.eye_open = 0.32;
                p.brow = 0.7;
                p.ear = 0.1;
                p.tail_lift = -0.7;
                p.tail_wag = 0.05;
                p.smile = -0.3;
            }
            Act::Cheer => {
                let hop = (t * 9.0).sin().abs();
                p.rear = 1.0;
                p.bob = hop * 6.0;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(18.0, 0.0 + hop * 3.0);
                p.arm_far = vec2(19.0, -3.0 + hop * 3.0);
                p.eyes = Eyes::Happy;
                p.mouth = 1.0;
                p.smile = 1.0;
                p.ear = 1.0;
                p.tail_wag = 1.0;
                p.tail_speed = 7.0;
                p.squash = -0.1 * hop;
            }
            Act::Stuffed => {
                p.sit = 1.0;
                p.rear = 0.55;
                p.arms = 1.0;
                p.arms_front = 1.0;
                p.arm_near = vec2(6.0 + (t * 5.0).sin() * 2.0, -3.0);
                p.arm_far = vec2(6.0, -6.0);
                p.squash = 0.18;
                p.eyes = Eyes::Happy;
                p.smile = 0.5;
                p.blush = 0.4;
                p.ear = 0.5;
            }
            Act::Watch => {
                p.rear = 0.7;
                p.sit = 0.6;
                p.head_pitch = 0.42;
                p.eyes = Eyes::Wide;
                p.pupil = 1.3;
                p.mouth = 0.35;
                p.ear = 1.0;
                p.tail_wag = 0.6;
                p.arms = 0.8;
                p.arm_near = vec2(7.0, -2.0);
                p.arm_far = vec2(8.0, -1.5);
            }
            Act::Petted => {
                p.sit = 0.7;
                p.rear = 0.4;
                p.crouch = 0.2;
                p.eyes = if self.affection > 0.85 { Eyes::Hearts } else { Eyes::Happy };
                p.smile = 1.0;
                p.blush = 1.0;
                p.ear = 0.25;
                p.head_roll = (tt * 3.0).sin() * 0.14;
                p.tail_wag = 0.9;
                p.tail_speed = 5.0;
                p.bob += (tt * 30.0).sin() * 0.15;
            }
            Act::Booped => {
                let early = t < 0.3;
                p.crouch = 0.5 * bell(prog);
                p.eyes = if early { Eyes::Closed } else { Eyes::Wide };
                p.eye_open = if early { 0.0 } else { 1.0 };
                p.mouth = if early { 0.0 } else { 0.4 };
                p.head_pitch = if early { -0.2 } else { 0.1 };
                p.head_roll = 0.2;
                p.ear = if early { 0.1 } else { 1.0 };
                p.squash = if early { 0.2 } else { 0.0 };
            }
            Act::Annoyed => {
                let stomp = (t * 10.0).sin().max(0.0);
                p.rear = 0.5;
                p.sit = 0.4;
                p.brow = -1.0;
                p.smile = -1.0;
                p.eye_open = 0.75;
                p.ear = 0.15;
                p.arms = 1.0;
                p.arm_near = vec2(8.0, -12.0 + stomp * 6.0);
                p.arm_far = vec2(6.0, -9.0);
                p.tail_lift = 1.0;
                p.tail_wag = 0.6;
                p.tail_speed = 6.0;
                p.head_roll = (t * 14.0).sin() * 0.05;
            }
            Act::Dragged => {
                p.dangle = 1.0;
                p.eyes = Eyes::Wide;
                p.mouth = 0.45;
                p.smile = -0.1;
                p.ear = 1.0;
                p.brow = 0.6;
                p.rear = 0.25;
            }
            Act::Thrown | Act::Bounce => {
                p.tuck = -0.4 + (tt * 20.0).sin() * 0.3;
                p.eyes = if self.act == Act::Bounce { Eyes::Closed } else { Eyes::Wide };
                p.eye_open = if self.act == Act::Bounce { 0.0 } else { 1.0 };
                p.mouth = 1.0;
                p.smile = 0.3;
                p.ear = 0.1;
                p.tail_lift = 1.0;
            }
            Act::Splat => {
                p.rear = 1.0;
                p.arms = 1.0;
                p.arm_near = vec2(10.0, 8.0);
                p.arm_far = vec2(11.0, -6.0);
                p.eyes = Eyes::Swirl;
                p.mouth = 0.6;
                p.ear = 0.0;
                p.tail_lift = -1.0;
            }
            Act::Dizzy => {
                p.sit = 1.0;
                p.rear = 0.5;
                p.head_roll = (tt * 4.0).sin() * 0.3;
                p.lean = (tt * 3.0).sin() * 2.5;
                p.eyes = Eyes::Swirl;
                p.mouth = 0.3;
                p.smile = -0.2;
                p.ear = 0.2;
                p.tail_wag = 0.1;
            }
            Act::GetUp => {
                p.crouch = 0.5 * (1.0 - prog);
                p.eye_open = 0.6 + 0.4 * prog;
                p.head_roll = (t * 15.0).sin() * 0.1 * (1.0 - prog);
            }
        }

        // The prop only shows while it is being held.
        p.prop = if p.prop_hold > 0.5 { self.prop } else { Prop::None };
        p
    }

    /// Advances blending, gait, blinking and eye tracking.
    pub(crate) fn animate(&mut self, dt: f32) {
        self.body_springs(dt);
        let speed = self.cfg.animation_speed;
        let adt = dt * speed;

        // Gait phase is driven by distance actually covered, so the feet
        // never slide.
        let s = self.scale();
        let moved = if self.climbing().is_some() {
            self.cfg.climb_speed * dt
        } else if self.support.is_some() {
            self.vel.x.abs() * dt
        } else {
            0.0
        };
        let stride = (28.0 + 20.0 * self.pose.gallop) * s;
        self.gait_phase = (self.gait_phase + moved / stride * TAU).rem_euclid(TAU * 64.0);
        if self.act == Act::TailChase {
            self.gait_phase += adt * 12.0;
        }

        let mut target = self.target_pose();
        // Fast blends for snappy reactions, slow ones for settling down.
        let rate = match self.act {
            Act::Booped | Act::Alert | Act::Splat | Act::Flip | Act::Dragged => 22.0,
            Act::Sleep | Act::Loaf => 3.5,
            Act::Jump | Act::Hop | Act::Pounce | Act::Fall | Act::Thrown => 14.0,
            _ => 9.0,
        } * speed;
        // A full flip must not be blended the short way round.
        if self.act == Act::Flip || (self.pose.roll - target.roll).abs() > PI * 1.5 {
            self.pose.roll = target.roll;
        }
        let k = 1.0 - (-rate * dt).exp();
        self.pose.blend(&target, k);

        // Blinking.
        self.blink_t += adt;
        if self.blink_t > self.next_blink {
            self.blink_t = 0.0;
            self.next_blink = self.rng.range(2.0, 6.0);
            if self.rng.chance(0.2) {
                self.next_blink = 0.25;
            }
        }
        let blink = if self.blink_t < 0.14 { 1.0 - bell(self.blink_t / 0.14) } else { 1.0 };

        // Ear twitches now and then.
        if self.rng.chance(dt * 0.25) {
            self.ear_twitch = 1.0;
        }
        self.ear_twitch = approach(self.ear_twitch, 0.0, 9.0, dt);

        // Eyes follow the cursor.
        let xf = self.xform();
        let sk = Skeleton::build(&self.pose, self.time);
        let head = xf.apply(sk.head);
        let mut look = Vec2::ZERO;
        if self.cfg.eyes_follow_cursor
            && let Some(c) = self.cursor
        {
            let d = c - head;
            let dist = d.length();
            if dist < 900.0 && dist > 1.0 {
                let m = xf.dir_to_model(d);
                look = m * (dist / 60.0).clamp(0.0, 1.0);
            }
        }
        if look == Vec2::ZERO {
            look = vec2(0.6, 0.0);
        }
        self.look = self.look + (look - self.look) * (1.0 - (-14.0 * dt).exp());

        target = self.pose;
        target.time = self.time * speed;
        target.phase = self.gait_phase;
        target.tail_lag = self.tail_lag;
        target.down = xf.dir_to_model(vec2(0.0, 1.0));
        target.look = self.look;
        target.ear_twitch = self.ear_twitch;
        target.blink = if matches!(target.eyes, Eyes::Normal | Eyes::Wide) { blink } else { 1.0 };
        self.pose = target;

        self.fx.scale = s;
        self.fx.enabled = self.cfg.particles;
    }
}
