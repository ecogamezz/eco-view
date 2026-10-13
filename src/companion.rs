use std::f32::consts::{PI, TAU};

use eframe::egui::{self, Color32, Mesh, Order, Pos2, Rect, Sense, Shape, Vec2};

use crate::icons::{self, Icon};
use crate::metrics::Snapshot;
use crate::theme::Palette;

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn norm(self) -> Self {
        let l = (self.dot(self)).sqrt().max(0.0001);
        self.scale(1.0 / l)
    }
}

struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    color: Color32,
}

#[derive(Clone, Copy)]
enum Act {
    Idle,
    Look,
    Walk,
    Jump,
    Alert,
    Cheer,
    Sleep,
}

pub struct Companion {
    pub enabled: bool,
    pos: Pos2,
    target: Pos2,
    vel: Vec2,
    on_ground: bool,
    act: Act,
    act_t: f32,
    walk_phase: f32,
    look_yaw: f32,
    look_pitch: f32,
    jump_t: f32,
    jump_from: Pos2,
    jump_to: Pos2,
    time: f32,
    last_cpu: f32,
    mood: f32,
    /// Time until which a load-drop celebration stays eligible. `mood` alone
    /// decays too fast for the current action's timer to expire.
    cheer_until: f32,
    /// Current window rect, used to keep the NPC inside the visible area.
    bounds: Rect,
    facing: f32,
    chip_pos: Option<Pos2>,
    /// Set for the single frame a drag finishes, so the caller can save once
    /// instead of rewriting settings on every frame of the drag.
    chip_settled: bool,
}

