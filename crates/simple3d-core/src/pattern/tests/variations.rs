//! Several variations per stage, building up or repeating, and migration of older rules (issue 79).

use super::*;
use crate::primitive::ParamValue;
use simple3d_geom::Vec3;

/// The angle copy `copy` is turned to about Z, in degrees.
fn turned_to(copy: &Instance) -> f64 {
    let x = copy.xform.axis_vector(0);
    x.y.atan2(x.x).to_degrees()
}

/// Two shifts reaching different copies on one stage: every other copy along X, every third up Z.
#[test]
pub(crate) fn two_variations_of_one_kind_on_one_stage_add_up() {
    let staged = Stage::run(6, Vec3::new(0.0, 10.0, 0.0))
        .with(Variation::shift(0, 5.0).repeating(2))
        .with(Variation::shift(2, 3.0).repeating(3));
    let params = rule(std::slice::from_ref(&staged));
    assert_eq!(stage(&params, 0), staged, "the two shifts did not survive being written and read back");
    for (i, copy) in instances(&params).iter().enumerate() {
        let x = if i % 2 == 1 { 5.0 } else { 0.0 };
        let z = if i % 3 == 1 { 3.0 } else { 0.0 };
        let want = Vec3::new(x, 10.0 * i as f64, z);
        assert!(near(copy.xform.t, want), "copy {i} is at {:?}, not {want:?}", copy.xform.t);
    }
}

/// "Every" can reach the original: every one from the first reaches all copies, every other one
/// from the first reaches the original and every other copy.
#[test]
pub(crate) fn a_variation_can_reach_every_copy_and_the_original() {
    let run = || Stage::run(4, Vec3::new(10.0, 0.0, 0.0));
    let lift = |every: u32, start: u32| {
        instances(&rule(&[run().with(Variation::shift(2, 1.0).repeating(1).reaching(every, start))]))
            .iter()
            .map(|c| c.xform.t.z)
            .collect::<Vec<f64>>()
    };
    assert_eq!(lift(1, 1), vec![1.0, 1.0, 1.0, 1.0], "every copy from the first missed one");
    assert_eq!(lift(2, 1), vec![1.0, 0.0, 1.0, 0.0], "every other copy from the first missed the original");
    assert_eq!(lift(2, 2), vec![0.0, 1.0, 0.0, 1.0]);
    assert_eq!(lift(3, 2), vec![0.0, 1.0, 0.0, 0.0], "every third copy from the second reached another");

    let climbing = run().with(Variation::shift(2, 1.0).reaching(2, 1));
    let zs: Vec<f64> = instances(&rule(&[climbing])).iter().map(|c| c.xform.t.z).collect();
    assert_eq!(zs, vec![1.0, 0.0, 2.0, 0.0]);
}

/// Every kind can build up or repeat: a flipping spin, an alternating size, paired gaps.
#[test]
pub(crate) fn every_kind_of_variation_can_build_up_or_repeat() {
    let run = || Stage::run(4, Vec3::new(10.0, 0.0, 0.0));

    let flipped = instances(&rule(&[run().with(Variation::spin(2, 180.0).repeating(2))]));
    for (i, copy) in flipped.iter().enumerate() {
        let want = if i % 2 == 0 { 0.0 } else { 180.0 };
        assert!((turned_to(copy).abs() - want).abs() < 1e-9, "copy {i} is turned {}", turned_to(copy));
    }

    let sizes: Vec<f64> = instances(&rule(&[run().with(Variation::resize(ALL_AXES, 0.5).repeating(2))]))
        .iter()
        .map(|c| c.xform.axis_vector(0).length())
        .collect();
    for (got, want) in sizes.iter().zip([1.0, 0.5, 1.0, 0.5]) {
        assert!((got - want).abs() < 1e-12, "sizes came out {sizes:?}");
    }

    let paired = Stage::run(5, Vec3::new(10.0, 0.0, 0.0)).with(Variation::widen(5.0).repeating(2));
    let xs: Vec<f64> = instances(&rule(&[paired])).iter().map(|c| c.xform.t.x).collect();
    // Gaps of 10, 15, 10, 15: the gap after every other copy from the second is wider.
    assert_eq!(xs, vec![0.0, 10.0, 25.0, 35.0, 50.0]);

    let climbing = run().with(Variation::shift(2, 2.0));
    let zs: Vec<f64> = instances(&rule(&[climbing])).iter().map(|c| c.xform.t.z).collect();
    assert_eq!(zs, vec![0.0, 2.0, 4.0, 6.0], "a shift that builds up did not build up");
}

