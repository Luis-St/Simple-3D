//! Push/pull (issue 73): the faces of a result traced to their objects, and the solids made from them.

use super::*;
use crate::scene::{CapturedFace, GroupOp, Placing, Scene};
use crate::xform::Xform;
use simple3d_geom::push_pull::{face_basis, flat_face};
use simple3d_geom::Vec3;

pub(super) fn evaluate(scene: &Scene) -> Evaluated {
    let out = Evaluator::new().evaluate(scene, &Cancel::new());
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    out
}

/// The face of the result whose outward normal is `normal` and that contains `near`, captured.
pub(super) fn face_at(out: &Evaluated, normal: Vec3, near: Vec3) -> (u32, CapturedFace) {
    let mesh = &out.mesh;
    let triangle = (0..mesh.indices.len())
        .find(|&t| {
            let [a, b, c] = mesh.corners(mesh.indices[t]);
            let n = mesh.triangle_normal(mesh.indices[t]);
            n.dot(normal) > 0.999 && (n.dot(near - a)).abs() < 1e-6 && {
                let inside = |p: Vec3, q: Vec3| (q - p).cross(near - p).dot(n) >= -1e-9;
                inside(a, b) && inside(b, c) && inside(c, a)
            }
        })
        .expect("no face there");
    let face = flat_face(mesh, triangle).unwrap();
    let (u, v) = face_basis(face.normal);
    let outline = face.outline(face.centre, u, v).unwrap();
    (mesh.source(triangle), CapturedFace { outline, origin: face.centre, u, v, normal: face.normal })
}

pub(super) fn volume(out: &Evaluated) -> f64 {
    out.mesh.signed_volume()
}

#[test]
pub(crate) fn a_face_of_a_union_is_traced_to_its_own_object() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = scene.add_primitive("box", root, 0).unwrap();
    let b = scene.add_primitive("box", root, 1).unwrap();
    scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    let out = evaluate(&scene);
    let (lo, hi) = out.bounds.unwrap();
    let (left, _) = face_at(&out, Vec3::new(-1.0, 0.0, 0.0), Vec3::new(lo.x, 0.0, 0.0));
    let (right, _) = face_at(&out, Vec3::new(1.0, 0.0, 0.0), Vec3::new(hi.x, 0.0, 0.0));
    assert_eq!((left, right), (source_of(a), source_of(b)));
}

#[test]
pub(crate) fn pushing_a_face_adds_to_its_object_and_pulling_cuts_it() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let out = evaluate(&scene);
    let before = volume(&out);
    let (lo, hi) = out.bounds.unwrap();
    let top = Vec3::new(0.0, 0.0, hi.z);
    let (source, face) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), top);
    assert_eq!(source, source_of(block));
    let area = face.outline.area();

    assert_eq!(scene.push_pull(block, &face, 5.0, &out.node_frames), Some((block, 0)));
    assert_eq!(scene.node(root).children, vec![block], "the push made a node instead of changing the box");
    assert_eq!(scene.node(block).edits[0].as_push().unwrap().placing, Placing::Add);
    let out = evaluate(&scene);
    assert!((volume(&out) - (before + area * 5.0)).abs() < 1e-6, "{} vs {}", volume(&out), before + area * 5.0);
    assert!((out.bounds.unwrap().1.z - (hi.z + 5.0)).abs() < 1e-9);
    assert!(out.mesh.manifold_issue().is_none());

    // Pulling the bottom in by 2 cuts the box itself; so does pulling the floor that left.
    let bottom = Vec3::new(0.0, 0.0, lo.z);
    let (_, face) = face_at(&out, Vec3::new(0.0, 0.0, -1.0), bottom);
    assert_eq!(scene.push_pull(block, &face, -2.0, &out.node_frames), Some((block, 1)));
    assert_eq!(scene.node(block).edits[1].as_push().unwrap().placing, Placing::Cut);
    let out = evaluate(&scene);
    assert!((volume(&out) - (before + area * 3.0)).abs() < 1e-6, "{}", volume(&out));
    assert!((out.bounds.unwrap().0.z - (lo.z + 2.0)).abs() < 1e-9);
    let (source, face) = face_at(&out, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, lo.z + 2.0));
    assert_eq!(source, source_of(block), "the cut's floor is the box's own");
    scene.push_pull(block, &face, -1.0, &out.node_frames).unwrap();
    assert_eq!(scene.node(root).children, vec![block]);
    let out = evaluate(&scene);
    assert!((volume(&out) - (before + area * 2.0)).abs() < 1e-6, "{}", volume(&out));
}

