//! The randomness laid over the rule (issue 79).

use super::*;
use crate::primitive::ParamValue;
use simple3d_geom::Vec3;

/// Asked for as "a bit of noise, like for placing planks on a surface where
/// some randomness is needed": every copy nudged off where the rule put it,
/// by no more than the amount asked for, and the same nudge every time the
/// file is opened.
#[test]
pub(crate) fn noise_moves_every_copy_a_little_and_never_further_than_it_was_told() {
    let exact = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(40)),
        ("step_x", ParamValue::Length(10.0)),
    ]);
    let mut scattered = exact.clone();
    scattered.insert("noise_x".to_string(), ParamValue::Length(2.0));
    scattered.insert("noise_y".to_string(), ParamValue::Length(1.0));

    let (before, after) = (instances(&exact), instances(&scattered));
    assert_eq!(before.len(), after.len(), "the scatter changed how many copies there are");
    let mut moved = 0;
    for (was, now) in before.iter().zip(&after) {
        let off = now.xform.t - was.xform.t;
        assert!(off.x.abs() <= 2.0 + 1e-9, "a copy wandered {} mm along X, further than the 2 asked for", off.x);
        assert!(off.y.abs() <= 1.0 + 1e-9, "a copy wandered {} mm along Y, further than the 1 asked for", off.y);
        assert!(off.z.abs() < 1e-9, "a copy moved along an axis with no jitter on it");
        if off.length() > 1e-9 {
            moved += 1;
        }
    }
    assert!(moved >= 38, "only {moved} of 40 copies were nudged at all, which is not a scatter");

    // Deterministic: the same seed lays the same copies down, so a file opened
    // tomorrow is the file that was saved today.
    assert_eq!(instances(&scattered), after, "the same rule scattered differently the second time");

    // And the seed is what makes it a choice: the next number is a different
    // scatter of the same size.
    let mut reseeded = scattered.clone();
    reseeded.insert("noise_seed".to_string(), ParamValue::Count(2));
    assert_ne!(instances(&reseeded), after, "changing the seed changed nothing");
}

/// A pattern with no noise on it is the pattern it always was -- to the bit,
/// not to a tolerance.
#[test]
pub(crate) fn a_pattern_with_no_noise_is_left_exactly_where_the_rule_put_it() {
    for kind in 0..KINDS.len() as u32 {
        let mut params = default_params();
        params.insert("kind".to_string(), ParamValue::Choice(kind));
        let copies = instances(&params);
        assert!(!copies.is_empty());
        let mut asked = params.clone();
        asked.insert("noise_seed".to_string(), ParamValue::Count(7));
        assert_eq!(instances(&asked), copies, "kind {kind} moved for a seed with no jitter to go with it");
    }
}

/// The turn is about the copy's own middle, not about the centre of the
/// pattern: a plank is meant to sit askew where it lies, not to swing round
/// the whole deck.
#[test]
pub(crate) fn a_jittered_turn_spins_each_copy_where_it_stands() {
    let mut params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(12)),
        ("step_x", ParamValue::Length(50.0)),
    ]);
    params.insert("noise_turn_z".to_string(), ParamValue::Angle(6.0));
    let copies = instances(&params);
    for (index, copy) in copies.iter().enumerate() {
        assert!(
            (copy.xform.t - Vec3::new(50.0 * index as f64, 0.0, 0.0)).length() < 1e-9,
            "copy {index} was carried off its place by a turn that should have spun it there"
        );
        // Turned, and by no more than the six degrees asked for.
        let turned = copy.xform.axis_vector(0);
        let angle = turned.y.atan2(turned.x).to_degrees();
        assert!(angle.abs() <= 6.0 + 1e-9, "copy {index} turned {angle} degrees, past the 6 asked for");
    }
    assert!(
        copies.iter().any(|c| c.xform.axis_vector(0).y.abs() > 1e-6),
        "nothing turned at all, so the jitter did nothing"
    );
    assert_eq!(Noise::of(&params).turn, Vec3::new(0.0, 0.0, 6.0));
}

