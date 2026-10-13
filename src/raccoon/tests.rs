//! Behaviour tests. The world is deterministic given its inputs, so all of
//! this runs without a window.

use eframe::egui::{Pos2, Rect, pos2, vec2};

use super::acts::{self, Act};
use super::render::{Canvas, Look, draw_pose};
use super::*;

const STAGE: Rect = Rect { min: Pos2::ZERO, max: pos2(1440.0, 900.0) };

fn cfg() -> RaccoonSettings {
    let mut c = RaccoonSettings::default();
    c.enabled = true;
    c
}

fn world() -> World {
    let mut w = World::new(&cfg(), 42);
    w.set_stage(STAGE, STAGE);
    w.placed = true;
    w.pos = pos2(700.0, 900.0);
    w.support = Some(FLOOR);
    w.enter(Act::Idle);
    w
}

fn input(cpu: f32) -> Inputs {
    Inputs { dt: 1.0 / 60.0, vitals: Vitals { cpu, ..Default::default() }, hour: 12, ..Default::default() }
}

fn run(w: &mut World, cpu: f32, secs: f32) {
    for _ in 0..(secs * 60.0) as usize {
        w.tick(&input(cpu));
    }
}

fn win(id: u32, x: f32, y: f32, wd: f32, h: f32) -> WinRect {
    WinRect { id, rect: Rect::from_min_size(pos2(x, y), vec2(wd, h)), own: false }
}

// --- physics -------------------------------------------------------------

#[test]
fn settles_on_the_floor() {
    let mut w = world();
    w.pos = pos2(700.0, 200.0);
    w.support = None;
    w.enter(Act::Fall);
    run(&mut w, 30.0, 4.0);
    assert!(w.support.is_some(), "should land");
    assert!((w.pos.y - 900.0).abs() < 1.0 || w.support != Some(FLOOR), "y = {}", w.pos.y);
}

#[test]
fn never_teleports_up_onto_a_window_above() {
    // The old raccoon snapped to any platform whose top was above his feet.
    let mut w = world();
    w.set_windows(vec![win(1, 500.0, 300.0, 400.0, 300.0)]);
    w.cfg.walk_on_windows = true;
    w.rebuild_surfaces();
    w.tick(&input(30.0));
    assert_eq!(w.support, Some(FLOOR));
    assert!(w.pos.y > 850.0, "jumped to {}", w.pos.y);
}

#[test]
fn lands_on_a_window_when_falling_onto_it() {
    let mut w = world();
    w.set_windows(vec![win(1, 500.0, 400.0, 400.0, 300.0)]);
    w.pos = pos2(700.0, 200.0);
    w.support = None;
    w.enter(Act::Fall);
    run(&mut w, 30.0, 2.0);
    assert_eq!(w.support, Some(WINDOW_TAG | 1));
    assert!((w.pos.y - 400.0).abs() < 1.0);
}

#[test]
fn rides_a_window_that_moves() {
    let mut w = world();
    w.set_windows(vec![win(1, 500.0, 400.0, 400.0, 300.0)]);
    w.pos = pos2(700.0, 400.0);
    w.support = Some(WINDOW_TAG | 1);
    w.enter(Act::Sit);
    w.tick(&input(30.0));
    w.set_windows(vec![win(1, 560.0, 350.0, 400.0, 300.0)]);
    w.tick(&input(30.0));
    assert_eq!(w.support, Some(WINDOW_TAG | 1));
    assert!((w.pos.x - 760.0).abs() < 3.0, "x = {}", w.pos.x);
    assert!((w.pos.y - 350.0).abs() < 1.0, "y = {}", w.pos.y);
}

#[test]
fn falls_when_his_window_closes() {
    let mut w = world();
    w.set_windows(vec![win(1, 500.0, 400.0, 400.0, 300.0)]);
    w.pos = pos2(700.0, 400.0);
    w.support = Some(WINDOW_TAG | 1);
    w.enter(Act::Sit);
    w.tick(&input(30.0));
    w.set_windows(vec![]);
    run(&mut w, 30.0, 2.0);
    assert_eq!(w.support, Some(FLOOR));
    assert!(w.pos.y > 850.0);
}

#[test]
fn covered_parts_of_a_window_are_not_walkable() {
    let mut w = world();
    // Window 2 is in front and covers the left half of window 1's top edge.
    w.set_windows(vec![win(2, 400.0, 300.0, 300.0, 400.0), win(1, 500.0, 400.0, 500.0, 300.0)]);
    w.rebuild_surfaces();
    let segs: Vec<_> = w.surfaces.iter().filter(|s| s.owner == WINDOW_TAG | 1).collect();
    assert_eq!(segs.len(), 1);
    assert!((segs[0].x0 - 700.0).abs() < 0.1, "{:?}", segs[0]);
}

