//! Time-series charts: smoothed lines over gradient fills, a light grid,
//! axis labels and a hover readout.

use std::collections::VecDeque;

use eframe::egui::{self, Align2, Color32, Id, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::theme::{self, Palette};

pub struct Series<'a> {
    pub name: &'a str,
    pub values: &'a VecDeque<f32>,
    pub color: Color32,
    pub fill: bool,
}

pub struct Chart<'a> {
    pub id: Id,
    pub height: f32,
    /// Fixed top of the scale; `None` scales to the data.
    pub max: Option<f32>,
    /// Number of most recent samples to show.
    pub window: usize,
    /// Seconds between samples, for the time axis and tooltips.
    pub step_secs: f32,
    pub fmt: &'a dyn Fn(f32) -> String,
    pub axis: bool,
    pub hover: bool,
    pub grid: bool,
}

impl<'a> Chart<'a> {
    pub fn new(id: impl std::hash::Hash, height: f32, fmt: &'a dyn Fn(f32) -> String) -> Self {
        Self { id: Id::new(id), height, max: None, window: 60, step_secs: 1.0, fmt, axis: true, hover: true, grid: true }
    }

    pub fn max(mut self, m: f32) -> Self {
        self.max = Some(m);
        self
    }

    pub fn window(mut self, n: usize, step_secs: f32) -> Self {
        self.window = n.max(2);
        self.step_secs = step_secs.max(0.01);
        self
    }

    /// A bare sparkline: no axis, grid or hover.
    pub fn spark(mut self) -> Self {
        self.axis = false;
        self.hover = false;
        self.grid = false;
        self
    }

