//! What is under the pointer in the sketch, measured on screen so reach does not change with zoom.

use super::*;
use crate::view::View;
use simple3d_sketch::sample_curve;

/// How close the pointer must be to a point to catch it, in pixels.
pub(crate) const POINT_REACH: f32 = 9.0;
/// How close to a curve, in pixels.
pub(crate) const CURVE_REACH: f32 = 6.0;

/// The pointer's distance from screen segment `a`-`b`.
pub(crate) fn segment_gap(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + d * t).distance(p)
}

impl SketchTool {
    /// Sketch point `p` on screen.
    pub(crate) fn screen(&self, view: &View, p: [f64; 2]) -> Option<egui::Pos2> {
        view.project(self.world(p)).map(|(at, _)| at)
    }

    /// Where the pointer meets the sketch plane, in sketch coordinates.
    pub(crate) fn under(&self, view: &View, cursor: egui::Pos2) -> Option<[f64; 2]> {
        let frame = self.frame?;
        view.ray_plane(cursor, frame.t, frame.axis(2)).map(|p| self.local(p))
    }

    /// The sketch point nearest the pointer within reach, skipping `except`.
    pub(crate) fn point_at(&self, view: &View, cursor: egui::Pos2, except: &[PointId]) -> Option<PointId> {
        self.sketch
            .points
            .iter()
            .filter(|p| !except.contains(&p.id))
            .filter_map(|p| Some((p.id, self.screen(view, p.at)?.distance(cursor))))
            .filter(|(_, gap)| *gap <= POINT_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// The curve nearest the pointer within reach.
    pub(crate) fn curve_at(&self, view: &View, cursor: egui::Pos2) -> Option<CurveId> {
        let mut best: Option<(CurveId, f32)> = None;
        for curve in &self.sketch.curves {
            let samples = sample_curve(&self.sketch, curve, 64);
            let mut screen: Vec<egui::Pos2> = samples.points.iter().filter_map(|&p| self.screen(view, p)).collect();
            if samples.closed && !screen.is_empty() {
                screen.push(screen[0]);
            }
            for pair in screen.windows(2) {
                let gap = segment_gap(cursor, pair[0], pair[1]);
                if gap <= CURVE_REACH && best.is_none_or(|(_, most)| gap < most) {
                    best = Some((curve.id, gap));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    /// What a click would select: a point, else a constraint's label, else a curve.
    pub(crate) fn pick_at(&self, view: &View, cursor: egui::Pos2) -> Option<Pick> {
        if let Some(point) = self.point_at(view, cursor, &[]) {
            return Some(Pick::Point(point));
        }
        if let Some(constraint) = super::glyphs::label_at(self, view, cursor) {
            return Some(Pick::Constraint(constraint));
        }
        self.curve_at(view, cursor).map(Pick::Curve)
    }

    /// Where a drawing click lands: on a sketch point within reach, else on the plane, rounded to the
    /// grid's `spacing` when snapping is asked for.
    pub(crate) fn click_at(&self, view: &View, cursor: egui::Pos2, spacing: Option<f64>) -> Option<Click> {
        if let Some(point) = self.point_at(view, cursor, &[]) {
            return Some(Click { at: self.sketch.at(point), point: Some(point) });
        }
        let mut at = self.under(view, cursor)?;
        if let Some(step) = spacing.filter(|step| *step > 0.0) {
            at = at.map(|v| (v / step).round() * step);
        }
        Some(Click { at, point: None })
    }
}
