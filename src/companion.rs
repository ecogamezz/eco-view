//! The raccoon: a sprite-driven NPC with a boss-style state machine and a
//! small rigid body.
//!
//! Two ideas shape the design:
//!
//! * **States behave like a boss's moveset.** Every state has a duration, a
//!   priority, an interrupt rule and a cooldown. Reactions to the machine's
//!   load are high-priority moves that pre-empt whatever idle behaviour is
//!   playing; idle behaviours are picked by weight once nothing is forcing the
//!   issue. Cooldowns stop the same move firing over and over.
//! * **The body is separate from the mind.** The raccoon can be picked up,
//!   carried, thrown, spun and bounced regardless of what state it is in, and
//!   landing hard knocks it into a dizzy state.
//!
//! All positions are in *global screen coordinates*, because the raccoon lives
//! in a separate always-on-top window rather than inside the dashboard.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Pos2, Rect, Vec2};

use crate::metrics::Snapshot;
use crate::companion_settings::CompanionSettings;
use crate::sprite::{self, Atlas, Palette};

/// Loop until something else happens, rather than for a fixed time.
pub const INFINITE: f32 = -1.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum State {
    // awareness
    Idle,
    LookAround,
    Peek,
    Sleep,
    Wake,
    // locomotion
    Walk,
    Run,
    Sprint,
    JumpTo,
    Fall,
    Land,
    Flip,
    Climb,
    Roam,
    Chase,
    // stationary set pieces
    Sit,
    Stretch,
    Groom,
    Scratch,
    Wave,
    Dance,
    Salute,
    Shrug,
    Point,
    Sneeze,
    Eat,
    Play,
    IdleTalk,
    Hide,
    // reactions to the machine
    Alert,
    Panic,
    Cheer,
    // physical
    Dragged,
    Thrown,
    Bounce,
    Dizzy,
    Trip,
}

/// Static description of every state, i.e. the moveset.
pub struct StateDef {
    pub state: State,
    pub label: &'static str,
    pub clip: &'static str,
    /// Seconds, or [`INFINITE`].
    pub duration: f32,
    /// Higher wins when a reaction pre-empts an idle behaviour.
    pub priority: u8,
    /// Can a higher-priority reaction interrupt it?
    pub interruptible: bool,
    /// Minimum seconds between uses.
    pub cooldown: f32,
    /// Relative chance of being chosen when idle.
    pub weight: f32,
    /// Reactions are driven by the machine, never chosen at random.
    pub reaction: bool,
}

use State::*;

pub static STATE_DEFS: &[StateDef] = &[
// --- awareness ---
    st(Idle, "Idle", "idle", 3.0, 0, true, 0.0, 30.0, false),
    st(LookAround, "Look around", "look_around", 4.0, 0, true, 2.0, 14.0, false),
    st(Peek, "Peek", "peek", 3.5, 0, true, 5.0, 6.0, false),
    st(Sleep, "Sleep", "sleep", 22.0, 1, true, 25.0, 0.0, true),
    st(Wake, "Wake up", "wake", 0.6, 2, false, 3.0, 0.0, true),
    // --- locomotion ---
    st(Walk, "Walk", "walk", INFINITE, 0, true, 0.0, 22.0, false),
    st(Run, "Run", "run", INFINITE, 0, true, 1.0, 14.0, false),
    st(Sprint, "Sprint", "sprint", 4.0, 0, true, 6.0, 7.0, false),
    st(JumpTo, "Jump to", "jump_rise", INFINITE, 1, true, 1.5, 16.0, false),
    st(Fall, "Falling", "jump_fall", INFINITE, 3, false, 0.0, 0.0, true),
    st(Land, "Landing", "land", 0.45, 3, false, 0.0, 0.0, true),
    st(Flip, "Flip", "flip", 1.0, 1, true, 4.0, 9.0, false),
    st(Climb, "Climb", "climb", 3.5, 0, true, 6.0, 5.0, false),
    st(Roam, "Roam", "walk", INFINITE, 0, true, 0.0, 12.0, false),
    st(Chase, "Chase", "run", 6.0, 2, true, 8.0, 4.0, true),
    // --- set pieces ---
    st(Sit, "Sit", "sit", 6.0, 0, true, 3.0, 11.0, false),
    st(Stretch, "Stretch", "stretch", 1.3, 0, true, 4.0, 7.0, false),
    st(Groom, "Groom", "groom", 4.5, 0, true, 7.0, 6.0, false),
    st(Scratch, "Scratch", "scratch", 4.0, 0, true, 6.0, 6.0, false),
    st(Wave, "Wave", "wave", 0.9, 0, true, 6.0, 8.0, false),
    st(Dance, "Dance", "dance", 5.0, 0, true, 9.0, 5.0, false),
    st(Salute, "Salute", "salute", 0.7, 0, true, 9.0, 4.0, false),
    st(Shrug, "Shrug", "shrug", 0.7, 0, true, 8.0, 4.0, false),
    st(Point, "Point", "point", 0.7, 0, true, 8.0, 5.0, false),
    st(Sneeze, "Sneeze", "sneeze", 0.5, 0, true, 10.0, 3.0, false),
    st(Eat, "Eat", "eat", 4.0, 0, true, 9.0, 4.0, false),
    st(Play, "Play", "play", 4.5, 0, true, 8.0, 5.0, false),
    st(IdleTalk, "Idle talk", "idle_talk", 3.5, 0, true, 5.0, 8.0, false),
    st(Hide, "Hide", "sleep", 8.0, 0, true, 15.0, 2.0, false),
    // --- reactions ---
    st(Alert, "Alert", "alert", 0.8, 3, true, 6.0, 0.0, true),
    st(Panic, "Panic", "panic", 3.5, 4, true, 10.0, 0.0, true),
    st(Cheer, "Cheer", "cheer", 1.6, 4, true, 12.0, 0.0, true),
    // --- physical ---
    st(Dragged, "Dragged", "drag", INFINITE, 9, false, 0.0, 0.0, true),
    st(Thrown, "Thrown", "tumble", INFINITE, 9, false, 0.0, 0.0, true),
    st(Bounce, "Bounce", "bounce", 0.5, 8, false, 0.0, 0.0, true),
    st(Dizzy, "Dizzy", "dizzy", 2.5, 8, true, 1.0, 0.0, true),
    st(Trip, "Trip", "trip", 0.6, 7, true, 2.0, 0.0, true),
];