/// A size along one axis only: a plank growing longer keeps its width and thickness.
#[test]
pub(crate) fn a_size_can_stretch_the_copies_along_one_axis() {
    let longer = Stage::run(3, Vec3::new(0.0, 10.0, 0.0)).with(Variation::resize(0, 1.5));
    let copies = instances(&rule(&[longer]));
    let x = copies[2].xform.axis_vector(0).length();
    let y = copies[2].xform.axis_vector(1).length();
    assert!((x - 2.25).abs() < 1e-12 && (y - 1.0).abs() < 1e-12, "the third copy is {x} by {y}");
}

/// Variations apply in list order: a shift before a spin moves then turns; after, it follows
/// the new facing.
#[test]
pub(crate) fn variations_are_applied_in_the_order_they_are_listed() {
    let shift = Variation::shift(0, 10.0);
    let spin = Variation::spin(2, 90.0);
    let first = instances(&rule(&[Stage::run(2, Vec3::ZERO).with(shift).with(spin)]))[1];
    let second = instances(&rule(&[Stage::run(2, Vec3::ZERO).with(spin).with(shift)]))[1];
    assert!(near(first.xform.t, Vec3::new(10.0, 0.0, 0.0)), "shift then spin put the copy at {:?}", first.xform.t);
    assert!(near(second.xform.t, Vec3::new(0.0, 10.0, 0.0)), "spin then shift put the copy at {:?}", second.xform.t);
}

/// A stage accepts distinct variations, refuses duplicates, and keeps order when one is removed.
#[test]
pub(crate) fn variations_are_added_while_they_differ_and_dropped_from_anywhere() {
    let mut params = rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0))]);
    let list = [
        Variation::shift(0, 1.0),
        Variation::shift(1, 1.0),
        Variation::shift(0, 1.0).repeating(2),
        Variation::shift(0, 1.0).repeating(2).reaching(2, 1),
        Variation::spin(2, 5.0),
        Variation::resize(ALL_AXES, 0.8),
        Variation::widen(1.0),
    ];
    for variation in list {
        assert!(add_variation(&mut params, 0, variation), "{variation:?} was refused");
    }
    assert!(!add_variation(&mut params, 0, Variation::shift(0, 7.0)), "a second shift just like the first was taken");
    assert_eq!(variation_count(&params, 0), list.len());

    remove_variation(&mut params, 0, 1);
    assert_eq!(stage(&params, 0).variations(), [&list[..1], &list[2..]].concat());
    assert!(
        !params.contains_key(&vary_key(0, list.len() - 1, VaryField::Gap)),
        "the slot left behind still held the last variation"
    );
    remove_variation(&mut params, 0, 70);
    assert_eq!(variation_count(&params, 0), list.len() - 1, "dropping a slot nobody has dropped one that is");
}

/// The limit is one variation per kind, axis, stepping and reach, so a one-copy stage holds six
/// shifts and no seventh.
#[test]
pub(crate) fn a_stage_holds_every_different_variation_its_copies_allow() {
    let size = Vec3::new(10.0, 10.0, 10.0);
    let mut params = rule(&[Stage::run(1, Vec3::ZERO)]);
    assert_eq!(combinations(Vary::Shift, 1), 6);
    for added in 0..6 {
        let fresh = fresh_variation(&params, 0, Vary::Shift, size)
            .unwrap_or_else(|| panic!("no shift offered after {added} of them"));
        assert!(add_variation(&mut params, 0, fresh), "the offered shift was one the stage held");
    }
    assert_eq!(fresh_variation(&params, 0, Vary::Shift, size), None, "a seventh shift was offered");
    assert!(!has_room_for(&stage(&params, 0), Vary::Shift));
    assert!(has_room_for(&stage(&params, 0), Vary::Spin), "the shifts used up the spins' room");

    // More copies, more to reach: a stage of four has room again.
    params.insert("stage1_count".to_string(), ParamValue::Count(4));
    assert!(has_room_for(&stage(&params, 0), Vary::Shift));
}