#[test]
fn stays_on_screen_when_thrown_hard() {
    let mut w = world();
    w.support = None;
    w.vel = vec2(5000.0, -3000.0);
    w.enter(Act::Thrown);
    run(&mut w, 30.0, 8.0);
    assert!(STAGE.expand(1.0).contains(w.pos), "escaped to {:?}", w.pos);
    assert!(w.support.is_some());
}

// --- handling ------------------------------------------------------------

fn cursor_at(w: &World) -> Pos2 {
    w.hit_circles()[1].0
}

#[test]
fn drag_and_throw() {
    let mut w = world();
    run(&mut w, 30.0, 0.2);
    let mut c = cursor_at(&w);
    let mut i = input(30.0);
    i.cursor = Some(c);
    i.button_down = true;
    i.presses = 1;
    w.tick(&i);
    i.presses = 0;
    for _ in 0..30 {
        c += vec2(14.0, -9.0);
        i.cursor = Some(c);
        w.tick(&i);
    }
    assert!(w.is_grabbed(), "should be carried");
    assert_eq!(w.act, Act::Dragged);
    assert!(w.pos.y < 880.0, "lifted: {}", w.pos.y);
    assert!(w.want_mouse);
    i.button_down = false;
    w.tick(&i);
    assert!(!w.is_grabbed());
    assert_eq!(w.act, Act::Thrown);
    assert!(w.vel.x > 0.0);
}

#[test]
fn a_click_boops_him() {
    let mut w = world();
    run(&mut w, 30.0, 0.2);
    let c = cursor_at(&w);
    let mut i = input(30.0);
    i.cursor = Some(c);
    // A click that starts and ends between two frames.
    i.presses = 1;
    w.tick(&i);
    i.presses = 0;
    w.tick(&i);
    assert_eq!(w.act, Act::Booped);
}

#[test]
fn rubbing_pets_him() {
    let mut w = world();
    run(&mut w, 30.0, 0.2);
    let c = cursor_at(&w);
    let mut i = input(30.0);
    for k in 0..40 {
        let dx = if (k / 3) % 2 == 0 { 4.0 } else { -4.0 };
        i.cursor = Some(c + vec2(dx * (k % 3) as f32, 0.0));
        w.tick(&i);
    }
    assert_eq!(w.act, Act::Petted);
    assert!(w.affection > 0.3);
}

#[test]
fn clicks_elsewhere_pass_through() {
    let mut w = world();
    let mut i = input(30.0);
    i.cursor = Some(pos2(100.0, 100.0));
    i.button_down = true;
    i.presses = 1;
    w.tick(&i);
    assert!(!w.want_mouse);
    assert!(!w.is_grabbed());
}

#[test]
fn decorative_mode_ignores_the_mouse() {
    let mut w = world();
    w.cfg.click_through = true;
    run(&mut w, 30.0, 0.2);
    let mut i = input(30.0);
    i.cursor = Some(cursor_at(&w));
    i.presses = 1;
    i.button_down = true;
    w.tick(&i);
    assert!(!w.want_mouse);
}

// --- mind ----------------------------------------------------------------

#[test]
fn naps_when_the_mac_is_idle() {
    let mut w = world();
    w.cfg.sleep_after_secs = 10.0;
    let mut slept = false;
    for _ in 0..(60.0 * 60.0) as usize {
        w.tick(&input(2.0));
        slept |= w.act == Act::Sleep;
    }
    assert!(slept, "never dozed off");
}

#[test]
fn panics_when_the_mac_is_pinned() {
    let mut w = world();
    let mut seen = false;
    for _ in 0..(10.0 * 60.0) as usize {
        w.tick(&input(99.0));
        seen |= matches!(w.act, Act::Panic | Act::Alert);
    }
    assert!(seen, "never reacted to 99% CPU");
}

#[test]
fn zero_weight_disables_an_activity() {
    let mut c = cfg();
    c.states.insert("Dance".into(), settings::Tuning { duration_mul: 1.0, weight_mul: 0.0, cooldown_mul: 1.0 });
    let mut w = World::new(&c, 3);
    w.set_stage(STAGE, STAGE);
    w.placed = true;
    w.support = Some(FLOOR);
    for _ in 0..(200.0 * 60.0) as usize {
        w.tick(&input(40.0));
        assert_ne!(w.act, Act::Dance);
    }
}

#[test]
fn explores_onto_windows_eventually() {
    let mut w = world();
    w.cfg.sleep_when_idle = false;
    w.set_windows(vec![win(1, 300.0, 700.0, 700.0, 200.0)]);
    let mut reached = false;
    for _ in 0..(240.0 * 60.0) as usize {
        w.tick(&input(40.0));
        reached |= w.support == Some(WINDOW_TAG | 1);
        assert!(w.pos.x.is_finite() && w.pos.y.is_finite());
    }
    assert!(reached, "never got onto the window");
}

