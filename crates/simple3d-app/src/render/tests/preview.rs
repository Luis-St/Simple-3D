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

/// A template buried in a body shows its edges faintly through it and never solid (issue 70): solid
/// through the body, they made the body look removed. Regression: they were solid, and before that
/// missing, so the template looked cut off by the body.
#[test]
pub(crate) fn a_template_buried_in_a_body_shows_its_edges_faintly_through_it() {
    let solid = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let small = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0));
    let lift = simple3d_core::xform::Xform::from_translation(Vec3::new(4.0, -4.0, 5.0));
    let mut req = request(vec![Item { renderable: &solid, style: Style::Solid }], DisplayMode::Shaded);
    let edge = req.palette.template;
    let bare = render(&req);
    req.templates = vec![(&small, lift)];
    let frame = render(&req);
    assert_eq!(pixels_of(&frame, edge), 0, "the buried template's edges were drawn solid through the body");
    for &corner in &small.mesh.positions {
        let (at, _) = req.view.project(lift.point(corner)).expect("the corner is in front of the eye");
        assert!(changed_near(&bare, &frame, at), "no faint edge at the corner {corner:?}, at {at:?}");
    }
}

/// A template in front of a body is drawn over it, solid edges and all (issue 70).
#[test]
pub(crate) fn a_template_in_front_of_a_body_shows_solid_edges_over_it() {
    let solid = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let small = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0));
    let mut req = request(vec![Item { renderable: &solid, style: Style::Solid }], DisplayMode::Shaded);
    // Between the body and the eye, so the body is behind every corner.
    let lift = simple3d_core::xform::Xform::from_translation(req.view.forward() * -60.0);
    req.templates = vec![(&small, lift)];
    let frame = render(&req);
    let edge = req.palette.template;
    for &corner in &small.mesh.positions {
        let (at, _) = req.view.project(lift.point(corner)).expect("the corner is in front of the eye");
        assert!(has_near(&frame, at, edge), "no solid template edge at the corner {corner:?}, at {at:?}");
    }
}

/// The byte offsets of the pixels round `at`.
fn near(frame: &Image, at: egui::Pos2) -> impl Iterator<Item = usize> {
    let width = frame.width as i32;
    (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| ((at.y as i32 + dy) * width + at.x as i32 + dx) as usize * 4))
}

fn has_near(frame: &Image, at: egui::Pos2, colour: Rgba) -> bool {
    near(frame, at).any(|o| frame.color[o..o + 3] == colour[..3])
}

fn changed_near(before: &Image, after: &Image, at: egui::Pos2) -> bool {
    near(before, at).any(|o| before.color[o..o + 3] != after.color[o..o + 3])
}
