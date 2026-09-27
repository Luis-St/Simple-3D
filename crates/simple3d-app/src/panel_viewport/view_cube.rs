//! The orientation cube in the corner.

mod faces;
pub(crate) use faces::*;

use crate::app::{App, Status};
use crate::theme::{self, token};
use simple3d_geom::aabb::box_corner;
use simple3d_geom::Vec3;

/// The orientation cube's id, fixed so it is stable and tests can click it.
pub fn cube_id() -> egui::Id {
    egui::Id::new("view-cube")
}

/// What the frame's pointer told the cube: where it is, which zone is hovered, whether it took the
/// pointer. Split because it must claim the pointer before orbiting and draw after the picture
/// (issue 102); the paint half reads the camera itself.
pub(crate) struct CubeHit {
    box_rect: egui::Rect,
    over_centre: bool,
    hovered_zone: Option<[i32; 3]>,
    /// True when a click on the cube must not also orbit.
    pub taken: bool,
}

/// The cube's pointer half: a face turns the camera to face it, the centre dot returns to
/// isometric.
pub(crate) fn view_cube_interact(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) -> CubeHit {
    let side = theme::metric::VIEW_CUBE;
    let box_rect =
        egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(side + 12.0, side + 12.0), egui::Vec2::splat(side));
    // Dragging spins the cube alone, to reach the sides the camera cannot see (issue 34).
    let response = ui.interact(box_rect, cube_id(), egui::Sense::click_and_drag());
    let centre = box_rect.center();
    let reach = side * 0.30;

    // A hand spin is remembered with the camera it started from and dropped once the camera moves.
    let camera = (app.scene.camera.yaw, app.scene.camera.pitch);
    if app.cube_spin.is_some_and(|spin| spin.camera != camera) {
        app.cube_spin = None;
    }
    if response.dragged() {
        let delta = response.drag_delta();
        let (from_yaw, from_pitch) = app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch));
        app.cube_spin = Some(crate::app::CubeSpin {
            yaw: from_yaw - delta.x as f64 * 0.5,
            pitch: (from_pitch + delta.y as f64 * 0.5).clamp(-90.0, 90.0),
            camera,
        });
    }
    let (yaw, pitch) = cube_angles(app);

    let hover = response.hover_pos();
    let over_centre = hover.is_some_and(|p| (p - centre).length() < side * 0.11);
    // The hovered face, edge or corner among those facing the eye; nothing during a spin drag.
    let hovered_zone = match (over_centre, response.dragged(), hover) {
        (false, false, Some(p)) => crate::view::cube_zone_at(yaw, pitch, p - centre, reach),
        _ => None,
    };

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(if response.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::PointingHand
        });
    }
    let hint = match (over_centre, hovered_zone) {
        (true, _) => Some("Isometric".to_string()),
        (false, Some(zone)) => Some(crate::view::cube_zone_label(zone)),
        _ => None,
    };
    if let Some(hint) = hint {
        response.clone().on_hover_text(format!("{hint}\nDrag the cube to turn it without moving the model"));
    }
    if response.clicked() {
        // After a click the cube follows the camera again: a spin only reaches a view.
        app.cube_spin = None;
        if over_centre {
            app.set_view(crate::view::ViewPreset::Isometric);
        } else if let Some(zone) = hovered_zone {
            let (to_yaw, to_pitch) = crate::view::cube_zone_angles(zone, app.scene.camera.yaw);
            app.turn_camera_to(to_yaw, to_pitch);
            app.status = Status::Info(format!("View: {}", crate::view::cube_zone_label(zone)));
        }
    }
    CubeHit {
        box_rect,
        over_centre,
        hovered_zone,
        taken: response.hovered() || response.clicked() || response.dragged(),
    }
}

/// The cube's angles: the camera's, unless spun by hand.
fn cube_angles(app: &App) -> (f64, f64) {
    let camera = (app.scene.camera.yaw, app.scene.camera.pitch);
    app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch))
}

/// How nearly edge-on a face may be and still be drawn: about a degree.
const EDGE_ON: f64 = 0.02;

/// How far a face must face the eye for its label to fit.
const LABELLED: f64 = 0.2;