impl Companion {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            pos: Pos2::new(240.0, 220.0),
            target: Pos2::new(240.0, 220.0),
            vel: Vec2::ZERO,
            on_ground: true,
            act: Act::Idle,
            act_t: 1.5,
            walk_phase: 0.0,
            look_yaw: 0.0,
            look_pitch: 0.0,
            jump_t: 0.0,
            jump_from: Pos2::new(240.0, 220.0),
            jump_to: Pos2::new(240.0, 220.0),
            time: 0.0,
            last_cpu: 0.0,
            mood: 0.0,
            cheer_until: 0.0,
            bounds: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0)),
            facing: 1.0,
            chip_pos: None,
            chip_settled: false,
        }
    }

    /// Last known window-space position of the floating chip, so it can be
    /// persisted. `movable(true)` already handles the drag; we just record it.
    pub fn chip_pos(&self) -> Option<[f32; 2]> {
        self.chip_pos.map(|p| [p.x, p.y])
    }

    pub fn set_chip_pos(&mut self, pos: [f32; 2]) {
        self.chip_pos = Some(Pos2::new(pos[0], pos[1]));
    }

    /// True on the frame a drag ends and the chip landed somewhere new.
    pub fn chip_settled(&self) -> bool {
        self.chip_settled
    }

    pub fn paint_toggle(&mut self, ctx: &egui::Context, pal: Palette) -> bool {
        let mut clicked = false;
        egui::Area::new(egui::Id::new("eco-companion-chip"))
            .order(Order::Foreground)
            .movable(true)
            .constrain(true)
            .default_pos(self.chip_pos.unwrap_or(Pos2::new(24.0, 72.0)))
            .show(ctx, |ui| {
                ui.set_width(52.0);
                let (resp, painter) = ui.allocate_painter(Vec2::splat(52.0), Sense::click());
                // `movable(true)` makes the Area itself the drag target, so the inner click
                // widget's `resp.dragged()` is never true. Latch on the pointer
                // being released instead, which fires once per drag.
                let released = ui.input(|i| i.pointer.any_released());
                let prev = self.chip_pos;
                let now = resp.rect.min;
                self.chip_settled = released && prev.is_some_and(|p| (p - now).length() > 0.5);
                self.chip_pos = Some(now);
                let glow = if self.enabled {
                    pal.accent
                } else {
                    pal.muted
                };
                painter.circle_filled(resp.rect.center(), 24.0, Color32::from_rgba_unmultiplied(12, 14, 22, 230));
                painter.circle_stroke(
                    resp.rect.center(),
                    24.0,
                    egui::Stroke::new(2.0_f32, glow),
                );
                icons::paint(
                    &painter,
                    resp.rect.center() - Vec2::splat(16.0),
                    32.0,
                    Icon::Raccoon,
                    Color32::WHITE,
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.enabled = !self.enabled;
                    clicked = true;
                }
                resp.on_hover_text(if self.enabled {
                    "Raccoon companion on — drag this chip anywhere"
                } else {
                    "Raccoon companion off — drag this chip anywhere"
                });
            });
        clicked
    }

    pub fn tick_and_draw(
        &mut self,
        ctx: &egui::Context,
        platforms: &[Rect],
        snap: &Snapshot,
        pal: Palette,
        dt: f32,
    ) {
        if !self.enabled {
            return;
        }
        let screen = ctx.screen_rect();
        self.bounds = screen;
        let owned: Vec<Rect> = if platforms.is_empty() {
            vec![Rect::from_min_max(
                Pos2::new(screen.left() + 80.0, screen.top() + 90.0),
                Pos2::new(screen.right() - 80.0, screen.bottom() - 80.0),
            )]
        } else {
            platforms.to_vec()
        };
        self.time += dt;
        self.think(&owned, snap, dt);
        self.integrate(&owned, dt);
        paint_raccoon(ctx, self, pal);
    }

    fn think(&mut self, platforms: &[Rect], snap: &Snapshot, dt: f32) {
        // `cpu_global` only moves on sensor refresh ticks, so a delta detected
        // on one tick has to latch until the next one instead of decaying away
        // within a couple of frames.
        let cpu_delta = snap.cpu_global - self.last_cpu;
        if cpu_delta.abs() > 0.01 {
            self.last_cpu = snap.cpu_global;
            if cpu_delta < -18.0 {
                self.cheer_until = self.time + 2.0;
            }
        }
        if snap.cpu_global > 88.0 || snap.gpu.load.unwrap_or(0.0) > 90.0 {
            self.mood = 1.0;
        } else if self.time < self.cheer_until {
            self.mood = -0.4;
        } else {
            // Frame-rate independent decay.
            self.mood *= (-dt * 1.5).exp();
        }

        self.act_t -= dt;
        match self.act {
            Act::Jump => {
                self.jump_t += dt / 0.55;
                let t = self.jump_t.clamp(0.0, 1.0);
                let lift = 4.0 * t * (1.0 - t);
                self.pos = lerp_pos(self.jump_from, self.jump_to, t);
                self.pos.y -= lift * 54.0;
                self.on_ground = false;
                if t >= 1.0 {
                    self.pos = self.jump_to;
                    self.on_ground = true;
                    self.act = Act::Idle;
                    self.act_t = 0.4 + hash(self.time) * 0.8;
                }
                return;
            }
            Act::Walk | Act::Alert => {
                self.walk_phase += dt * if matches!(self.act, Act::Alert) { 14.0 } else { 9.0 };
                let speed = if matches!(self.act, Act::Alert) { 170.0 } else { 95.0 };
                let dir = self.target - self.pos;
                let dist = dir.length();
                if dist > 4.0 {
                    let step = dir.normalized() * speed * dt;
                    self.pos += step;
                    self.facing = if dir.x < 0.0 { -1.0 } else { 1.0 };
                } else {
                    self.act = Act::Idle;
                    self.act_t = 0.6;
                }
            }
            Act::Look => {
                self.look_yaw = (self.time * 1.7).sin() * 0.55;
                self.look_pitch = (self.time * 1.1).cos() * 0.18;
            }
            Act::Cheer => {
                // `act_t` counts down, so a raw hop would decay instead of
                // bounce. Use elapsed time for the sine.
                self.walk_phase += dt * 12.0;
            }
            Act::Sleep => {
                self.look_pitch = 0.35;
            }
            Act::Idle => {
                self.look_yaw *= 0.9;
                self.look_pitch *= 0.9;
            }
        }

        if self.act_t > 0.0 {
            return;
        }
        if self.mood > 0.7 {
            self.act = Act::Alert;
            self.act_t = 2.2;
            self.target = pick_platform_top(platforms, self.pos, self.time + 3.1);
            return;
        }
        if self.mood < -0.2 {
            self.act = Act::Cheer;
            self.act_t = 1.6;
            self.mood = 0.0;
            return;
        }
        if snap.cpu_global < 8.0 && hash(self.time) > 0.72 {
            self.act = Act::Sleep;
            self.act_t = 3.5;
            return;
        }

        let roll = hash(self.time * 1.7);
        if roll < 0.28 {
            self.act = Act::Look;
            self.act_t = 1.4;
        } else if roll < 0.55 && platforms.len() >= 2 {
            let dest = pick_platform_top(platforms, self.pos, self.time);
            self.start_jump(dest);
        } else {
            self.act = Act::Walk;
            self.act_t = 3.0;
            self.target = pick_platform_top(platforms, self.pos, self.time + 1.3);
        }
    }

    fn start_jump(&mut self, dest: Pos2) {
        self.act = Act::Jump;
        self.jump_t = 0.0;
        self.jump_from = self.pos;
        self.jump_to = dest;
        self.act_t = 0.6;
        self.facing = if dest.x < self.pos.x { -1.0 } else { 1.0 };
    }

    fn integrate(&mut self, platforms: &[Rect], dt: f32) {
        if matches!(self.act, Act::Jump) {
            return;
        }
        let mut grounded = None;
        for plat in platforms {
            let top = plat.top() - 2.0;
            if self.pos.x > plat.left() + 8.0
                && self.pos.x < plat.right() - 8.0
                && (self.pos.y - top).abs() < 18.0
            {
                grounded = Some(top);
                break;
            }
        }
        if let Some(top) = grounded {
            self.pos.y += (top - self.pos.y) * (1.0 - (-dt * 12.0).exp());
            self.on_ground = true;
        } else {
            self.vel.y += 420.0 * dt;
            self.pos.y += self.vel.y * dt;
            self.on_ground = false;
            if let Some(plat) = platforms
                .iter()
                .filter(|p| self.pos.x > p.left() && self.pos.x < p.right() && self.pos.y >= p.top() - 6.0 && self.pos.y < p.top() + 24.0)
                .min_by(|a, b| a.top().partial_cmp(&b.top()).unwrap_or(std::cmp::Ordering::Equal))
            {
                self.pos.y = plat.top();
                self.vel.y = 0.0;
                self.on_ground = true;
            }
        }
        // Recover from wandering or falling off the edge. Clamping to the screen keeps
        // the raccoon roughly where it was instead of teleporting it to the
        // centre of an unrelated card.
        let screen = self.bounds;
        if self.pos.x < screen.left() + 30.0 || self.pos.x > screen.right() - 30.0 {
            self.pos.x = self.pos.x.clamp(screen.left() + 30.0, screen.right() - 30.0);
            self.vel.x = 0.0;
            self.target.x = self.pos.x;
        }
        if self.pos.y > screen.bottom() - 20.0 {
            self.pos.y = platforms
                .iter()
                .filter(|p| self.pos.x > p.left() && self.pos.x < p.right())
                .map(|p| p.top())
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(self.pos.y);
            self.vel = Vec2::ZERO;
        }
    }
}

