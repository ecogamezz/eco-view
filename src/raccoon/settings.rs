//! Everything about the raccoon a user might reasonably want to tune.
//!
//! Stored under the `companion` key of the main settings file. Every field has
//! a default (`#[serde(default)]`) and unknown keys are ignored, so settings
//! files written by older builds still load.

use std::collections::BTreeMap;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use super::acts::{self, Act};

/// Per-activity tuning. `duration_mul` scales how long it lasts, `weight_mul`
/// how often it is picked, and `cooldown_mul` how long before it may repeat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    pub duration_mul: f32,
    pub weight_mul: f32,
    pub cooldown_mul: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self { duration_mul: 1.0, weight_mul: 1.0, cooldown_mul: 1.0 }
    }
}

/// Coat colours. `Custom` uses the five colours stored in the settings; the
/// rest of the palette (highlights, rings, paws) is derived from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Coat {
    #[default]
    Classic,
    Silver,
    Ginger,
    Midnight,
    Snow,
    Neon,
    Custom,
}

impl Coat {
    pub const ALL: [Coat; 7] = [
        Coat::Classic,
        Coat::Silver,
        Coat::Ginger,
        Coat::Midnight,
        Coat::Snow,
        Coat::Neon,
        Coat::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Coat::Classic => "Classic",
            Coat::Silver => "Silver",
            Coat::Ginger => "Ginger",
            Coat::Midnight => "Midnight",
            Coat::Snow => "Snow",
            Coat::Neon => "Neon",
            Coat::Custom => "Custom",
        }
    }

    /// (fur, belly, mask, eyes, accent)
    fn base(self) -> [[u8; 3]; 5] {
        match self {
            Coat::Classic | Coat::Custom => [
                [138, 126, 116],
                [236, 229, 218],
                [43, 37, 34],
                [24, 18, 15],
                [214, 152, 148],
            ],
            Coat::Silver => [
                [150, 156, 166],
                [238, 240, 244],
                [44, 48, 58],
                [20, 22, 30],
                [206, 160, 176],
            ],
            Coat::Ginger => [
                [196, 110, 62],
                [248, 226, 196],
                [86, 42, 26],
                [36, 20, 12],
                [236, 150, 120],
            ],
            Coat::Midnight => [
                [70, 74, 96],
                [168, 176, 204],
                [18, 18, 28],
                [120, 200, 255],
                [150, 130, 220],
            ],
            Coat::Snow => [
                [232, 230, 226],
                [252, 250, 246],
                [196, 188, 182],
                [190, 60, 80],
                [244, 170, 180],
            ],
            Coat::Neon => [
                [64, 58, 92],
                [120, 240, 220],
                [24, 16, 40],
                [255, 80, 200],
                [255, 120, 220],
            ],
        }
    }
}

/// The resolved colours the renderer uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub fur: Color32,
    pub fur_light: Color32,
    pub fur_dark: Color32,
    pub belly: Color32,
    pub mask: Color32,
    pub ring_dark: Color32,
    pub ring_light: Color32,
    pub paw: Color32,
    pub eye: Color32,
    pub nose: Color32,
    pub ear_inner: Color32,
    pub outline: Color32,
    pub blush: Color32,
}