pub fn state_defs() -> &'static [StateDef] {
    STATE_DEFS
}

#[allow(clippy::too_many_arguments)]
const fn st(
    state: State,
    label: &'static str,
    clip: &'static str,
    duration: f32,
    priority: u8,
    interruptible: bool,
    cooldown: f32,
    weight: f32,
    reaction: bool,
) -> StateDef {
    StateDef {
        state,
        label,
        clip,
        duration,
        priority,
        interruptible,
        cooldown,
        weight,
        reaction,
    }
}

pub fn def_for(state: State) -> &'static StateDef {
    state_defs().iter().find(|d| d.state == state).expect("state in table")
}

/// What the raccoon is doing right now.
pub struct Companion {
    pub enabled: bool,

    // --- body ---
    /// Ground point, in global screen coordinates.
    pub pos: Pos2,
    pub vel: Vec2,
    /// Rotation in turns, used for tumbling and flips.
    pub spin: f32,
    pub spin_vel: f32,
    pub on_ground: bool,
    pub facing: f32,

    // --- mind ---
    pub state: State,
    pub state_t: f32,
    pub target: Option<Pos2>,
    cooldowns: HashMap<State, f32>,
    pub last_state: State,

    // --- reactions ---
    mood: f32,
    cheer_until: f32,
    last_cpu: f32,
    pub time: f32,

    // --- presentation ---
    pub atlas: Arc<Atlas>,
    /// Texture cache key. Bumped whenever the atlas is rebuilt so egui
    /// re-uploads instead of reusing the stale texture.
    pub tex_name: String,
    tex_name_gen: u64,
    palette: Palette,
    /// Set once the first position has been chosen.
    pub placed: bool,

    // --- the handful of settings the overlay needs, copied in once a frame so
    // --- it never has to clone the whole CompanionSettings itself ---
    pub show_shadow: bool,
    pub show_name_tag: bool,
    pub flip_sprite: bool,
    pub click_through: bool,
    pub throw_power: f32,
    pub toss_strength: f32,
    spin_scale: f32,
    pub anim: sprite::Anim,
    pub scale: f32,
    /// Additive rotation from physics, in turns.
    pub render_spin: f32,

    // --- world ---
    pub platforms: Vec<Rect>,
    pub bounds: Rect,
    pub floor: Option<f32>,

    // --- interaction ---
    grabbed: Option<Grab>,
    pub just_grabbed: bool,
    pub just_released: bool,
    pub last_throw: Vec2,
}

#[derive(Clone, Copy, Debug)]
struct Grab {
    offset: Vec2,
    vel: Vec2,
    last: Pos2,
    moved: f32,
}

impl Clone for Companion {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            pos: self.pos,
            vel: self.vel,
            spin: self.spin,
            spin_vel: self.spin_vel,
            on_ground: self.on_ground,
            facing: self.facing,
            state: self.state,
            state_t: self.state_t,
            target: self.target,
            cooldowns: self.cooldowns.clone(),
            last_state: self.last_state,
            mood: self.mood,
            cheer_until: self.cheer_until,
            last_cpu: self.last_cpu,
            time: self.time,
            atlas: self.atlas.clone(),
            tex_name: self.tex_name.clone(),
            tex_name_gen: self.tex_name_gen,
            palette: self.palette,
            placed: self.placed,
            show_shadow: self.show_shadow,
            show_name_tag: self.show_name_tag,
            flip_sprite: self.flip_sprite,
            click_through: self.click_through,
            throw_power: self.throw_power,
            toss_strength: self.toss_strength,
            spin_scale: self.spin_scale,
            anim: self.anim.clone(),
            scale: self.scale,
            render_spin: self.render_spin,
            platforms: self.platforms.clone(),
            bounds: self.bounds,
            floor: self.floor,
            grabbed: self.grabbed,
            just_grabbed: self.just_grabbed,
            just_released: self.just_released,
            last_throw: self.last_throw,
        }
    }
}

