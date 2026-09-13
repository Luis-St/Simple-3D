//! More than one variation on a stage, each building up or repeating over the
//! copies it reaches, and the rules from before a stage held such a list
//! (issue 79).

use super::*;
use crate::primitive::{ParamValue, Params};
use simple3d_geom::Vec3;

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

fn rule(stages: &[Stage]) -> Params {
    let mut params = with(&[("kind", ParamValue::Choice(CUSTOM))]);
    for (index, stage) in stages.iter().enumerate() {
        set_stage(&mut params, index, stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len() as u32));
    params
}

/// The angle copy `copy` is turned to about Z, in degrees.
fn turned_to(copy: &Instance) -> f64 {
    let x = copy.xform.axis_vector(0);
    x.y.atan2(x.x).to_degrees()
}

/// Asked for from the running application: a stage could vary its copies in
/// only one section. Two shifts reaching different copies is the case one
/// section could not say -- every other copy moved along X, every third lifted
/// up Z.
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

/// Asked for from the running application: "every" could not reach the
/// original, nor every copy. Every one from the first reaches all of them;
/// every other one from the first reaches the original and every other copy
/// after it.
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

    // Building up, the copies it reaches get a step more each time and the ones
    // between them get none.
    let climbing = run().with(Variation::shift(2, 1.0).reaching(2, 1));
    let zs: Vec<f64> = instances(&rule(&[climbing])).iter().map(|c| c.xform.t.z).collect();
    assert_eq!(zs, vec![1.0, 0.0, 2.0, 0.0]);
}

/// Every kind steps either way: building up copy by copy, or repeating on the
/// copies it reaches -- a spin that flips every other copy, a size that
/// alternates, a gap that pairs the copies off.
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

/// A size is along one axis or all three: a plank that grows longer copy by
/// copy stays as wide and as thick as the first.
#[test]
pub(crate) fn a_size_can_stretch_the_copies_along_one_axis() {
    let longer = Stage::run(3, Vec3::new(0.0, 10.0, 0.0)).with(Variation::resize(0, 1.5));
    let copies = instances(&rule(&[longer]));
    let x = copies[2].xform.axis_vector(0).length();
    let y = copies[2].xform.axis_vector(1).length();
    assert!((x - 2.25).abs() < 1e-12 && (y - 1.0).abs() < 1e-12, "the third copy is {x} by {y}");
}

/// Variations are applied in the order they are listed: a shift before a spin
/// moves the copy and turns it where it landed; after it, the shift goes the
/// way the copy now faces.
#[test]
pub(crate) fn variations_are_applied_in_the_order_they_are_listed() {
    let shift = Variation::shift(0, 10.0);
    let spin = Variation::spin(2, 90.0);
    let first = instances(&rule(&[Stage::run(2, Vec3::ZERO).with(shift).with(spin)]))[1];
    let second = instances(&rule(&[Stage::run(2, Vec3::ZERO).with(spin).with(shift)]))[1];
    assert!(near(first.xform.t, Vec3::new(10.0, 0.0, 0.0)), "shift then spin put the copy at {:?}", first.xform.t);
    assert!(near(second.xform.t, Vec3::new(0.0, 10.0, 0.0)), "spin then shift put the copy at {:?}", second.xform.t);
}

/// A rule saved while a stage carried one of each -- a growing gap, a shift on
/// a cycle, a spin and a size -- comes back as a list of variations that lays
/// the copies down where they always were.
#[test]
pub(crate) fn a_stage_from_before_it_held_a_list_keeps_what_it_varied() {
    let mut old = rule(&[Stage::run(4, Vec3::new(10.0, 0.0, 0.0)), Stage::turning(3, 30.0, 20.0, 0.0, 0.0, 2)]);
    for k in &STAGES {
        old.remove(k.variations);
    }
    for (key, value) in [
        ("stage1_gap_growth", ParamValue::Length(2.0)),
        ("stage1_shift_x", ParamValue::Length(5.0)),
        ("stage1_shift_every", ParamValue::Count(3)),
        ("stage1_spin", ParamValue::Angle(10.0)),
        ("stage1_scale", ParamValue::Count(90)),
        // A turn never had gaps: this one must not become a variation.
        ("stage2_gap_growth", ParamValue::Length(4.0)),
        ("stage2_spin", ParamValue::Angle(5.0)),
    ] {
        old.insert(key.to_string(), value);
    }
    let migrated = migrate_params(&old);
    let run = stage(&migrated, 0);
    assert_eq!(
        run.variations(),
        [
            Variation::widen(2.0),
            // A cycle of three gave nought, one and two steps: two variations,
            // one reaching the copies it gave one step and one the copies it
            // gave two.
            Variation::shift(0, 5.0).repeating(3).reaching(3, 2),
            Variation::shift(0, 10.0).repeating(3).reaching(3, 3),
            Variation::spin(2, 10.0),
            Variation::resize(ALL_AXES, 0.9),
        ],
        "the old stage's one section did not become its list"
    );
    assert_eq!(stage(&migrated, 1).variations(), [Variation::spin(2, 5.0)], "a turn was given a gap it never had");

    // Copy 2 of the run: two steps and one growth along, shifted two fifths of
    // its cycle on, and spun and shrunk twice where it stands.
    let copy = run.place(2);
    assert!(near(copy.t, Vec3::new(22.0 + 10.0, 0.0, 0.0)), "copy 2 landed at {:?}", copy.t);
    assert!((copy.axis_vector(0).length() - 0.81).abs() < 1e-12);
    let x = copy.axis_vector(0);
    assert!((x.y.atan2(x.x).to_degrees() - 20.0).abs() < 1e-9);
    // Migrated once, a rule is not migrated again.
    assert_eq!(migrate_params(&migrated), migrated);
}

