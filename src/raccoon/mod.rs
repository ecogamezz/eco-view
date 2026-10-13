//! The raccoon desktop companion.
//!
//! He lives in a transparent, click-through overlay window that covers one
//! display and floats above every other app. Clicks pass straight through to
//! whatever is underneath except where he actually is, so he can be picked up,
//! petted, booped and thrown without ever getting in the way.
//!
//! The pieces:
//!
//! * [`acts`]     - the moveset table (durations, cooldowns, priorities).
//! * [`brain`]    - picks what to do: needs, reactions to the machine, plans.
//! * [`body`]     - one-way platforms, walking, jumping, climbing, throwing.
//! * [`rig`]      - procedural poses and the skeleton they produce.
//! * [`render`]   - anti-aliased vector drawing with outlines and shading.
//! * [`fx`]       - particles, emotes and speech bubbles.
//! * [`desktop`]  - macOS glue: cursor, other apps' windows, displays.
//! * [`overlay`]  - the always-on-top window itself.
//!
//! [`World`] ties it together and is entirely deterministic given its inputs,
//! which is what makes the behaviour testable without a window.

pub mod acts;
mod body;
mod brain;
pub mod chip;
pub mod desktop;
pub mod fx;
pub mod overlay;
pub mod preview;
pub mod render;
pub mod rig;
pub mod settings;
pub mod ui;

use eframe::egui::{Pos2, Rect, Vec2};

pub use acts::Act;
pub use settings::RaccoonSettings;

use fx::Fx;
use rig::Pose;
use settings::Palette;

/// What the machine is doing, boiled down to what the raccoon cares about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vitals {
    pub cpu: f32,
    pub gpu: Option<f32>,
    pub mem_pct: f32,
    pub temp: Option<f32>,
    /// (percent, charging)
    pub battery: Option<(f32, bool)>,
    pub net_down_mbps: f32,
}

impl Vitals {
    pub fn from_snapshot(s: &crate::metrics::Snapshot) -> Self {
        Self {
            cpu: s.cpu_global,
            gpu: s.gpu.load,
            mem_pct: crate::metrics::pct(s.mem_used, s.mem_total),
            temp: s.temperature.as_ref().map(|t| t.celsius),
            battery: s.battery.as_ref().map(|b| (b.percent, b.charging)),
            net_down_mbps: (s.net_down_bps * 8.0 / 1_000_000.0) as f32,
        }
    }
}

/// Per-frame inputs. Everything is in global desktop points (top-left origin).
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub dt: f32,
    pub cursor: Option<Pos2>,
    /// Left button currently held.
    pub button_down: bool,
    /// Left-button presses since the last frame, so a click shorter than a
    /// frame is never lost.
    pub presses: u32,
    pub vitals: Vitals,
    /// Local hour, 0..24.
    pub hour: u32,
}

/// Another app's window, front-to-back order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinRect {
    pub id: u32,
    pub rect: Rect,
    /// This process's own window (the dashboard).
    pub own: bool,
}

/// Who a surface belongs to. Supports are tracked by owner so that a window
/// being dragged carries him along.
pub type Owner = u64;
pub const FLOOR: Owner = 0;
const WINDOW_TAG: u64 = 1 << 40;
const CARD_TAG: u64 = 2 << 40;

/// A visible, walkable stretch of a top edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub owner: Owner,
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

impl Surface {
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn contains_x(&self, x: f32) -> bool {
        x >= self.x0 && x <= self.x1
    }
}

/// Sound effects, mapped onto macOS system sounds by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Pickup,
    Throw,
    Thud,
    Boop,
    Purr,
    Cheer,
    Alarm,
}

/// Small, fast, deterministic RNG (xorshift64*).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }

    pub fn sign(&mut self) -> f32 {
        if self.f() < 0.5 { -1.0 } else { 1.0 }
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[((self.f() * items.len() as f32) as usize).min(items.len() - 1)]
    }
}

/// Something he is holding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Prop {
    #[default]
    None,
    Cookie,
    Apple,
    Leaf,
}

