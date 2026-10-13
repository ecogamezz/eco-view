//! The raccoon's body: a point mass with a facing, a roll and a squash spring.
//!
//! Surfaces are one-way platforms. He only lands on one by crossing its top
//! edge from above while falling, so a window above him never teleports him
//! up onto it, and he can jump up through one to reach a higher perch.

use eframe::egui::{Pos2, Vec2};

use super::acts::Act;
use super::fx::{Emote, Quip};
use super::{Climb, FLOOR, Grab, Inputs, Owner, Press, Sfx, World, approach};

/// Body centre height above the feet, in model units.
pub const CENTRE_H: f32 = 30.0;
/// Half the body's length, in model units, for walls and ledges.
pub const HALF_LEN: f32 = 30.0;

impl World {
    // ------------------------------------------------------------------
    // Pointer: hover, petting, clicks, picking up
    // ------------------------------------------------------------------

    pub(crate) fn pointer(&mut self, input: &Inputs, dt: f32) {
        let cursor = input.cursor;
        let down = input.button_down;
        let pressed_now = (down && !self.pointer.prev_down) || input.presses > 0;

        // Cursor velocity, smoothed over a few frames so a throw uses the
        // flick rather than the last jittery sample.
        if let (Some(c), Some(last)) = (cursor, self.pointer.last)
            && dt > 0.0
        {
            let v = (c - last) / dt;
            let k = 1.0 - (-dt * 25.0).exp();
            self.pointer.vel = self.pointer.vel + (v - self.pointer.vel) * k;
            if (c - last).length() > 1.0 {
                self.pointer.moved_t = self.time;
            }
        }

        let hovered = !self.cfg.click_through && cursor.is_some_and(|c| self.hit(c));
        self.pointer.hovered = hovered;
        if hovered {
            self.pointer.hover_t += dt;
        } else {
            self.pointer.hover_t = 0.0;
        }

        // Petting: rubbing back and forth over him without pressing.
        if hovered
            && !down
            && self.cfg.allow_petting
            && let (Some(c), Some(last)) = (cursor, self.pointer.last)
        {
            let dx = c.x - last.x;
            if dx.abs() > 1.5 {
                let dir = dx.signum();
                if dir != self.pointer.stroke_dir {
                    self.pointer.stroke_dir = dir;
                    self.pointer.strokes.push(self.time);
                }
            }
        }
        let now = self.time;
        self.pointer.strokes.retain(|t| now - t < 1.4);
        if self.pointer.strokes.len() >= 4 && self.grab.is_none() {
            self.pet(dt);
        }

        if let Some(c) = cursor {
            if self.grab.is_some() {
                if down {
                    self.drag_to(c, dt);
                } else {
                    self.release();
                }
            } else if pressed_now && hovered {
                self.pointer.press = Some(Press { at: c, t: self.time });
            }

            if let Some(p) = self.pointer.press {
                if down && (c - p.at).length() > 5.0 {
                    self.pointer.press = None;
                    self.pick_up(c);
                } else if !down {
                    // Released without moving: that's a click.
                    self.pointer.press = None;
                    self.clicked();
                } else if self.time - p.t > 0.45 {
                    // Long press: pick him up even without moving.
                    self.pointer.press = None;
                    self.pick_up(c);
                }
            }
        } else if self.grab.is_some() && !down {
            self.release();
        }

        self.pointer.prev_down = down;
        self.pointer.last = cursor;
        self.want_mouse = hovered || self.pointer.press.is_some() || self.grab.is_some();
    }

    fn pet(&mut self, dt: f32) {
        if matches!(self.act, Act::Annoyed) || self.act.handled() || self.act.airborne() {
            return;
        }
        self.affection = (self.affection + dt * 0.35).min(1.0);
        self.annoyance = (self.annoyance - dt * 0.5).max(0.0);
        if self.act == Act::Sleep {
            // Being petted in his sleep just makes him smile.
            return;
        }
        if self.act != Act::Petted {
            self.plan = None;
            self.enter(Act::Petted);
            self.say_key(Quip::Petted);
            self.sfx(Sfx::Purr);
        } else {
            // Keep it going while the strokes continue.
            self.act_t = self.act_t.min(0.2);
        }
        if self.rng.chance(dt * 4.0) {
            let p = self.head_point();
            self.fx.heart(p, &mut self.rng);
        }
    }

