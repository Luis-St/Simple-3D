use super::pick::*;
use super::ray::*;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_geom::Vec3;

use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::primitive::ParamValue;
use simple3d_core::scene::GroupOp;
use simple3d_geom::primitives;

fn boxed(scene: &mut Scene, parent: NodeId, at: Vec3) -> NodeId {
    let index = scene.node(parent).children.len();
    let id = scene.add_primitive("box", parent, index).unwrap();
    scene.get_mut(id).unwrap().position = at;
    id
}

#[test]
fn a_ray_through_a_triangle_reports_the_distance() {
    let (a, b, c) = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0));
    let t = ray_triangle(Vec3::new(0.0, -5.0, 0.0), Vec3::new(0.0, 1.0, 0.0), a, b, c).unwrap();
    assert!((t - 5.0).abs() < 1e-9, "got {t}");
    // From the other side too: a click inside a solid still finds its walls.
    let t = ray_triangle(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0), a, b, c).unwrap();
    assert!((t - 5.0).abs() < 1e-9);
}

#[test]
fn a_ray_missing_a_triangle_reports_nothing() {
    let (a, b, c) = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0));
    assert!(ray_triangle(Vec3::new(10.0, -5.0, 0.0), Vec3::new(0.0, 1.0, 0.0), a, b, c).is_none());
    // Parallel to the triangle's plane.
    assert!(ray_triangle(Vec3::new(0.0, -5.0, 0.0), Vec3::new(1.0, 0.0, 0.0), a, b, c).is_none());
    // Behind the eye.
    assert!(ray_triangle(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, 1.0, 0.0), a, b, c).is_none());
}

#[test]
fn the_box_test_rejects_misses_and_accepts_hits() {
    let (lo, hi) = (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(ray_box(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, 1.0, 0.0), lo, hi).is_some());
    assert!(ray_box(Vec3::new(5.0, -10.0, 0.0), Vec3::new(0.0, 1.0, 0.0), lo, hi).is_none());
    // Starting inside.
    assert_eq!(ray_box(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), lo, hi), Some(0.0));
    // Pointing away.
    assert!(ray_box(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, -1.0, 0.0), lo, hi).is_none());
}

#[test]
fn a_ray_finds_the_near_face_of_a_mesh() {
    let mesh = primitives::box_mesh(10.0, 10.0, 10.0);
    let t = ray_mesh(&mesh, Vec3::new(0.0, -50.0, 0.0), Vec3::new(0.0, 1.0, 0.0)).unwrap();
    assert!((t - 45.0).abs() < 1e-6, "got {t}, expected the near wall at y = -5");
}

#[test]
fn clicking_geometry_selects_the_node_it_belongs_to() {
    // Spec acceptance criterion 12.
    let mut scene = Scene::new();
    let root = scene.root();
    let left = boxed(&mut scene, root, Vec3::new(-40.0, 0.0, 0.0));
    let right = boxed(&mut scene, root, Vec3::new(40.0, 0.0, 0.0));
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());

    let from = Vec3::new(0.0, -200.0, 0.0);
    let aim = |target: Vec3| (target - from).normalized();
    assert_eq!(pick(&scene, &out, from, aim(Vec3::new(-40.0, 0.0, 0.0))), Some(left));
    assert_eq!(pick(&scene, &out, from, aim(Vec3::new(40.0, 0.0, 0.0))), Some(right));
    // Empty space selects nothing, which is how a click deselects.
    assert_eq!(pick(&scene, &out, from, aim(Vec3::new(0.0, 0.0, 500.0))), None);
}

#[test]
fn the_nearest_node_wins_when_two_overlap_on_screen() {
    let mut scene = Scene::new();
    let root = scene.root();
    let far = boxed(&mut scene, root, Vec3::new(0.0, 100.0, 0.0));
    let near = boxed(&mut scene, root, Vec3::new(0.0, -100.0, 0.0));
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, -300.0, 0.0), Vec3::new(0.0, 1.0, 0.0)), Some(near));
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, 300.0, 0.0), Vec3::new(0.0, -1.0, 0.0)), Some(far));
}