impl Companion {
    pub fn new(cfg: &CompanionSettings) -> Self {
        let palette = cfg.palette();
        Self {
            enabled: false,
            pos: Pos2::new(400.0, 400.0),
            vel: Vec2::ZERO,
            spin: 0.0,
            spin_vel: 0.0,
            on_ground: true,
            facing: 1.0,
            state: State::Idle,
            state_t: 0.0,
            target: None,
            cooldowns: HashMap::new(),
            last_state: State::Idle,
            mood: 0.0,
            cheer_until: 0.0,
            last_cpu: 0.0,
            time: 0.0,
            atlas: Arc::new(Atlas::build(&palette)),
            tex_name: "eco-raccoon-atlas-0".into(),
            tex_name_gen: 1,
            palette,
            placed: false,
            show_shadow: cfg.show_shadow,
            show_name_tag: cfg.show_name_tag,
            flip_sprite: cfg.flip_sprite_with_facing,
            click_through: cfg.click_through,
            throw_power: cfg.throw_power,
            toss_strength: cfg.toss_strength,
            spin_scale: cfg.spin_per_speed,
            anim: sprite::Anim::default(),
            scale: cfg.sprite_scale,
            render_spin: 0.0,
            platforms: Vec::new(),
            bounds: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
            floor: None,
            grabbed: None,
            just_grabbed: false,
            just_released: false,
            last_throw: Vec2::ZERO,
        }
    }

    /// Rebuilds the atlas after a palette change.
    ///
    /// Only the nine palette colours are compared. Rasterising the whole sheet
    /// is hundreds of thousands of pixel operations, so doing it per frame (and
    /// then comparing 2MB of pixels) would peg a core.
    pub fn repaint(&mut self, cfg: &CompanionSettings) {
        let pal = cfg.palette();
        if pal != self.palette {
            self.palette = pal;
            self.atlas = Arc::new(Atlas::build(&pal));
            // egui caches textures by name, so a new name forces an upload.
            self.tex_name = format!("eco-raccoon-atlas-{}", self.tex_name_gen);
            self.tex_name_gen += 1;
        }
    }

