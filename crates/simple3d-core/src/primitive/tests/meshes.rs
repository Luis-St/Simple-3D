//! Every primitive builds a solid at the size it claims.

use super::*;

#[test]
pub(crate) fn every_primitive_builds_a_manifold_mesh_from_its_defaults() {
    for spec in REGISTRY {
        let params = spec.default_params();
        let mesh = (spec.build)(&params, 32);
        assert!(mesh.triangle_count() > 0, "{}: empty mesh", spec.type_id);
        assert!(mesh.manifold_issue().is_none(), "{}: {}", spec.type_id, mesh.manifold_issue().unwrap());
    }
}

#[test]
pub(crate) fn a_chamfered_box_measures_the_dimensions_it_was_given() {
    // Whichever edges are cut, and however large the chamfer typed in, the
    // box is still exactly its stated size -- criterion 2 for a shape whose
    // parameter has to be clamped.
    let spec = lookup("chamfered_box").expect("the chamfered box is declared");
    for edges in 0..3 {
        for chamfer in [0.0, 1.0, 4.0, 1000.0] {
            let mut params = spec.default_params();
            params.insert("edges".into(), ParamValue::Choice(edges));
            params.insert("chamfer".into(), ParamValue::Length(chamfer));
            let mesh = (spec.build)(&params, 0);
            assert!(
                mesh.manifold_issue().is_none(),
                "edges {edges} chamfer {chamfer}: {}",
                mesh.manifold_issue().unwrap()
            );
            let (lo, hi) = mesh.bounds().expect("a solid has bounds");
            let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
            for (axis, key) in ["width", "depth", "height"].iter().enumerate() {
                assert!(
                    (params.num(key) - size[axis]).abs() < 1e-9,
                    "edges {edges} chamfer {chamfer}: {key} is {} but measures {}",
                    params.num(key),
                    size[axis]
                );
            }
        }
    }
}

#[test]
pub(crate) fn flat_shape_aliases_match_their_general_forms() {
    // Spec section 3.2: "must produce identical geometry to their equivalents".
    let plate = lookup("plate").unwrap();
    let boxy = lookup("box").unwrap();
    let mut bp = boxy.default_params();
    bp.insert("width".into(), ParamValue::Length(40.0));
    bp.insert("depth".into(), ParamValue::Length(20.0));
    bp.insert("height".into(), ParamValue::Length(4.0));
    let a = (plate.build)(&plate.default_params(), 32);
    let b = (boxy.build)(&bp, 32);
    assert_eq!(a.indices, b.indices);
    assert_eq!(a.positions, b.positions);

    // A rounded plate with no radius is still exactly the box, which is
    // what lets the corner radius be optional on a shape that is an alias.
    let mut sharp = plate.default_params();
    sharp.insert("corner_radius".into(), ParamValue::Length(0.0));
    assert_eq!((plate.build)(&sharp, 32).positions, b.positions);

    let disc = lookup("disc").unwrap();
    let cyl = lookup("cylinder").unwrap();
    let mut cp = cyl.default_params();
    cp.insert("height".into(), ParamValue::Length(2.0));
    let a = (disc.build)(&disc.default_params(), 32);
    let b = (cyl.build)(&cp, 32);
    assert_eq!(a.positions, b.positions);
}

#[test]
pub(crate) fn tube_wall_modes_agree() {
    let tube = lookup("tube").unwrap();
    let mut by_wall = tube.default_params();
    by_wall.insert("wall_mode".into(), ParamValue::Choice(0));
    by_wall.insert("wall_thickness".into(), ParamValue::Length(2.0));
    let mut by_inner = tube.default_params();
    by_inner.insert("wall_mode".into(), ParamValue::Choice(1));
    by_inner.insert("inner_diameter".into(), ParamValue::Length(16.0));
    assert_eq!(tube_inner(&by_wall), tube_inner(&by_inner));
    let a = (tube.build)(&by_wall, 32);
    let b = (tube.build)(&by_inner, 32);
    assert_eq!(a.positions, b.positions);
}