fn lerp_pos(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn hash(t: f32) -> f32 {
    let x = (t * 12.9898).sin() * 43758.545;
    x.fract().abs()
}

fn pick_platform_top(platforms: &[Rect], from: Pos2, seed: f32) -> Pos2 {
    if platforms.is_empty() {
        return from;
    }
    let idx = ((hash(seed) * platforms.len() as f32) as usize).min(platforms.len() - 1);
    let p = platforms[idx];
    // Inset from the card edges so the raccoon stands fully on the platform
// instead of balancing on a corner or half off the side.
    let inset = 20.0_f32.min(p.width() * 0.25);
    let x = p.left() + inset + hash(seed + 2.2) * (p.width() - 2.0 * inset).max(8.0);
    Pos2::new(x, p.top())
}

fn paint_raccoon(ctx: &egui::Context, npc: &Companion, _pal: Palette) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        Order::Foreground,
        egui::Id::new("eco-raccoon-mesh"),
    ));
    // Contact shadow, sized to the model's footprint.
    painter.circle_filled(
        npc.pos + Vec2::new(0.0, 4.0),
        26.0,
        Color32::from_rgba_unmultiplied(0, 0, 0, 60),
    );

    let hop = if matches!(npc.act, Act::Cheer) {
        (npc.time * 9.0).sin().abs() * 2.2
    } else {
        0.0
    };
    let bob = if matches!(npc.act, Act::Walk | Act::Alert | Act::Cheer) {
        npc.walk_phase.sin().abs() * 0.08
    } else if matches!(npc.act, Act::Sleep) {
        (npc.time * 1.4).sin() * 0.03
    } else {
        (npc.time * 2.0).sin() * 0.02
    };
    let leg = npc.walk_phase.sin() * if matches!(npc.act, Act::Idle | Act::Sleep | Act::Look) { 0.08 } else { 0.45 };
    let tris = build_raccoon(npc.look_yaw, npc.look_pitch, bob, leg, npc.facing, npc.time, npc.act);
    let mut items: Vec<(f32, Tri)> = tris
        .into_iter()
        .map(|tri| {
            let n = tri.b.sub(tri.a).cross(tri.c.sub(tri.a)).norm();
            ((tri.a.z + tri.b.z + tri.c.z) / 3.0, tri, n)
        })
        .filter(|(_, _, n)| n.z > -0.05)
        .map(|(z, tri, n)| (z, shade(tri, n)))
        .collect();
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // The raccoon stands roughly 95px tall, so scale the floating "zz"/"!" marks
    // and the shadow to that footprint rather than to fixed guesses.
    let body = 95.0_f32;
    let mut mesh = Mesh::default();
    for (_, tri) in items {
        // `hop` is applied to the origin, not to the triangle positions, so the
        // whole body lifts off the platform together.
        let origin = npc.pos - Vec2::new(0.0, hop);
        let pa = project(tri.a, origin, npc.facing);
        let pb = project(tri.b, origin, npc.facing);
        let pc = project(tri.c, origin, npc.facing);
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(pa, tri.color);
        mesh.colored_vertex(pb, tri.color);
        mesh.colored_vertex(pc, tri.color);
        mesh.add_triangle(i, i + 1, i + 2);
    }
    painter.add(Shape::mesh(mesh));

    if matches!(npc.act, Act::Sleep) {
        let zzz = format!("z{}", if npc.time.sin() > 0.0 { "z" } else { "" });
        painter.text(
            npc.pos + Vec2::new(body * 0.34, -body * 1.02),
            egui::Align2::LEFT_CENTER,
            zzz,
            egui::FontId::proportional(15.0),
            Color32::from_rgb(200, 210, 230),
        );
    }
    if matches!(npc.act, Act::Alert) {
        painter.text(
            npc.pos + Vec2::new(0.0, -body * 1.16),
            egui::Align2::CENTER_CENTER,
            "!",
            egui::FontId::proportional(22.0),
            Color32::from_rgb(255, 90, 110),
        );
    }
}

