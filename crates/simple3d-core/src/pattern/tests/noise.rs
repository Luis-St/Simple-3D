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
    params.insert("noise_turn".to_string(), ParamValue::Angle(6.0));
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
    assert_eq!(Noise::of(&params).turn, 6.0);
}