/// A jitter typed with a minus sign is a jitter of that size, not an error and
/// not nothing.
///
/// Asked for from the running application: the three distances refused a
/// negative number where every other distance in the panel takes one. They are
/// read either way round whatever the sign -- a scatter has no direction -- so
/// the sign costs nothing, and a field that silently snapped -2 back to zero
/// looked like a jitter that would not switch on.
#[test]
pub(crate) fn a_negative_jitter_scatters_exactly_as_far_as_the_positive_one() {
    let base = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(20)),
        ("step_x", ParamValue::Length(10.0)),
    ]);
    let mut positive = base.clone();
    positive.insert("noise_x".to_string(), ParamValue::Length(2.0));
    positive.insert("noise_turn_z".to_string(), ParamValue::Angle(6.0));
    let mut negative = base.clone();
    negative.insert("noise_x".to_string(), ParamValue::Length(-2.0));
    negative.insert("noise_turn_z".to_string(), ParamValue::Angle(-6.0));

    assert_eq!(Noise::of(&negative), Noise::of(&positive), "a minus sign changed what the scatter is");
    assert_eq!(instances(&negative), instances(&positive), "a minus sign moved the copies");
    assert_ne!(instances(&negative), instances(&base), "the negative jitter did nothing at all");
}

/// The original can be left exactly where it is while every copy after it
/// wanders -- the first plank against the wall, the part the rest are measured
/// from.
#[test]
pub(crate) fn the_original_can_be_left_in_place_while_the_copies_wander() {
    let mut params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(10)),
        ("step_x", ParamValue::Length(10.0)),
        ("noise_x", ParamValue::Length(2.0)),
    ]);
    let wandering = instances(&params);
    assert_ne!(wandering[0].xform, crate::xform::Xform::IDENTITY, "the seed happened to leave the original alone");

    params.insert("noise_keep_first".to_string(), ParamValue::Bool(true));
    let kept = instances(&params);
    assert_eq!(kept[0].xform, crate::xform::Xform::IDENTITY, "the original moved though it was to stay put");
    assert_eq!(kept[1..], wandering[1..], "keeping the original changed where the others went");
}

/// A size jitter makes each copy anything up to that much bigger or smaller,
/// and never more.
#[test]
pub(crate) fn a_size_jitter_resizes_every_copy_within_the_bound() {
    let params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(40)),
        ("step_x", ParamValue::Length(10.0)),
        ("noise_scale", ParamValue::Count(20)),
    ]);
    let sizes: Vec<f64> = instances(&params).iter().map(|c| c.xform.axis_vector(0).length()).collect();
    assert!(sizes.iter().all(|s| (0.8 - 1e-9..=1.2 + 1e-9).contains(s)), "a size fell outside 80..120 %: {sizes:?}");
    assert!(sizes.iter().any(|s| *s < 0.95) && sizes.iter().any(|s| *s > 1.05), "the sizes hardly varied");
    // Uniformly: a copy made bigger is bigger every way, not stretched.
    for copy in instances(&params) {
        let (x, y, z) = (copy.xform.axis_vector(0), copy.xform.axis_vector(1), copy.xform.axis_vector(2));
        assert!((x.length() - y.length()).abs() < 1e-12 && (y.length() - z.length()).abs() < 1e-12);
    }
}

/// A turn about each axis turns each copy about all three at once, each by its
/// own amount: a stone dropped on a path tilts every way, not only about Z.
#[test]
pub(crate) fn a_turn_about_every_axis_tilts_every_way() {
    let params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(20)),
        ("step_x", ParamValue::Length(50.0)),
        ("noise_turn_x", ParamValue::Angle(10.0)),
        ("noise_turn_y", ParamValue::Angle(10.0)),
        ("noise_turn_z", ParamValue::Angle(10.0)),
    ]);
    let copies = instances(&params);
    // A turn about Z alone leaves each copy's Z axis pointing straight up.
    assert!(copies.iter().any(|c| c.xform.axis_vector(2).z < 1.0 - 1e-6), "no copy tilted off Z");
    assert!(copies.iter().any(|c| c.xform.axis_vector(0).y.abs() > 1e-6), "no copy turned about Z");

    // About one axis only, by more about that one than the others are.
    let tilted = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(20)),
        ("step_x", ParamValue::Length(50.0)),
        ("noise_turn_x", ParamValue::Angle(10.0)),
    ]);
    for copy in instances(&tilted) {
        assert!((copy.xform.axis_vector(0) - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-9, "a turn about X moved X");
    }
}