fn shade(mut tri: Tri, n: Vec3) -> Tri {
    let light = Vec3::new(-0.4, -0.75, 0.52).norm();
    let lambert = (n.dot(light) * 0.55 + 0.45).clamp(0.25, 1.0);
    tri.color = Color32::from_rgba_unmultiplied(
        (tri.color.r() as f32 * lambert) as u8,
        (tri.color.g() as f32 * lambert) as u8,
        (tri.color.b() as f32 * lambert) as u8,
        255,
    );
    tri
}

/// Camera distance from the model plane, and focal length in pixels. Together
/// they set the raccoon's on-screen size (~95px tall) and give a mild but
/// visible perspective. Model z runs about -1.4 (tail) to +0.5 (nose), so the
/// depth term never approaches zero.
const CAM_DIST: f32 = 8.0;
const FOCAL: f32 = 520.0;
/// The model's lowest point (its paws) sits at y = -0.55; offsetting by that
/// makes `origin` land on the ground rather than the model's centre.
const GROUND_Y: f32 = 0.55;

fn project(p: Vec3, origin: Pos2, facing: f32) -> Pos2 {
    let depth = (CAM_DIST - p.z).max(0.5);
    let x = p.x * facing;
    Pos2::new(
        origin.x + (x / depth) * FOCAL,
        origin.y + ((GROUND_Y - p.y) / depth) * FOCAL,
    )
}

