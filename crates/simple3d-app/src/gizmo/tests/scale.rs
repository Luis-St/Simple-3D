//! Scaling: a factor on the node, never a dimension.

use super::*;
use crate::view::View;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::scene::Camera;
use simple3d_core::scene::{Node, Scene};
use simple3d_core::unit::Unit;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_scale_drag_writes_a_factor_and_leaves_the_dimension_alone() {
    // The difference between the two sizing tools, in one assertion: resize
    // rewrites the number the shape is defined by, scale multiplies what is
    // there and leaves that number where it was.
    let mut f = Fixture::new("box");
    let width = f.param("width");
    let (lo, hi) = f.world_bounds();
    let handle = Handle::ResizeFace(0, true);
    let gizmo = f.gizmo(Mode::Scale);
    let face = gizmo.handle_point(handle, &f.view);

    // Out along +X by the width again: twice as wide.
    drag_in(&mut f, Mode::Scale, handle, face + Vec3::new(width, 0.0, 0.0), Mods::default(), 1.0);

    assert!((f.param("width") - width).abs() < 1e-9, "scaling rewrote the dimension");
    let scale = f.scene.node(f.node).scale;
    assert!((scale.x - 2.0).abs() < 0.05, "the X factor came out {}", scale.x);
    assert_eq!((scale.y, scale.z), (1.0, 1.0), "one axis was dragged and three were scaled");

    let (nlo, nhi) = f.world_bounds();
    assert!((nhi.x - nlo.x - (hi.x - lo.x) * 2.0).abs() < 0.2, "it is not twice as wide on screen");
    // The face that was not dragged did not move.
    assert!((nlo.x - lo.x).abs() < 0.05, "the opposite face moved: {nlo:?} vs {lo:?}");
}

#[test]
pub(crate) fn a_group_can_be_scaled_although_it_cannot_be_resized() {
    // Resize refuses a group -- scaling one truthfully would mean rewriting
    // every descendant. A factor needs none of that, and it is the only way
    // to make an assembly a proportion of what it was.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(simple3d_core::scene::GroupOp::Union, root, 0);
    let child = scene.add_primitive("box", group, 0).unwrap();
    scene.get_mut(child).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    scene.camera = Camera { yaw: -55.0, pitch: 28.0, distance: 160.0, ..Camera::default() };
    let mut evaluator = Evaluator::new();
    let evaluated = evaluator.evaluate(&scene, &Cancel::new());
    let view = View::new(scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));

    assert!(
        Gizmo::build(&scene, &evaluated, group, Mode::Resize, false).unwrap().handles(true).is_empty(),
        "resize offered a group handles"
    );
    let gizmo = Gizmo::build(&scene, &evaluated, group, Mode::Scale, false).unwrap();
    assert!(!gizmo.handles(true).is_empty(), "scale offered a group no handles");

    let handle = Handle::ResizeFace(0, true);
    let face = gizmo.handle_point(handle, &view);
    let from = view.project(face).unwrap().0;
    let to = view.project(face + Vec3::new(20.0, 0.0, 0.0)).unwrap().0;
    let mut drag = Drag::begin(&scene, &gizmo, group, handle, &view, from).unwrap();
    drag.update(&mut scene, &view, to, Mods { free: true, ..Default::default() }, 1.0, 15.0, Unit::Millimetre);

    assert!(scene.node(group).scale.x > 1.5, "the group was not scaled: {:?}", scene.node(group).scale);
    let after = evaluator.evaluate(&scene, &Cancel::new());
    let (lo, hi) = after.node_meshes[&child].bounds().unwrap();
    assert!((hi.x - lo.x) > 12.0, "the child did not grow with the group it is in");
}

#[test]
pub(crate) fn a_resize_on_a_scaled_node_still_lands_on_the_dimension_that_was_dragged_to() {
    // A drag is measured on screen, and the screen shows the node *after*
    // its scale. Writing the on-screen distance straight into the dimension
    // would resize by the factor again.
    let mut f = Fixture::new("box");
    f.scene.get_mut(f.node).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);
    f.reevaluate();
    let handle = Handle::ResizeFace(0, true);
    let gizmo = f.gizmo(Mode::Resize);
    let face = gizmo.handle_point(handle, &f.view);
    let width = f.param("width");

    // Ten world millimetres out is five of the node's own, at a factor of 2.
    drag_in(
        &mut f,
        Mode::Resize,
        handle,
        face + Vec3::new(10.0, 0.0, 0.0),
        Mods { free: true, ..Default::default() },
        1.0,
    );
    assert!((f.param("width") - (width + 5.0)).abs() < 0.2, "the width came out {}", f.param("width"));
    assert_eq!(f.scene.node(f.node).scale, Vec3::new(2.0, 1.0, 1.0), "the resize touched the scale");
}

#[test]
pub(crate) fn a_scale_cannot_be_dragged_to_zero_or_through_it() {
    let mut f = Fixture::new("box");
    let handle = Handle::ResizeFace(0, true);
    let gizmo = f.gizmo(Mode::Scale);
    let face = gizmo.handle_point(handle, &f.view);
    // Far past the opposite face, which would invert the solid.
    drag_in(
        &mut f,
        Mode::Scale,
        handle,
        face - Vec3::new(500.0, 0.0, 0.0),
        Mods { free: true, ..Default::default() },
        1.0,
    );
    assert!(f.scene.node(f.node).scale.x >= Node::MIN_SCALE, "{:?}", f.scene.node(f.node).scale);
    assert!(f.world_bounds().1.x > f.world_bounds().0.x, "the solid was turned inside out");
}

#[test]
pub(crate) fn escape_puts_a_scale_back_where_a_drag_found_it() {
    let mut f = Fixture::new("box");
    f.scene.get_mut(f.node).unwrap().scale = Vec3::new(1.5, 1.0, 1.0);
    f.reevaluate();
    let handle = Handle::ResizeFace(0, true);
    let target = f.gizmo(Mode::Scale).handle_point(handle, &f.view) + Vec3::new(25.0, 0.0, 0.0);
    let drag = drag_in(&mut f, Mode::Scale, handle, target, Mods::default(), 1.0);
    assert_ne!(f.scene.node(f.node).scale.x, 1.5);
    drag.cancel(&mut f.scene);
    assert_eq!(f.scene.node(f.node).scale, Vec3::new(1.5, 1.0, 1.0));
}

#[test]
pub(crate) fn resizing_never_writes_a_scale_factor_into_the_project() {
    // The other half of criterion 24: the saved file has no scale anywhere.
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeFace(2, true);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(0.0, 0.0, 20.0), Mods::default(), 10.0);
    let text = simple3d_core::project::to_string(&f.scene);
    assert!(!text.to_lowercase().contains("scale"), "a scale factor reached the project file");
    assert!(text.contains("\"height\""));
}
