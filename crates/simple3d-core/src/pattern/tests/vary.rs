//! A stage that changes its copies from one to the next, the layouts that
//! ship ready made, and editing the stack of stages (issue 79).

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

/// A rule of the given stages, and nothing else.
fn rule(stages: &[Stage]) -> Params {
    let mut params = with(&[("kind", ParamValue::Choice(CUSTOM))]);
    for (index, stage) in stages.iter().enumerate() {
        set_stage(&mut params, index, stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len() as u32));
    params
}

/// The planks the issue asked for: rows end to end, every other row moved on
/// by half a plank. Nothing a stage could say while each copy was a fixed step
/// further along than the last.
#[test]
pub(crate) fn a_shift_every_other_copy_staggers_the_rows() {
    let rows = Stage::run(3, Vec3::new(0.0, 4.0, 0.0)).with(Variation::shift(0, 5.0).repeating(2));
    let copies = instances(&rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0)), rows]));
    assert_eq!(copies.len(), 9);
    let at = |index: usize| copies[index].xform.t;
    // The first row where the run put it, the second half a plank on, the
    // third back in line with the first.
    assert!(near(at(0), Vec3::ZERO) && near(at(2), Vec3::new(20.0, 0.0, 0.0)));
    assert!(near(at(3), Vec3::new(5.0, 4.0, 0.0)), "the second row was not shifted: {:?}", at(3));
    assert!(near(at(5), Vec3::new(25.0, 4.0, 0.0)));
    assert!(near(at(6), Vec3::new(0.0, 8.0, 0.0)), "the shift did not come round again: {:?}", at(6));

    // Every third row from the second.
    let thirds = Stage::run(5, Vec3::new(0.0, 4.0, 0.0)).with(Variation::shift(0, 3.0).repeating(3));
    let xs: Vec<f64> = instances(&rule(&[thirds])).iter().map(|c| c.xform.t.x).collect();
    assert_eq!(xs, vec![0.0, 3.0, 0.0, 0.0, 3.0]);
}

/// Each gap of a run wider than the one before it: the copies spread out
/// rather than standing a fixed step apart.
#[test]
pub(crate) fn a_growing_gap_spreads_the_copies_out() {
    let run = Stage::run(4, Vec3::new(10.0, 0.0, 0.0)).with(Variation::widen(2.0));
    let xs: Vec<f64> = instances(&rule(&[run])).iter().map(|c| c.xform.t.x).collect();
    // Gaps of 10, 12 and 14.
    assert_eq!(xs, vec![0.0, 10.0, 22.0, 36.0]);

    // Along whatever way the run points, not along X.
    let diagonal = Stage::run(3, Vec3::new(3.0, 4.0, 0.0)).with(Variation::widen(2.0));
    let last = instances(&rule(&[diagonal]))[2].xform.t;
    assert!(near(last, Vec3::new(3.0, 4.0, 0.0) * (12.0 / 5.0)), "the growth left the run's line: {last:?}");
}

/// Spin turns each copy where it stands, further for every copy, and leaves
/// it where the run put it.
#[test]
pub(crate) fn spin_turns_each_copy_where_it_stands() {
    let run = Stage::run(3, Vec3::new(10.0, 0.0, 0.0)).with(Variation::spin(2, 30.0));
    let copies = instances(&rule(&[run]));
    for (index, copy) in copies.iter().enumerate() {
        assert!(near(copy.xform.t, Vec3::new(10.0 * index as f64, 0.0, 0.0)), "copy {index} was carried off");
        let x = copy.xform.axis_vector(0);
        let angle = x.y.atan2(x.x).to_degrees();
        assert!((angle - 30.0 * index as f64).abs() < 1e-9, "copy {index} turned {angle} degrees");
    }
}