fn build_raccoon(look_yaw: f32, look_pitch: f32, bob: f32, leg: f32, facing: f32, time: f32, act: Act) -> Vec<Tri> {
    let _ = facing;
    let mut tris = Vec::new();
    let fur = Color32::from_rgb(118, 96, 82);
    let fur_d = Color32::from_rgb(78, 62, 52);
    let cream = Color32::from_rgb(232, 214, 196);
    let mask = Color32::from_rgb(28, 24, 22);
    let eye_w = Color32::from_rgb(245, 242, 232);
    let iris = Color32::from_rgb(212, 148, 42);
    let tail_l = Color32::from_rgb(96, 78, 66);
    let tail_d = Color32::from_rgb(36, 30, 28);

    ellipsoid(&mut tris, Vec3::new(0.0, 0.35 + bob, 0.0), 0.55, 0.42, 0.38, fur, 7, 6);
    ellipsoid(&mut tris, Vec3::new(0.0, 0.55 + bob, 0.18), 0.42, 0.22, 0.32, fur_d, 6, 5);

    let head = Vec3::new(look_yaw * 0.12, 1.05 + bob, look_pitch * 0.1);
    ellipsoid(&mut tris, head, 0.46, 0.42, 0.42, fur, 8, 7);
    ellipsoid(
        &mut tris,
        head.add(Vec3::new(0.0, -0.02, 0.28)),
        0.28,
        0.18,
        0.22,
        cream,
        6,
        5,
    );
    ellipsoid(
        &mut tris,
        head.add(Vec3::new(0.0, 0.02, 0.12)),
        0.42,
        0.16,
        0.22,
        mask,
        6,
        4,
    );
    ear(&mut tris, head.add(Vec3::new(-0.28, 0.38, 0.0)), fur, cream);
    ear(&mut tris, head.add(Vec3::new(0.28, 0.38, 0.0)), fur, cream);

    for side in [-1.0, 1.0] {
        let e = head.add(Vec3::new(side * 0.16, 0.04, 0.28));
        ellipsoid(&mut tris, e, 0.09, 0.1, 0.05, eye_w, 5, 4);
        ellipsoid(&mut tris, e.add(Vec3::new(0.0, 0.0, 0.03)), 0.045, 0.05, 0.04, iris, 4, 3);
        ellipsoid(&mut tris, e.add(Vec3::new(side * 0.015, 0.0, 0.05)), 0.02, 0.025, 0.02, mask, 3, 3);
    }
    ellipsoid(&mut tris, head.add(Vec3::new(0.0, -0.04, 0.46)), 0.07, 0.05, 0.06, mask, 4, 4);

    let arm_swing = -leg * 0.35;
    arm(&mut tris, Vec3::new(-0.46, 0.45 + bob, arm_swing), fur, fur_d);
    arm(&mut tris, Vec3::new(0.46, 0.45 + bob, -arm_swing), fur, fur_d);

    let sleep_tuck = if matches!(act, Act::Sleep) { 0.2 } else { 0.0 };
    leg_mesh(&mut tris, Vec3::new(-0.22, -0.15 + bob, leg * 0.22 - sleep_tuck), fur, cream);
    leg_mesh(&mut tris, Vec3::new(0.22, -0.15 + bob, -leg * 0.22 - sleep_tuck), fur, cream);

    let wag = if matches!(act, Act::Cheer | Act::Walk | Act::Alert) {
        (time * 8.0).sin() * 0.35
    } else {
        (time * 1.6).sin() * 0.08
    };
    for i in 0..7 {
        let t = i as f32 / 6.0;
        let ring = if i % 2 == 0 { tail_l } else { tail_d };
        let p = Vec3::new(
            wag * t * 0.45,
            0.22 + bob + (t * 0.15),
            -0.35 - t * 0.85,
        );
        ellipsoid(&mut tris, p, 0.16 - t * 0.06, 0.14 - t * 0.05, 0.16, ring, 5, 4);
    }
    tris
}

