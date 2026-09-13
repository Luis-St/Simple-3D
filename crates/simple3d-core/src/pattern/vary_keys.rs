//! The parameters a stage's variations are stored under (issue 79).

use super::*;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The numbers one variation is stored under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VaryField {
    What,
    Steps,
    Every,
    Start,
    Axis,
    Shift,
    Angle,
    Size,
    Gap,
}

impl VaryField {
    pub const ALL: [VaryField; 9] = [
        VaryField::What,
        VaryField::Steps,
        VaryField::Every,
        VaryField::Start,
        VaryField::Axis,
        VaryField::Shift,
        VaryField::Angle,
        VaryField::Size,
        VaryField::Gap,
    ];

    fn suffix(self) -> &'static str {
        match self {
            VaryField::What => "what",
            VaryField::Steps => "steps",
            VaryField::Every => "every",
            VaryField::Start => "start",
            VaryField::Axis => "axis",
            VaryField::Shift => "shift",
            VaryField::Angle => "angle",
            VaryField::Size => "size",
            VaryField::Gap => "gap",
        }
    }

    /// The field that holds the amount of a variation of `what`.
    pub fn amount_of(what: Vary) -> VaryField {
        match what {
            Vary::Shift => VaryField::Shift,
            Vary::Spin => VaryField::Angle,
            Vary::Size => VaryField::Size,
            Vary::Gap => VaryField::Gap,
        }
    }
}

/// The parameter one variation's number is stored under: stage `stage` and slot
/// `slot`, both from nought.
///
/// Built rather than tabled. A stage used to have four variation slots spelled
/// out as ordinary parameters, and four was therefore as many as it could have.
/// The names are the same shape for every slot, so they are made when asked for.
pub fn vary_key(stage: usize, slot: usize, field: VaryField) -> String {
    format!("stage{}_var{}_{}", stage.min(MAX_STAGES - 1) + 1, slot + 1, field.suffix())
}

/// Which stage, slot and field a variation's parameter is, from nought.
pub fn parse_vary_key(key: &str) -> Option<(usize, usize, VaryField)> {
    let rest = key.strip_prefix("stage")?;
    let (stage, rest) = rest.split_once("_var")?;
    let (slot, suffix) = rest.split_once('_')?;
    let stage: usize = stage.parse().ok()?;
    let slot: usize = slot.parse().ok()?;
    let field = VaryField::ALL.into_iter().find(|f| f.suffix() == suffix)?;
    (1..=MAX_STAGES).contains(&stage).then_some(())?;
    (slot >= 1).then_some((stage - 1, slot - 1, field))
}

/// What one variation's parameter is -- its kind, bounds, default and the label
/// its field answers to -- for any stage and slot.
///
/// The property editor, the tool and the migration all want a [`ParamSpec`],
/// and a spec's key and label are `'static`. They are made the first time a
/// slot is asked for and kept for the life of the program: a stage with a
/// dozen variations costs a dozen small entries, once.
pub fn vary_spec(stage: usize, slot: usize, field: VaryField) -> &'static ParamSpec {
    type Specs = HashMap<(usize, usize, VaryField), &'static ParamSpec>;
    static SPECS: OnceLock<Mutex<Specs>> = OnceLock::new();
    let stage = stage.min(MAX_STAGES - 1);
    let mut specs = SPECS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    specs.entry((stage, slot, field)).or_insert_with(|| {
        let key: &'static str = Box::leak(vary_key(stage, slot, field).into_boxed_str());
        let number = |name: &str| -> &'static str { Box::leak(format!("{}.{} {name}", stage + 1, slot + 1).into()) };
        let when = Some(("kind", CUSTOM));
        let (label, kind, default) = match field {
            VaryField::What => ("Varies", ParamKind::Choice { options: VARIES }, ParamValue::Choice(0)),
            VaryField::Steps => ("Steps", ParamKind::Choice { options: STEPS }, ParamValue::Choice(0)),
            VaryField::Axis => ("Axis", ParamKind::Choice { options: VARY_AXES }, ParamValue::Choice(2)),
            VaryField::Every => (number("Every"), ParamKind::Count { min: 1, max: MAX_CYCLE }, ParamValue::Count(1)),
            VaryField::Start => (number("Start at"), ParamKind::Count { min: 1, max: MAX_CYCLE }, ParamValue::Count(2)),
            VaryField::Shift => {
                (number("Shift"), ParamKind::Length { min: f64::NEG_INFINITY }, ParamValue::Length(0.0))
            }
            VaryField::Angle => {
                (number("Spin"), ParamKind::Angle { min: -360.0, max: 360.0, wrap: false }, ParamValue::Angle(0.0))
            }
            VaryField::Size => (number("Size (%)"), ParamKind::Count { min: 10, max: 1000 }, ParamValue::Count(100)),
            VaryField::Gap => (number("Gap"), ParamKind::Length { min: f64::NEG_INFINITY }, ParamValue::Length(0.0)),
        };
        Box::leak(Box::new(ParamSpec { key, label, kind, default, lock_group: 0, shown_when: when }))
    })
}

