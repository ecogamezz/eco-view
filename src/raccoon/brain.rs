//! The raccoon's mind: needs, reactions to the machine and the user, and the
//! weighted choice of what to do next.

use eframe::egui::{Pos2, Vec2};

use super::acts::{self, Act, Kind};
use super::fx::{Emote, Quip};
use super::{FLOOR, Inputs, Plan, Prop, Sfx, Surface, World};

impl World {
    pub(crate) fn think(&mut self, input: &Inputs, dt: f32) {
        self.act_t += dt;
        self.needs(input, dt);
        if self.grab.is_some() {
            self.desired_vx = 0.0;
            return;
        }
        self.react(input, dt);
        self.behave(dt);
        self.ambient_chatter(input);
    }

    // ------------------------------------------------------------------
    // Needs
    // ------------------------------------------------------------------

    fn needs(&mut self, input: &Inputs, dt: f32) {
        let live = self.cfg.liveliness;
        let night = self.cfg.sleepy_at_night && input.hour < 6;
        let effort = match self.act {
            Act::Run | Act::Panic | Act::ChaseCursor | Act::Dance | Act::TailChase | Act::Climb => 3.0,
            Act::Wander | Act::Explore | Act::FollowCursor | Act::Dig => 1.5,
            Act::Sit | Act::Loaf | Act::Petted => 0.4,
            _ => 1.0,
        };
        match self.act {
            Act::Sleep => self.energy += dt / 35.0,
            Act::Loaf => self.energy += dt / 400.0,
            _ => self.energy -= dt * effort * live * if night { 2.5 } else { 1.0 } / 900.0,
        }
        self.energy = self.energy.clamp(0.0, 1.0);

        let playing = matches!(
            acts::def(self.act).kind,
            Kind::Social
        ) || matches!(self.act, Act::Dance | Act::TailChase | Act::Explore | Act::Petted | Act::Thrown);
        if playing {
            self.boredom -= dt / 18.0;
        } else if self.act.calm() {
            self.boredom += dt * live / 120.0;
        }
        self.boredom = self.boredom.clamp(0.0, 1.0);
        self.affection = (self.affection - dt / 400.0).clamp(0.0, 1.0);
        self.annoyance = (self.annoyance - dt * 0.12).max(0.0);
    }

    fn ready(&self, act: Act) -> bool {
        self.cooldowns[act as usize] <= 0.0
    }

    /// May a reaction cut into whatever he is doing now?
    fn can_interrupt(&self, new: Act) -> bool {
        if self.act.airborne() || self.act.handled() || self.grab.is_some() || self.support.is_none() {
            return false;
        }
        if matches!(self.act, Act::Climb | Act::Mantle | Act::Land | Act::GetUp) {
            return false;
        }
        let cur = acts::def(self.act);
        let next = acts::def(new);
        next.priority > cur.priority || (cur.interruptible && next.priority >= cur.priority && new != self.act)
    }

    // ------------------------------------------------------------------
    // Reactions
    // ------------------------------------------------------------------