/// A multi-step journey to another surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// Walk to `takeoff`, then jump to `land` (on the surface owned by `onto`).
    Leap { takeoff: f32, land: Pos2, onto: Owner },
    /// Walk to the wall at `wall_x`, then climb the window owned by `onto`.
    ClimbUp { wall_x: f32, side: f32, onto: Owner },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Climb {
    pub owner: Owner,
    /// -1: on the window's left side (facing right), +1: right side.
    pub side: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Grab {
    /// Pendulum length from the cursor to the body centre.
    pub len: f32,
    /// Swing angle: 0 hangs straight down, positive swings to the right.
    pub theta: f32,
    pub omega: f32,
    pub last_anchor: Pos2,
    pub anchor_vel: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Press {
    pub at: Pos2,
    pub t: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Pointer {
    pub prev_down: bool,
    pub press: Option<Press>,
    pub hovered: bool,
    pub last: Option<Pos2>,
    /// Smoothed cursor velocity.
    pub vel: Vec2,
    /// Time the cursor last moved noticeably.
    pub moved_t: f32,
    pub stroke_dir: f32,
    pub strokes: Vec<f32>,
    pub clicks: Vec<f32>,
    pub hover_t: f32,
}

/// The whole simulation.
pub struct World {
    pub cfg: RaccoonSettings,
    pub pal: Palette,
    pub enabled: bool,
    pub time: f32,
    pub rng: Rng,

    // --- desktop ---
    /// The display the overlay covers.
    pub stage: Rect,
    /// The usable part of it: below the menu bar, above the Dock.
    pub area: Rect,
    windows: Vec<WinRect>,
    cards: Vec<Rect>,
    owners: Vec<(Owner, Rect)>,
    pub surfaces: Vec<Surface>,
    surfaces_dirty: bool,
    pub placed: bool,

    // --- body ---
    /// Ground point between the feet, global coordinates.
    pub pos: Pos2,
    pub vel: Vec2,
    pub support: Option<Owner>,
    /// Physical rotation (radians, clockwise on screen) from tumbling and dangling.
    pub roll: f32,
    pub roll_vel: f32,
    /// Direction he wants to face, +1 right / -1 left.
    pub facing: f32,
    /// Animated facing, sweeps through zero when he turns around.
    pub face: f32,
    pub squash: f32,
    squash_vel: f32,
    climb: Option<Climb>,
    grab: Option<Grab>,
    hard_landing: bool,
    bounces: u32,
    pub desired_vx: f32,

    // --- pointer ---
    pointer: Pointer,
    /// The overlay should catch the mouse this frame (he is under the cursor
    /// or being handled). Everywhere else clicks pass through.
    pub want_mouse: bool,
    pub cursor: Option<Pos2>,

    // --- brain ---
    pub act: Act,
    pub act_t: f32,
    pub act_dur: f32,
    cooldowns: [f32; acts::COUNT],
    pub plan: Option<Plan>,
    pub target_x: Option<f32>,
    pub energy: f32,
    pub boredom: f32,
    pub affection: f32,
    pub annoyance: f32,
    pub prop: Prop,
    vitals: Vitals,
    cpu_smooth: f32,
    load_high_since: Option<f32>,
    last_high_t: f32,
    calm_since: f32,
    was_charging: Option<bool>,
    next_quip_t: f32,
    greeted: bool,

    // --- presentation ---
    pub pose: Pose,
    pub gait_phase: f32,
    pub tail_lag: f32,
    pub look: Vec2,
    blink_t: f32,
    next_blink: f32,
    ear_twitch: f32,
    pub fx: Fx,
    pub sounds: Vec<Sfx>,
    last_sound_t: f32,
}

impl World {
    pub fn new(cfg: &RaccoonSettings, seed: u64) -> Self {
        let stage = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let mut w = Self {
            cfg: cfg.clone(),
            pal: cfg.palette(),
            enabled: cfg.enabled,
            time: 0.0,
            rng: Rng::new(seed),
            stage,
            area: stage,
            windows: Vec::new(),
            cards: Vec::new(),
            owners: Vec::new(),
            surfaces: Vec::new(),
            surfaces_dirty: true,
            placed: false,
            pos: Pos2::new(stage.center().x, stage.bottom()),
            vel: Vec2::ZERO,
            support: Some(FLOOR),
            roll: 0.0,
            roll_vel: 0.0,
            facing: 1.0,
            face: 1.0,
            squash: 0.0,
            squash_vel: 0.0,
            climb: None,
            grab: None,
            hard_landing: false,
            bounces: 0,
            desired_vx: 0.0,
            pointer: Pointer::default(),
            want_mouse: false,
            cursor: None,
            act: Act::Idle,
            act_t: 0.0,
            act_dur: 3.0,
            cooldowns: [0.0; acts::COUNT],
            plan: None,
            target_x: None,
            energy: 0.85,
            boredom: 0.2,
            affection: 0.3,
            annoyance: 0.0,
            prop: Prop::None,
            vitals: Vitals::default(),
            cpu_smooth: 0.0,
            load_high_since: None,
            last_high_t: -100.0,
            calm_since: 0.0,
            was_charging: None,
            next_quip_t: 20.0,
            greeted: false,
            pose: Pose::default(),
            gait_phase: 0.0,
            tail_lag: 0.0,
            look: Vec2::ZERO,
            blink_t: -1.0,
            next_blink: 2.0,
            ear_twitch: 0.0,
            fx: Fx::default(),
            sounds: Vec::new(),
            last_sound_t: -10.0,
        };
        w.rebuild_surfaces();
        w
    }

    /// Applies new settings (called whenever the user edits them).
    pub fn set_settings(&mut self, cfg: &RaccoonSettings) {
        self.pal = cfg.palette();
        self.cfg = cfg.clone();
        self.enabled = cfg.enabled;
    }

    pub fn last_vitals(&self) -> Vitals {
        self.vitals
    }

    /// Points per model unit.
    pub fn scale(&self) -> f32 {
        self.cfg.size * 0.95
    }

    /// Updates the display he lives on. Moving to another display carries him
    /// across proportionally rather than leaving him off-screen.
    pub fn set_stage(&mut self, stage: Rect, area: Rect) {
        if stage == self.stage && area == self.area {
            return;
        }
        let old = self.stage;
        self.stage = stage;
        self.area = area.intersect(stage);
        if !stage.contains(self.pos) {
            let fx = ((self.pos.x - old.left()) / old.width().max(1.0)).clamp(0.05, 0.95);
            self.pos = Pos2::new(stage.left() + fx * stage.width(), self.area.bottom());
            self.vel = Vec2::ZERO;
            self.support = Some(FLOOR);
            self.climb = None;
        }
        self.surfaces_dirty = true;
    }

    /// Other apps' windows, front to back.
    pub fn set_windows(&mut self, windows: Vec<WinRect>) {
        if windows != self.windows {
            self.carry_with_owner(&windows, &self.cards.clone());
            self.windows = windows;
            self.surfaces_dirty = true;
        }
    }

    /// The dashboard's cards in global coordinates.
    pub fn set_cards(&mut self, cards: Vec<Rect>) {
        if cards != self.cards {
            self.carry_with_owner(&self.windows.clone(), &cards);
            self.cards = cards;
            self.surfaces_dirty = true;
        }
    }

    /// If the thing he stands on (or clings to) moved, move with it.
    fn carry_with_owner(&mut self, windows: &[WinRect], cards: &[Rect]) {
        let Some(owner) = self.support.or(self.climb.map(|c| c.owner)) else { return };
        if owner == FLOOR {
            return;
        }
        let Some(old) = self.owner_rect(owner) else { return };
        let new = if owner & CARD_TAG != 0 {
            cards.get((owner & 0xFFFF_FFFF) as usize).copied()
        } else {
            windows.iter().find(|w| WINDOW_TAG | w.id as u64 == owner).map(|w| w.rect)
        };
        if let Some(new) = new
            && new.size() == old.size()
        {
            let d = new.min - old.min;
            if d.length() < 2000.0 {
                self.pos += d;
            }
        }
    }

    pub(crate) fn owner_rect(&self, owner: Owner) -> Option<Rect> {
        self.owners.iter().find(|(o, _)| *o == owner).map(|(_, r)| *r)
    }

    /// Rebuilds the walkable surfaces: the floor plus the visible parts of
    /// every window's top edge (and the dashboard cards), with anything
    /// covered by a window in front cut out.
    pub fn rebuild_surfaces(&mut self) {
        self.surfaces_dirty = false;
        self.owners.clear();
        self.surfaces.clear();

        let area = self.area;
        self.surfaces.push(Surface { owner: FLOOR, x0: area.left(), x1: area.right(), y: area.bottom() });

        // Occluders, front to back. Cards live inside our own window, so they
        // share its depth.
        let mut stack: Vec<(Owner, Rect)> = Vec::new();
        let own_z = self.windows.iter().position(|w| w.own);
        for (z, w) in self.windows.iter().enumerate() {
            if Some(z) == own_z && self.cfg.walk_on_dashboard {
                for (i, c) in self.cards.iter().enumerate() {
                    stack.push((CARD_TAG | i as u64, *c));
                }
            }
            stack.push((WINDOW_TAG | w.id as u64, w.rect));
        }
        if own_z.is_none() && self.cfg.walk_on_dashboard {
            for (i, c) in self.cards.iter().enumerate() {
                stack.push((CARD_TAG | i as u64, *c));
            }
        }

        for (i, &(owner, rect)) in stack.iter().enumerate() {
            let is_card = owner & CARD_TAG != 0;
            if !is_card && !self.cfg.walk_on_windows {
                continue;
            }
            self.owners.push((owner, rect));
            let y = rect.top();
            // Too close to the menu bar or the floor to be worth standing on.
            if y < area.top() + 24.0 || y > area.bottom() - 30.0 {
                continue;
            }
            let mut segs = vec![(rect.left().max(area.left()), rect.right().min(area.right()))];
            for &(o2, r2) in &stack[..i] {
                // Cards don't hide windows; only real windows occlude.
                if o2 & CARD_TAG != 0 && !is_card {
                    continue;
                }
                // Our own window contains the cards; it doesn't hide them.
                if is_card && o2 & WINDOW_TAG != 0 && self.windows.iter().any(|w| w.own && WINDOW_TAG | w.id as u64 == o2) {
                    continue;
                }
                if r2.top() <= y - 2.0 && r2.bottom() > y - 2.0 {
                    segs = cut(&segs, r2.left(), r2.right());
                }
            }
            for (x0, x1) in segs {
                if x1 - x0 >= 48.0 {
                    self.surfaces.push(Surface { owner, x0, x1, y });
                }
            }
        }
        // Keep a valid support after the world changed under him.
        if let Some(s) = self.support
            && s != FLOOR
            && self.owner_rect(s).is_none()
        {
            self.support = None;
        }
    }

    /// The surface segment he is standing on, if any.
    pub fn current_surface(&self) -> Option<Surface> {
        let owner = self.support?;
        self.surfaces
            .iter()
            .filter(|s| s.owner == owner)
            .find(|s| s.contains_x(self.pos.x))
            .copied()
            .or_else(|| {
                // Standing on a covered stretch: treat the whole edge as his.
                let r = self.owner_rect(owner)?;
                Some(Surface { owner, x0: r.left().max(self.area.left()), x1: r.right().min(self.area.right()), y: r.top() })
            })
            .or_else(|| {
                (owner == FLOOR).then(|| Surface {
                    owner,
                    x0: self.area.left(),
                    x1: self.area.right(),
                    y: self.area.bottom(),
                })
            })
    }

    /// Advances everything by one frame.
    pub fn tick(&mut self, input: &Inputs) {
        let dt = input.dt.clamp(0.0, 0.1);
        if !self.enabled {
            return;
        }
        if self.surfaces_dirty {
            self.rebuild_surfaces();
        }
        if !self.placed {
            self.placed = true;
            self.pos = Pos2::new(self.area.center().x, self.area.top() + 40.0);
            self.support = None;
            self.enter(Act::Fall);
            self.say_once_greeting();
        }
        self.time += dt;
        self.vitals = input.vitals;
        self.cursor = input.cursor;
        for c in &mut self.cooldowns {
            *c -= dt;
        }

        self.pointer(input, dt);
        self.think(input, dt);
        // Fixed sub-steps keep fast throws from tunnelling through platforms.
        let steps = (dt / (1.0 / 240.0)).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.physics(h);
        }
        self.animate(dt);
        let anchor = self.head_point();
        self.fx.tick(dt, &mut self.rng, anchor, self.area);
    }

    pub(crate) fn sfx(&mut self, s: Sfx) {
        if !self.cfg.sounds {
            return;
        }
        // A burst of bounces shouldn't machine-gun the speaker.
        if self.time - self.last_sound_t < 0.18 {
            return;
        }
        self.last_sound_t = self.time;
        self.sounds.push(s);
    }

    /// The screen transform for his current pose.
    pub fn xform(&self) -> rig::Xform {
        rig::Xform::new(self)
    }

    /// Top of the head in global coordinates: where emotes and bubbles go.
    pub fn head_point(&self) -> Pos2 {
        let sk = rig::Skeleton::build(&self.pose, self.time);
        let xf = self.xform();
        xf.apply(sk.head + Vec2::new(0.0, sk.head_r + 4.0))
    }

    /// Circles (global) covering his body, for hover and grab tests.
    pub fn hit_circles(&self) -> Vec<(Pos2, f32)> {
        let sk = rig::Skeleton::build(&self.pose, self.time);
        let xf = self.xform();
        let s = self.scale();
        vec![
            (xf.apply(sk.hip), (sk.hip_r + 4.0) * s),
            (xf.apply(sk.chest), (sk.chest_r + 4.0) * s),
            (xf.apply(sk.head), (sk.head_r + 4.0) * s),
            (xf.apply(sk.tail[sk.tail.len() / 2].0), 8.0 * s),
        ]
    }

    pub fn hit(&self, p: Pos2) -> bool {
        self.hit_circles().iter().any(|(c, r)| c.distance(p) <= *r)
    }

    /// Drops him in from the top of the screen above `at`.
    pub fn summon(&mut self, at: Pos2) {
        let x = at.x.clamp(self.area.left() + 30.0, self.area.right() - 30.0);
        self.pos = Pos2::new(x, self.area.top() + 10.0);
        self.vel = Vec2::new(0.0, 200.0);
        self.support = None;
        self.climb = None;
        self.grab = None;
        self.roll = 0.0;
        self.roll_vel = 0.0;
        self.plan = None;
        self.enter(Act::Fall);
        self.say_key(fx::Quip::Summoned);
    }

    /// Throws him across the screen.
    pub fn toss(&mut self) {
        let dir = if self.pos.x < self.area.center().x { 1.0 } else { -1.0 };
        self.support = None;
        self.climb = None;
        self.vel = Vec2::new(dir * 900.0, -1100.0);
        self.roll_vel = dir * 9.0;
        self.enter(Act::Thrown);
        self.say_key(fx::Quip::Thrown);
    }

    /// Forces an activity (from the settings panel).
    pub fn force(&mut self, act: Act) {
        if self.grab.is_some() {
            return;
        }
        self.plan = None;
        if act.airborne() || matches!(act, Act::Jump | Act::Pounce) {
            self.start_hop(act);
        } else {
            self.enter(act);
        }
    }

    /// True while he is doing something that should be drawn smoothly.
    pub fn busy(&self) -> bool {
        !matches!(self.act, Act::Sleep | Act::Loaf) || !self.fx.is_empty() || self.pointer.hovered
    }
}

/// Removes the interval `[a, b]` from a set of disjoint segments.
fn cut(segs: &[(f32, f32)], a: f32, b: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(segs.len() + 1);
    for &(x0, x1) in segs {
        if b <= x0 || a >= x1 {
            out.push((x0, x1));
            continue;
        }
        if a > x0 {
            out.push((x0, a));
        }
        if b < x1 {
            out.push((b, x1));
        }
    }
    out
}

pub(crate) fn approach(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-rate * dt).exp())
}

#[cfg(test)]
mod tests;