/// Draw the cube from the camera as it stands at the end of the frame.
pub(crate) fn view_cube_paint(app: &App, ui: &egui::Ui, hit: &CubeHit) {
    let box_rect = hit.box_rect;
    let side = box_rect.width();
    let painter = ui.painter_at(box_rect.expand(2.0));
    painter.rect_filled(box_rect, 3.0, token::SURFACE_1.gamma_multiply(0.80));
    painter.rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, token::SURFACE_3), egui::StrokeKind::Inside);

    let centre = box_rect.center();
    let reach = side * 0.30;
    let (yaw, pitch) = cube_angles(app);

    let project = |v: Vec3| crate::view::cube_project(yaw, pitch, v, reach);
    let corner = |i: usize| box_corner(Vec3::splat(-1.0), Vec3::splat(1.0), i);
    let at_zone = |zone: [i32; 3]| centre + project(Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64)).0;

    let hovered_face = hit
        .hovered_zone
        .filter(|zone| crate::view::zone_order(*zone) == 1)
        .and_then(|zone| crate::view::CUBE_FACES.iter().position(|(normal, _, _)| *normal == zone));
    let faces: Vec<(usize, egui::Pos2, f64)> = crate::view::CUBE_FACES
        .iter()
        .enumerate()
        .map(|(index, (normal, _, _))| {
            let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
            let (offset, depth) = project(n);
            (index, centre + offset, depth)
        })
        .collect();

    // Far faces first, so near ones draw over them.
    let mut order: Vec<usize> = (0..crate::view::CUBE_FACES.len()).collect();
    order.sort_by(|a, b| faces[*b].2.partial_cmp(&faces[*a].2).unwrap_or(std::cmp::Ordering::Equal));
    for index in order {
        let (normal, _, label) = crate::view::CUBE_FACES[index];
        let (_, at, depth) = faces[index];
        // Edge-on faces are skipped; their labels hung outside and were clipped ("GT" for RGT).
        if depth >= -EDGE_ON {
            continue;
        }
        // The face as a quad: the four cube corners sharing this normal.
        let axis = normal.iter().position(|c| *c != 0).unwrap_or(0);
        let sign = normal[axis] as f64;
        let quad: Vec<egui::Pos2> =
            (0..8).filter(|i| corner(*i).get(axis) * sign > 0.0).map(|i| centre + project(corner(i)).0).collect();
        let quad = sort_ring(quad, at);
        let tint = crate::theme::axis_colour(axis);
        let fill = if hovered_face == Some(index) {
            token::ACCENT.gamma_multiply(0.55)
        } else {
            tint.gamma_multiply(0.16).blend(token::SURFACE_2)
        };
        painter.add(egui::Shape::convex_polygon(
            quad,
            fill,
            egui::Stroke::new(1.0_f32, token::SURFACE_3.gamma_multiply(0.9)),
        ));
        let text_colour = if hovered_face == Some(index) { token::SURFACE_0 } else { token::TEXT_LO };
        // Pushed out from the centre, where the three visible face centres meet the projection dot.
        let mut text_at = centre + (at - centre) * 1.2;
        // A face turned too far away for its label is named by its neighbours instead.
        if depth > -LABELLED {
            continue;
        }
        // Seen square on, the label goes above the centre dot rather than under it ("B.M" for BTM).
        if (text_at - centre).length() < 9.0 {
            text_at = centre - egui::vec2(0.0, 9.0);
        }
        painter.text(text_at, egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(9.0), text_colour);
    }

    // Edges and corners have no quad, so their highlight (a bar or a dot) is drawn over the faces in
    // the selection colour.
    if let Some(zone) = hit.hovered_zone {
        match crate::view::zone_order(zone) {
            2 => {
                let axis = zone.iter().position(|c| *c == 0).unwrap_or(0);
                let mut a = zone;
                let mut b = zone;
                a[axis] = -1;
                b[axis] = 1;
                painter.line_segment([at_zone(a), at_zone(b)], egui::Stroke::new(3.5_f32, token::ACCENT));
            }
            3 => {
                painter.circle_filled(at_zone(zone), 4.5, token::ACCENT);
            }
            _ => {}
        }
    }

    // The centre dot returns to isometric; labels are moved so it never covers one.
    let dot = if hit.over_centre { token::ACCENT } else { token::TEXT_LO };
    painter.circle_filled(centre, side * 0.11 * 0.45, dot);
}
