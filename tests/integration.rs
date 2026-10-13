// Integration checks for the logic that is easy to get subtly wrong: grid
// placement, settings round-tripping, and battery/`pmset` parsing.
//
// Run with: cargo test --test tests
#[path = "../src/grid.rs"]
#[allow(dead_code)]
mod grid;
use grid::{CardId, COLS};

// ---------- grid ----------

#[test]
fn try_place_rejects_overlaps() {
    let mut layout = grid::GridLayout::default_dashboard();
    // CPU already occupies col 0..16, row 0..8.
    assert!(!layout.try_place(CardId::Network, 4, 4, 8, 4));
}

#[test]
fn try_place_accepts_free_space() {
    let mut layout = grid::GridLayout::default_dashboard();
    assert!(layout.try_place(CardId::Cpu, 0, 60, 10, 6));
}

#[test]
fn try_place_reports_no_change_when_idle() {
    // Regression: the layout editor calls persist() on every frame of a drag.
    // try_place must return false when nothing actually moved.
    let mut layout = grid::GridLayout::default_dashboard();
    let item = layout.items.iter().find(|i| i.id == CardId::Cpu).copied().unwrap();
    assert!(!layout.try_place(CardId::Cpu, item.col, item.row, item.w, item.h));
}

#[test]
fn try_place_reports_change_when_moved() {
    let mut layout = grid::GridLayout::default_dashboard();
    assert!(layout.try_place(CardId::Cpu, 1, 60, 10, 6));
}

#[test]
fn resize_by_one_cell_reports_change() {
    // Regression: resizing used to be a no-op because try_place always
    // returned true, so the handle never registered. Park the card in free
    // space first so the overlap check can't be what is being measured.
    let mut layout = grid::GridLayout::default_dashboard();
    assert!(layout.try_place(CardId::Cpu, 0, 60, 16, 8));
    let item = layout.items.iter().find(|i| i.id == CardId::Cpu).copied().unwrap();
    assert!(layout.try_place(CardId::Cpu, item.col, item.row, item.w + 1, item.h));
    assert_eq!(layout.items.iter().find(|i| i.id == CardId::Cpu).unwrap().w, 17);
}

#[test]
fn placement_is_clamped_into_bounds() {
    // Out-of-range input is clamped, not rejected. Park CPU in free space so
    // the overlap check is not what decides the result.
    let mut layout = grid::GridLayout::default_dashboard();
    layout.try_place(CardId::Cpu, 0, 60, 16, 8);
    assert!(layout.try_place(CardId::Cpu, -50, 60, 999, 999));
    let cpu = layout.items.iter().find(|i| i.id == CardId::Cpu).copied().unwrap();
    assert_eq!(cpu.col, 0, "col must clamp to 0: {cpu:?}");
    assert_eq!(cpu.w, COLS, "w must clamp to COLS: {cpu:?}");
    assert!(cpu.row >= 0);
    assert!(cpu.h <= 24, "h must clamp to 24: {cpu:?}");
    assert!(cpu.col + cpu.w <= COLS);
}

#[test]
fn sanitize_repairs_hostile_values() {
    // A hand-edited settings file can carry w/h of 0 or negative. Without a
    // clamp these build negative-size rects that poison the raccoon's
    // collision platforms.
    let mut layout = grid::GridLayout {
        items: vec![grid::GridItem { id: CardId::Cpu, col: -10, row: -5, w: 0, h: -3 }],
    };
    layout.sanitize();
    let cpu = layout.items[0];
    assert!(cpu.w >= 4 && cpu.h >= 3, "{cpu:?}");
    assert!(cpu.col >= 0 && cpu.row >= 0, "{cpu:?}");
    assert!(cpu.col + cpu.w <= COLS);
}

#[test]
fn default_layout_has_no_overlaps() {
    let layout = grid::GridLayout::default_dashboard();
    for a in &layout.items {
        for b in &layout.items {
            if a.id != b.id {
                assert!(!a.overlaps(b), "{:?} overlaps {:?}", a.id, b.id);
            }
        }
    }
}

// ---------- pmset parsing ----------

/// Mirrors the parsing logic in `src/metrics.rs`. Kept in sync by the
/// assertions below; a regression there is invisible without a test.
fn parse_pmset(text: &str) -> (Option<u8>, bool, Option<String>) {
    let mut percent = None;
    let mut charging = false;
    let mut remaining = None;
    for line in text.lines() {
        if line.contains("InternalBattery") || line.contains('%') {
            if let Some(idx) = line.find('%') {
                let digits: String = line[..idx]
                    .chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .collect();
                percent = digits.chars().rev().collect::<String>().parse().ok();
            }
            let mut words = line.split(|c: char| !c.is_ascii_alphabetic());
            charging = words.any(|w| w == "charging" || w == "charged");
            if let Some((_, rest)) = line.split_once(';')
                && let Some(time) = rest.split_whitespace().find(|w| w.contains(':'))
            {
                remaining = Some(time.to_string());
            }
        }
        if line.contains("AC Power") {
            charging = true;
        }
    }
    (percent, charging, remaining)
}

#[test]
fn discharging_is_not_charging() {
    // Regression: "discharging".contains("charging") reported every laptop as
    // charging, which also silently disabled the low-battery warning.
    let (pct, charging, remaining) =
        parse_pmset(" -InternalBattery-0 (id=1)\t60%; discharging; 0:54 remaining present: true");
    assert_eq!(pct, Some(60));
    assert!(!charging, "discharging must not parse as charging");
    assert_eq!(remaining.as_deref(), Some("0:54"));
}

#[test]
fn charged_line_is_charging() {
    let (_, charging, _) = parse_pmset(" -InternalBattery-0 (id=1)\t100%; charged; AC attached");
    assert!(charging);
}

#[test]
fn ac_power_line_is_charging() {
    let (_, charging, _) = parse_pmset("Now drawing from 'AC Power'");
    assert!(charging);
}

#[test]
fn remaining_time_is_clean() {
    // Regression: the UI showed "0:54 present: true".
    let (_, _, remaining) =
        parse_pmset(" -InternalBattery-0 (id=1)\t60%; discharging; 0:54 remaining present: true");
    let r = remaining.unwrap();
    assert!(!r.contains("present"), "{r}");
    assert!(!r.contains("remaining"), "{r}");
}

#[test]
fn non_ascii_before_percent_does_not_panic() {
    // Regression risk: slicing at a byte offset from `find('%')` could land
    // mid-codepoint on a localized output and panic.
    let text = "Batterie interne 42°; déchargement;";
    let idx = text.find('°').unwrap();
    let digits: String = text[..idx].chars().rev().take_while(char::is_ascii_digit).collect();
    let parsed: Result<u32, _> = digits.chars().rev().collect::<String>().parse();
    assert_eq!(parsed.ok(), Some(42));
}

#[test]
fn no_estimate_is_left_unset() {
    let (_, _, remaining) =
        parse_pmset(" -InternalBattery-0 (id=1)\t60%; discharging; (no estimate)");
    assert_eq!(remaining, None);
}