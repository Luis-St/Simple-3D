//! The orientation cube in the corner.

use crate::app::{App, Status};
use crate::theme::{self, token};
use crate::view::View;
use simple3d_geom::Vec3;

/// The orientation cube's id. Fixed rather than derived from the viewport's Ui,
/// so it is the same cube whatever else the panel contains -- and so a test can
/// click a face of it.
pub fn cube_id() -> egui::Id {
    egui::Id::new("view-cube")
}

/// The orientation cube in the bottom-right corner.
///
/// It answers which way the model faces, and it is also the fastest way to
/// change that: a face turns the camera to look at it straight on, and the dot
/// at its centre returns to the isometric view the cube is drawn from. Returns
/// true when it took the pointer, so a click on it does not also orbit.
pub(crate) fn view_cube(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, view: &View) -> bool {
    let side = theme::metric::VIEW_CUBE;
    let box_rect =
        egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(side + 12.0, side + 12.0), egui::Vec2::splat(side));
    // Click *and drag*: the drag turns the cube alone, which is the only way to
    // reach the three sides the camera cannot currently see (issue 34).
    let response = ui.interact(box_rect, cube_id(), egui::Sense::click_and_drag());
    let painter = ui.painter_at(box_rect.expand(2.0));
    painter.rect_filled(box_rect, 3.0, token::SURFACE_1.gamma_multiply(0.80));
    painter.rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, token::SURFACE_3), egui::StrokeKind::Inside);

    let centre = box_rect.center();
    let reach = side * 0.30;

    // The cube follows the camera unless it has been turned by hand, and a
    // camera that moves takes the cube back with it: a spin is remembered
    // along with the camera it was started from, and is dropped the moment the
    // scene turns underneath it.
    let camera = (app.scene.camera.yaw, app.scene.camera.pitch);
    if app.cube_spin.is_some_and(|spin| spin.camera != camera) {
        app.cube_spin = None;
    }
    if response.dragged() {
        let delta = response.drag_delta();
        let (from_yaw, from_pitch) = app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch));
        app.cube_spin = Some(crate::app::CubeSpin {
            yaw: from_yaw - delta.x as f64 * 0.5,
            pitch: (from_pitch + delta.y as f64 * 0.5).clamp(-89.9, 89.9),
            camera,
        });
    }
    let (yaw, pitch) = app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch));

    let project = |v: Vec3| crate::view::cube_project(yaw, pitch, v, reach);
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -1.0 } else { 1.0 },
            if i & 4 == 0 { -1.0 } else { 1.0 },
        )
    };
    let at_zone = |zone: [i32; 3]| centre + project(Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64)).0;

    let hover = response.hover_pos();
    let centre_radius = side * 0.11;
    let over_centre = hover.is_some_and(|p| (p - centre).length() < centre_radius);
    // Which part of the cube the pointer is over -- a face, an edge or a corner
    // -- among the ones turned towards the eye, so a click never asks for the
    // side of the cube that cannot be seen. A drag in progress is turning the
    // cube, not choosing a view, so nothing is highlighted during one.
    let hovered_zone = match (over_centre, response.dragged(), hover) {
        (false, false, Some(p)) => crate::view::cube_zone_at(yaw, pitch, p - centre, reach),
        _ => None,
    };
    let hovered_face = hovered_zone
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

    // Far faces first, so a near one draws over them.
    let mut order: Vec<usize> = (0..crate::view::CUBE_FACES.len()).collect();
    order.sort_by(|a, b| faces[*b].2.partial_cmp(&faces[*a].2).unwrap_or(std::cmp::Ordering::Equal));
    for index in order {
        let (normal, _, label) = crate::view::CUBE_FACES[index];
        let (_, at, depth) = faces[index];
        if depth >= 0.0 {
            continue;
        }
        // The face as a quad: the four cube corners that share this normal.
        let axis = normal.iter().position(|c| *c != 0).unwrap_or(0);
        let sign = normal[axis] as f64;
        let quad: Vec<egui::Pos2> = (0..8)
            .filter(|i| crate::gizmo::get_axis(corner(*i), axis) * sign > 0.0)
            .map(|i| centre + project(corner(i)).0)
            .collect();
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
        // Pushed a little away from the cube's centre: in an isometric view the
        // three visible face centres meet at the near corner, and that is where
        // the projection dot lives.
        let text_at = centre + (at - centre) * 1.2;
        painter.text(text_at, egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(9.0), text_colour);
    }

    // An edge or a corner has no quad of its own, so the highlight is drawn
    // over the faces: the edge as a bar along itself, the corner as a dot on
    // it. Both in the selection colour, which is what "this is what a click
    // would take" means everywhere else in the application.
    if let Some(zone) = hovered_zone {
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

    // The centre dot: an isometric view, back to where the cube itself is
    // drawn from. It sits where no face label does, so it never covers one.
    let dot = if over_centre { token::ACCENT } else { token::TEXT_LO };
    painter.circle_filled(centre, centre_radius * 0.45, dot);

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
        // Whatever was chosen, the cube goes back to matching the camera: the
        // spin is a way of *reaching* a view, not a second orientation to keep.
        app.cube_spin = None;
        if over_centre {
            app.set_view(crate::view::ViewPreset::Isometric);
        } else if let Some(zone) = hovered_zone {
            let (to_yaw, to_pitch) = crate::view::cube_zone_angles(zone, app.scene.camera.yaw);
            app.turn_camera_to(to_yaw, to_pitch);
            app.status = Status::Info(format!("View: {}", crate::view::cube_zone_label(zone)));
        }
    }
    let _ = view;
    response.hovered() || response.clicked() || response.dragged()
}