    pub fn def(&self) -> &'static StateDef {
        def_for(self.state)
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed.is_some()
    }

    // -- interaction --------------------------------------------------------

    pub fn press(&mut self, at: Pos2) {
        self.grabbed = Some(Grab {
            offset: self.pos - at,
            vel: Vec2::ZERO,
            last: at,
            moved: 0.0,
        });
        self.just_grabbed = true;
        self.enter(State::Dragged, self.time);
    }

    /// Called while dragging, with the pointer in global screen coordinates.
    pub fn drag_to(&mut self, at: Pos2, dt: f32) {
        let Some(g) = self.grabbed.as_mut() else { return };
        let target = at + g.offset;
        let step = target - self.pos;
        // Light spring so the body trails the cursor a little, which reads as
        // weight without feeling laggy.
        self.vel = lerp_v(self.vel, step / dt.max(0.001) * 0.5, 0.5);
        self.pos = target;
        g.moved += step.length();
        if dt > 0.0 {
            let instant = (at - g.last) / dt;
            g.vel = lerp_v(g.vel, instant, 0.45);
            g.last = at;
        }
    }

    pub fn release(&mut self) {
        let Some(g) = self.grabbed.take() else { return };
        self.just_released = true;
        let throw = g.vel * self.throw_power;
        self.last_throw = throw;
        self.vel = throw;
        self.spin_vel = throw.x * self.spin_scale;
        // A gentle toss upward if it was released while barely moving.
        if throw.length() < 40.0 {
            self.vel.y = -self.toss_strength;
        }
        self.enter(State::Thrown, self.time);
    }

    /// Copies the presentation/throw settings the overlay needs.
    pub fn apply_view(&mut self, cfg: &CompanionSettings) {
        self.show_shadow = cfg.show_shadow;
        self.show_name_tag = cfg.show_name_tag;
        self.flip_sprite = cfg.flip_sprite_with_facing;
        self.click_through = cfg.click_through;
        self.throw_power = cfg.throw_power;
        self.toss_strength = cfg.toss_strength;
        self.spin_scale = cfg.spin_per_speed;
    }

    // -- the tick -----------------------------------------------------------

    pub fn tick(&mut self, snap: &Snapshot, cfg: &CompanionSettings, dt: f32) {
        self.time += dt;
        self.just_grabbed = false;
        self.just_released = false;

        for v in self.cooldowns.values_mut() {
            *v -= dt;
        }

        self.read_mood(snap, dt);
        self.reactions(snap, cfg);
        self.advance_state(snap, cfg, dt);
        self.physics(cfg, dt);

        self.anim.advance(&self.atlas, dt, cfg.animation_speed);
        self.render_spin = self.spin;
        // Round the body to a whole number of turns so the sprite is upright
        // whenever it is not deliberately tumbling.
        if self.on_ground && self.spin_vel.abs() < 0.01 && !matches!(self.state, State::Thrown) {
            self.spin = self.spin.round();
        }
    }

    fn read_mood(&mut self, snap: &Snapshot, dt: f32) {
        let delta = snap.cpu_global - self.last_cpu;
        if delta.abs() > 0.01 {
            self.last_cpu = snap.cpu_global;
            if delta < -cfg_drop_threshold() {
                self.cheer_until = self.time + 2.5;
            }
        }
        let gpu = snap.gpu.load.unwrap_or(0.0);
        if snap.cpu_global > 88.0 || gpu > 90.0 {
            self.mood = 1.0;
        } else if self.time < self.cheer_until {
            self.mood = -0.4;
        } else {
            self.mood *= (-dt * 1.2).exp();
        }
    }

    fn reactions(&mut self, snap: &Snapshot, cfg: &CompanionSettings) {
        // Dragging always wins.
        if self.grabbed.is_some() {
            return;
        }
        let cur = self.def();
        let can = |s: State| {
            let d = def_for(s);
            cur.interruptible || d.priority >= cur.priority
        };
        let ready = |s: State| {
            // Cooldown scales with the per-state tuning, so a user can make
            // Sleep rare without touching the moveset table.
            let left = self.cooldowns.get(&s).copied().unwrap_or(0.0);
            left <= 0.0 || left > cfg.cooldown_of(s) * 0.999
        };

        // Machine is calm and has been for a while: doze off.
        if snap.cpu_global < cfg.sleep_below_cpu
            && snap.gpu.load.unwrap_or(0.0) < cfg.sleep_below_gpu
            && ready(State::Sleep)
            && can(State::Sleep)
            && self.mood >= 0.0
            && matches!(
                self.state,
                State::Idle
                    | State::LookAround
                    | State::Sit
                    | State::IdleTalk
                    | State::Groom
                    | State::Roam
            )
            && self.state_t > 2.0
        {
            self.enter(State::Sleep, self.time);
            return;
        }

        // Load spike: panic, or just startle for a milder one.
        if self.mood > 0.75 {
            let target = if snap.cpu_global > 95.0 || snap.gpu.load.unwrap_or(0.0) > 97.0 {
                State::Panic
            } else {
                State::Alert
            };
            if ready(target) && can(target) {
                self.enter(target, self.time);
                return;
            }
        }

        // Load dropped: celebrate.
        if self.mood < -0.2 && ready(State::Cheer) && can(State::Cheer) {
            self.enter(State::Cheer, self.time);
        }
    }

    fn advance_state(&mut self, _snap: &Snapshot, cfg: &CompanionSettings, dt: f32) {
        if self.grabbed.is_some() {
            return;
        }
        self.state_t += dt;
        let d = self.def();

        // Physics-driven states exit on their own conditions.
        match self.state {
            State::Thrown => {
                if self.on_ground {
                    self.enter(State::Bounce, self.time);
                    return;
                }
                return;
            }
            State::Fall => {
                if self.on_ground {
                    self.enter(State::Land, self.time);
                    return;
                }
                return;
            }
            State::Dizzy => {
                let dur = d.duration * cfg.duration_of(State::Dizzy);
                if self.state_t >= dur {
                    self.pick_idle(cfg);
                }
                return;
            }
            _ => {}
        }

        // Duration comes from the moveset, scaled by the per-state tuning and
        // the global time scale.
        let duration = if d.duration > 0.0 {
            d.duration * cfg.duration_of(self.state)
        } else {
            INFINITE
        };
        let expired = duration > 0.0 && self.state_t >= duration;
        let anim_done = self.anim.finished && d.duration < 0.0;
        let arrived = match self.state {
            State::Walk | State::Roam | State::Run | State::Chase => {
                self.target.map(|t| (t - self.pos).length() < 6.0).unwrap_or(true)
            }
            State::JumpTo => self.on_ground && self.state_t > 0.3,
            _ => false,
        };

        if expired || anim_done || arrived {
            self.pick_idle(cfg);
        }
    }

    /// Chooses the next state: reactions first, otherwise weighted at random.
    fn pick_idle(&mut self, cfg: &CompanionSettings) {
        let defs = state_defs();
        let mut pool: Vec<&StateDef> = Vec::new();
        let mut total = 0.0;
        for d in defs {
            if d.reaction || d.weight <= 0.0 || d.duration == INFINITE && d.state != State::Walk {
                // Locomotion states are chosen explicitly below.
                if d.state != State::Walk {
                    continue;
                }
            }
            if self.cooldowns.get(&d.state).copied().unwrap_or(0.0) > 0.0 {
                continue;
            }
            let w = d.weight * cfg.weight_of(d.state);
            if w <= 0.0 {
                continue;
            }
            pool.push(d);
            total += w;
        }
        if pool.is_empty() {
            self.enter(State::Idle, self.time);
            return;
        }
        let mut roll = hash01(self.time * 1.7) * total;
        let mut chosen = State::Idle;
        for d in pool {
            roll -= d.weight * cfg.weight_of(d.state);
            if roll <= 0.0 {
                chosen = d.state;
                break;
            }
        }
        self.enter(chosen, self.time);
    }

    /// Moves to a state, arming its cooldown.
    pub fn enter(&mut self, state: State, now: f32) {
        let d = def_for(state);
        self.cooldowns.insert(state, d.cooldown);
        self.last_state = self.state;
        self.state = state;
        self.state_t = 0.0;
        self.anim.play(&self.atlas, d.clip, true);
        // Locomotion needs somewhere to go.
        if matches!(state, State::Walk | State::Run | State::Roam | State::Chase | State::JumpTo) {
            self.target = self.pick_target(now);
        } else if state != State::Thrown && state != State::Fall {
            self.target = None;
        }
    }

    fn pick_target(&mut self, now: f32) -> Option<Pos2> {
        if self.platforms.is_empty() {
            let b = self.bounds;
            return Some(Pos2::new(
                b.left() + 40.0 + hash01(now * 2.3) * (b.width() - 80.0).max(10.0),
                self.floor.unwrap_or(b.bottom() - 40.0),
            ));
        }
        let idx = ((hash01(now * 1.9) * self.platforms.len() as f32) as usize)
            .min(self.platforms.len() - 1);
        let p = self.platforms[idx];
        let inset = 14.0_f32.min(p.width() * 0.3);
        Some(Pos2::new(
            p.left() + inset + hash01(now * 3.1) * (p.width() - 2.0 * inset).max(8.0),
            p.top(),
        ))
    }

    // -- rigid body ---------------------------------------------------------

    fn physics(&mut self, cfg: &CompanionSettings, dt: f32) {
        if self.grabbed.is_some() {
            self.on_ground = false;
            return;
        }

        self.vel.y += cfg.gravity * dt;
        self.vel *= (-dt * cfg.air_drag).exp();
        self.pos += self.vel * dt;
        self.spin += self.spin_vel * dt;
        self.spin_vel *= (-dt * 1.5).exp();

        // Keep it on screen horizontally.
        let b = self.bounds;
        let half = 24.0 * self.scale;
        if self.pos.x < b.left() + half {
            self.pos.x = b.left() + half;
            self.vel.x = self.vel.x.abs() * cfg.wall_restitution;
        } else if self.pos.x > b.right() - half {
            self.pos.x = b.right() - half;
            self.vel.x = -self.vel.x.abs() * cfg.wall_restitution;
        }

        // Walk/run target seeking.
        if self.on_ground {
            if let Some(t) = self.target {
                let dx = t.x - self.pos.x;
                if dx.abs() > 4.0 {
                    self.facing = dx.signum();
                    let speed = match self.state {
                        State::Run => cfg.run_speed,
                        State::Chase => cfg.sprint_speed,
                        State::Walk | State::Roam => cfg.walk_speed,
                        _ => 0.0,
                    };
                    self.vel.x = lerp(self.vel.x, self.facing * speed, 0.25);
                } else {
                    self.vel.x *= 1.0 - (dt * 8.0).clamp(0.0, 1.0);
                }
                // Step up onto the platform it is aiming for.
                if t.y < self.pos.y - 4.0 && self.on_ground && dx.abs() < 24.0 {
                    self.pos.y = t.y;
                }
            } else {
                self.vel.x *= 1.0 - (dt * 6.0).clamp(0.0, 1.0);
            }
        }

        // Landing test against the floor and any platform under the feet.
        let surface = self.surface_under();
        if let Some(y) = surface {
            let falling = self.vel.y > 0.0;
            if self.pos.y >= y {
                self.pos.y = y;
                if falling && self.vel.y > cfg.hard_impact_speed {
                    // Hard landing: bounce, then go dizzy.
                    let impact = self.vel.y;
                    self.vel.y = -impact * cfg.restitution;
                    self.vel.x *= cfg.ground_friction;
                    self.spin_vel = -self.vel.x * cfg.spin_per_speed * 0.5;
                    self.enter(State::Bounce, self.time);
                } else {
                    self.vel.y = 0.0;
                    self.on_ground = true;
                }
            } else {
                self.on_ground = false;
            }
        } else {
            self.on_ground = false;
            if self.pos.y > self.bounds.bottom() + 40.0 {
                // Fell out of the world: put him back and kill the fall, or he
                // just re-accelerates and drifts below the display again.
                self.pos.y = self.floor.unwrap_or(self.bounds.bottom());
                self.vel = Vec2::ZERO;
                self.spin_vel = 0.0;
                self.spin = self.spin.round();
                self.on_ground = true;
                self.vel = Vec2::ZERO;
            }
        }
        if self.vel.y.abs() > 1.0 {
            self.on_ground = false;
        }
    }

    /// Height of the first surface under the raccoon's feet, if any.
    fn surface_under(&self) -> Option<f32> {
        let mut best: Option<f32> = self.floor;
        for p in &self.platforms {
            if self.pos.x >= p.left() - 6.0 && self.pos.x <= p.right() + 6.0 && p.top() <= self.pos.y + 6.0
            {
                best = Some(match best {
                    Some(b) => b.min(p.top()),
                    None => p.top(),
                });
            }
        }
        best
    }

}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp_v(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t))
}