    fn react(&mut self, input: &Inputs, dt: f32) {
        let v = input.vitals;
        let cfg = &self.cfg;
        self.cpu_smooth += (v.cpu - self.cpu_smooth) * (1.0 - (-dt * 1.5).exp());
        let gpu = v.gpu.unwrap_or(0.0);
        let high = (cfg.react_cpu && self.cpu_smooth > cfg.panic_above_cpu)
            || (cfg.react_gpu && gpu > cfg.panic_above_gpu);
        if high {
            self.load_high_since.get_or_insert(self.time);
            self.last_high_t = self.time;
        } else {
            self.load_high_since = None;
        }
        if v.cpu >= self.cfg.sleep_below_cpu || self.pointer.moved_t > self.time - 2.0 && self.near_cursor(200.0) {
            self.calm_since = self.time;
        }

        // Asleep: only real disturbances wake him.
        if self.act == Act::Sleep {
            let rested = self.energy >= 0.98 && self.act_t > self.act_dur.max(20.0);
            if high || rested {
                self.enter(Act::Wake);
                if high {
                    self.fx.emote(Emote::Exclaim);
                    self.say_key(Quip::Woken);
                }
            }
            return;
        }

        // Machine pinned: startle, then panic.
        if high && self.ready(Act::Panic) && self.can_interrupt(Act::Alert) {
            if self.act != Act::Alert && self.ready(Act::Alert) {
                self.enter(Act::Alert);
                self.fx.emote(Emote::Exclaim);
                self.sfx(Sfx::Alarm);
                return;
            }
        }
        // Sustained load or a hot machine: overheating.
        let hot_temp = self.cfg.react_temperature && v.temp.is_some_and(|t| t > self.cfg.hot_above_celsius);
        let sustained = self.load_high_since.is_some_and(|t| self.time - t > 20.0);
        if (hot_temp || sustained) && self.ready(Act::Hot) && self.can_interrupt(Act::Hot) {
            self.enter(Act::Hot);
            self.say_key(Quip::Hot);
            return;
        }
        // Load just dropped away: celebrate.
        if !high && self.last_high_t > 0.0 && self.time - self.last_high_t < 6.0 && self.time - self.last_high_t > 1.0 {
            if self.ready(Act::Cheer) && self.can_interrupt(Act::Cheer) {
                self.last_high_t = -100.0;
                self.enter(Act::Cheer);
                self.say_key(Quip::Calm);
                self.sfx(Sfx::Cheer);
                return;
            }
        }
        // Power.
        if self.cfg.react_battery
            && let Some((pct, charging)) = v.battery
        {
            let was = self.was_charging.replace(charging);
            if was == Some(false) && charging && self.can_interrupt(Act::Cheer) {
                self.enter(Act::Cheer);
                self.say_key(Quip::Charging);
                self.sfx(Sfx::Cheer);
                return;
            }
            if !charging && pct < self.cfg.low_battery_percent && self.ready(Act::Tired) && self.can_interrupt(Act::Tired) {
                self.enter(Act::Tired);
                self.say_key(Quip::LowBattery);
                return;
            }
        }
        if self.cfg.react_memory
            && v.mem_pct > self.cfg.memory_full_percent
            && self.ready(Act::Stuffed)
            && self.can_interrupt(Act::Stuffed)
        {
            self.enter(Act::Stuffed);
            self.say_key(Quip::MemoryFull);
            return;
        }
        if self.cfg.react_network
            && v.net_down_mbps > self.cfg.download_mbps
            && self.ready(Act::Watch)
            && self.can_interrupt(Act::Watch)
        {
            self.enter(Act::Watch);
            self.fx.emote(Emote::Question);
            self.say_key(Quip::Download);
            return;
        }

        // Dozing off: a quiet machine for a while, or simply exhausted.
        let quiet = self.cfg.sleep_when_idle && self.time - self.calm_since > self.cfg.sleep_after_secs;
        let exhausted = self.energy < 0.1;
        if (quiet || exhausted)
            && self.act.calm()
            && self.act_t > 1.5
            && self.ready(Act::Sleep)
            && self.support.is_some()
        {
            if self.act != Act::Loaf && self.ready(Act::Yawn) && self.rng.chance(0.5) {
                self.enter(Act::Yawn);
            } else {
                self.enter(Act::Sleep);
                self.say_key(Quip::Sleepy);
            }
        }
    }

    // ------------------------------------------------------------------
    // Carrying out the current activity
    // ------------------------------------------------------------------