/// Any pattern parameter's spec: one from the table, or one of a variation's.
pub fn param_spec(key: &str) -> Option<&'static ParamSpec> {
    PARAMS
        .iter()
        .find(|spec| spec.key == key)
        .or_else(|| parse_vary_key(key).map(|(stage, slot, field)| vary_spec(stage, slot, field)))
}

/// Read variation `slot` of stage `stage` out of a pattern's parameters.
pub(crate) fn read_variation(params: &Params, stage: usize, slot: usize) -> Variation {
    // Read with their defaults in hand, as a stage's own are: a zero size is a
    // copy shrunk to nothing, not "no change".
    let value = |field: VaryField| {
        let spec = vary_spec(stage, slot, field);
        params.get(spec.key).copied().unwrap_or(spec.default)
    };
    let what = Vary::from_index(value(VaryField::What).as_u32());
    let amount = match what {
        Vary::Size => value(VaryField::Size).as_u32().clamp(10, 1000) as f64 / 100.0,
        _ => value(VaryField::amount_of(what)).as_f64(),
    };
    let most_axis = if what == Vary::Size { ALL_AXES } else { 2 };
    Variation {
        what,
        axis: (value(VaryField::Axis).as_u32() as usize).min(most_axis),
        amount,
        repeats: value(VaryField::Steps).as_u32() == 1,
        every: value(VaryField::Every).as_u32().clamp(1, MAX_CYCLE),
        start: value(VaryField::Start).as_u32().clamp(1, MAX_CYCLE),
    }
}

/// Write variation `slot` of stage `stage` back into a pattern's parameters.
/// Only the amount its own kind reads is written.
pub(crate) fn write_variation(params: &mut Params, stage: usize, slot: usize, v: &Variation) {
    let mut put = |field: VaryField, value: ParamValue| {
        params.insert(vary_key(stage, slot, field), value);
    };
    put(VaryField::What, ParamValue::Choice(v.what.index()));
    put(VaryField::Steps, ParamValue::Choice(u32::from(v.repeats)));
    put(VaryField::Every, ParamValue::Count(v.every.clamp(1, MAX_CYCLE)));
    put(VaryField::Start, ParamValue::Count(v.start.clamp(1, MAX_CYCLE)));
    put(VaryField::Axis, ParamValue::Choice(v.axis.min(ALL_AXES) as u32));
    match v.what {
        Vary::Shift => put(VaryField::Shift, ParamValue::Length(v.amount)),
        Vary::Spin => put(VaryField::Angle, ParamValue::Angle(v.amount.clamp(-360.0, 360.0))),
        Vary::Size => put(VaryField::Size, ParamValue::Count((v.amount * 100.0).round().clamp(10.0, 1000.0) as u32)),
        Vary::Gap => put(VaryField::Gap, ParamValue::Length(v.amount)),
    }
}

/// Take every parameter of stage `stage`'s variations from slot `from` on out
/// of the map, so a list that got shorter leaves nothing behind it.
pub(crate) fn clear_variations_from(params: &mut Params, stage: usize, from: usize) {
    params.retain(|key, _| !matches!(parse_vary_key(key), Some((s, slot, _)) if s == stage && slot >= from));
}

/// Every variation parameter of stage `stage`'s first `used` slots, in slot
/// order.
pub fn variation_keys(stage: usize, used: usize) -> Vec<String> {
    (0..used).flat_map(|slot| VaryField::ALL.map(|field| vary_key(stage, slot, field))).collect()
}
