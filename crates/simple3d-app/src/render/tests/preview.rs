//! Preview loops, drawn on the solid and hidden behind it.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;
use simple3d_geom::primitives;

/// The preview is its own step after the model. Regression: the GPU sorts steps into passes, and a
/// no-depth line meant the grid under the model, so the preview was hidden while pixel tests passed.
#[test]
pub(crate) fn a_preview_is_its_own_kind_of_step_and_comes_after_the_model() {
    let prepared = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.preview = vec![vec![
        Vec3::new(-15.0, -15.0, 20.0),
        Vec3::new(15.0, -15.0, 20.0),
        Vec3::new(15.0, 15.0, 20.0),
        Vec3::new(-15.0, 15.0, 20.0),
    ]];
    let steps = prepare(&req);
    let overlays = steps.iter().filter(|step| matches!(step, Step::Overlay { .. })).count();
    assert_eq!(overlays, 4, "a four-cornered loop is four lines");
    let first = steps.iter().position(|step| matches!(step, Step::Overlay { .. })).expect("it is in there");
    let last_face = steps.iter().rposition(|step| matches!(step, Step::Triangle { .. })).expect("the box is drawn");
    assert!(first > last_face, "the preview was prepared before the model it is drawn over");
}

/// A preview is drawn on the model, not through it (issue 82): near-face cells show, far-face cells
/// are hidden behind the solid.
#[test]
pub(crate) fn a_preview_loop_is_drawn_on_the_solid_and_hidden_behind_it() {
    let prepared = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let square = |z: f64| {
        vec![Vec3::new(-15.0, -15.0, z), Vec3::new(15.0, -15.0, z), Vec3::new(15.0, 15.0, z), Vec3::new(-15.0, 15.0, z)]
    };
    let drawn = |loops: Vec<Vec<Vec3>>| {
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.preview = loops;
        let frame = render(&req);
        pixels_of(&frame, req.palette.selected)
    };
    assert!(drawn(vec![square(20.0)]) > 0, "the cells on the face turned towards the camera were not drawn");
    assert_eq!(drawn(vec![square(-20.0)]), 0, "the cells on the far side of the solid were drawn through it");
}

/// A template's edges are drawn over a body it is buried in (issue 70), at every corner. Regression:
/// only its faces were, blended so faintly over the body that the template read as cut off by it.
#[test]
pub(crate) fn a_template_buried_in_a_body_still_shows_its_edges_over_it() {
    let solid = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let small = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0));
    let lift = simple3d_core::xform::Xform::from_translation(Vec3::new(4.0, -4.0, 5.0));
    let mut req = request(vec![Item { renderable: &solid, style: Style::Solid }], DisplayMode::Shaded);
    let edge = req.palette.template;
    assert_eq!(pixels_of(&render(&req), edge), 0, "the edge colour is drawn without any template");
    req.templates = vec![(&small, lift)];
    let frame = render(&req);
    for &corner in &small.mesh.positions {
        let (at, _) = req.view.project(lift.point(corner)).expect("the corner is in front of the eye");
        let near = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (at.x as i32 + dx, at.y as i32 + dy)));
        let seen = near.into_iter().any(|(x, y)| {
            let o = (y as usize * frame.width + x as usize) * 4;
            frame.color[o..o + 3] == edge[..3]
        });
        assert!(seen, "no template edge at the corner {corner:?}, at {at:?} inside the body");
    }
}
