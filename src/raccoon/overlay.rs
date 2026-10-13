//! The raccoon's window: transparent, borderless, always on top, covering one
//! display, and click-through everywhere except where he is.
//!
//! It is a *deferred* egui viewport, so it runs its own render loop: he keeps
//! moving when the dashboard is minimised or hidden behind other apps.
//!
//! Hit testing doesn't rely on the window receiving events at all. The global
//! cursor is polled from CoreGraphics every frame; the window only stops
//! passing clicks through while the cursor is actually over him (or he is
//! being carried), so the rest of the desktop is never blocked.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Timelike;
use eframe::egui::{self, Color32, Context, FontId, Id, LayerId, Order, Rect, Shape, Vec2, ViewportBuilder, ViewportCommand, ViewportId, vec2};

use super::desktop::{self, Focus, Mouse, WindowWatcher};
use super::{Inputs, Sfx, Vitals, World, render};
use crate::metrics::Metrics;
use crate::sound::{Cue, SoundTx};

pub const TITLE: &str = "eco view raccoon";

pub fn viewport_id() -> ViewportId {
    ViewportId::from_hash_of("eco-raccoon-overlay")
}

/// Everything the overlay's render loop needs. Cheap to clone.
#[derive(Clone)]
pub struct Overlay {
    pub world: Arc<Mutex<World>>,
    metrics: Arc<Mutex<Metrics>>,
    sound: SoundTx,
    windows: WindowWatcher,
    state: Arc<Mutex<State>>,
}

struct State {
    mouse: Mouse,
    seen_windows: u64,
    last: Option<Instant>,
    passthrough: bool,
    configured: bool,
    configure_tries: u32,
    focus: Option<Focus>,
    pressed: bool,
    cursor_icon: egui::CursorIcon,
}

impl Overlay {
    pub fn new(world: World, metrics: Arc<Mutex<Metrics>>, sound: SoundTx) -> Self {
        Self {
            world: Arc::new(Mutex::new(world)),
            metrics,
            sound,
            windows: WindowWatcher::spawn(),
            state: Arc::new(Mutex::new(State {
                mouse: Mouse::default(),
                seen_windows: 0,
                last: None,
                passthrough: true,
                configured: false,
                configure_tries: 0,
                focus: None,
                pressed: false,
                cursor_icon: egui::CursorIcon::Default,
            })),
        }
    }

    /// Called from the dashboard every frame while the raccoon is enabled.
    pub fn show(&self, ctx: &Context, stage: Rect) {
        self.windows.set_active(true);
        let me = self.clone();
        ctx.show_viewport_deferred(viewport_id(), builder(stage), move |vctx, _class| {
            me.frame(vctx, stage);
        });
    }

    /// Called once when the raccoon is put away, so the next window that
    /// opens gets configured again.
    pub fn hidden(&self) {
        self.windows.set_active(false);
        if let Ok(mut s) = self.state.lock() {
            s.configured = false;
            s.configure_tries = 0;
            s.passthrough = true;
            s.last = None;
            s.pressed = false;
        }
    }

    fn frame(&self, ctx: &Context, stage: Rect) {
        let Ok(mut st) = self.state.lock() else { return };
        let now = Instant::now();
        let dt = st.last.map(|l| (now - l).as_secs_f32()).unwrap_or(1.0 / 60.0).clamp(0.0, 0.1);
        st.last = Some(now);

        // Configure once the window exists, and again if AppKit moved it.
        let actual = ctx.input(|i| i.viewport().inner_rect);
        let misplaced = actual.is_some_and(|r| (r.min - stage.min).length() > 1.0 || (r.size() - stage.size()).length() > 1.0);
        if (!st.configured || misplaced) && st.configure_tries < 600 {
            st.configure_tries += 1;
            st.configured = desktop::configure_overlay(TITLE, stage);
        }

        let mouse = st.mouse.poll();
        let vitals = self
            .metrics
            .try_lock()
            .map(|m| Vitals::from_snapshot(&m.snap))
            .ok();

        let Ok(mut w) = self.world.lock() else { return };
        if let Some(list) = self.windows.take_if_newer(&mut st.seen_windows) {
            w.set_windows(list);
        }
        let input = Inputs {
            dt,
            cursor: mouse.map(|m| m.pos),
            button_down: mouse.is_some_and(|m| m.down),
            presses: mouse.map(|m| m.presses).unwrap_or(0),
            vitals: vitals.unwrap_or(w.last_vitals()),
            hour: chrono::Local::now().hour(),
        };
        w.tick(&input);

        // Catch clicks only where he is.
        let pass = !w.want_mouse;
        if pass != st.passthrough {
            st.passthrough = pass;
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(pass));
        }
        // Clicking him activates eco view; hand focus back once he's let go.
        if w.want_mouse && st.focus.is_none() {
            st.focus = Some(Focus::capture());
        }
        let pressing = input.button_down && w.want_mouse;
        if pressing {
            st.pressed = true;
        }
        if !w.want_mouse && !input.button_down {
            if st.pressed
                && let Some(f) = &st.focus
            {
                f.restore();
            }
            st.focus = None;
            st.pressed = false;
        }
        let icon = if w.is_grabbed() {
            egui::CursorIcon::Grabbing
        } else if w.want_mouse {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Default
        };
        st.cursor_icon = icon;
        ctx.set_cursor_icon(icon);

        // Paint.
        let origin = ctx.input(|i| i.viewport().inner_rect).map(|r| r.min).unwrap_or(stage.min);
        let offset = -origin.to_vec2();
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("raccoon")));
        let ppp = ctx.pixels_per_point();
        if w.enabled {
            let mesh = render::draw_world(&w, offset, ppp);
            painter.add(Shape::mesh(mesh));
            if w.cfg.show_name_tag {
                name_tag(&painter, &w, offset);
            }
            w.fx.paint(&painter, offset, w.pal.outline);
        }

        for s in w.sounds.drain(..) {
            self.sound.play(match s {
                Sfx::Pickup => Cue::Pickup,
                Sfx::Throw => Cue::Whoosh,
                Sfx::Thud => Cue::Thud,
                Sfx::Boop => Cue::Boop,
                Sfx::Purr => Cue::Purr,
                Sfx::Cheer => Cue::Cheer,
                Sfx::Alarm => Cue::Alarm,
            });
        }

        // Sleeping quietly in a corner doesn't need 60fps.
        if w.busy() || w.want_mouse {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

fn builder(stage: Rect) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title(TITLE)
        .with_app_id("com.ecoview.raccoon")
        .with_position(stage.min)
        .with_inner_size(stage.size())
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_always_on_top()
        .with_mouse_passthrough(true)
        .with_taskbar(false)
        .with_active(false)
        .with_visible(true)
}

fn name_tag(painter: &egui::Painter, w: &World, offset: Vec2) {
    let at = w.pos + offset + vec2(0.0, 12.0);
    let font = FontId::proportional(11.0);
    let galley = painter.layout_no_wrap(w.cfg.name.clone(), font, Color32::WHITE);
    let rect = Rect::from_center_size(at, galley.size() + vec2(10.0, 4.0));
    painter.rect_filled(rect, 6.0, Color32::from_black_alpha(140));
    painter.galley(rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
}
