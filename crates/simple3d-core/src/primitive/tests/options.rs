//! Every option a primitive offers builds a solid that its resize handles still measure.

use super::*;

/// The values tried for one parameter: every choice, both ways of a flag, both ends of an angle and
/// a count, and a length halved, doubled, or set where it defaults to off.
fn variations(param: &ParamSpec) -> Vec<ParamValue> {
    match (param.kind, param.default) {
        (ParamKind::Choice { options }, _) => (0..options.len() as u32).map(ParamValue::Choice).collect(),
        (ParamKind::Bool, _) => vec![ParamValue::Bool(false), ParamValue::Bool(true)],
        (ParamKind::Angle { min, max, .. }, _) => vec![ParamValue::Angle(min), ParamValue::Angle(max)],
        (ParamKind::Count { min, max }, ParamValue::Count(default)) => {
            vec![ParamValue::Count(min), ParamValue::Count((default * 2).min(max).min(64))]
        }
        (ParamKind::Length { .. }, ParamValue::Length(v)) if v == 0.0 => {
            vec![ParamValue::Length(2.0), ParamValue::Length(6.0), ParamValue::Length(-2.0)]
        }
        (ParamKind::Length { min }, ParamValue::Length(v)) => {
            [v * 0.5, v * 2.0].into_iter().filter(|&x| x >= min).map(ParamValue::Length).collect()
        }
        _ => Vec::new(),
    }
}

/// What is wrong with the shape `params` build, if anything.
fn fault(spec: &PrimitiveSpec, params: &Params) -> Option<String> {
    let mesh = (spec.build)(params, 32);
    if mesh.triangle_count() == 0 {
        return Some("empty mesh".into());
    }
    if let Some(issue) = mesh.manifold_issue() {
        return Some(issue);
    }
    if mesh.signed_volume() <= 0.0 {
        return Some("inside out".into());
    }
    let (lo, hi) = mesh.bounds()?;
    let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
    for (axis, driver) in (spec.axes)(params).iter().enumerate() {
        let Some(driver) = driver else { continue };
        let predicted = params.num(driver.param) * driver.factor;
        if (predicted - size[axis]).abs() >= 1e-6 {
            return Some(format!(
                "axis {axis} driver {} predicts {predicted}, mesh measures {}",
                driver.param, size[axis]
            ));
        }
    }
    None
}

#[test]
pub(crate) fn every_option_builds_a_solid_its_handles_measure() {
    // Each choice and flag in every setting, and with each of them every other parameter varied, so an
    // option is tried with the values it shows.
    let mut faults = Vec::new();
    let mut check = |spec: &PrimitiveSpec, params: &Params, what: &str| {
        if let Some(fault) = fault(spec, params) {
            faults.push(format!("{} with {what}: {fault}", spec.type_id));
        }
    };
    for spec in REGISTRY {
        let base = spec.default_params();
        let modes: Vec<(&ParamSpec, ParamValue)> = spec
            .params
            .iter()
            .filter(|p| matches!(p.kind, ParamKind::Choice { .. } | ParamKind::Bool))
            .flat_map(|p| variations(p).into_iter().map(move |v| (p, v)))
            .collect();
        let mut settings: Vec<(String, Params)> = vec![("defaults".into(), base.clone())];
        for (mode, value) in &modes {
            let mut params = base.clone();
            params.insert(mode.key.into(), *value);
            settings.push((format!("{}={value:?}", mode.key), params));
        }
        for (label, params) in &settings {
            check(spec, params, label);
            for param in spec.params {
                for value in variations(param) {
                    if matches!(value, ParamValue::Length(v) if v < 0.0) && !signed(param) {
                        continue;
                    }
                    let mut varied = params.clone();
                    varied.insert(param.key.into(), value);
                    check(spec, &varied, &format!("{label}, {}={value:?}", param.key));
                }
            }
        }
    }
    assert!(faults.is_empty(), "{} faults:\n{}", faults.len(), faults.join("\n"));
}

fn signed(param: &ParamSpec) -> bool {
    matches!(param.kind, ParamKind::Length { min } if min < 0.0)
}