    fn behave(&mut self, dt: f32) {
        let cfg_walk = self.cfg.walk_speed * (0.6 + 0.4 * self.energy) * self.cfg.liveliness.sqrt();
        let cfg_run = self.cfg.run_speed * self.cfg.liveliness.sqrt();
        let sprint = self.cfg.sprint_speed;
        self.desired_vx = 0.0;

        let done = self.act_dur > 0.0 && self.act_t >= self.act_dur;
        match self.act {
            Act::Wander | Act::Run => {
                let speed = if self.act == Act::Run { cfg_run } else { cfg_walk };
                if self.walk_toward(speed) || self.act_t > 15.0 {
                    self.choose();
                }
            }
            Act::Explore => self.explore(cfg_walk, cfg_run),
            Act::FollowCursor => {
                if !self.cfg.follow_cursor {
                    self.choose();
                    return;
                }
                if let Some(c) = self.cursor {
                    if let Some(plan) = self.plan_toward_cursor(c) {
                        self.plan = Some(plan);
                        self.enter(Act::Explore);
                        return;
                    }
                    let srf = self.current_surface();
                    let m = 20.0;
                    let tx = srf.map(|s| c.x.clamp(s.x0 + m, s.x1 - m)).unwrap_or(c.x);
                    self.target_x = Some(tx);
                    let far = (tx - self.pos.x).abs() > 260.0;
                    if !self.walk_toward(if far { cfg_run } else { cfg_walk }) {
                        return;
                    }
                    self.face_point(c);
                }
            }
            Act::ChaseCursor => {
                if let Some(c) = self.cursor {
                    let srf = self.current_surface();
                    let tx = srf.map(|s| c.x.clamp(s.x0 + 16.0, s.x1 - 16.0)).unwrap_or(c.x);
                    self.target_x = Some(tx);
                    if self.walk_toward(sprint * 0.8) && self.rng.chance(dt * 2.0) {
                        self.start_hop(Act::Hop);
                        return;
                    }
                }
                if done {
                    self.choose();
                }
            }
            Act::Panic => {
                // Back and forth across whatever he's standing on.
                let reached = self.walk_toward(sprint);
                if reached || self.target_x.is_none() {
                    self.target_x = self.random_x_on_surface(120.0);
                }
                if self.rng.chance(dt * 1.5) {
                    let p = self.head_point();
                    self.fx.sweat(p, &mut self.rng);
                }
                if done {
                    self.choose();
                }
            }
            Act::Sniff => {
                self.desired_vx = self.facing * 10.0;
                if done {
                    self.choose();
                }
            }
            Act::TailChase => {
                if (self.act_t * 3.0).fract() < dt * 3.0 {
                    self.facing = -self.facing;
                }
                if done {
                    self.choose();
                }
            }
            Act::Stalk => {
                if let Some(c) = self.cursor {
                    self.face_point(c);
                }
                if done {
                    self.pounce();
                }
            }
            Act::Curious | Act::Wave | Act::Watch => {
                if let Some(c) = self.cursor {
                    self.face_point(c);
                }
                if done {
                    self.choose();
                }
            }
            Act::Dig => {
                if self.rng.chance(dt * 6.0) {
                    let p = self.pos + Vec2::new(self.face * 26.0 * self.scale(), -6.0);
                    self.fx.debris(p, self.facing, &mut self.rng);
                }
                if done {
                    if self.rng.chance(0.4) {
                        self.say_key(Quip::Treasure);
                    }
                    self.choose();
                }
            }
            Act::Snack => {
                if self.rng.chance(dt * 3.0) && self.act_t > 0.6 {
                    let p = self.head_point() + Vec2::new(self.face * 14.0 * self.scale(), 18.0 * self.scale());
                    self.fx.crumbs(p, &mut self.rng);
                }
                if done {
                    self.choose();
                }
            }
            Act::Sleep => {
                if self.rng.chance(dt * 0.7) {
                    let p = self.head_point();
                    self.fx.zzz(p, self.face, &mut self.rng);
                }
            }
            Act::Dance | Act::Cheer => {
                if self.rng.chance(dt * if self.act == Act::Cheer { 18.0 } else { 1.5 }) {
                    let p = self.head_point();
                    if self.act == Act::Cheer {
                        self.fx.confetti(p, &mut self.rng);
                    } else {
                        self.fx.note(p, &mut self.rng);
                    }
                }
                if done {
                    self.choose();
                }
            }
            Act::Hot | Act::Tired | Act::Stuffed => {
                if self.act == Act::Hot && self.rng.chance(dt * 2.0) {
                    let p = self.head_point();
                    self.fx.sweat(p, &mut self.rng);
                }
                if done {
                    self.choose();
                }
            }
            Act::Alert => {
                if done {
                    if self.load_high_since.is_some() && self.ready(Act::Panic) {
                        self.enter(Act::Panic);
                        self.say_key(Quip::HighLoad);
                    } else {
                        self.choose();
                    }
                }
            }
            Act::Splat => {
                if done {
                    self.vel = Vec2::new(-self.facing * 60.0, 40.0);
                    self.enter(Act::Fall);
                }
            }
            Act::Wake => {
                if done {
                    if self.ready(Act::Stretch) {
                        self.enter(Act::Stretch);
                    } else {
                        self.choose();
                    }
                }
            }
            Act::Dizzy => {
                if self.rng.chance(dt * 2.0) {
                    let p = self.head_point();
                    self.fx.star(p, &mut self.rng);
                }
                if done {
                    self.choose();
                }
            }
            Act::Annoyed => {
                if done {
                    self.annoyance *= 0.3;
                    self.pointer.clicks.clear();
                    self.choose();
                }
            }
            Act::Sneeze => {
                if (self.act_t - 0.75).abs() < dt * 0.6 {
                    let p = self.head_point() + Vec2::new(self.face * 16.0 * self.scale(), 20.0 * self.scale());
                    self.fx.dust(p, 5, &mut self.rng);
                    self.squash_kick(-3.0);
                }
                if done {
                    self.choose();
                }
            }
            // Airborne and handled states end through the physics.
            Act::Jump | Act::Pounce | Act::Fall | Act::Thrown | Act::Bounce | Act::Hop | Act::Flip | Act::Climb
            | Act::Dragged => {}
            _ => {
                if done {
                    self.choose();
                }
            }
        }
    }