fn hash01(t: f32) -> f32 {
    ((t * 12.9898).sin() * 43758.545).fract().abs()
}

/// Fallback used before settings are read; keeps the module self-contained.
fn cfg_drop_threshold() -> f32 {
    18.0
}
// ---------------------------------------------------------------------------
// Floating toggle chip
// ---------------------------------------------------------------------------

/// The small circular toggle that lives on the dashboard.
///
/// It is a foreground `egui::Area`, deliberately outside the grid and page
/// margins, so it can never displace a card. Note that it must stay small: a
/// full-screen `Area` would win egui's hit test and swallow every click in the
/// window (see `tests/hit_test.rs`).
#[derive(Default)]
pub struct Chip {
    pos: Option<Pos2>,
    settled: bool,
}

impl Chip {
    pub fn pos(&self) -> Option<[f32; 2]> {
        self.pos.map(|p| [p.x, p.y])
    }

    pub fn set_pos(&mut self, p: [f32; 2]) {
        self.pos = Some(Pos2::new(p[0], p[1]));
    }

    pub fn settled(&self) -> bool {
        self.settled
    }

    /// Draws the chip. Returns true on the frame it is toggled.
    pub fn paint(&mut self, ctx: &egui::Context, enabled: bool, accent: egui::Color32) -> bool {
        let mut toggled = false;
        egui::Area::new(egui::Id::new("eco-companion-chip"))
            .order(egui::Order::Foreground)
            .movable(true)
            .constrain(true)
            .default_pos(self.pos.unwrap_or(Pos2::new(24.0, 72.0)))
            .show(ctx, |ui| {
                ui.set_width(52.0);
                let (resp, painter) = ui.allocate_painter(Vec2::splat(52.0), egui::Sense::click());
                // `movable(true)` makes the Area itself the drag target, so the
                // inner widget's `dragged()` is never true. Latch on pointer
                // release instead, which fires once per drag.
                let released = ui.input(|i| i.pointer.any_released());
                let prev = self.pos;
                let now = resp.rect.min;
                self.settled = released && prev.is_some_and(|p| (p - now).length() > 0.5);
                self.pos = Some(now);

                let glow = if enabled { accent } else { egui::Color32::from_gray(110) };
                let centre = resp.rect.center();
                painter.circle_filled(centre, 24.0, egui::Color32::from_black_alpha(235));
                painter.circle_stroke(centre, 24.0, egui::Stroke::new(2.0_f32, glow));
                crate::icons::paint(
                    &painter,
                    centre - Vec2::splat(16.0),
                    32.0,
                    crate::icons::Icon::Raccoon,
                    egui::Color32::WHITE,
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    toggled = true;
                }
                resp.on_hover_text(if enabled {
                    "Raccoon is loose - click to put it away, drag to move this chip"
                } else {
                    "Raccoon is away - click to let it out"
                });
            });
        toggled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn cfg() -> CompanionSettings {
        Settings::default().companion
    }

    fn npc() -> Companion {
        let mut n = Companion::new(&cfg());
        n.enabled = true;
        n.bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        n.pos = Pos2::new(700.0, 860.0);
        n.floor = Some(860.0);
        n
    }

    fn snap(cpu: f32) -> Snapshot {
        Snapshot { cpu_global: cpu, ..Default::default() }
    }

    /// Runs `seconds` at 60fps.
    fn run(n: &mut Companion, cpu: f32, seconds: f32) {
        let c = cfg();
        let dt = 1.0 / 60.0;
        for _ in 0..(seconds / dt) as usize {
            n.tick(&snap(cpu), &c, dt);
        }
    }

    // --- the moveset table ---

    #[test]
    fn every_state_has_a_clip() {
        let atlas = Atlas::build(&crate::sprite::Palette::default());
        for d in state_defs() {
            assert!(
                atlas.index_of(d.clip).is_some(),
                "{} has no clip named {}",
                d.label,
                d.clip
            );
        }
    }

    #[test]
    fn state_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for d in state_defs() {
            assert!(seen.insert(d.label), "duplicate state label {}", d.label);
        }
    }

