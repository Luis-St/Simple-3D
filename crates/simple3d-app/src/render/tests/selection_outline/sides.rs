//! Every side of a selected body is outlined, wherever it sits in frame.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;
use simple3d_core::scene::Camera;
use simple3d_geom::primitives;

#[test]
pub(crate) fn a_box_away_from_the_centre_of_the_frame_keeps_all_its_faces() {
    // Regression: culling against `eye - centroid` made facing depend on frame position and dropped a
    // near-edge-on wall. A convex box must cover its projected corners' hull; a box 100 mm off target at
    // 120 mm distance is nearly 40 degrees between the two answers.
    let mesh = primitives::box_mesh(40.0, 20.0, 4.0).translated(Vec3::new(100.0, 0.0, 0.0));
    let prepared = Renderable::prepare(&mesh);
    let camera = Camera { yaw: 2.0, pitch: -15.0, distance: 120.0, ..Camera::default() };
    let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));

    let mut empty = request(Vec::new(), DisplayMode::Shaded);
    empty.view = view;
    let empty = render(&empty);

    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.view = view;
    let frame = render(&req);

    assert!(count_non_background(&frame, &req.palette) > 500, "the box did not draw at all");
    let corners: Vec<egui::Pos2> =
        prepared.mesh.positions.iter().filter_map(|&p| view.project(p)).map(|(s, _)| s).collect();
    let missing = unpainted_inside(&hull(corners), &frame, &empty, 1.0);
    assert_eq!(missing, 0, "{missing} pixels inside the box's own silhouette were never drawn");
}

/// Two boxes side by side, one selected, seen straight on. Regression: edge-on side faces' dot
/// product is noise, which once outlined the back edge instead of the front and left the seam side
/// unoutlined.
#[test]
pub(crate) fn a_selection_seen_straight_on_is_outlined_on_every_side() {
    let (left, right) = (shifted_box(-10.0), shifted_box(10.0));
    let mut both = left.clone();
    both.append(&right);
    let (scene, selected) = (Renderable::prepare(&both), Renderable::prepare_outlined(&left));
    let frame = straight_on(vec![
        Item { renderable: &scene, style: Style::Solid },
        Item { renderable: &selected, style: Style::Selected },
    ]);
    let accent = Palette::dark().selected;
    // The outline runs from the box's left edge to the seam, both vertical lines present between top
    // and bottom.
    let (left_edge, seam) = accent_span(&frame, accent);
    let (top, bottom) = accent_rows(&frame, accent);
    let (from, to) = (top + 4, bottom - 4);
    assert!(
        column_has_between(&frame, left_edge, accent, from, to),
        "the outer edge of the selected box was not outlined"
    );
    assert!(
        column_has_between(&frame, seam, accent, from, to),
        "the edge against the neighbouring box was not outlined"
    );
    assert!(seam - left_edge > 30, "the outline spans {} pixels, which is not a whole box", seam - left_edge);
    // And it is the near edge at the seam that is drawn, not the far one.
    assert!(seam.abs_diff(frame.width / 2) <= 2, "the outline's right edge is at {seam}, not at the seam");
}

#[test]
pub(crate) fn the_selection_is_outlined_in_the_selection_colour() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let req = request(
        vec![
            Item { renderable: &prepared, style: Style::Solid },
            Item { renderable: &prepared, style: Style::Selected },
        ],
        DisplayMode::Shaded,
    );
    let frame = render(&req);
    let highlighted = frame.color.chunks_exact(4).filter(|p| *p == req.palette.selected).count();
    assert!(highlighted > 50, "the selection outline is missing");
}