    fn clicked(&mut self) {
        let now = self.time;
        self.pointer.clicks.retain(|t| now - t < 6.0);
        let double = self.pointer.clicks.last().is_some_and(|t| now - t < 0.35);
        self.pointer.clicks.push(now);
        self.annoyance += 0.22;

        if self.act == Act::Sleep {
            self.annoyance += 0.2;
            self.enter(Act::Wake);
            self.say_key(Quip::Woken);
            return;
        }
        if self.annoyance > 0.9 && self.pointer.clicks.len() >= 4 {
            self.enter(Act::Annoyed);
            self.fx.emote(Emote::Anger);
            self.say_key(Quip::Annoyed);
            return;
        }
        if double && self.support.is_some() {
            self.start_hop(Act::Flip);
            return;
        }
        if self.act.airborne() || self.act.handled() || matches!(self.act, Act::Climb | Act::Mantle) {
            return;
        }
        self.plan = None;
        self.enter(Act::Booped);
        self.fx.emote(Emote::Exclaim);
        self.say_key(Quip::Booped);
        self.sfx(Sfx::Boop);
    }

    fn pick_up(&mut self, at: Pos2) {
        let s = self.scale();
        let centre = self.pos - Vec2::new(0.0, CENTRE_H * s);
        let d = centre - at;
        let len = d.length().clamp(6.0 * s, 40.0 * s);
        let theta = d.x.atan2(d.y.max(0.1));
        self.grab = Some(Grab {
            len,
            theta: theta.clamp(-1.2, 1.2),
            omega: 0.0,
            last_anchor: at,
            anchor_vel: Vec2::ZERO,
        });
        self.support = None;
        self.climb = None;
        self.plan = None;
        self.vel = Vec2::ZERO;
        self.enter(Act::Dragged);
        self.fx.emote(Emote::Exclaim);
        self.say_key(Quip::PickedUp);
        self.sfx(Sfx::Pickup);
    }

    /// Hangs from the cursor like a pendulum: swinging the cursor sideways
    /// swings him the other way, and he settles back underneath.
    fn drag_to(&mut self, anchor: Pos2, dt: f32) {
        let Some(mut g) = self.grab else { return };
        if dt <= 0.0 {
            return;
        }
        let v = (anchor - g.last_anchor) / dt;
        let accel = (v - g.anchor_vel) / dt;
        g.anchor_vel = g.anchor_vel + (v - g.anchor_vel) * (1.0 - (-dt * 30.0).exp());
        g.last_anchor = anchor;

        let gravity = 2600.0 * self.cfg.dangle_stiffness;
        let len = g.len.max(1.0);
        let alpha = -(gravity / len) * g.theta.sin() - (accel.x.clamp(-60000.0, 60000.0) / len) * g.theta.cos()
            - 5.0 * g.omega;
        g.omega += alpha * dt;
        g.omega = g.omega.clamp(-30.0, 30.0);
        g.theta = (g.theta + g.omega * dt).clamp(-2.6, 2.6);

        let s = self.scale();
        let centre = anchor + Vec2::new(g.theta.sin(), g.theta.cos()) * len;
        self.pos = centre + Vec2::new(0.0, CENTRE_H * s);
        self.roll = -g.theta;
        self.vel = g.anchor_vel;
        if g.anchor_vel.x.abs() > 40.0 {
            self.facing = g.anchor_vel.x.signum();
        }
        self.grab = Some(g);
    }

    fn release(&mut self) {
        let Some(g) = self.grab.take() else { return };
        let tangential = Vec2::new(g.theta.cos(), -g.theta.sin()) * g.omega * g.len;
        let mut v = self.pointer.vel * self.cfg.throw_power + tangential * 0.5;
        let max = self.cfg.max_throw_speed;
        if v.length() > max {
            v = v.normalized() * max;
        }
        self.vel = v;
        self.roll_vel = g.omega * 0.6 + v.x * self.cfg.spin_per_speed;
        self.support = None;
        self.bounces = 0;
        self.hard_landing = false;
        if v.length() > 500.0 {
            self.enter(Act::Thrown);
            self.say_key(Quip::Thrown);
            self.sfx(Sfx::Throw);
        } else {
            self.enter(Act::Fall);
        }
    }