#[test]
pub(crate) fn a_pushed_wall_is_one_face_with_the_wall_it_grew_flush_with() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let out = evaluate(&scene);
    let (lo, hi) = out.bounds.unwrap();
    let (_, face) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, hi.z));
    scene.push_pull(block, &face, 5.0, &out.node_frames).unwrap();
    let out = evaluate(&scene);
    // The +X side now runs from the bottom to the pushed top as one face.
    let mid = Vec3::new(hi.x, (lo.y + hi.y) / 2.0, (lo.z + hi.z) / 2.0);
    let (_, side) = face_at(&out, Vec3::new(1.0, 0.0, 0.0), mid);
    let whole = (hi.y - lo.y) * (hi.z + 5.0 - lo.z);
    assert!((side.outline.area() - whole).abs() < 1e-6, "{} vs {whole}", side.outline.area());
    // And the face pushed again moves the whole wall.
    let before = volume(&out);
    scene.push_pull(block, &side, 2.0, &out.node_frames).unwrap();
    assert!((volume(&evaluate(&scene)) - (before + whole * 2.0)).abs() < 1e-6);
}

#[test]
pub(crate) fn a_push_on_a_drilled_plate_is_held_by_its_difference() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut scene, group);
    cylinder(&mut scene, group, 6.0, 20.0);
    scene.get_mut(group).unwrap().position = Vec3::new(3.0, 4.0, 5.0);
    scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 30.0);
    let out = evaluate(&scene);
    let before = volume(&out);
    let up = Vec3::new(0.0, 0.0, 1.0);
    let top = out.bounds.unwrap().1.z;
    // A point on the top face well away from the hole.
    let at = Xform::from_pos_rot(Vec3::new(3.0, 4.0, 5.0), Vec3::new(0.0, 0.0, 30.0)).point(Vec3::new(15.0, 5.0, 0.0));
    let (source, face) = face_at(&out, up, Vec3::new(at.x, at.y, top));
    assert_eq!(source, source_of(base));
    assert_eq!(face.outline.holes.len(), 1, "the hole was lost from the outline");
    let area = face.outline.area();
    // Held by the difference, so the hole's cylinder does not cut what was added.
    assert_eq!(scene.push_pull(base, &face, 3.0, &out.node_frames), Some((group, 0)));
    assert_eq!(scene.node(group).children.len(), 2);
    let out = evaluate(&scene);
    assert!((volume(&out) - (before + area * 3.0)).abs() < 1e-3, "{} vs {}", volume(&out), before + area * 3.0);
    assert!((out.bounds.unwrap().1.z - (top + 3.0)).abs() < 1e-9);
}

#[test]
pub(crate) fn an_extracted_extrusion_round_trips_through_a_project_file_as_format_five() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let out = evaluate(&scene);
    let (_, face) = face_at(&out, Vec3::new(1.0, 0.0, 0.0), Vec3::new(out.bounds.unwrap().1.x, 0.0, 0.0));
    scene.push_pull(block, &face, 4.0, &out.node_frames).unwrap();
    let out = evaluate(&scene);
    scene.extract_face_edit(block, 0, &out.node_frames).unwrap();
    let text = crate::project::to_string(&scene);
    assert!(text.contains("\"format\": 5") && text.contains("\"extrusion\""), "{text}");
    let back = crate::project::from_str(&text).unwrap();
    assert!((volume(&evaluate(&back)) - volume(&evaluate(&scene))).abs() < 1e-9);
    // Without one, the plain version is still written.
    assert!(crate::project::to_string(&Scene::new()).contains("\"format\": 3"));
}