fn ear(tris: &mut Vec<Tri>, pos: Vec3, fur: Color32, inner: Color32) {
    let tip = pos.add(Vec3::new(0.0, 0.28, -0.02));
    let a = pos.add(Vec3::new(-0.12, -0.02, 0.04));
    let b = pos.add(Vec3::new(0.12, -0.02, 0.04));
    let c = pos.add(Vec3::new(0.0, 0.0, -0.1));
    // Same counter-clockwise-from-+z convention as `ellipsoid`.
    tris.push(Tri { a, b, c: tip, color: fur });
    tris.push(Tri { a: b, b: c, c: tip, color: fur });
    tris.push(Tri { a: c, b: a, c: tip, color: fur });
    tris.push(Tri {
        a: pos.add(Vec3::new(-0.05, 0.04, 0.05)),
        b: pos.add(Vec3::new(0.05, 0.04, 0.05)),
        c: pos.add(Vec3::new(0.0, 0.18, 0.04)),
        color: inner,
    });
}

fn arm(tris: &mut Vec<Tri>, pos: Vec3, fur: Color32, paw: Color32) {
    ellipsoid(tris, pos, 0.12, 0.28, 0.12, fur, 5, 4);
    ellipsoid(tris, pos.add(Vec3::new(0.0, -0.28, 0.04)), 0.11, 0.08, 0.11, paw, 4, 4);
}

fn leg_mesh(tris: &mut Vec<Tri>, pos: Vec3, fur: Color32, cream: Color32) {
    ellipsoid(tris, pos, 0.14, 0.32, 0.14, fur, 5, 4);
    ellipsoid(tris, pos.add(Vec3::new(0.0, -0.32, 0.08)), 0.13, 0.08, 0.16, cream, 4, 4);
}

/// Axis-aligned ellipsoid built from a lat/long grid. `slices`/`stacks`
/// control the roundness; low values give the faceted low-poly look.
#[allow(clippy::too_many_arguments)]
fn ellipsoid(
    tris: &mut Vec<Tri>,
    center: Vec3,
    rx: f32,
    ry: f32,
    rz: f32,
    color: Color32,
    slices: usize,
    stacks: usize,
) {
    let mut points = Vec::new();
    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let phi = v * PI;
        for slice in 0..=slices {
            let u = slice as f32 / slices as f32;
            let th = u * TAU;
            points.push(Vec3::new(
                center.x + rx * phi.sin() * th.cos(),
                center.y + ry * phi.cos(),
                center.z + rz * phi.sin() * th.sin(),
            ));
        }
    }
    let cols = slices + 1;
    for stack in 0..stacks {
        for slice in 0..slices {
            let i0 = stack * cols + slice;
            let i1 = i0 + 1;
            let i2 = i0 + cols;
            let i3 = i2 + 1;
            // Winding must be counter-clockwise seen from +z: the painter's
            // back-face cull keeps `n.z > 0`, and `shade` uses the same normal
            // as its Lambert term, so flipping this would also flip lighting.
            tris.push(Tri {
                a: points[i0],
                b: points[i1],
                c: points[i2],
                color,
            });
            tris.push(Tri {
                a: points[i1],
                b: points[i3],
                c: points[i2],
                color,
            });
        }
    }
}