    pub fn is_grabbed(&self) -> bool {
        self.grab.is_some()
    }

    // ------------------------------------------------------------------
    // Locomotion helpers used by the brain
    // ------------------------------------------------------------------

    /// Launches a ballistic jump that lands on `to`.
    pub(crate) fn jump_to(&mut self, to: Pos2, act: Act) {
        let g = self.cfg.gravity;
        let from = self.pos;
        // Apex a little above the higher of the two ends.
        let apex = from.y.min(to.y) - (40.0 + (to.x - from.x).abs() * 0.15).min(160.0);
        let up = (from.y - apex).max(8.0);
        let down = (to.y - apex).max(8.0);
        let vy = -(2.0 * g * up).sqrt();
        let t = (2.0 * up / g).sqrt() + (2.0 * down / g).sqrt();
        let vx = (to.x - from.x) / t.max(0.05);
        self.vel = Vec2::new(vx, vy);
        self.support = None;
        self.climb = None;
        if vx.abs() > 5.0 {
            self.facing = vx.signum();
        }
        self.squash_vel -= 9.0;
        self.enter(act);
    }

    /// Small in-place jumps: hops, flips, pounces with no target.
    pub(crate) fn start_hop(&mut self, act: Act) {
        if self.support.is_none() && self.climb.is_none() && self.grab.is_none() && act != Act::Fall {
            self.enter(act);
            return;
        }
        let (vx, vy) = match act {
            Act::Flip => (self.facing * 40.0, -820.0),
            Act::Pounce => (self.facing * 380.0, -560.0),
            Act::Jump => (self.facing * 160.0, -700.0),
            _ => (self.facing * 30.0, -430.0),
        };
        let k = (self.cfg.gravity / 1900.0).sqrt();
        self.vel = Vec2::new(vx, vy * k);
        self.support = None;
        self.climb = None;
        self.squash_vel -= 8.0;
        self.enter(act);
    }

    pub(crate) fn start_climb(&mut self, owner: Owner, side: f32) {
        self.climb = Some(Climb { owner, side });
        self.support = None;
        self.vel = Vec2::ZERO;
        self.facing = -side;
        self.enter(Act::Climb);
    }

    /// Highest jump he can make, in points.
    pub(crate) fn jump_reach(&self) -> f32 {
        self.cfg.jump_height
    }

    // ------------------------------------------------------------------
    // Integration
    // ------------------------------------------------------------------