    fn squash_kick(&mut self, v: f32) {
        self.squash += v * 0.02;
    }

    /// Walks toward `target_x` at `speed`. Returns true on arrival.
    fn walk_toward(&mut self, speed: f32) -> bool {
        let Some(tx) = self.target_x else { return true };
        let dx = tx - self.pos.x;
        if dx.abs() < 4.0 {
            self.desired_vx = 0.0;
            return true;
        }
        self.facing = dx.signum();
        // Ease off on the last stretch so he doesn't overshoot.
        let brake = (dx.abs() / 40.0).clamp(0.25, 1.0);
        self.desired_vx = dx.signum() * speed * brake;
        false
    }

    fn face_point(&mut self, p: Pos2) {
        let dx = p.x - self.pos.x;
        if dx.abs() > 20.0 {
            self.facing = dx.signum();
        }
    }

    fn near_cursor(&self, r: f32) -> bool {
        self.cursor.is_some_and(|c| c.distance(self.pos - Vec2::new(0.0, 30.0 * self.scale())) < r)
    }

    fn random_x_on_surface(&mut self, min_dist: f32) -> Option<f32> {
        let s = self.current_surface()?;
        let m = (30.0 * self.scale()).min(s.width() * 0.3);
        let (lo, hi) = (s.x0 + m, s.x1 - m);
        if hi <= lo {
            return Some(s.x0 + s.width() * 0.5);
        }
        for _ in 0..8 {
            let x = self.rng.range(lo, hi);
            if (x - self.pos.x).abs() >= min_dist.min((hi - lo) * 0.4) {
                return Some(x);
            }
        }
        Some(self.rng.range(lo, hi))
    }