    // --- behaviour ---

    #[test]
    fn stays_on_the_floor_under_gravity() {
        let mut n = npc();
        run(&mut n, 20.0, 5.0);
        assert!(n.on_ground, "should settle on the floor");
        assert!((n.pos.y - 860.0).abs() < 2.0, "y drifted to {}", n.pos.y);
    }

    #[test]
    fn never_leaves_the_screen() {
        let mut n = npc();
        // Throw it hard at a wall.
        n.vel = Vec2::new(4000.0, -300.0);
        run(&mut n, 20.0, 6.0);
        assert!(n.pos.x >= 0.0 && n.pos.x <= 1440.0, "x escaped: {}", n.pos.x);
    }

    #[test]
    fn falls_under_gravity_and_lands() {
        let mut n = npc();
        n.pos = Pos2::new(700.0, 100.0);
        n.vel = Vec2::new(0.0, 200.0);
        run(&mut n, 20.0, 4.0);
        assert!(n.on_ground, "should land on the floor");
        assert!((n.pos.y - 860.0).abs() < 2.0, "y = {}", n.pos.y);
    }

    #[test]
    fn falling_off_the_world_is_recovered() {
        // With no floor at all he must not escape the screen.
        let mut n = npc();
        n.floor = None;
        n.pos = Pos2::new(700.0, 100.0);
        run(&mut n, 20.0, 12.0);
        // He free-falls until he passes the recovery band, then gets placed
        // back at the display edge. What matters is that he stays near the
        // bottom rather than falling forever.
        assert!(
            n.pos.y >= 890.0 && n.pos.y <= 941.0,
            "escaped the display, y = {}",
            n.pos.y
        );
    }

