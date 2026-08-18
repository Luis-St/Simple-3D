//! Clicking geometry in the viewport selects the corresponding node in the
//! outliner (spec section 6.1, acceptance criterion 12).
//!
//! The ray is cast against each node's *own* world-space mesh rather than
//! against the evaluated result, because the evaluated result is one merged mesh
//! with no memory of where its triangles came from. A consequence worth knowing:
//! clicking the wall of a drilled hole selects the cylinder that cut it, which is
//! the node you would want to adjust.

use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_geom::{Mesh, Vec3};

/// Distance along the ray at which it enters the triangle, if it does.
/// Moeller-Trumbore, accepting either facing so a click inside a solid still
/// finds it.
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(h) * inv;
    if u < -1e-9 || u > 1.0 + 1e-9 {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) * inv;
    if t < 1e-6 {
        return None;
    }
    Some(t)
}

/// Nearest hit on a mesh, if the ray meets it at all.
pub fn ray_mesh(mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<f64> {
    // A bounding-box reject first: with 200 primitives this is the difference
    // between a click feeling instant and feeling like work.
    let (lo, hi) = mesh.bounds()?;
    ray_box(origin, dir, lo, hi)?;
    let mut nearest: Option<f64> = None;
    for tri in &mesh.indices {
        let hit = ray_triangle(
            origin,
            dir,
            mesh.positions[tri[0] as usize],
            mesh.positions[tri[1] as usize],
            mesh.positions[tri[2] as usize],
        );
        if let Some(t) = hit {
            if nearest.is_none_or(|best| t < best) {
                nearest = Some(t);
            }
        }
    }
    nearest
}

/// Slab test. Returns the entry distance, or `None` if the ray misses.
pub fn ray_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<f64> {
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    for axis in 0..3 {
        let (o, d) = match axis {
            0 => (origin.x, dir.x),
            1 => (origin.y, dir.y),
            _ => (origin.z, dir.z),
        };
        let (l, h) = match axis {
            0 => (lo.x, hi.x),
            1 => (lo.y, hi.y),
            _ => (lo.z, hi.z),
        };
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (mut near, mut far) = ((l - o) / d, (h - o) / d);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t_min = t_min.max(near);
        t_max = t_max.min(far);
        if t_min > t_max {
            return None;
        }
    }
    if t_max < 0.0 {
        return None;
    }
    Some(t_min.max(0.0))
}

/// The visible node the ray hits first. Hidden nodes are skipped, and so is
/// anything under a hidden group: a hidden node is not part of the model, so a
/// click passes through it to whatever is actually there.
pub fn pick(scene: &Scene, evaluated: &Evaluated, origin: Vec3, dir: Vec3) -> Option<NodeId> {
    let mut best: Option<(f64, NodeId)> = None;
    for (&id, mesh) in &evaluated.node_meshes {
        if !scene.contains(id) || !scene.is_shown(id) {
            continue;
        }
        if let Some(t) = ray_mesh(mesh, origin, dir) {
            if best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, id));
            }
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