    // ------------------------------------------------------------------
    // Getting to other surfaces
    // ------------------------------------------------------------------

    fn explore(&mut self, walk: f32, run: f32) {
        let Some(plan) = self.plan else {
            self.choose();
            return;
        };
        let x = match plan {
            Plan::Leap { takeoff, .. } => takeoff,
            Plan::ClimbUp { wall_x, .. } => wall_x,
        };
        self.target_x = Some(x);
        let far = (x - self.pos.x).abs() > 300.0;
        if !self.walk_toward(if far { run } else { walk }) && self.act_t < 20.0 {
            return;
        }
        self.plan = None;
        if self.act_t >= 20.0 {
            self.choose();
            return;
        }
        match plan {
            Plan::Leap { land, onto, .. } => {
                // The destination may have moved or vanished meanwhile.
                let still = onto == FLOOR || self.surfaces.iter().any(|s| s.owner == onto && s.contains_x(land.x));
                if still {
                    let y = if onto == FLOOR {
                        self.area.bottom()
                    } else {
                        self.owner_rect(onto).map(|r| r.top()).unwrap_or(land.y)
                    };
                    self.jump_to(Pos2::new(land.x, y), Act::Jump);
                } else {
                    self.choose();
                }
            }
            Plan::ClimbUp { side, onto, .. } => {
                if self.owner_rect(onto).is_some() {
                    self.start_climb(onto, side);
                } else {
                    self.choose();
                }
            }
        }
    }

    /// Finds a way onto `dest`, landing near `land_x`.
    pub(crate) fn plan_to(&mut self, dest: Surface, land_x: f32) -> Option<Plan> {
        let cur = self.current_surface()?;
        let s = self.scale();
        let m = (24.0 * s).min(cur.width() * 0.3);
        let land_x = land_x.clamp(dest.x0 + 20.0, dest.x1 - 20.0);
        let rise = cur.y - dest.y;
        if rise <= self.jump_reach() {
            let takeoff = land_x.clamp(cur.x0 + m, cur.x1 - m);
            if (land_x - takeoff).abs() > 520.0 {
                return None;
            }
            // Dropping down from a window: step off the edge, don't jump up.
            return Some(Plan::Leap { takeoff, land: Pos2::new(land_x, dest.y), onto: dest.owner });
        }
        if !self.cfg.can_climb || dest.owner == FLOOR {
            return None;
        }
        let r = self.owner_rect(dest.owner)?;
        // The wall must reach down to where he stands.
        if cur.y < r.top() || cur.y > r.bottom() + 6.0 {
            return None;
        }
        let left = r.left() - 10.0 * s;
        let right = r.right() + 10.0 * s;
        let ok = |x: f32| x >= cur.x0 + m && x <= cur.x1 - m && x > self.area.left() + 20.0 && x < self.area.right() - 20.0;
        let (wall_x, side) = match (ok(left), ok(right)) {
            (true, true) => {
                if (left - self.pos.x).abs() < (right - self.pos.x).abs() { (left, -1.0) } else { (right, 1.0) }
            }
            (true, false) => (left, -1.0),
            (false, true) => (right, 1.0),
            _ => return None,
        };
        Some(Plan::ClimbUp { wall_x, side, onto: dest.owner })
    }

    fn plan_explore(&mut self) -> Option<Plan> {
        let cur = self.current_surface()?;
        let options: Vec<Surface> = self
            .surfaces
            .iter()
            .filter(|s| s.owner != cur.owner && s.width() >= 70.0)
            .copied()
            .collect();
        if options.is_empty() {
            return None;
        }
        for _ in 0..10 {
            let dest = *self.rng.pick(&options);
            let x = self.rng.range(dest.x0 + 30.0, (dest.x1 - 30.0).max(dest.x0 + 31.0));
            if let Some(p) = self.plan_to(dest, x) {
                return Some(p);
            }
        }
        None
    }