/// Put the corners of a face in ring order around its centre, so the quad drawn
/// from them is the face and not a bow tie.
pub(crate) fn sort_ring(mut points: Vec<egui::Pos2>, centre: egui::Pos2) -> Vec<egui::Pos2> {
    points.sort_by(|a, b| {
        let angle = |p: &egui::Pos2| (p.y - centre.y).atan2(p.x - centre.x);
        angle(a).partial_cmp(&angle(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    points
}

pub(crate) fn draw_box(painter: &egui::Painter, view: &View, lo: Vec3, hi: Vec3, colour: egui::Color32, width: f32) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in EDGES {
        let (Some((pa, _)), Some((pb, _))) = (view.project(corner(a)), view.project(corner(b))) else {
            continue;
        };
        painter.line_segment([pa, pb], egui::Stroke::new(width, colour));
    }
}

/// The bounding box's dimensions, numerically -- the fastest way to answer "will
/// this fit" (spec section 6.1).
pub(crate) fn label_box(
    painter: &egui::Painter,
    ui: &egui::Ui,
    view: &View,
    lo: Vec3,
    hi: Vec3,
    unit: simple3d_core::unit::Unit,
    colour: egui::Color32,
) {
    let size = hi - lo;
    let centre = (lo + hi) * 0.5;
    let labels = [
        (Vec3::new(centre.x, lo.y, lo.z), simple3d_core::unit::format_length(size.x, unit)),
        (Vec3::new(hi.x, centre.y, lo.z), simple3d_core::unit::format_length(size.y, unit)),
        (Vec3::new(hi.x, lo.y, centre.z), simple3d_core::unit::format_length(size.z, unit)),
    ];
    for (world, text) in labels {
        let Some((screen, _)) = view.project(world) else { continue };
        let galley = painter.layout_no_wrap(format!("{text}{}", unit.suffix()), egui::FontId::monospace(11.0), colour);
        let at = screen + egui::vec2(6.0, -6.0);
        painter.rect_filled(
            egui::Rect::from_min_size(at, galley.size()).expand(2.0),
            2.0,
            ui.visuals().extreme_bg_color.gamma_multiply(0.75),
        );
        painter.galley(at, galley, colour);
    }
}