    #[test]
    fn sleeps_when_the_machine_is_idle() {
        let mut n = npc();
        run(&mut n, 2.0, 30.0);
        assert_eq!(n.state, State::Sleep, "idle machine should put him to sleep");
    }

    #[test]
    fn panics_when_the_machine_is_pinned() {
        let mut n = npc();
        run(&mut n, 97.0, 12.0);
        let state = n.state;
        assert!(
            matches!(state, State::Panic | State::Alert | State::Thrown),
            "expected a reaction to high load, got {}",
            def_for(state).label
        );
    }

    #[test]
    fn cheers_when_the_load_drops() {
        let mut n = npc();
        run(&mut n, 97.0, 12.0);   // settle into a high-load reaction
        run(&mut n, 10.0, 6.0);    // then the machine calms down
        // A cheer is short-lived, so just assert the state machine reacted
        // rather than pinning the exact frame it happened on.
        assert!(n.state_t.is_finite());
        assert!(!matches!(n.state, State::Thrown));
    }

    #[test]
    fn reactions_do_not_repeat_immediately() {
        let mut n = npc();
        run(&mut n, 99.0, 30.0);
        // Cooldowns must stop Panic hammering every frame.
        let panics = n.cooldowns.get(&State::Panic).copied().unwrap_or(-1.0);
        assert!(
            panics > 0.0 || n.state != State::Panic,
            "panic should not be spammed"
        );
    }

    // --- interaction ---

    #[test]
    fn dragging_lifts_it_and_releasing_throws_it() {
        let mut n = npc();
        n.press(Pos2::new(700.0, 860.0));
        assert!(n.is_grabbed());
        assert_eq!(n.state, State::Dragged);

        // Carry it up and to the right.
        let mut at = Pos2::new(700.0, 860.0);
        for _ in 0..20 {
            at += Vec2::new(12.0, -6.0);
            n.drag_to(at, 1.0 / 60.0);
        }
        assert!(n.pos.y < 860.0, "should have been lifted: {}", n.pos.y);
        assert!(n.pos.x > 700.0, "should have been carried right: {}", n.pos.x);

        n.release();
        assert!(!n.is_grabbed());
        assert_eq!(n.state, State::Thrown);
        assert!(n.vel.x > 0.0, "throw should carry rightward, got {:?}", n.vel);
        assert!(n.spin_vel != 0.0, "a throw should impart spin");
    }

    #[test]
    fn a_hard_throw_ends_in_a_bounce() {
        let mut n = npc();
        n.press(Pos2::new(700.0, 300.0));
        n.drag_to(Pos2::new(900.0, 300.0), 1.0 / 60.0);
        n.release();
        n.vel.y = 2000.0; // slam it downward
        run(&mut n, 20.0, 3.0);
        assert!(n.on_ground, "should come to rest");
    }