    fn plan_toward_cursor(&mut self, c: Pos2) -> Option<Plan> {
        let cur = self.current_surface()?;
        // Only bother when the cursor is clearly over a different perch.
        let under = self
            .surfaces
            .iter()
            .filter(|s| s.contains_x(c.x) && s.y >= c.y - 10.0)
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .copied()?;
        if under.owner == cur.owner || (under.y - cur.y).abs() < 40.0 {
            return None;
        }
        self.plan_to(under, c.x)
    }

    fn pounce(&mut self) {
        let Some(c) = self.cursor else {
            self.choose();
            return;
        };
        let Some(cur) = self.current_surface() else {
            self.choose();
            return;
        };
        let x = c.x.clamp(cur.x0 + 10.0, cur.x1 - 10.0);
        if (x - self.pos.x).abs() < 500.0 {
            self.jump_to(Pos2::new(x, cur.y), Act::Pounce);
        } else {
            self.start_hop(Act::Pounce);
        }
    }

    pub(crate) fn pounce_landed(&mut self) {
        if let Some(c) = self.cursor
            && c.distance(self.head_point()) < 70.0 * self.scale()
        {
            self.fx.emote(Emote::Heart);
            self.say_key(Quip::Caught);
            self.boredom = 0.0;
        }
    }

    // ------------------------------------------------------------------
    // Choosing what to do next
    // ------------------------------------------------------------------

    pub(crate) fn choose(&mut self) {
        self.plan = None;
        self.target_x = None;
        if self.support.is_none() {
            self.enter(Act::Fall);
            return;
        }
        if self.cfg.follow_cursor && self.cursor.is_some() {
            self.enter(Act::FollowCursor);
            return;
        }

        let cursor = self.cursor;
        let head = self.pos - Vec2::new(0.0, 40.0 * self.scale());
        let cdist = cursor.map(|c| c.distance(head)).unwrap_or(f32::MAX);
        let cursor_still = self.time - self.pointer.moved_t > 0.6;
        let cursor_fast = self.pointer.vel.length() > 700.0;
        let level = cursor.is_some_and(|c| (c.y - self.pos.y).abs() < 160.0 && c.y < self.pos.y + 10.0);
        let play = self.cfg.play_with_cursor;
        let can_explore = self.surfaces.len() > 1;

        let mut pool: Vec<(Act, f32)> = Vec::new();
        for d in acts::all() {
            if !matches!(d.kind, Kind::Ambient | Kind::Social) || !self.ready(d.act) {
                continue;
            }
            let mut w = d.weight * self.cfg.weight_of(d.act);
            if w <= 0.0 {
                continue;
            }
            let ok = match d.act {
                Act::Curious => cdist < 320.0 && cursor_still,
                Act::Wave => cdist < 600.0,
                Act::ChaseCursor => play && cursor_fast && cdist < 700.0 && self.boredom > 0.25,
                Act::Stalk => play && cursor_still && level && cdist < 420.0 && cdist > 90.0,
                Act::Explore => can_explore,
                _ => true,
            };
            if !ok {
                continue;
            }
            let energetic = matches!(
                d.act,
                Act::Run | Act::Dance | Act::Explore | Act::TailChase | Act::Flip | Act::Hop | Act::ChaseCursor | Act::Stalk
            );
            let restful = matches!(d.act, Act::Sit | Act::Loaf | Act::Groom | Act::Yawn | Act::WashPaws);
            if energetic {
                w *= (0.25 + self.energy * self.cfg.liveliness) * (0.7 + self.boredom);
            }
            if restful {
                w *= 1.6 - self.energy;
            }
            if d.act == Act::Yawn {
                w *= 1.0 + (1.0 - self.energy) * 3.0;
            }
            if d.act == self.act {
                w *= 0.3;
            }
            pool.push((d.act, w));
        }
        let total: f32 = pool.iter().map(|(_, w)| w).sum();
        if pool.is_empty() || total <= 0.0 {
            self.enter(Act::Idle);
            return;
        }
        let mut roll = self.rng.f() * total;
        let mut chosen = Act::Idle;
        for (a, w) in &pool {
            roll -= w;
            if roll <= 0.0 {
                chosen = *a;
                break;
            }
        }

        match chosen {
            Act::Explore => match self.plan_explore() {
                Some(p) => {
                    self.plan = Some(p);
                    self.enter(Act::Explore);
                }
                None => self.enter_wander(),
            },
            Act::Wander | Act::Run => {
                self.target_x = self.random_x_on_surface(80.0);
                self.enter(chosen);
            }
            Act::Hop | Act::Flip => self.start_hop(chosen),
            a => self.enter(a),
        }
    }

