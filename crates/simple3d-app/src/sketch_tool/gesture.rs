//! The gesture in progress: the curves it would add, from its clicks to the pointer, dashed, and
//! its size beside the pointer as it is drawn.

use super::overlay::DRAWN_SEGMENTS;
use super::shapes::{angle_of, arc_through, centred_rectangle, circle_through, gap, polygon};
use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_core::unit::{format_angle, format_length};
use simple3d_sketch::{sample_curve, Shape};

pub(crate) fn draw(app: &App, painter: &egui::Painter, view: &View, hover: Option<egui::Pos2>) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if !tool.clicks.is_empty() && tool.draw.selection_clicks() > 0 {
        return selection_preview(app, tool, painter, view, hover);
    }
    if tool.clicks.is_empty() || !tool.draw.draws() {
        return;
    }
    let spacing = super::grid_step(app, view);
    let Some(cursor) = hover else { return };
    let Some((next, _)) = tool.drawing_click(view, cursor, spacing) else { return };
    let stroke = egui::Stroke::new(1.5_f32, token::ACCENT.gamma_multiply(0.8));
    let dashed = |sketch: &Sketch, curve: &simple3d_sketch::Curve| {
        let line: Vec<egui::Pos2> =
            sample_curve(sketch, curve, DRAWN_SEGMENTS).points.iter().filter_map(|&p| tool.screen(view, p)).collect();
        painter.extend(egui::Shape::dashed_line(&line, stroke, 6.0, 3.0));
    };
    if let Some((sketch, newest)) = preview_more(tool, next) {
        for curve in sketch.curves.iter().filter(|c| c.id > newest) {
            dashed(&sketch, curve);
        }
        let from = tool.clicks[tool.clicks.len() - 1].at;
        let readout = format_length(gap(from, next.at), tool.unit);
        super::readout::tag(painter, cursor + egui::vec2(16.0, 18.0), &readout, token::ACCENT);
        return;
    }
    let mut sketch = Sketch::default();
    let ats: Vec<[f64; 2]> = tool.clicks.iter().chain(std::iter::once(&next)).map(|c| c.at).collect();
    let ids: Vec<PointId> = ats.iter().map(|&at| sketch.add_point(at)).collect();
    let loop_of = |sketch: &mut Sketch, corners: Vec<[f64; 2]>| -> Vec<Shape> {
        let ids: Vec<PointId> = corners.into_iter().map(|at| sketch.add_point(at)).collect();
        (0..ids.len()).map(|i| Shape::Line { a: ids[i], b: ids[(i + 1) % ids.len()] }).collect()
    };
    let (first, last) = (ats[0], ats[ats.len() - 1]);
    let unit = tool.unit;
    let length = |v: f64| format_length(v, unit);
    let mut readout = String::new();
    let shapes: Vec<Shape> = match tool.draw {
        Draw::Line => {
            let from = ats[ats.len() - 2];
            let turn = angle_of(from, last).to_degrees().rem_euclid(360.0);
            readout = format!("{}  {}\u{00B0}", length(gap(from, last)), format_angle(turn));
            vec![Shape::Line { a: ids[ids.len() - 2], b: ids[ids.len() - 1] }]
        }
        Draw::Rectangle => {
            readout = format!("{} x {}", length((last[0] - first[0]).abs()), length((last[1] - first[1]).abs()));
            loop_of(&mut sketch, vec![first, [last[0], first[1]], last, [first[0], last[1]]])
        }
        Draw::CenterRectangle => {
            readout =
                format!("{} x {}", length(2.0 * (last[0] - first[0]).abs()), length(2.0 * (last[1] - first[1]).abs()));
            loop_of(&mut sketch, centred_rectangle(first, last).to_vec())
        }
        Draw::Polygon => {
            readout = format!("R {}", length(gap(first, last)));
            loop_of(&mut sketch, polygon(first, last, tool.settings.sides()))
        }
        Draw::Circle => {
            readout = format!("R {}", length(gap(first, last)));
            vec![Shape::Circle { center: ids[0], radius: gap(first, last) }]
        }
        Draw::ThreePointCircle if ids.len() == 3 => match circle_through(ats[0], ats[1], ats[2]) {
            Some((centre, radius)) => {
                readout = format!("R {}", length(radius));
                vec![Shape::Circle { center: sketch.add_point(centre), radius }]
            }
            None => Vec::new(),
        },
        Draw::ThreePointArc if ids.len() == 3 => match arc_through(ats[0], ats[1], ats[2]) {
            Some((centre, start, end)) => {
                readout = format!("R {}", length(gap(centre, start)));
                let (center, start, end) = (sketch.add_point(centre), sketch.add_point(start), sketch.add_point(end));
                vec![Shape::Arc { center, start, end }]
            }
            None => vec![Shape::Line { a: ids[0], b: ids[1] }],
        },
        Draw::Arc if ids.len() == 3 => {
            readout = format!("R {}", length(gap(ats[0], ats[1])));
            vec![Shape::Arc { center: ids[0], start: ids[1], end: ids[2] }]
        }
        Draw::ThreePointCircle | Draw::ThreePointArc | Draw::Arc => {
            readout = length(gap(first, last));
            vec![Shape::Line { a: ids[0], b: ids[1] }]
        }
        Draw::Spline => vec![Shape::Spline { points: ids.clone(), closed: false }],
        Draw::Bspline => vec![Shape::Bspline { points: ids.clone(), closed: false }],
        _ => Vec::new(),
    };
    for shape in shapes {
        dashed(&sketch, &simple3d_sketch::Curve { id: 0, shape, construction: false, reference: false });
    }
    if !readout.is_empty() {
        super::readout::tag(painter, cursor + egui::vec2(16.0, 18.0), &readout, token::ACCENT);
    }
}