/// A rule saved while a stage had four variation slots -- a shift as a vector,
/// cycles that always left the original alone -- lays its copies down where it
/// did, and loses the slots' old names on the way.
#[test]
pub(crate) fn a_stage_from_when_it_had_four_slots_keeps_its_copies_where_they_were() {
    let mut old = rule(&[Stage::run(5, Vec3::new(10.0, 0.0, 0.0))]);
    old.remove("stage1_variations");
    for (key, value) in [
        ("stage1_varied", ParamValue::Count(2)),
        ("stage1_vary1_what", ParamValue::Choice(0)),
        ("stage1_vary1_steps", ParamValue::Choice(1)),
        ("stage1_vary1_every", ParamValue::Count(3)),
        ("stage1_vary1_x", ParamValue::Length(1.0)),
        ("stage1_vary1_z", ParamValue::Length(2.0)),
        ("stage1_vary2_what", ParamValue::Choice(3)),
        ("stage1_vary2_steps", ParamValue::Choice(0)),
        ("stage1_vary2_gap", ParamValue::Length(4.0)),
    ] {
        old.insert(key.to_string(), value);
    }
    let migrated = migrate_params(&old);
    assert!(!migrated.keys().any(|key| key.contains("_vary")), "the four slots' old names were kept");
    // Copy i was shifted (i mod 3) steps, and its gaps grew by 4 a copy.
    let copies = instances(&migrated);
    for (i, copy) in copies.iter().enumerate() {
        let cycle = (i % 3) as f64;
        let along = 10.0 * i as f64 + 4.0 * (i * i.saturating_sub(1) / 2) as f64;
        let want = Vec3::new(along + cycle, 0.0, 2.0 * cycle);
        assert!(near(copy.xform.t, want), "copy {i} is at {:?}, not {want:?}", copy.xform.t);
    }
    assert_eq!(migrate_params(&migrated), migrated);
}

/// A stage takes as many variations as there are different ones, refuses a
/// second one just like one it holds, and one taken from the middle leaves the
/// others in their order.
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

/// The limit is the stage's own: one variation of a kind for each axis, way of
/// stepping and set of copies it can reach -- so a stage of one copy holds six
/// shifts, and a seventh is nowhere to be found.
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

    // More copies, more to reach: the same stage of four has room again.
    params.insert("stage1_count".to_string(), ParamValue::Count(4));
    assert!(has_room_for(&stage(&params, 0), Vary::Shift));
}

/// A variation that is added shows what it does straight away, sized to the
/// shape and to the stage it is on -- and where the stage holds one just like
/// it, the chip gives the next one along rather than nothing.
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

/// A gap that repeats is judged at its narrowest, not at its first: copies
/// paired off stand closest inside each pair.
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

/// Every variation's numbers are parameters with a label of their own, however
/// many slots a stage has, and name themselves back.
#[test]
pub(crate) fn every_variation_key_is_a_parameter_with_a_label_of_its_own() {
    let mut labels: Vec<&'static str> = Vec::new();
    for index in 0..MAX_STAGES {
        for key in variation_keys(index, 12) {
            let spec = param_spec(&key).unwrap_or_else(|| panic!("{key} is not a parameter"));
            assert_eq!(spec.key, key);
            assert!(parse_vary_key(&key).is_some(), "{key} does not read back");
            // Choices carry no value field, so only numbers need a name of
            // their own.
            if !matches!(spec.kind, crate::primitive::ParamKind::Choice { .. }) {
                assert!(!labels.contains(&spec.label), "two numbers answer to {}", spec.label);
                labels.push(spec.label);
            }
        }
    }
    assert_eq!(parse_vary_key("stage1_vary1_x"), None, "an old slot's name was read as a new one");
    assert_eq!(parse_vary_key("stage5_var1_what"), None, "a fifth stage's name was read");
}
