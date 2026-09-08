//! Preview loops, drawn on the solid and hidden behind it.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

/// The preview is a step of its own, and it comes after the model.
///
/// The two engines are handed the same prepared steps, and only the
/// software one draws them in the order it was given: the GPU sorts them
/// into passes by kind, and a line that writes no depth meant the ground
/// grid, which goes *under* the model. Drawn as one, the preview was
/// covered by the very shape it is drawn on -- invisible in the running
/// application while every pixel test on this file passed.
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

/// A tool's preview is drawn *on* the model, not through it (issue 82).
///
/// The cells at the near end of a run lie in the face turned towards the
/// camera and have to be drawn over it; the ones at the far end lie in the
/// face turned away and are behind forty millimetres of solid. A grid that
/// shows through the shape reads as floating in front of it, which is the
/// one thing the preview must not say.
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