    pub(crate) fn physics(&mut self, dt: f32) {
        if self.grab.is_some() {
            return;
        }
        let s = self.scale();
        let half = HALF_LEN * s * 0.7;
        let area = self.area;

        if let Some(c) = self.climb {
            self.climb_step(c, dt);
            return;
        }

        if let Some(owner) = self.support {
            let surf = if owner == FLOOR {
                Some((area.left(), area.right(), area.bottom()))
            } else {
                self.owner_rect(owner).map(|r| (r.left(), r.right(), r.top()))
            };
            let Some((x0, x1, y)) = surf else {
                self.support = None;
                self.enter_fall();
                return;
            };
            self.pos.y = y;
            // Ground movement: ease toward the speed the brain asked for.
            let accel = if self.act == Act::Panic { 2400.0 } else { 1300.0 };
            let dv = self.desired_vx - self.vel.x;
            self.vel.x += dv.clamp(-accel * dt, accel * dt);
            self.vel.y = 0.0;
            self.pos.x += self.vel.x * dt;

            // Walls of the display.
            if self.pos.x < area.left() + half {
                self.pos.x = area.left() + half;
                self.vel.x = 0.0;
            } else if self.pos.x > area.right() - half {
                self.pos.x = area.right() - half;
                self.vel.x = 0.0;
            }
            // Walked off the end of a window.
            if owner != FLOOR && (self.pos.x < x0 - 2.0 || self.pos.x > x1 + 2.0) {
                self.support = None;
                self.vel.y = -60.0;
                self.enter_fall();
            }
            // Stand back up after a tumble.
            let upright = (self.roll / std::f32::consts::TAU).round() * std::f32::consts::TAU;
            self.roll = approach(self.roll, upright, 10.0, dt);
            self.roll_vel = 0.0;
            if (self.roll - upright).abs() < 0.01 {
                self.roll = 0.0;
            }
            return;
        }

        // --- airborne ---
        let prev = self.pos;
        self.vel.y += self.cfg.gravity * dt;
        self.vel *= (-dt * self.cfg.air_drag).exp();
        self.pos += self.vel * dt;
        self.roll += self.roll_vel * dt;
        if !matches!(self.act, Act::Thrown | Act::Bounce | Act::Flip) {
            // Cats and raccoons right themselves in the air.
            self.roll_vel *= (-dt * 3.0).exp();
            let upright = (self.roll / std::f32::consts::TAU).round() * std::f32::consts::TAU;
            self.roll = approach(self.roll, upright, 3.0, dt);
        }

        // Side walls.
        let wall_hit = if self.pos.x < area.left() + half {
            self.pos.x = area.left() + half;
            Some(-1.0)
        } else if self.pos.x > area.right() - half {
            self.pos.x = area.right() - half;
            Some(1.0)
        } else {
            None
        };
        if let Some(side) = wall_hit {
            let speed = self.vel.x.abs();
            if self.cfg.splat_on_walls && speed > self.cfg.hard_impact_speed * 0.9 && self.act == Act::Thrown {
                self.vel = Vec2::ZERO;
                self.roll_vel = 0.0;
                self.roll = 0.0;
                self.facing = side;
                self.squash = -0.6;
                self.enter(Act::Splat);
                self.fx.dust(self.pos - Vec2::new(0.0, CENTRE_H * s), 8, &mut self.rng);
                self.say_key(Quip::Splat);
                self.sfx(Sfx::Thud);
                return;
            }
            self.vel.x = -side * speed * self.cfg.wall_restitution;
            self.roll_vel = -self.roll_vel * 0.6;
        }
        // Ceiling (the menu bar).
        let top = self.pos.y - 62.0 * s;
        if top < area.top() {
            self.pos.y += area.top() - top;
            self.vel.y = self.vel.y.abs() * self.cfg.wall_restitution;
        }

        if self.act == Act::Splat {
            // Stuck to the wall until the timer runs out.
            self.pos = prev;
            self.vel = Vec2::ZERO;
            return;
        }

        // Landing: the first surface whose top edge we crossed from above.
        if self.vel.y >= 0.0 {
            let mut hit: Option<(Owner, f32)> = None;
            for srf in &self.surfaces {
                if prev.y <= srf.y + 0.5 && self.pos.y >= srf.y && srf.contains_x(self.pos.x)
                    && hit.is_none_or(|(_, y)| srf.y < y)
                {
                    hit = Some((srf.owner, srf.y));
                }
            }
            if let Some((owner, y)) = hit {
                self.touch_down(owner, y);
            }
        }

        // Safety net: never leave the display.
        if self.pos.y > self.stage.bottom() + 200.0 || !self.pos.x.is_finite() || !self.pos.y.is_finite() {
            self.pos = Pos2::new(area.center().x, area.bottom());
            self.vel = Vec2::ZERO;
            self.roll = 0.0;
            self.roll_vel = 0.0;
            self.support = Some(FLOOR);
            self.enter(Act::Land);
        }
    }