/// Each copy a fixed fraction of the size of the one before it.
#[test]
pub(crate) fn size_per_copy_multiplies_from_one_copy_to_the_next() {
    let run = Stage::run(3, Vec3::new(10.0, 0.0, 0.0)).with(Variation::resize(ALL_AXES, 0.5));
    let params = rule(&[run]);
    assert_eq!(params.int("stage1_var1_size"), 50, "the size was not written as a percentage");
    let sizes: Vec<f64> = instances(&params).iter().map(|c| c.xform.axis_vector(0).length()).collect();
    for (got, want) in sizes.iter().zip([1.0, 0.5, 0.25]) {
        assert!((got - want).abs() < 1e-12, "sizes came out {sizes:?}");
    }
    assert!(instances(&params).iter().all(|c| !c.mirrored), "a smaller copy is not a reflection");
}

/// A stage that varies nothing composes nothing: its copies are the plain
/// run they always were, to the bit -- whatever the cycle says, since a cycle
/// with no shift to cycle is a number nobody has used.
#[test]
pub(crate) fn a_stage_that_varies_nothing_is_the_run_it_always_was() {
    let run = Stage::run(4, Vec3::new(7.0, 1.0, 0.0)).with(Variation::shift(0, 0.0).repeating(5));
    assert!(!run.varies());
    for (index, copy) in instances(&rule(std::slice::from_ref(&run))).iter().enumerate() {
        assert_eq!(copy.xform, crate::xform::Xform::from_translation(Vec3::new(7.0, 1.0, 0.0) * index as f64));
    }
    assert!(run.clone().with(Variation::spin(2, 1.0)).varies());
    assert!(run.clone().with(Variation::widen(1.0)).varies());
    let turn = Stage::turning(3, 10.0, 5.0, 0.0, 0.0, 2);
    assert!(!turn.with(Variation::widen(1.0)).varies(), "a turn has no gaps");
    assert!(
        !Stage::mirrored(0).with(Variation::resize(ALL_AXES, 0.5)).varies(),
        "a mirror is two copies nothing varies"
    );
}

/// A file from before a stage could vary lays out exactly as it did: every one
/// of the new numbers comes in as "no change".
#[test]
pub(crate) fn a_rule_from_before_the_stages_could_vary_lays_out_the_same_copies() {
    let mut old = rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0)), Stage::turning(4, 90.0, 30.0, 0.0, 0.0, 2)]);
    let laid_out = instances(&old);
    for k in &STAGES {
        old.remove(k.variations);
    }
    assert_eq!(instances(&migrate_params(&old)), laid_out);
    // And read straight off the map without the migration, a stage with no
    // variations says so rather than reading slots nobody filled in.
    assert_eq!(instances(&old), laid_out, "a stage missing its size shrank its copies");
}

/// A variation's numbers are offered only while the stage has it, and only the
/// ones its own kind reads: numbers that mean nothing until another is set are
/// not on screen until then.
#[test]
pub(crate) fn the_numbers_that_vary_a_stage_are_offered_where_they_mean_something() {
    let shown = |params: &Params| -> Vec<String> {
        let mut keys: Vec<String> = PARAMS.iter().map(|p| p.key.to_string()).collect();
        keys.extend(variation_keys(0, 3));
        keys.into_iter().filter(|key| param_visible(param_spec(key).unwrap(), params)).collect()
    };
    let has = |keys: &[String], key: &str| keys.iter().any(|k| k == key);
    let run = rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0))]);
    assert!(has(&shown(&run), "stage1_variations"));
    assert!(!shown(&run).iter().any(|key| key.starts_with("stage1_var1")), "a stage varying nothing showed a slot");
    assert!(!has(&shown(&run), "stage1_axis"), "a run was offered an axis it has no use for");

    let shifted = rule(&[Stage::run(3, Vec3::ZERO).with(Variation::shift(0, 1.0))]);
    for key in ["stage1_var1_what", "stage1_var1_steps", "stage1_var1_every", "stage1_var1_start", "stage1_var1_axis"] {
        assert!(has(&shown(&shifted), key), "a shift was not offered {key}");
    }
    assert!(has(&shown(&shifted), "stage1_var1_shift"));
    for key in ["stage1_var1_angle", "stage1_var1_size", "stage1_var1_gap", "stage1_var2_shift"] {
        assert!(!has(&shown(&shifted), key), "a shift was offered {key}");
    }
    let gap = rule(&[Stage::run(3, Vec3::ZERO).with(Variation::widen(1.0))]);
    assert!(!has(&shown(&gap), "stage1_var1_axis"), "a gap was offered an axis");

    let turn = rule(&[Stage::turning(4, 90.0, 20.0, 0.0, 0.0, 2).with(Variation::widen(2.0))]);
    assert!(!has(&shown(&turn), "stage1_var1_gap"), "a turn was offered a run's gaps");
    let mirrored = rule(&[Stage::mirrored(0).with(Variation::spin(2, 5.0))]);
    assert!(!has(&shown(&mirrored), "stage1_var1_angle"), "a mirror was offered something to vary");

    // And every one of them is a key of the rule, so a saved kind carries it.
    let keys = rule_keys(&shifted);
    for key in variation_keys(0, 1) {
        assert!(keys.contains(&key), "{key} varies a stage but is not one of the rule's keys");
    }
}

