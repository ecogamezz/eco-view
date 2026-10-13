// Regression test for "can't click anything but the raccoon chip".
//
// A full-screen `egui::Area` on `Order::Foreground` registers a widget rect that
// contains every other widget's rect. egui's hit test walks layers top-down and
// stops at the first hit covering the search radius, so that area becomes the
// only candidate layer and every click aimed at the nav, cards or settings is
// discarded — regardless of the area's `Sense` or `interactable(false)`.
//
// This drives a real `egui::Context` through the same Area/LayerId pattern the
// app used and asserts that clicks still reach the widgets underneath.
//
// Note: pointer events must be supplied on `RawInput::events`; anything pushed
// via `input_mut` is overwritten by `begin_pass` at the start of the next frame.

use eframe::egui::{
    self, Context, Event, Id, Order, PointerButton, Pos2, RawInput, Rect, Vec2, pos2,
};

const W: f32 = 800.0;
const H: f32 = 600.0;

/// Where the floating chip sits, matching the app's layout.
const CHIP_POS: Pos2 = pos2(600.0, 500.0);
/// A point on the nav button at the top-left of the central panel.
const NAV_POS: Pos2 = pos2(20.0, 20.0);

fn screen_rect() -> Rect {
    Rect { min: Pos2::ZERO, max: pos2(W, H) }
}

fn button_event(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Runs one frame, feeding `events` as the raw input for this pass.
fn frame(ctx: &Context, use_fullscreen_area: bool, events: Vec<Event>) -> (bool, bool) {
    let mut nav_clicked = false;
    let mut chip_clicked = false;

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect()),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| {
            // Background layer: a nav button the user must be able to press.
            egui::CentralPanel::default().show(ctx, |ui| {
                if ui.button("Nav").clicked() {
                    nav_clicked = true;
                }
            });

            // The floating toggle chip, as in the app.
            egui::Area::new(Id::new("eco-companion-chip"))
                .order(Order::Foreground)
                .movable(true)
                .fixed_pos(CHIP_POS)
                .show(ctx, |ui| {
                    let (resp, _) = ui.allocate_painter(Vec2::splat(52.0), egui::Sense::click());
                    if resp.clicked() {
                        chip_clicked = true;
                    }
                });

            // The regression: a full-screen area on the foreground layer.
            if use_fullscreen_area {
                egui::Area::new(Id::new("raccoon-stage"))
                    .order(Order::Foreground)
                    .fixed_pos(Pos2::ZERO)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.set_min_size(ctx.screen_rect().size());
                    });
            }
        },
    );

    (nav_clicked, chip_clicked)
}

/// Warms up widget rects, then performs a full press+release click at `pos`.
/// The click is reported on the release frame.
fn click_at(ctx: &Context, use_fullscreen_area: bool, pos: Pos2) -> (bool, bool) {
    // Widget rects must exist from a previous frame before a click registers,
    // and the pointer must be known before the press is hit-tested.
    frame(ctx, use_fullscreen_area, vec![Event::PointerMoved(pos)]);
    frame(ctx, use_fullscreen_area, vec![]);
    frame(ctx, use_fullscreen_area, vec![button_event(pos, true)]);
    frame(ctx, use_fullscreen_area, vec![button_event(pos, false)])
}

#[test]
fn fullscreen_foreground_area_swallows_clicks() {
    // Establishes the bug: this is why only the chip was clickable.
    let ctx = Context::default();
    let (nav, _chip) = click_at(&ctx, true, NAV_POS);
    assert!(
        !nav,
        "full-screen foreground Area is EXPECTED to block the nav click (this is the bug)"
    );
}

#[test]
fn no_fullscreen_area_clicks_reach_widgets() {
    // The fix: painting the raccoon to a layer without an Area leaves the
    // dashboard fully interactive.
    let ctx = Context::default();
    let (nav, _chip) = click_at(&ctx, false, NAV_POS);
    assert!(nav, "nav button must be clickable once the full-screen Area is gone");
}

#[test]
fn chip_click_still_works() {
    // The floating chip must remain clickable after the fix.
    let ctx = Context::default();
    let (_nav, chip) = click_at(&ctx, false, pos2(CHIP_POS.x + 26.0, CHIP_POS.y + 26.0));
    assert!(chip, "the floating chip must remain clickable");
}

#[test]
fn stage_area_blocks_every_widget_including_the_chip() {
    // With the stage area present the full-screen layer wins the hit test for
    // the whole window, so even the chip stops responding. (In the real app
    // the chip kept working only because a press promotes its layer above the
    // stage; the dashboard itself never did recover.)
    let ctx = Context::default();
    let (nav, chip) = click_at(&ctx, true, pos2(CHIP_POS.x + 26.0, CHIP_POS.y + 26.0));
    assert!(!nav && !chip, "the stage area is expected to block both");
}

#[test]
fn repeated_clicks_on_the_same_button_all_land() {
    // Sanity: the button responds more than once, not just on the first frame.
    let ctx = Context::default();
    let (a, _) = click_at(&ctx, false, NAV_POS);
    let (b, _) = click_at(&ctx, false, NAV_POS);
    assert!(a && b, "two consecutive clicks must both register");
}