#[test]
fn hidden_nodes_are_not_pickable_even_though_they_are_drawn_as_ghosts() {
    let mut scene = Scene::new();
    let root = scene.root();
    let hidden = boxed(&mut scene, root, Vec3::new(0.0, -100.0, 0.0));
    let behind = boxed(&mut scene, root, Vec3::new(0.0, 100.0, 0.0));
    scene.get_mut(hidden).unwrap().visible = false;
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.node_meshes.contains_key(&hidden), "the ghost mesh should still exist");
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, -300.0, 0.0), Vec3::new(0.0, 1.0, 0.0)), Some(behind));
}

#[test]
fn a_node_inside_a_hidden_group_is_not_pickable_either() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    boxed(&mut scene, group, Vec3::ZERO);
    scene.get_mut(group).unwrap().visible = false;
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, -300.0, 0.0), Vec3::new(0.0, 1.0, 0.0)), None);
}

#[test]
fn clicking_the_wall_of_a_hole_selects_the_tool_that_cut_it() {
    // A consequence of picking per node rather than on the merged result,
    // and the behaviour you want: the cylinder is what you would adjust.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let _plate = scene.add_primitive("plate", group, 0).unwrap();
    let hole = scene.add_primitive("cylinder", group, 1).unwrap();
    {
        let params = scene.get_mut(hole).unwrap().params_mut().unwrap();
        params.insert("diameter_x".into(), ParamValue::Length(6.0));
        params.insert("diameter_y".into(), ParamValue::Length(6.0));
        params.insert("height".into(), ParamValue::Length(20.0));
    }
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    // Straight down the hole's axis from above.
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -1.0)), Some(hole));
}

#[test]
fn picking_respects_a_groups_transform() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    scene.get_mut(group).unwrap().position = Vec3::new(0.0, 0.0, 60.0);
    let id = boxed(&mut scene, group, Vec3::ZERO);
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    // At the group's original position there is nothing any more.
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, -300.0, 0.0), Vec3::new(0.0, 1.0, 0.0)), None);
    assert_eq!(pick(&scene, &out, Vec3::new(0.0, -300.0, 60.0), Vec3::new(0.0, 1.0, 0.0)), Some(id));
}

#[test]
fn a_click_anywhere_on_a_pattern_selects_the_pattern_however_it_was_built() {
    // Issue 67: over the original copy a pattern's own mesh and the mesh of
    // the child it repeats are the same triangles at the same distance. The
    // tie used to fall to the lower node id, so the answer depended on the
    // order the two nodes happened to be made in: wrapping a shape (child
    // first) answered "child" on the original and "pattern" on every copy,
    // while filling an empty pattern (pattern first) answered "pattern" even
    // over the child. Both orders must now agree.
    for wrap in [false, true] {
        let mut scene = Scene::new();
        let root = scene.root();
        let (pat, child) = if wrap {
            let c = boxed(&mut scene, root, Vec3::ZERO);
            let p = scene.add_pattern(root, 1);
            scene.reparent(c, p, 0).unwrap();
            (p, c)
        } else {
            let p = scene.add_pattern(root, 0);
            let c = boxed(&mut scene, p, Vec3::ZERO);
            (p, c)
        };
        {
            let params = scene.get_mut(pat).unwrap().params_mut().unwrap();
            params.insert("count".into(), ParamValue::Count(3));
            params.insert("step_x".into(), ParamValue::Length(50.0));
        }
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let down = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(
            pick(&scene, &out, Vec3::new(0.0, 0.0, 100.0), down),
            Some(pat),
            "wrap={wrap}: the original copy picked something other than the pattern (child is {child:?})"
        );
        assert_eq!(
            pick(&scene, &out, Vec3::new(50.0, 0.0, 100.0), down),
            Some(pat),
            "wrap={wrap}: a repeated copy did not pick the pattern"
        );
    }
}
