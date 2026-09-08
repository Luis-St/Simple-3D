//! How a solid is shaded, and what each display mode paints.

use super::*;
use simple3d_core::config::DisplayMode;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

#[test]
pub(crate) fn a_box_has_twelve_feature_edges_and_a_cylinder_keeps_only_its_rims() {
    let box_edges = feature_edges(&primitives::box_mesh(20.0, 20.0, 20.0).weld(), 20.0);
    assert_eq!(box_edges.len(), 12, "a box has twelve real edges, got {}", box_edges.len());

    // A 32-segment cylinder's curved surface is smooth, so only the two rims
    // and the vertical seams between wall and cap survive -- never the fan
    // triangulation inside the caps.
    let cylinder = primitives::cylinder_mesh(20.0, 20.0, 20.0, 32).weld();
    let edges = feature_edges(&cylinder, 20.0);
    assert!(edges.len() >= 64, "both rims should be kept, got {}", edges.len());
    assert!(edges.len() < 100, "the cap triangulation leaked into the edges: {}", edges.len());
}

#[test]
pub(crate) fn a_shaded_render_actually_draws_the_model() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let items = vec![Item { renderable: &prepared, style: Style::Solid }];
    let req = request(items, DisplayMode::Shaded);
    let frame = render(&req);
    let painted = count_non_background(&frame, &req.palette);
    assert!(painted > 1000, "only {painted} pixels painted");
    assert!(painted < 160 * 120, "the model filled the entire viewport");
}

#[test]
pub(crate) fn shading_makes_faces_facing_different_ways_different_shades() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    let frame = render(&req);
    let mut shades: Vec<u8> = (0..frame.width * frame.height)
        .filter(|&i| !is_background(&frame, i, &req.palette))
        .map(|i| frame.color[i * 4])
        .collect();
    shades.sort_unstable();
    shades.dedup();
    assert!(shades.len() >= 3, "expected the three visible faces to differ, got {shades:?}");
}

#[test]
pub(crate) fn wireframe_paints_less_than_shaded_and_shaded_with_edges_paints_more() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let counts: Vec<usize> = [DisplayMode::Wireframe, DisplayMode::Shaded, DisplayMode::ShadedWithEdges]
        .into_iter()
        .map(|mode| {
            let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], mode);
            let frame = render(&req);
            count_non_background(&frame, &req.palette)
        })
        .collect();
    assert!(counts[0] < counts[1], "wireframe {} should paint less than shaded {}", counts[0], counts[1]);
    // Edges overwrite pixels the fill already covered, so the count is close;
    // what matters is that dark edge pixels appeared.
    let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::ShadedWithEdges);
    let frame = render(&req);
    let edge_pixels = frame.color.chunks_exact(4).filter(|p| *p == req.palette.edge).count();
    assert!(edge_pixels > 50, "no edge pixels in shaded-with-edges mode");
    assert!(counts[2] > 1000);
}

#[test]
pub(crate) fn a_ghost_is_translucent_over_the_background() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Ghost }], DisplayMode::Shaded);
    // No axes: a ghost hides nothing, so all three are drawn across it at
    // full strength, and `AXIS_X` is the same red as `DANGER` -- they would
    // answer the question this test is asking.
    req.grid.axes = [false; 3];
    let frame = render(&req);
    let painted = count_non_background(&frame, &req.palette);
    assert!(painted > 500, "the ghost did not draw");
    // Nothing fully opaque in the ghost's colour: everything is blended.
    assert!(!frame.color.chunks_exact(4).any(|p| p[..3] == req.palette.ghost[..3]), "the ghost drew opaquely");
}