/// A scatter saved while it had one turn and a choice of axis turns every copy
/// exactly as it did: the turn becomes the amount about that axis, or about
/// each of the three.
#[test]
pub(crate) fn a_scatter_from_when_it_had_one_turn_lands_every_copy_where_it_did() {
    let base =
        [("kind", ParamValue::Choice(LINEAR)), ("count", ParamValue::Count(12)), ("step_x", ParamValue::Length(40.0))];
    // What the one turn laid down, worked out the way it was: one channel for a
    // turn about one axis, three for all of them.
    let old_turn = |axis: usize, index: usize| -> Vec3 {
        let noise = Noise { offset: Vec3::ZERO, turn: Vec3::splat(8.0), scale: 0.0, seed: 1, keep_first: false };
        let about_one = Noise { turn: unit(axis) * 8.0, ..noise };
        let rotation = if axis == 3 { noise.wobble(index) } else { about_one.wobble(index) };
        rotation.axis_vector(0)
    };
    for axis in 0..4 {
        let mut old = with(&base);
        for key in NOISE_TURN_KEYS {
            old.remove(key);
        }
        old.insert("noise_turn".to_string(), ParamValue::Angle(8.0));
        old.insert("noise_axis".to_string(), ParamValue::Choice(axis as u32));
        let migrated = migrate_params(&old);
        assert!(!migrated.contains_key("noise_turn"), "the old turn was kept");
        let turned = instances(&migrated);
        for (index, copy) in turned.iter().enumerate() {
            let want = old_turn(axis, index);
            assert!((copy.xform.axis_vector(0) - want).length() < 1e-9, "copy {index} of a turn about {axis} moved");
        }
    }
}

/// The scatter says when it can make two copies meet: they would be welded
/// into one body, and a deck of planks a millimetre too generously scattered
/// stops being planks.
#[test]
pub(crate) fn a_scatter_that_can_close_the_gap_between_copies_says_so() {
    // 10 mm shapes 12 mm apart: 2 mm between them.
    let size = Vec3::new(10.0, 10.0, 10.0);
    let mut params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(5)),
        ("step_x", ParamValue::Length(12.0)),
    ]);
    assert_eq!(crowding(&params, size), None, "a pattern with no scatter was said to crowd");

    // Half a millimetre each way: two neighbours can close a millimetre of it.
    params.insert("noise_x".to_string(), ParamValue::Length(0.5));
    assert_eq!(crowding(&params, size), None);

    // A millimetre and a half each way can close three: more than there is.
    params.insert("noise_x".to_string(), ParamValue::Length(1.5));
    let crowded = crowding(&params, size).expect("a scatter wider than the gap was not caught");
    assert!((crowded.gap - 2.0).abs() < 1e-9 && (crowded.reach - 3.0).abs() < 1e-9, "{crowded:?}");

    // Jitter across the run does not close the gap along it.
    params.insert("noise_x".to_string(), ParamValue::Length(0.0));
    params.insert("noise_y".to_string(), ParamValue::Length(5.0));
    assert_eq!(crowding(&params, size), None, "a sideways scatter was said to close a gap along the run");

    // And copies that touch already are the rule's doing, not the scatter's.
    params.insert("step_x".to_string(), ParamValue::Length(10.0));
    params.insert("noise_x".to_string(), ParamValue::Length(1.0));
    assert_eq!(crowding(&params, size), None);
}