    pub fn show(self, ui: &mut Ui, pal: &Palette, series: &[Series]) -> Response {
        let (outer, resp) = ui.allocate_exact_size(vec2(ui.available_width(), self.height), Sense::hover());
        let p = ui.painter_at(outer.expand(4.0));
        let label_w = if self.axis { 44.0 } else { 0.0 };
        let axis_h = if self.axis { 16.0 } else { 0.0 };
        let plot = Rect::from_min_max(outer.min, pos2(outer.right() - label_w, outer.bottom() - axis_h));
        if plot.width() < 10.0 || plot.height() < 6.0 {
            return resp;
        }

        // Scale.
        let n = self.window;
        let data_max = series
            .iter()
            .flat_map(|s| s.values.iter().rev().take(n))
            .copied()
            .fold(0.0_f32, f32::max);
        let top = match self.max {
            Some(m) => m,
            None => nice_ceiling(data_max * 1.15),
        };
        let top = theme_tween(ui, self.id.with("top"), top.max(1e-6));

        // Grid.
        if self.grid {
            for i in 0..=3 {
                let y = plot.bottom() - plot.height() * i as f32 / 3.0;
                dashed(&p, plot.left(), plot.right(), y, pal.border);
                if self.axis {
                    let v = top * i as f32 / 3.0;
                    p.text(pos2(outer.right(), y), Align2::RIGHT_CENTER, (self.fmt)(v), theme::regular(10.0), pal.muted);
                }
            }
        }
        if self.axis {
            let span = self.step_secs * (n - 1) as f32;
            for (frac, label) in [(0.0, ago(span)), (0.5, ago(span * 0.5)), (1.0, "now".to_string())] {
                let x = plot.left() + plot.width() * frac;
                let align = if frac == 0.0 { Align2::LEFT_TOP } else if frac == 1.0 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
                p.text(pos2(x, plot.bottom() + 4.0), align, label, theme::regular(10.0), pal.muted);
            }
        }

        // Series, back to front.
        let mut tips: Vec<(String, f32, Color32, Pos2)> = Vec::new();
        let hover_x = resp.hover_pos().filter(|h| self.hover && plot.contains(*h)).map(|h| h.x);
        for s in series {
            let len = s.values.len().min(n);
            if len < 2 {
                continue;
            }
            let start = s.values.len() - len;
            let x_of = |i: usize| plot.right() - plot.width() * ((len - 1 - i) as f32 / (n - 1) as f32);
            let y_of = |v: f32| plot.bottom() - (v / top).clamp(0.0, 1.0) * plot.height();
            let raw: Vec<Pos2> = (0..len).map(|i| pos2(x_of(i), y_of(s.values[start + i]))).collect();
            let pts = smooth(&downsample(&raw, (plot.width() / 2.0) as usize));

            if s.fill {
                let mut mesh = Mesh::default();
                let top_c = pal.tint(s.color, if pal.dark { 0.30 } else { 0.22 });
                let bottom_c = pal.tint(s.color, 0.0);
                for w in pts.windows(2) {
                    let base = mesh.vertices.len() as u32;
                    mesh.colored_vertex(w[0], top_c);
                    mesh.colored_vertex(w[1], top_c);
                    mesh.colored_vertex(pos2(w[1].x, plot.bottom()), bottom_c);
                    mesh.colored_vertex(pos2(w[0].x, plot.bottom()), bottom_c);
                    mesh.add_triangle(base, base + 1, base + 2);
                    mesh.add_triangle(base, base + 2, base + 3);
                }
                p.add(Shape::mesh(mesh));
            }
            p.add(Shape::line(pts.clone(), Stroke::new(1.8_f32, s.color)));
            let last = *raw.last().unwrap_or(&plot.right_bottom());
            p.circle_filled(last, 5.5, pal.tint(s.color, 0.22));
            p.circle_filled(last, 2.8, s.color);

            if let Some(hx) = hover_x {
                let i = (0..len)
                    .min_by(|a, b| (x_of(*a) - hx).abs().total_cmp(&(x_of(*b) - hx).abs()))
                    .unwrap_or(len - 1);
                let v = s.values[start + i];
                let secs = (len - 1 - i) as f32 * self.step_secs;
                tips.push((s.name.to_string(), v, s.color, pos2(x_of(i), y_of(v))));
                if tips.len() == 1 {
                    tips[0].0 = format!("{}|{}", s.name, ago(secs));
                }
            }
        }

        if !tips.is_empty() {
            let x = tips[0].3.x;
            p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0_f32, pal.border_hi));
            for (_, _, c, at) in &tips {
                p.circle_filled(*at, 4.0, pal.surface);
                p.circle_stroke(*at, 4.0, Stroke::new(2.0_f32, *c));
            }
            // Tooltip box.
            let when = tips[0].0.split('|').nth(1).unwrap_or("").to_string();
            let mut lines: Vec<(String, Color32)> = vec![(when, pal.muted)];
            for (name, v, c, _) in &tips {
                let name = name.split('|').next().unwrap_or("");
                lines.push((format!("{name}  {}", (self.fmt)(*v)), *c));
            }
            let galleys: Vec<_> = lines
                .iter()
                .enumerate()
                .map(|(i, (t, c))| {
                    let f = if i == 0 { theme::regular(10.5) } else { theme::medium(12.0) };
                    (p.layout_no_wrap(t.clone(), f, if i == 0 { *c } else { pal.text }), *c)
                })
                .collect();
            let w = galleys.iter().map(|(g, _)| g.size().x).fold(0.0, f32::max) + 26.0;
            let h = galleys.iter().map(|(g, _)| g.size().y + 2.0).sum::<f32>() + 12.0;
            let mut bx = x + 12.0;
            if bx + w > plot.right() {
                bx = x - 12.0 - w;
            }
            let rect = Rect::from_min_size(pos2(bx, plot.top() + 4.0), vec2(w, h));
            p.add(egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: pal.shadow }.as_shape(rect, 8));
            p.rect_filled(rect, 8.0, pal.surface_hi);
            p.rect_stroke(rect, 8.0, Stroke::new(1.0_f32, pal.border_hi), StrokeKind::Inside);
            let mut y = rect.top() + 6.0;
            for (i, (g, c)) in galleys.into_iter().enumerate() {
                if i > 0 {
                    p.circle_filled(pos2(rect.left() + 12.0, y + g.size().y * 0.5), 3.0, c);
                }
                let gx = if i == 0 { rect.left() + 10.0 } else { rect.left() + 20.0 };
                let gh = g.size().y;
                p.galley(pos2(gx, y), g, pal.text);
                y += gh + 2.0;
            }
        }
        resp
    }
}

fn theme_tween(ui: &Ui, id: Id, v: f32) -> f32 {
    crate::widgets::tween(ui.ctx(), id, v, 0.4)
}

