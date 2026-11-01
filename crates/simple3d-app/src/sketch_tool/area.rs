//! Selecting by area: a left drag from empty space in the pointer tool draws a rectangle and selects
//! the curves in it, and the points that belong to no curve. As in other CAD programs, a drag to the
//! right takes only what lies wholly inside; a drag to the left also what it touches.

use super::*;
use crate::theme::token;
use crate::view::View;
use simple3d_sketch::sample_curve;

/// Samples per curve when testing it against the area.
const SAMPLES: u32 = 64;

/// The rectangle being drawn, from where the press began to the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Area {
    pub start: egui::Pos2,
    pub end: egui::Pos2,
}

impl Area {
    pub(crate) fn rect(&self) -> egui::Rect {
        egui::Rect::from_two_pos(self.start, self.end)
    }

    /// Whether it also takes what it only touches: drawn leftwards.
    pub(crate) fn crossing(&self) -> bool {
        self.end.x < self.start.x
    }
}

impl SketchTool {
    /// Run an area selection for this frame: start it on a drag from `origin` over empty space, follow
    /// the pointer, and select on release; Shift adds to what is selected. `true` while it owns the drag.
    pub(crate) fn area_select(
        &mut self,
        response: &egui::Response,
        view: &View,
        cursor: Option<egui::Pos2>,
        (origin, shift): (Option<egui::Pos2>, bool),
    ) -> bool {
        let empty = !matches!(self.hover, Some(Pick::Point(_) | Pick::Curve(_)));
        if self.area.is_none() && empty && response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(start) = origin {
                self.area = Some(Area { start, end: cursor.unwrap_or(start) });
                self.inspect = None;
            }
        }
        let Some(mut area) = self.area else { return false };
        if let Some(cursor) = cursor {
            area.end = cursor;
            self.area = Some(area);
        }
        if !response.dragged() {
            self.area = None;
            if !shift {
                self.selection.clear();
            }
            for pick in self.in_area(view, area) {
                if !self.selection.contains(&pick) {
                    self.selection.push(pick);
                }
            }
        }
        true
    }

    /// What `area` takes: its curves, and the points of no curve.
    pub(crate) fn in_area(&self, view: &View, area: Area) -> Vec<Pick> {
        let rect = area.rect();
        let mut picks = Vec::new();
        for curve in &self.sketch.curves {
            let samples = sample_curve(&self.sketch, curve, SAMPLES);
            let mut screen: Vec<egui::Pos2> = samples.points.iter().filter_map(|&p| self.screen(view, p)).collect();
            if screen.is_empty() {
                continue;
            }
            if samples.closed {
                screen.push(screen[0]);
            }
            let taken = match area.crossing() {
                true => {
                    screen.windows(2).any(|pair| crosses(rect, pair[0], pair[1]))
                        || screen.iter().any(|p| rect.contains(*p))
                }
                false => screen.iter().all(|p| rect.contains(*p)),
            };
            if taken {
                picks.push(Pick::Curve(curve.id));
            }
        }
        let used: Vec<PointId> = self.sketch.curves.iter().flat_map(|c| c.shape.points()).collect();
        for point in self.sketch.points.iter().filter(|p| !used.contains(&p.id)) {
            if self.screen(view, point.at).is_some_and(|at| rect.contains(at)) {
                picks.push(Pick::Point(point.id));
            }
        }
        picks
    }
}

/// Whether segment `a`-`b` passes through `rect`, by clipping it to the rectangle.
fn crosses(rect: egui::Rect, a: egui::Pos2, b: egui::Pos2) -> bool {
    let d = b - a;
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for (p, q) in
        [(-d.x, a.x - rect.left()), (d.x, rect.right() - a.x), (-d.y, a.y - rect.top()), (d.y, rect.bottom() - a.y)]
    {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return false;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            low = low.max(t);
        } else {
            high = high.min(t);
        }
        if low > high {
            return false;
        }
    }
    true
}

/// The area being drawn: solid for a window, dashed for a crossing selection.
pub(crate) fn draw(tool: &SketchTool, painter: &egui::Painter) {
    let Some(area) = tool.area else { return };
    let rect = area.rect();
    painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(0.08));
    let stroke = egui::Stroke::new(1.0_f32, token::ACCENT);
    if area.crossing() {
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
        painter.extend(egui::Shape::dashed_line(&corners, stroke, 5.0, 3.0));
    } else {
        painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
    }
}