impl Palette {
    pub fn from_base(fur: [u8; 3], belly: [u8; 3], mask: [u8; 3], eye: [u8; 3], accent: [u8; 3]) -> Self {
        let fur = rgb(fur);
        let belly = rgb(belly);
        let mask = rgb(mask);
        let accent = rgb(accent);
        Self {
            fur,
            fur_light: mix(fur, Color32::WHITE, 0.22),
            fur_dark: mix(fur, mask, 0.32),
            belly,
            mask,
            ring_dark: mix(mask, fur, 0.22),
            ring_light: mix(belly, fur, 0.28),
            paw: mix(mask, fur, 0.3),
            eye: rgb(eye),
            nose: mix(mask, Color32::BLACK, 0.35),
            ear_inner: mix(accent, mask, 0.35),
            outline: mix(mask, Color32::BLACK, 0.55),
            blush: accent,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RaccoonSettings {
    pub enabled: bool,
    pub name: String,

    // --- appearance ---
    pub size: f32,
    pub animation_speed: f32,
    pub coat: Coat,
    pub fur: [u8; 3],
    pub belly: [u8; 3],
    pub mask: [u8; 3],
    pub eyes: [u8; 3],
    pub accent: [u8; 3],
    pub outline_width: f32,
    pub show_shadow: bool,
    pub show_name_tag: bool,
    pub particles: bool,
    pub speech_bubbles: bool,
    /// 0 = only speaks when something happens, 1 = chatty.
    pub chattiness: f32,

    // --- behaviour ---
    /// Ignore the mouse completely, so he is purely decorative.
    pub click_through: bool,
    pub walk_on_windows: bool,
    pub walk_on_dashboard: bool,
    pub follow_cursor: bool,
    pub eyes_follow_cursor: bool,
    pub play_with_cursor: bool,
    pub allow_petting: bool,
    /// How hyperactive he is: scales energy drain and how often he moves.
    pub liveliness: f32,
    pub sleep_when_idle: bool,
    pub sleep_after_secs: f32,
    pub sleepy_at_night: bool,

    // --- movement ---
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    pub jump_height: f32,
    pub climb_speed: f32,
    pub can_climb: bool,

    // --- physics ---
    pub gravity: f32,
    pub air_drag: f32,
    pub restitution: f32,
    pub ground_friction: f32,
    pub wall_restitution: f32,
    pub throw_power: f32,
    pub max_throw_speed: f32,
    pub spin_per_speed: f32,
    pub hard_impact_speed: f32,
    pub dizzy_after_hard_landing: bool,
    pub splat_on_walls: bool,
    pub dangle_stiffness: f32,

    // --- reactions ---
    pub react_cpu: bool,
    pub react_gpu: bool,
    pub react_temperature: bool,
    pub react_battery: bool,
    pub react_memory: bool,
    pub react_network: bool,
    pub sleep_below_cpu: f32,
    pub panic_above_cpu: f32,
    pub panic_above_gpu: f32,
    pub hot_above_celsius: f32,
    pub low_battery_percent: f32,
    pub memory_full_percent: f32,
    pub download_mbps: f32,

    // --- sound ---
    pub sounds: bool,

    // --- timing ---
    pub state_time_scale: f32,
    pub states: BTreeMap<String, Tuning>,
}

impl Default for RaccoonSettings {
    fn default() -> Self {
        let [fur, belly, mask, eyes, accent] = Coat::Classic.base();
        Self {
            enabled: false,
            name: "Bandit".into(),
            size: 1.0,
            animation_speed: 1.0,
            coat: Coat::Classic,
            fur,
            belly,
            mask,
            eyes,
            accent,
            outline_width: 1.6,
            show_shadow: true,
            show_name_tag: false,
            particles: true,
            speech_bubbles: true,
            chattiness: 0.5,
            click_through: false,
            walk_on_windows: true,
            walk_on_dashboard: true,
            follow_cursor: false,
            eyes_follow_cursor: true,
            play_with_cursor: true,
            allow_petting: true,
            liveliness: 1.0,
            sleep_when_idle: true,
            sleep_after_secs: 45.0,
            sleepy_at_night: true,
            walk_speed: 62.0,
            run_speed: 170.0,
            sprint_speed: 300.0,
            jump_height: 190.0,
            climb_speed: 90.0,
            can_climb: true,
            gravity: 1900.0,
            air_drag: 0.25,
            restitution: 0.42,
            ground_friction: 0.65,
            wall_restitution: 0.55,
            throw_power: 1.0,
            max_throw_speed: 3200.0,
            spin_per_speed: 0.006,
            hard_impact_speed: 900.0,
            dizzy_after_hard_landing: true,
            splat_on_walls: true,
            dangle_stiffness: 1.0,
            react_cpu: true,
            react_gpu: true,
            react_temperature: true,
            react_battery: true,
            react_memory: true,
            react_network: true,
            sleep_below_cpu: 12.0,
            panic_above_cpu: 88.0,
            panic_above_gpu: 92.0,
            hot_above_celsius: 90.0,
            low_battery_percent: 15.0,
            memory_full_percent: 92.0,
            download_mbps: 20.0,
            sounds: true,
            state_time_scale: 1.0,
            states: BTreeMap::new(),
        }
    }
}

impl RaccoonSettings {
    pub fn palette(&self) -> Palette {
        if self.coat == Coat::Custom {
            Palette::from_base(self.fur, self.belly, self.mask, self.eyes, self.accent)
        } else {
            let [a, b, c, d, e] = self.coat.base();
            Palette::from_base(a, b, c, d, e)
        }
    }

    /// Copies a preset's colours into the custom slots, so "Custom" starts
    /// from whatever coat was showing.
    pub fn load_coat_into_custom(&mut self, coat: Coat) {
        let [a, b, c, d, e] = coat.base();
        self.fur = a;
        self.belly = b;
        self.mask = c;
        self.eyes = d;
        self.accent = e;
    }

    pub fn tuning(&self, act: Act) -> Tuning {
        self.states.get(acts::def(act).label).copied().unwrap_or_default()
    }

    pub fn weight_of(&self, act: Act) -> f32 {
        self.tuning(act).weight_mul
    }

    pub fn duration_of(&self, act: Act) -> f32 {
        self.tuning(act).duration_mul * self.state_time_scale
    }

    pub fn cooldown_of(&self, act: Act) -> f32 {
        self.tuning(act).cooldown_mul
    }

    /// Clamps everything that could destabilise the render loop or the physics.
    ///
    /// NaN is handled explicitly: `f32::clamp` propagates it, so a hand-edited
    /// `"gravity": null` would otherwise reach the integrator.
    pub fn sanitize(&mut self) {
        fn n(v: &mut f32, lo: f32, hi: f32, fallback: f32) {
            *v = if v.is_finite() { v.clamp(lo, hi) } else { fallback };
        }
        let d = Self::default();
        n(&mut self.size, 0.4, 3.5, d.size);
        n(&mut self.animation_speed, 0.2, 4.0, d.animation_speed);
        n(&mut self.outline_width, 0.0, 5.0, d.outline_width);
        n(&mut self.chattiness, 0.0, 1.0, d.chattiness);
        n(&mut self.liveliness, 0.1, 3.0, d.liveliness);
        n(&mut self.sleep_after_secs, 5.0, 3600.0, d.sleep_after_secs);
        n(&mut self.walk_speed, 5.0, 600.0, d.walk_speed);
        n(&mut self.run_speed, 10.0, 1200.0, d.run_speed);
        n(&mut self.sprint_speed, 10.0, 2000.0, d.sprint_speed);
        n(&mut self.jump_height, 20.0, 900.0, d.jump_height);
        n(&mut self.climb_speed, 10.0, 600.0, d.climb_speed);
        n(&mut self.gravity, 100.0, 8000.0, d.gravity);
        n(&mut self.air_drag, 0.0, 8.0, d.air_drag);
        n(&mut self.restitution, 0.0, 0.95, d.restitution);
        n(&mut self.ground_friction, 0.0, 1.0, d.ground_friction);
        n(&mut self.wall_restitution, 0.0, 0.95, d.wall_restitution);
        n(&mut self.throw_power, 0.0, 5.0, d.throw_power);
        n(&mut self.max_throw_speed, 100.0, 10000.0, d.max_throw_speed);
        n(&mut self.spin_per_speed, 0.0, 0.05, d.spin_per_speed);
        n(&mut self.hard_impact_speed, 50.0, 6000.0, d.hard_impact_speed);
        n(&mut self.dangle_stiffness, 0.2, 4.0, d.dangle_stiffness);
        n(&mut self.sleep_below_cpu, 0.0, 100.0, d.sleep_below_cpu);
        n(&mut self.panic_above_cpu, 1.0, 100.0, d.panic_above_cpu);
        n(&mut self.panic_above_gpu, 1.0, 100.0, d.panic_above_gpu);
        n(&mut self.hot_above_celsius, 30.0, 130.0, d.hot_above_celsius);
        n(&mut self.low_battery_percent, 0.0, 100.0, d.low_battery_percent);
        n(&mut self.memory_full_percent, 10.0, 100.0, d.memory_full_percent);
        n(&mut self.download_mbps, 0.1, 10000.0, d.download_mbps);
        n(&mut self.state_time_scale, 0.1, 5.0, d.state_time_scale);
        for v in self.states.values_mut() {
            n(&mut v.duration_mul, 0.05, 20.0, 1.0);
            n(&mut v.weight_mul, 0.0, 50.0, 1.0);
            n(&mut v.cooldown_mul, 0.0, 10.0, 1.0);
        }
        self.name = self.name.trim().chars().take(24).collect();
        if self.name.is_empty() {
            self.name = d.name;
        }
    }
}

pub fn rgb(v: [u8; 3]) -> Color32 {
    Color32::from_rgb(v[0], v[1], v[2])
}

/// Linear blend of two opaque colours.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
