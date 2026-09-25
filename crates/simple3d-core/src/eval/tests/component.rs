//! Integrations: a node standing for a whole other component (issue 113).

use super::*;
use crate::scene::{link, reaches, Body, ComponentId, GroupOp, Scene};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A component of two overlapping boxes, the second 5 mm along X.
fn two_boxes() -> Scene {
    let mut part = Scene::new();
    let root = part.root();
    part.add_primitive("box", root, 0).unwrap();
    let second = part.add_primitive("box", root, 1).unwrap();
    part.get_mut(second).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    part
}

/// `scene` handed the components it integrates, the way the application does.
fn linked(mut scene: Scene, parts: &[(ComponentId, &Scene)]) -> Scene {
    let map: BTreeMap<ComponentId, &Scene> = parts.iter().copied().collect();
    scene.components = Arc::new(link(&map));
    scene
}

#[test]
pub(crate) fn an_integration_is_its_component_placed_where_it_stands() {
    let part = two_boxes();
    let own = Evaluator::new().evaluate(&part, &Cancel::new());
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_integration(7, "Part", root, 0);
    scene.get_mut(id).unwrap().position = Vec3::new(0.0, 100.0, 0.0);
    let scene = linked(scene, &[(7, &part)]);

    let result = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(size(&result.mesh), size(&own.mesh), "the integration is not the shape of its component");
    let (lo, _) = result.mesh.bounds().unwrap();
    let (own_lo, _) = own.mesh.bounds().unwrap();
    assert!((lo.y - own_lo.y - 100.0).abs() < 1e-9, "the integration was not placed where it stands");
    // One node in the tree, and so one mesh to pick.
    assert!(result.node_meshes.contains_key(&id));
    assert_eq!(result.node_meshes.len(), 1, "the component's own nodes leaked into this scene");
}

#[test]
pub(crate) fn an_edit_to_the_component_misses_the_cache() {
    let mut part = two_boxes();
    let mut scene = Scene::new();
    let root = scene.root();
    scene.add_integration(3, "Part", root, 0);
    let mut evaluator = Evaluator::new();
    let first = evaluator.evaluate(&linked(scene.clone(), &[(3, &part)]), &Cancel::new());

    let part_root = part.root();
    let third = part.add_primitive("box", part_root, 2).unwrap();
    part.get_mut(third).unwrap().position = Vec3::new(0.0, 0.0, 50.0);
    let second = evaluator.evaluate(&linked(scene, &[(3, &part)]), &Cancel::new());
    assert!(size(&second.mesh).z > size(&first.mesh).z + 40.0, "the edit to the component was not seen");
}

#[test]
pub(crate) fn an_integration_may_use_its_own_operation() {
    let part = two_boxes();
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_integration(1, "Part", root, 0);
    let union = Evaluator::new().evaluate(&linked(scene.clone(), &[(1, &part)]), &Cancel::new());
    scene.get_mut(id).unwrap().body = Body::Component { component: 1, op: Some(GroupOp::Intersection) };
    let cut = Evaluator::new().evaluate(&linked(scene, &[(1, &part)]), &Cancel::new());
    assert!(cut.errors.is_empty(), "{:?}", cut.errors);
    assert!(size(&cut.mesh).x < size(&union.mesh).x - 4.0, "the operation of the integration was ignored");
}

#[test]
pub(crate) fn a_missing_component_is_an_error_on_the_integration() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_integration(9, "Gone", root, 0);
    let result = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].node, id);
    assert_eq!(result.mesh.triangle_count(), 0);
}

#[test]
pub(crate) fn components_nest_and_a_loop_is_left_open_rather_than_followed() {
    let inner = two_boxes();
    let mut middle = Scene::new();
    let middle_root = middle.root();
    middle.add_integration(1, "Inner", middle_root, 0);
    let mut top = Scene::new();
    let top_root = top.root();
    top.add_integration(2, "Middle", top_root, 0);
    let top = linked(top, &[(1, &inner), (2, &middle)]);
    let result = Evaluator::new().evaluate(&top, &Cancel::new());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.mesh.triangle_count() > 0, "a component two levels down evaluated to nothing");

    // Two components holding each other, which only a hand-edited file can
    // say: evaluating either finishes, and says what is missing.
    let mut a = Scene::new();
    let a_root = a.root();
    a.add_integration(2, "B", a_root, 0);
    let mut b = Scene::new();
    let b_root = b.root();
    b.add_integration(1, "A", b_root, 0);
    let scenes = |id: ComponentId| match id {
        1 => Some(&a),
        2 => Some(&b),
        _ => None,
    };
    assert!(reaches(&scenes, 1, 2) && reaches(&scenes, 2, 1));
    let map: BTreeMap<ComponentId, &Scene> = [(1, &a), (2, &b)].into_iter().collect();
    let linked = link(&map);
    let result = Evaluator::new().evaluate(&linked[&1], &Cancel::new());
    assert!(!result.errors.is_empty(), "a component inside itself evaluated without complaint");
}