/// The newer variants' preview: what the clicks so far and `next` build, into a copy of the sketch,
/// with the newest curve id before it so only what they add is drawn; lines between the clicks until
/// there are enough of them. `None` for the tools previewed above.
fn preview_more(tool: &SketchTool, next: Click) -> Option<(Sketch, CurveId)> {
    let needed = super::drawing_more::clicks_needed(tool.draw);
    if needed.is_none() && tool.draw != Draw::TangentArc {
        return None;
    }
    let mut sketch = tool.sketch.clone();
    let newest = sketch.curves.iter().map(|c| c.id).max().unwrap_or(0);
    let clicks: Vec<Click> = tool.clicks.iter().copied().chain(std::iter::once(next)).collect();
    let built = match needed {
        None => tool.tangent_preview(&mut sketch, next),
        Some(needed) if clicks.len() >= needed => {
            super::drawing_more::build(tool.draw, &mut sketch, &clicks, &tool.settings).is_some()
        }
        Some(_) => false,
    };
    if !built {
        sketch = tool.sketch.clone();
        let ids: Vec<PointId> = clicks.iter().map(|c| sketch.add_point(c.at)).collect();
        for pair in ids.windows(2) {
            sketch.add_curve(Shape::Line { a: pair[0], b: pair[1] }, false);
        }
    }
    Some((sketch, newest))
}

/// The selection tools' preview: the copies or the moved curves the next click would make, dashed.
fn selection_preview(app: &App, tool: &SketchTool, painter: &egui::Painter, view: &View, hover: Option<egui::Pos2>) {
    let Some(cursor) = hover else { return };
    let Some(next) = tool.click_at(view, cursor, super::grid_step(app, view)) else { return };
    let at: Vec<[f64; 2]> = tool.clicks.iter().chain(std::iter::once(&next)).map(|c| c.at).collect();
    let stroke = egui::Stroke::new(1.5_f32, token::ACCENT.gamma_multiply(0.8));
    if at.len() < tool.draw.selection_clicks() {
        let line: Vec<egui::Pos2> = at.iter().filter_map(|&p| tool.screen(view, p)).collect();
        painter.extend(egui::Shape::dashed_line(&line, stroke, 6.0, 3.0));
        return;
    }
    let (curves, points) = super::modify_copy::selected(tool);
    let mut sketch = tool.sketch.clone();
    let newest = sketch.curves.iter().map(|c| c.id).max().unwrap_or(0);
    if !super::picks_more::apply(&mut sketch, tool.draw, (&curves, &points), &at, &tool.settings) {
        return;
    }
    let moved = matches!(tool.draw, Draw::Move | Draw::Rotate | Draw::Scale);
    for curve in sketch.curves.iter().filter(|c| c.id > newest || (moved && curves.contains(&c.id))) {
        let line: Vec<egui::Pos2> =
            sample_curve(&sketch, curve, DRAWN_SEGMENTS).points.iter().filter_map(|&p| tool.screen(view, p)).collect();
        painter.extend(egui::Shape::dashed_line(&line, stroke, 6.0, 3.0));
    }
}