fn dashed(p: &egui::Painter, x0: f32, x1: f32, y: f32, c: Color32) {
    let mut x = x0;
    while x < x1 {
        let e = (x + 4.0).min(x1);
        p.line_segment([pos2(x, y), pos2(e, y)], Stroke::new(1.0_f32, c));
        x += 8.0;
    }
}

/// Rounds up to 1, 2, 2.5 or 5 times a power of ten.
pub fn nice_ceiling(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return 1.0;
    }
    let mag = 10f32.powf(v.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if v <= m * mag {
            return m * mag;
        }
    }
    10.0 * mag
}

/// Keeps at most `max_pts` points, preferring each bucket's peak so spikes
/// survive.
fn downsample(pts: &[Pos2], max_pts: usize) -> Vec<Pos2> {
    let max_pts = max_pts.max(8);
    if pts.len() <= max_pts {
        return pts.to_vec();
    }
    let bucket = pts.len() as f32 / max_pts as f32;
    let mut out = Vec::with_capacity(max_pts + 1);
    let mut i = 0.0;
    while (i as usize) < pts.len() {
        let a = i as usize;
        let b = ((i + bucket) as usize).min(pts.len());
        let best = pts[a..b.max(a + 1)].iter().min_by(|p, q| p.y.total_cmp(&q.y)).copied().unwrap_or(pts[a]);
        out.push(pos2(pts[a].x, best.y));
        i += bucket;
    }
    if let Some(l) = pts.last() {
        out.push(*l);
    }
    out
}

/// Catmull-Rom smoothing, clamped so the curve never overshoots its neighbours.
fn smooth(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() < 3 || pts.len() > 400 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() * 4);
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        let (lo, hi) = (p1.y.min(p2.y), p1.y.max(p2.y));
        for k in 0..4 {
            let t = k as f32 / 4.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y).clamp(lo, hi)));
        }
    }
    out.push(*pts.last().unwrap());
    out
}

/// "45s ago", "5m ago".
pub fn ago(secs: f32) -> String {
    if secs < 1.0 {
        "now".into()
    } else if secs < 90.0 {
        format!("{:.0}s ago", secs)
    } else {
        format!("{:.0}m ago", secs / 60.0)
    }
}

/// Average and peak of the last `n` samples.
pub fn stats(values: &VecDeque<f32>, n: usize) -> (f32, f32) {
    let take: Vec<f32> = values.iter().rev().take(n).copied().collect();
    if take.is_empty() {
        return (0.0, 0.0);
    }
    let avg = take.iter().sum::<f32>() / take.len() as f32;
    let peak = take.iter().copied().fold(0.0, f32::max);
    (avg, peak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ceiling_rounds_up_to_friendly_numbers() {
        assert_eq!(nice_ceiling(0.0), 1.0);
        assert_eq!(nice_ceiling(7.0), 10.0);
        assert_eq!(nice_ceiling(12.0), 20.0);
        assert_eq!(nice_ceiling(230.0), 250.0);
        assert_eq!(nice_ceiling(4100.0), 5000.0);
        assert_eq!(nice_ceiling(f32::NAN), 1.0);
    }

    #[test]
    fn downsampling_keeps_peaks_and_ends() {
        let pts: Vec<Pos2> = (0..1000).map(|i| pos2(i as f32, if i == 500 { 0.0 } else { 100.0 })).collect();
        let out = downsample(&pts, 50);
        assert!(out.len() <= 52);
        assert!(out.iter().any(|p| p.y == 0.0), "the spike must survive");
        assert_eq!(out.last(), pts.last());
    }

    #[test]
    fn smoothing_never_overshoots() {
        let pts = vec![pos2(0.0, 10.0), pos2(1.0, 50.0), pos2(2.0, 10.0), pos2(3.0, 50.0)];
        for p in smooth(&pts) {
            assert!((10.0..=50.0).contains(&p.y), "overshoot to {}", p.y);
        }
    }

    #[test]
    fn stats_cover_the_window() {
        let v: VecDeque<f32> = [10.0, 20.0, 30.0, 40.0].into_iter().collect();
        assert_eq!(stats(&v, 2), (35.0, 40.0));
        assert_eq!(stats(&VecDeque::new(), 5), (0.0, 0.0));
    }

    #[test]
    fn ago_reads_naturally() {
        assert_eq!(ago(0.2), "now");
        assert_eq!(ago(45.0), "45s ago");
        assert_eq!(ago(300.0), "5m ago");
    }
}
