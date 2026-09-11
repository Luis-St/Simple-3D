//! More than one variation on a stage, each building up or coming round, and
//! the rules from before a stage could hold more than one (issue 79).

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

fn rule(stages: &[Stage]) -> Params {
    let mut params = with(&[("kind", ParamValue::Choice(CUSTOM))]);
    for (index, stage) in stages.iter().enumerate() {
        set_stage(&mut params, index, *stage);
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
/// only one section. Two shifts on two cycles is the case one section could
/// not say -- every other copy moved along X, every third lifted up Z.
#[test]
pub(crate) fn two_variations_of_one_kind_on_one_stage_add_up() {
    let staged = Stage::run(6, Vec3::new(0.0, 10.0, 0.0))
        .with(Variation::shift(Vec3::new(5.0, 0.0, 0.0)).repeating(2))
        .with(Variation::shift(Vec3::new(0.0, 0.0, 3.0)).repeating(3));
    let params = rule(&[staged]);
    assert_eq!(stage(&params, 0), staged, "the two shifts did not survive being written and read back");
    for (i, copy) in instances(&params).iter().enumerate() {
        let want = Vec3::new(5.0 * (i % 2) as f64, 10.0 * i as f64, 3.0 * (i % 3) as f64);
        assert!(near(copy.xform.t, want), "copy {i} is at {:?}, not {want:?}", copy.xform.t);
    }
}

/// Every kind steps either way: building up copy by copy, or coming round on
/// a cycle -- a spin that flips every other copy, a size that alternates, a gap
/// that pairs the copies off.
#[test]
pub(crate) fn every_kind_of_variation_can_build_up_or_repeat() {
    let run = || Stage::run(4, Vec3::new(10.0, 0.0, 0.0));

    let flipped = instances(&rule(&[run().with(Variation::spin(180.0, 2).repeating(2))]));
    for (i, copy) in flipped.iter().enumerate() {
        let want = if i % 2 == 0 { 0.0 } else { 180.0 };
        assert!((turned_to(copy).abs() - want).abs() < 1e-9, "copy {i} is turned {}", turned_to(copy));
    }

    let sizes: Vec<f64> = instances(&rule(&[run().with(Variation::resize(0.5).repeating(2))]))
        .iter()
        .map(|c| c.xform.axis_vector(0).length())
        .collect();
    for (got, want) in sizes.iter().zip([1.0, 0.5, 1.0, 0.5]) {
        assert!((got - want).abs() < 1e-12, "sizes came out {sizes:?}");
    }

    let paired = Stage::run(5, Vec3::new(10.0, 0.0, 0.0)).with(Variation::widen(5.0).repeating(2));
    let xs: Vec<f64> = instances(&rule(&[paired])).iter().map(|c| c.xform.t.x).collect();
    // Gaps of 10, 15, 10, 15.
    assert_eq!(xs, vec![0.0, 10.0, 25.0, 35.0, 50.0]);

    let climbing = run().with(Variation::shift(Vec3::new(0.0, 0.0, 2.0)));
    let zs: Vec<f64> = instances(&rule(&[climbing])).iter().map(|c| c.xform.t.z).collect();
    assert_eq!(zs, vec![0.0, 2.0, 4.0, 6.0], "a shift that builds up did not build up");
}

/// Variations are applied in the order they are listed: a shift before a spin
/// moves the copy and turns it where it landed; after it, the shift goes the
/// way the copy now faces.
#[test]
pub(crate) fn variations_are_applied_in_the_order_they_are_listed() {
    let shift = Variation::shift(Vec3::new(10.0, 0.0, 0.0));
    let spin = Variation::spin(90.0, 2);
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
    for (index, k) in STAGES.iter().enumerate() {
        old.remove(k.varied);
        for key in vary_keys(index) {
            old.remove(key);
        }
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
            Variation::shift(Vec3::new(5.0, 0.0, 0.0)).repeating(3),
            Variation::spin(10.0, 2),
            Variation::resize(0.9),
        ],
        "the old stage's one section did not become its list"
    );
    assert_eq!(stage(&migrated, 1).variations(), [Variation::spin(5.0, 2)], "a turn was given a gap it never had");

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

/// A list takes four, the fifth is refused, and one taken from the middle
/// leaves the others in their order.
#[test]
pub(crate) fn variations_are_added_up_to_the_limit_and_dropped_from_anywhere() {
    let mut params = rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0))]);
    let list = [
        Variation::shift(Vec3::new(1.0, 0.0, 0.0)),
        Variation::spin(5.0, 2),
        Variation::resize(0.8),
        Variation::widen(1.0),
    ];
    for variation in list {
        assert!(add_variation(&mut params, 0, variation));
    }
    assert!(!add_variation(&mut params, 0, Variation::spin(1.0, 0)), "a fifth variation was taken");
    assert_eq!(variation_count(&params, 0), MAX_VARIATIONS);

    remove_variation(&mut params, 0, 1);
    assert_eq!(stage(&params, 0).variations(), [list[0], list[2], list[3]]);
    assert_eq!(params.int("stage1_vary4_steps"), 0);
    assert_eq!(params.num("stage1_vary4_gap"), 0.0, "the slot left behind still held the last variation");
    remove_variation(&mut params, 0, 7);
    assert_eq!(variation_count(&params, 0), 3, "dropping a slot that is not in use dropped one that is");
}

/// A variation that is added shows what it does straight away, sized to the
/// shape and to the stage it is on.
#[test]
pub(crate) fn a_fresh_variation_is_sized_to_the_stage_it_is_added_to() {
    let size = Vec3::new(10.0, 20.0, 5.0);
    let params = rule(&[Stage::run(3, Vec3::new(15.0, 0.0, 0.0)), Stage::run(2, Vec3::new(0.0, 30.0, 0.0))]);
    let stagger = fresh_variation(&params, 1, Vary::Shift, size);
    assert!(stagger.repeats && stagger.every == 2, "a fresh shift does not stagger every other copy");
    assert!(near(stagger.offset, Vec3::new(7.5, 0.0, 0.0)), "the rows were staggered by {:?}", stagger.offset);
    let zigzag = fresh_variation(&params, 0, Vary::Shift, size);
    assert!(near(zigzag.offset, Vec3::new(0.0, 10.0, 0.0)), "a lone run zigzagged by {:?}", zigzag.offset);
    for what in Vary::ALL {
        assert!(fresh_variation(&params, 0, what, size).acts(StageMode::Move), "a fresh {what:?} does nothing");
    }
    assert_eq!(fresh_variation(&params, 0, Vary::Gap, size).gap, 15.0 * 0.25);
}

/// A gap that comes round on a cycle is judged at its narrowest, not at its
/// first: copies paired off stand closest inside each pair.
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

/// Every stage key the table names is a parameter, variation slots included.
#[test]
pub(crate) fn every_variation_key_is_a_parameter_with_a_label_of_its_own() {
    let mut labels: Vec<&'static str> = Vec::new();
    for index in 0..MAX_STAGES {
        for key in vary_keys(index) {
            let spec = PARAMS.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("{key} is not a parameter"));
            // Choices carry no value field, so only numbers need a name of
            // their own.
            if !matches!(spec.kind, crate::primitive::ParamKind::Choice { .. }) {
                assert!(!labels.contains(&spec.label), "two numbers answer to {}", spec.label);
                labels.push(spec.label);
            }
        }
    }
    assert_eq!(vary_keys(0).len(), MAX_VARIATIONS * VARY_KEY_COUNT);
}
