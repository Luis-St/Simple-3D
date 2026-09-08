//! Several cuts together: what they come to, and how a stored plan reads.

use super::*;
use crate::primitives::box_mesh;
use crate::vec3::Vec3;

/// The whole of the second half of the feature: two cuts, on two axes,
/// leave the blocks their grids come to between them -- and they still add
/// back up to the shape they were cut from.
#[test]
pub(crate) fn two_cuts_leave_the_pieces_both_of_them_make() {
    // A 30 x 30 x 10 plate in 10 mm squares through Z is nine columns of
    // 10 x 10 x 10. The second cut runs across the first -- through X, so
    // its cells lie in the Y-Z plane -- and cuts each of those columns into
    // the four a 5 mm grid makes of a 10 x 10 face.
    let plan = SplitPlan {
        passes: vec![
            Tiling { size: 10.0, axis: 2, ..Tiling::default() },
            Tiling { size: 5.0, axis: 0, ..Tiling::default() },
        ],
    };
    let pieces = cut_plan(&box_mesh(30.0, 30.0, 10.0), &plan, &|| {}, &never).expect("nothing abandoned it");
    assert_eq!(pieces.len(), 36, "nine columns cut four ways each are thirty-six blocks");
    let total: f64 = pieces.iter().map(volume).sum();
    assert!((total - 9000.0).abs() < 1.0, "the blocks hold {total} mm3 of the 9000 they were cut from");
    for piece in &pieces {
        assert!((volume(piece) - 250.0).abs() < 1e-6, "a block came out at {} mm3", volume(piece));
    }
}

/// A pass that cannot cut what it is given must not lose it: the pieces of
/// the cut before it come through whole.
#[test]
pub(crate) fn a_cut_that_misses_leaves_the_pieces_it_was_given() {
    let plan = SplitPlan {
        passes: vec![
            Tiling { size: 10.0, axis: 2, ..Tiling::default() },
            Tiling { size: 100.0, axis: 0, ..Tiling::default() },
        ],
    };
    let pieces = cut_plan(&box_mesh(30.0, 30.0, 10.0), &plan, &|| {}, &never).expect("nothing abandoned it");
    assert_eq!(pieces.len(), 9);
    let total: f64 = pieces.iter().map(volume).sum();
    assert!((total - 9000.0).abs() < 1.0);
}

/// What the two cuts come to between them is what is counted and what is
/// refused -- one cut that is fine on its own and a second that multiplies
/// it past the limit is a split nobody can find the pieces of.
#[test]
pub(crate) fn a_plan_is_counted_and_refused_by_what_its_cuts_come_to_together() {
    let bounds = (Vec3::new(-50.0, -50.0, -5.0), Vec3::new(50.0, 50.0, 5.0));
    let one = Tiling { size: 5.0, axis: 2, ..Tiling::default() };
    let plan = SplitPlan { passes: vec![one, Tiling { axis: 0, ..one }] };
    assert!(one.refusal(bounds).is_none(), "one cut this size is fine on its own");
    assert!(plan.refusal(bounds).is_some(), "two of them are far past the limit and were let through");
    // Progress is measured against every cell that will be tried, which is
    // the first cut over the shape plus the second over each piece it left.
    let pair = SplitPlan {
        passes: vec![
            Tiling { size: 25.0, axis: 2, ..Tiling::default() },
            Tiling { size: 25.0, axis: 0, ..Tiling::default() },
        ],
    };
    let (first, both) = (planned(&pair.passes[0], bounds), pair.planned(bounds));
    assert_eq!(pair.work(bounds), first + both, "the bar would run at two speeds");
}

/// A split written before a split could be cut more than once says its one
/// tiling as an object, and there is no reason to lose it over a pair of
/// brackets.
#[test]
pub(crate) fn a_plan_reads_both_a_list_of_cuts_and_the_single_one_that_came_before_it() {
    let plan = SplitPlan { passes: vec![Tiling::default(), Tiling { axis: 0, ..Tiling::default() }] };
    let text = serde_json::to_string(&plan).expect("it writes");
    assert!(text.starts_with('['), "a plan is written as the list of cuts it is: {text}");
    assert_eq!(serde_json::from_str::<SplitPlan>(&text).expect("it reads"), plan);

    let one = Tiling { size: 12.0, ..Tiling::default() };
    let older = serde_json::to_string(&one).expect("it writes");
    assert_eq!(serde_json::from_str::<SplitPlan>(&older).expect("it reads"), SplitPlan::of(one));
}