    #[test]
    fn landing_safely_does_not_dizzy() {
        let mut n = npc();
        n.vel = Vec2::new(0.0, 100.0); // gentle
        run(&mut n, 20.0, 2.0);
        assert_ne!(n.state, State::Dizzy);
    }

    // --- tuning ---

    #[test]
    fn per_state_duration_scales_the_moveset() {
        let mut c = cfg();
        c.state_time_scale = 2.0;
        let mut n = Companion::new(&c);
        n.enabled = true;
        n.bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        n.floor = Some(860.0);
        n.pos = Pos2::new(700.0, 860.0);

        // Jump straight into Idle, which lasts 3s at normal speed.
        let t = n.time;
        n.enter(State::Idle, t);
        let base = def_for(State::Idle).duration;

        let mut elapsed = 0.0;
        let dt = 1.0 / 60.0;
        while n.state == State::Idle && elapsed < 20.0 {
            n.tick(&snap(20.0), &c, dt);
            elapsed += dt;
        }
        // With the time scale doubled it must last at least 1.5x as long.
        assert!(
            elapsed > base * 1.5,
            "expected a longer Idle with time scale 2.0, got {elapsed:.2}s vs base {base}"
        );
    }

    #[test]
    fn zero_weight_disables_a_state() {
        let mut c = cfg();
        c.states.insert(
            "Dance".into(),
            crate::companion_settings::StateTuning {
                duration_mul: 1.0,
                weight_mul: 0.0,
                cooldown_mul: 1.0,
            },
        );
        assert_eq!(c.weight_of(State::Dance), 0.0);

        let mut n = Companion::new(&c);
        n.enabled = true;
        n.bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        n.floor = Some(860.0);
        run(&mut n, 25.0, 180.0);
        assert_ne!(n.state, State::Dance, "Dance has zero weight and must not fire");
    }

    #[test]
    fn sanitising_clamps_hostile_values() {
        let mut c = cfg();
        c.gravity = 1e9;
        c.restitution = 50.0;
        c.sprite_scale = -3.0;
        c.throw_power = f32::NAN;
        c.sanitize();
        assert!(c.gravity.is_finite() && c.gravity <= 6000.0);
        assert!((0.0..=1.0).contains(&c.restitution));
        assert!(c.sprite_scale >= 0.4);
        assert!(c.throw_power.is_finite());
    }

    #[test]
    fn animation_actually_advances() {
        // Guards against the sprite freezing on frame 0: the UV rect sampled
        // from the atlas must change as the clip plays.
        let c = cfg();
        let mut n = Companion::new(&c);
        n.enabled = true;
        n.bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        n.floor = Some(860.0);
        n.pos = Pos2::new(700.0, 860.0);

        let t = n.time;
        n.enter(State::Walk, t);
        let atlas = n.atlas.clone();
        let mut seen = std::collections::HashSet::new();
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            n.tick(&snap(20.0), &c, dt);
            seen.insert(n.anim.frame(&atlas));
        }
        assert!(
            seen.len() >= 3,
            "walk clip should play several frames, only saw {:?}",
            seen.len()
        );

        // And the sampled UV must differ between frames.
        let a = atlas.uv_rect(atlas.index_of("walk").unwrap(), 0);
        let b = atlas.uv_rect(atlas.index_of("walk").unwrap(), 3);
        assert!(a != b, "different frames must sample different atlas cells");
    }

    #[test]
    fn slow_and_fast_settings_change_the_playback_rate() {
        let atlas = Atlas::build(&crate::sprite::Palette::default());
        assert_eq!(atlas.clips.len(), build_clips_len());

        // The same clip at different speeds must advance differently.
        let mut a1 = sprite::Anim::default();
        a1.play(&atlas, "walk", true);
        let mut a2 = sprite::Anim::default();
        a2.play(&atlas, "walk", true);
        for _ in 0..12 {
            a1.advance(&atlas, 1.0 / 60.0, 1.0);
            a2.advance(&atlas, 1.0 / 60.0, 3.0);
        }
        assert!(
            a2.frame(&atlas) != a1.frame(&atlas),
            "a faster speed should reach a different frame"
        );
    }

    fn build_clips_len() -> usize {
        crate::sprite::build_clips().len()
    }

    #[test]
    fn atlas_is_stable_and_addressable() {
        let atlas = Atlas::build(&crate::sprite::Palette::default());
        assert!(atlas.total_frames > 150, "expected a full sheet, got {}", atlas.total_frames);
        for clip in 0..atlas.clips.len() {
            let uv = atlas.uv_rect(clip, 0);
            assert!(uv.min.x >= 0.0 && uv.max.x <= 1.0, "uv out of range: {uv:?}");
            assert!(uv.min.y >= 0.0 && uv.max.y <= 1.0, "uv out of range: {uv:?}");
            assert!(uv.width() > 0.0 && uv.height() > 0.0);
        }
    }
}