/// A new variation is visible immediately, sized to the shape and stage; with a duplicate present
/// the chip offers the next one along.
#[test]
pub(crate) fn a_fresh_variation_is_sized_to_the_stage_it_is_added_to() {
    let size = Vec3::new(10.0, 20.0, 5.0);
    let mut params = rule(&[Stage::run(3, Vec3::new(15.0, 0.0, 0.0)), Stage::run(2, Vec3::new(0.0, 30.0, 0.0))]);
    let stagger = fresh_variation(&params, 1, Vary::Shift, size).unwrap();
    assert!(
        stagger.repeats && stagger.every == 2 && stagger.start == 2,
        "a fresh shift does not stagger every other copy"
    );
    assert_eq!((stagger.axis, stagger.amount), (0, 7.5), "the rows were staggered by {stagger:?}");
    let zigzag = fresh_variation(&params, 0, Vary::Shift, size).unwrap();
    assert_eq!((zigzag.axis, zigzag.amount), (1, 10.0), "a lone run zigzagged by {zigzag:?}");
    for what in Vary::ALL {
        assert!(
            fresh_variation(&params, 0, what, size).unwrap().acts(StageMode::Move),
            "a fresh {what:?} does nothing"
        );
    }
    assert_eq!(fresh_variation(&params, 0, Vary::Gap, size).unwrap().amount, 15.0 * 0.25);

    add_variation(&mut params, 1, stagger);
    let next = fresh_variation(&params, 1, Vary::Shift, size).unwrap();
    assert_ne!(next.combination(), stagger.combination(), "the chip offered the stagger the stage already has");
    assert_eq!((next.what, next.axis), (Vary::Shift, 1), "the next shift is not along the next axis");
}

/// A repeating gap is judged at its narrowest: paired copies are closest within each pair.
#[test]
pub(crate) fn a_scatter_is_warned_about_at_the_narrowest_gap_a_cycle_leaves() {
    let size = Vec3::new(15.0, 5.0, 5.0);
    let paired = Stage::run(4, Vec3::new(30.0, 0.0, 0.0)).with(Variation::widen(-10.0).repeating(2));
    let mut params = rule(&[paired]);
    params.insert("noise_x".to_string(), ParamValue::Length(3.0));
    let crowded = crowding(&params, size).expect("the pairs' narrow gap can be closed");
    assert!((crowded.gap - 5.0).abs() < 1e-9, "the narrowest gap was taken as {}", crowded.gap);

    let even = rule(&[Stage::run(4, Vec3::new(30.0, 0.0, 0.0))]);
    let mut even = even;
    even.insert("noise_x".to_string(), ParamValue::Length(3.0));
    assert_eq!(crowding(&even, size), None, "an even run with room to spare was said to crowd");
}

/// Every variation number is a labelled parameter whose key parses back.
#[test]
pub(crate) fn every_variation_key_is_a_parameter_with_a_label_of_its_own() {
    let mut labels: Vec<&'static str> = Vec::new();
    for index in 0..MAX_STAGES {
        for key in variation_keys(index, 12) {
            let spec = param_spec(&key).unwrap_or_else(|| panic!("{key} is not a parameter"));
            assert_eq!(spec.key, key);
            assert!(parse_vary_key(&key).is_some(), "{key} does not read back");
            // Choices carry no value field, so only numbers need their own name.
            if !matches!(spec.kind, crate::primitive::ParamKind::Choice { .. }) {
                assert!(!labels.contains(&spec.label), "two numbers answer to {}", spec.label);
                labels.push(spec.label);
            }
        }
    }
    assert_eq!(parse_vary_key("stage1_vary1_x"), None, "an old slot's name was read as a new one");
    assert_eq!(parse_vary_key("stage5_var1_what"), None, "a fifth stage's name was read");
}