/// The three ready-made layouts: offset rows, sized to what is repeated, and a
/// custom rule like any other once started from.
#[test]
pub(crate) fn every_preset_lays_out_offset_rows_sized_to_the_shape() {
    let size = Vec3::new(100.0, 20.0, 10.0);
    for (preset, name) in PRESETS.iter().enumerate() {
        let mut params = default_params();
        use_preset(&mut params, preset, size);
        assert_eq!(params.int("kind"), CUSTOM, "{name} did not switch the pattern to a rule");
        assert_eq!(stage_count(&params), 2, "{name} is not a row and its rows");
        let rows = stage(&params, 1);
        let shift = rows.variations()[0];
        assert_eq!(shift.what, Vary::Shift, "{name} does not shift its rows");
        assert!(shift.repeats && shift.every == 2, "{name} does not offset every other row");
        assert!(shift.axis == 0 && shift.amount > 0.0, "{name} does not offset its rows along them");
        // The first copy of the second row is offset by half the pitch of the
        // first stage, which is what staggers the joints. Read from the rule
        // before its scatter: the planks come with one.
        let pitch = stage(&params, 0).step.x;
        let copies = instances_through(&params, 1);
        let second_row = copies[stage(&params, 0).copies()].xform.t;
        assert!((second_row.x - pitch / 2.0).abs() < 1e-9, "{name}'s second row starts at {second_row:?}");
        // Clear of each other: copies that touch are welded into one body.
        assert!(pitch > size.x, "{name} lays its copies down touching");
    }

    // A hexagon grid's neighbours are all one cell apart, across the row and
    // between rows alike.
    let mut hex = default_params();
    use_preset(&mut hex, 2, Vec3::new(20.0, 20.0, 5.0));
    let copies = instances(&hex);
    let cell = stage(&hex, 0).step.x;
    let first = copies[0].xform.t;
    let above = copies[stage(&hex, 0).copies()].xform.t;
    assert!(((above - first).length() - cell).abs() < 1e-9, "a hexagon's neighbour in the next row is off the cell");

    // Planks come with a little scatter, and not so much that two can meet.
    let mut planks = default_params();
    use_preset(&mut planks, 0, size);
    assert!(Noise::of(&planks).wanted(), "the planks came without the randomness they are for");
    assert_eq!(crowding(&planks, size), None, "the planks' own scatter can close their joints");
    let mut bricks = default_params();
    use_preset(&mut bricks, 1, size);
    assert!(!Noise::of(&bricks).wanted(), "a brick wall came with a scatter of its own");

    // A scatter set before the layout was picked is the user's, and stays --
    // on the planks too, which would otherwise bring their own.
    for preset in [0, 1] {
        let mut scattered = default_params();
        scattered.insert("noise_x".to_string(), ParamValue::Length(3.0));
        scattered.insert("noise_turn_z".to_string(), ParamValue::Angle(4.0));
        use_preset(&mut scattered, preset, size);
        assert_eq!(scattered.num("noise_x"), 3.0, "{} replaced the nudge it was given", PRESETS[preset]);
        assert_eq!(scattered.num("noise_turn_z"), 4.0, "{} dropped the turn it was given", PRESETS[preset]);
        assert_eq!(scattered.num("noise_y"), 0.0, "{} added to the scatter it was given", PRESETS[preset]);
    }
}