    fn enter_wander(&mut self) {
        self.target_x = self.random_x_on_surface(80.0);
        self.enter(Act::Wander);
    }

    /// Switches activity, arming its cooldown and rolling its duration.
    pub(crate) fn enter(&mut self, act: Act) {
        let d = acts::def(act);
        self.cooldowns[act as usize] = d.cooldown * self.cfg.cooldown_of(act);
        self.act = act;
        self.act_t = 0.0;
        self.act_dur = if d.dur.1 > 0.0 {
            self.rng.range(d.dur.0, d.dur.1) * self.cfg.duration_of(act)
        } else {
            0.0
        };
        self.prop = match act {
            Act::Snack => {
                if self.rng.chance(0.5) {
                    Prop::Cookie
                } else {
                    Prop::Apple
                }
            }
            Act::Hot => Prop::Leaf,
            _ => Prop::None,
        };
        match act {
            Act::Snack => self.say_key(Quip::Snack),
            Act::Dig => self.say_key(Quip::Dig),
            Act::Curious => self.fx.emote(Emote::Question),
            Act::Stalk => self.fx.emote(Emote::Dots),
            Act::Dance => self.say_key(Quip::Dance),
            Act::Land | Act::Mantle | Act::GetUp => self.desired_vx = 0.0,
            Act::Wander | Act::Run if self.target_x.is_none() => {
                self.target_x = self.random_x_on_surface(80.0);
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Talking
    // ------------------------------------------------------------------

    pub(crate) fn say_key(&mut self, q: Quip) {
        if !self.cfg.speech_bubbles {
            return;
        }
        let line = *self.rng.pick(q.lines());
        let v = self.vitals;
        let text = line
            .replace("{name}", &self.cfg.name)
            .replace("{cpu}", &format!("{:.0}", v.cpu))
            .replace("{temp}", &v.temp.map(|t| format!("{t:.0}")).unwrap_or_else(|| "??".into()))
            .replace("{bat}", &v.battery.map(|b| format!("{:.0}", b.0)).unwrap_or_else(|| "??".into()))
            .replace("{mem}", &format!("{:.0}", v.mem_pct))
            .replace("{net}", &format!("{:.0}", v.net_down_mbps));
        self.fx.say(text, q.priority());
    }

    pub(crate) fn say_once_greeting(&mut self) {
        if !self.greeted {
            self.greeted = true;
            self.say_key(Quip::Hello);
        }
    }

    fn ambient_chatter(&mut self, input: &Inputs) {
        if self.time < self.next_quip_t {
            return;
        }
        let c = self.cfg.chattiness;
        let gap = 150.0 - 125.0 * c;
        self.next_quip_t = self.time + gap * self.rng.range(0.7, 1.3);
        if c <= 0.01 || !self.act.calm() || self.act == Act::Sleep {
            return;
        }
        let q = if self.cfg.sleepy_at_night && input.hour < 5 {
            Quip::Night
        } else if self.boredom > 0.7 {
            Quip::Bored
        } else if self.affection > 0.6 {
            Quip::Content
        } else {
            Quip::Idle
        };
        self.say_key(q);
    }
}
