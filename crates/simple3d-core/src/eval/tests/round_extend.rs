//! An edge an earlier treatment shortened at a corner, carried on to the end of the body (issue 88).

use super::push_pull::{evaluate, volume};
use super::round_edits::{a_box, chamfer, treat};
use crate::scene::Scene;
use simple3d_geom::rounding::{extend_edge, feature_edges};

#[test]
pub(crate) fn an_edge_shortened_by_an_earlier_bevel_is_bevelled_whole_when_extended() {
    // Both top edges meeting at the box's front right corner, bevelled in one step.
    let mut together = Scene::new();
    let (block, front, _) = a_box(&mut together);
    let out = evaluate(&together);
    let (_, hi) = out.bounds.unwrap();
    let side_of = |mesh| {
        *feature_edges(mesh)
            .iter()
            .find(|e| {
                let m = (e.a + e.b) / 2.0;
                (m.x - hi.x).abs() < 1e-9 && (m.z - hi.z).abs() < 1e-9 && e.convex
            })
            .expect("no top right edge")
    };
    let side = side_of(&out.mesh);
    treat(&mut together, block, &[front, side], &[], &chamfer(2.0));
    let whole = volume(&evaluate(&together));

    // The front edge first, then the side edge found on the bevelled box, which now stops 2 short.
    let mut steps = Scene::new();
    let (block, front, _) = a_box(&mut steps);
    treat(&mut steps, block, &[front], &[], &chamfer(2.0));
    let bevelled = evaluate(&steps);
    let short = side_of(&bevelled.mesh);
    assert!((side.length() - short.length() - 2.0).abs() < 1e-6, "the first bevel did not shorten the side edge");
    let extended = extend_edge(&bevelled.mesh, &short, 2.0);
    assert!((extended.length() - side.length()).abs() < 1e-6, "extended to {}", extended.length());

    treat(&mut steps, block, &[extended], &[], &chamfer(2.0));
    assert!((volume(&evaluate(&steps)) - whole).abs() < 1e-6, "extended, the corner differs from one step");
}