/// A stage added to a rule starts as the next thing the rule is missing:
/// a run along a free axis, spaced clear of the shape, then a ring.
#[test]
pub(crate) fn a_fresh_stage_runs_along_the_first_free_axis_and_then_turns() {
    let size = Vec3::new(10.0, 20.0, 30.0);
    let one = rule(&[Stage::run(3, Vec3::new(15.0, 0.0, 0.0))]);
    let next = fresh_stage(&one, 1, size);
    assert_eq!(next.mode, StageMode::Move);
    assert!(near(next.step, Vec3::new(0.0, 30.0, 0.0)), "the second stage ran along {:?}", next.step);
    assert!(next.count >= 2, "a fresh stage that makes one copy does nothing visible");

    let three = rule(&[
        Stage::run(3, Vec3::new(15.0, 0.0, 0.0)),
        Stage::run(3, Vec3::new(0.0, 30.0, 0.0)),
        Stage::run(3, Vec3::new(0.0, 0.0, 45.0)),
    ]);
    let last = fresh_stage(&three, 3, size);
    assert_eq!(last.mode, StageMode::Turn, "with every axis run along, the next thing to do is turn");
    assert!(last.radius > 0.0);

    // A blank rule's one stage goes nowhere, so the fresh one takes X.
    let mut blank = default_params();
    clear_stages(&mut blank);
    assert!(near(fresh_stage(&blank, 1, size).step, Vec3::new(15.0, 0.0, 0.0)));
}

/// Any stage can go, and the ones below it move up; any two can swap.
#[test]
pub(crate) fn a_stage_can_be_dropped_from_the_middle_and_two_can_trade_places() {
    let (a, b, c) = (
        Stage::run(2, Vec3::new(10.0, 0.0, 0.0)),
        Stage::turning(4, 90.0, 30.0, 0.0, 0.0, 2),
        Stage::run(3, Vec3::new(0.0, 0.0, 5.0)),
    );
    let mut params = rule(&[a.clone(), b.clone(), c.clone()]);
    remove_stage(&mut params, 1);
    assert_eq!(stage_count(&params), 2);
    assert_eq!(stage(&params, 0), a);
    assert_eq!(stage(&params, 1), c, "the stage below the one dropped did not move up");
    assert_eq!(instances(&params), instances(&rule(&[a.clone(), c.clone()])));

    // The last one left cannot go.
    let mut single = rule(std::slice::from_ref(&a));
    remove_stage(&mut single, 0);
    assert_eq!(stage_count(&single), 1);

    // Order matters once a stage turns: a ring of rows is not a row of rings.
    let mut params = rule(&[a.clone(), b.clone()]);
    let before = instances(&params);
    swap_stages(&mut params, 0, 1);
    assert_eq!(stage(&params, 0), b);
    assert_eq!(stage(&params, 1), a);
    assert_ne!(instances(&params), before, "swapping a run and a turn changed nothing");
    swap_stages(&mut params, 0, 5);
    assert_eq!(stage(&params, 0), b, "a swap with a stage that is not in use went ahead");
}

/// What the tool marks while the pointer is over a stage: the copies made by
/// the end of it, and no further.
#[test]
pub(crate) fn the_copies_through_a_stage_are_what_the_stages_above_it_made() {
    let params = rule(&[Stage::run(3, Vec3::new(10.0, 0.0, 0.0)), Stage::run(2, Vec3::new(0.0, 10.0, 0.0))]);
    assert_eq!(instances_through(&params, 0).len(), 3);
    assert_eq!(instances_through(&params, 1).len(), 6);
    assert_eq!(instances_through(&params, 9).len(), 6, "a stage past the last one is the whole rule");
}