#[test]
fn climbs_a_tall_window() {
    let mut w = world();
    w.set_windows(vec![win(1, 300.0, 300.0, 600.0, 600.0)]);
    w.pos = pos2(1100.0, 900.0);
    w.rebuild_surfaces();
    let dest = *w.surfaces.iter().find(|s| s.owner == WINDOW_TAG | 1).unwrap();
    let plan = w.plan_to(dest, 600.0).expect("a route");
    assert!(matches!(plan, Plan::ClimbUp { .. }), "{plan:?}");
    w.plan = Some(plan);
    w.enter(Act::Explore);
    let mut ok = false;
    for _ in 0..(30.0 * 60.0) as usize {
        w.tick(&input(40.0));
        if w.support == Some(WINDOW_TAG | 1) {
            ok = true;
            break;
        }
    }
    assert!(ok, "didn't make it up; act {}", w.act.label());
}

#[test]
fn long_run_is_stable() {
    // A long soak with a moving cursor and varying load: nothing NaN, never
    // off-screen.
    let mut w = world();
    w.set_windows(vec![win(1, 200.0, 500.0, 500.0, 300.0), win(2, 800.0, 300.0, 400.0, 400.0)]);
    for k in 0..(600.0 * 60.0) as usize {
        let t = k as f32 / 60.0;
        let mut i = input(if (t / 40.0) as i32 % 2 == 0 { 15.0 } else { 97.0 });
        i.cursor = Some(pos2(720.0 + (t * 0.7).sin() * 600.0, 450.0 + (t * 0.3).cos() * 300.0));
        w.tick(&i);
        assert!(w.pos.x.is_finite() && w.pos.y.is_finite(), "NaN at {t}");
        assert!(STAGE.expand(260.0).contains(w.pos), "off-screen at {t}: {:?}", w.pos);
    }
}

// --- drawing -------------------------------------------------------------

#[test]
fn every_activity_draws_sensibly() {
    let c = cfg();
    let pal = c.palette();
    for d in acts::all() {
        let w = super::preview::staged(d.act, 0.5, &c);
        let mut cv = Canvas::new(2.0);
        let mut xf = w.xform();
        xf.origin = pos2(200.0, 200.0);
        draw_pose(&mut cv, &w.pose, xf, &Look { pal: &pal, outline_width: 1.6, alpha: 1.0 });
        assert!(cv.mesh.vertices.len() > 200, "{} drew almost nothing", d.label);
        for v in &cv.mesh.vertices {
            assert!(v.pos.x.is_finite() && v.pos.y.is_finite(), "{} has NaN vertices", d.label);
            assert!((v.pos - pos2(200.0, 170.0)).length() < 160.0, "{} drew far away: {:?}", d.label, v.pos);
        }
    }
}

#[test]
fn background_is_transparent() {
    // The raccoon's mesh must not cover his cell: corners stay clear.
    let c = cfg();
    let w = super::preview::staged(Act::Idle, 0.2, &c);
    let mut cv = Canvas::new(1.0);
    let mut xf = w.xform();
    xf.origin = pos2(100.0, 130.0);
    draw_pose(&mut cv, &w.pose, xf, &Look { pal: &c.palette(), outline_width: 1.6, alpha: 1.0 });
    let mut r = super::preview::Raster::new(200, 160, eframe::egui::Color32::TRANSPARENT);
    r.draw(&cv.mesh, 1.0, [0.0, 0.0]);
    for (x, y) in [(0, 0), (199, 0), (0, 159), (199, 159), (5, 80)] {
        assert_eq!(r.px[y * 200 + x][3], 0.0, "pixel {x},{y} is painted");
    }
    let opaque = r.px.iter().filter(|p| p[3] > 0.99).count();
    assert!(opaque > 2000, "only {opaque} opaque pixels");
}

#[test]
fn sanitising_clamps_hostile_values() {
    let mut c = cfg();
    c.gravity = f32::NAN;
    c.size = -3.0;
    c.restitution = 50.0;
    c.name = "   ".into();
    c.sanitize();
    assert!(c.gravity.is_finite());
    assert!(c.size >= 0.4);
    assert!(c.restitution <= 0.95);
    assert!(!c.name.is_empty());
}

#[test]
fn old_settings_files_still_load() {
    let json = r#"{"enabled": true, "sprite_scale": 1.6, "gravity": 900.0, "states": {}}"#;
    let c: RaccoonSettings = serde_json::from_str(json).expect("parses");
    assert!(c.enabled);
    assert_eq!(c.gravity, 900.0);
}