    fn touch_down(&mut self, owner: Owner, y: f32) {
        let impact = self.vel.y;
        self.pos.y = y;
        let hard = impact > self.cfg.hard_impact_speed;
        let tumbling = matches!(self.act, Act::Thrown | Act::Bounce);

        if tumbling && impact > 260.0 && self.bounces < 4 {
            // Bounce.
            self.bounces += 1;
            self.hard_landing |= hard;
            self.vel.y = -impact * self.cfg.restitution;
            self.vel.x *= self.cfg.ground_friction;
            self.roll_vel = self.vel.x * self.cfg.spin_per_speed * 1.4;
            self.squash_vel += (impact / 120.0).min(14.0);
            self.fx.dust(self.pos, 5 + (impact / 300.0) as usize, &mut self.rng);
            self.sfx(Sfx::Thud);
            if self.act != Act::Bounce {
                self.enter(Act::Bounce);
            }
            return;
        }

        self.support = Some(owner);
        self.vel.y = 0.0;
        self.vel.x *= if tumbling { self.cfg.ground_friction * 0.5 } else { 0.4 };
        self.roll_vel = 0.0;
        self.squash_vel += (impact / 160.0).clamp(1.0, 10.0);
        if impact > 500.0 {
            self.fx.dust(self.pos, 4, &mut self.rng);
        }

        let off_upright = {
            let r = self.roll.rem_euclid(std::f32::consts::TAU);
            r.min(std::f32::consts::TAU - r)
        };
        if (tumbling || hard) && (self.hard_landing || hard) && self.cfg.dizzy_after_hard_landing {
            self.hard_landing = false;
            self.enter(Act::Dizzy);
            self.fx.emote(super::fx::Emote::Dizzy);
            self.say_key(Quip::Dizzy);
        } else if off_upright > 0.7 {
            self.enter(Act::GetUp);
        } else {
            if self.act == Act::Pounce {
                self.pounce_landed();
            }
            self.enter(Act::Land);
        }
        if owner != FLOOR && self.rng.chance(0.25) {
            self.say_key(Quip::OnWindow);
        }
    }

    pub(crate) fn enter_fall(&mut self) {
        if !self.act.airborne() || self.act == Act::Hop {
            self.enter(Act::Fall);
        }
    }

    fn climb_step(&mut self, c: Climb, dt: f32) {
        let s = self.scale();
        let Some(r) = self.owner_rect(c.owner) else {
            self.climb = None;
            self.enter_fall();
            return;
        };
        let wall_x = if c.side < 0.0 { r.left() - 10.0 * s } else { r.right() + 10.0 * s };
        self.pos.x = wall_x;
        self.facing = -c.side;
        self.vel = Vec2::new(0.0, -self.cfg.climb_speed);
        self.pos.y -= self.cfg.climb_speed * dt;
        // Body is drawn rotated against the wall, so its feet are at pos and
        // its nose points up. Reaching the top: haul up onto it.
        if self.pos.y - 20.0 * s <= r.top() {
            self.climb = None;
            self.pos = Pos2::new(wall_x - c.side * HALF_LEN * s * 0.9, r.top());
            self.pos.x = self.pos.x.clamp(r.left() + 4.0, r.right() - 4.0);
            self.support = Some(c.owner);
            self.vel = Vec2::ZERO;
            self.enter(Act::Mantle);
        }
        if self.pos.y > self.area.bottom() + 1.0 {
            self.climb = None;
            self.enter_fall();
        }
    }

    // ------------------------------------------------------------------
    // Secondary motion
    // ------------------------------------------------------------------

    pub(crate) fn body_springs(&mut self, dt: f32) {
        // Squash and stretch spring.
        let k = 170.0;
        let c = 13.0;
        let mut target = 0.0;
        if self.support.is_none() && self.climb.is_none() && self.grab.is_none() {
            target = -(self.vel.y.abs() / 2400.0).min(0.22);
        }
        let a = -k * (self.squash - target) - c * self.squash_vel;
        self.squash_vel += a * dt;
        self.squash += self.squash_vel * dt;
        self.squash = self.squash.clamp(-0.6, 0.7);

        // Turning around sweeps the facing through zero.
        let turn_rate = if self.act == Act::Panic { 18.0 } else { 11.0 };
        let step = turn_rate * dt;
        if (self.facing - self.face).abs() <= step {
            self.face = self.facing;
        } else {
            self.face += step * (self.facing - self.face).signum();
        }

        // Tail lags behind motion.
        let lag_target = (-self.vel.x * 0.0009 * self.face).clamp(-0.6, 0.6) + (self.vel.y * 0.0004).clamp(-0.4, 0.4);
        self.tail_lag = approach(self.tail_lag, lag_target, 6.0, dt);
    }

    pub(crate) fn climbing(&self) -> Option<f32> {
        self.climb.map(|c| c.side)
    }
}
