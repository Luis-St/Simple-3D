//! A field's cap is where its shape stops following it (issue 131): just under it the shape still
//! changes, and past it nothing does.

use super::*;

/// Every shape at its defaults, and once more with each choice switched to each option, so the
/// parameters a choice shows get their turn.
fn setups(spec: &PrimitiveSpec) -> Vec<Params> {
    let mut out = vec![spec.default_params()];
    for param in spec.params {
        if let ParamKind::Choice { options } = param.kind {
            for option in 1..options.len() as u32 {
                let mut p = spec.default_params();
                p.insert(param.key.to_string(), ParamValue::Choice(option));
                out.push(p);
            }
        }
    }
    out
}

fn built(spec: &PrimitiveSpec, p: &Params, key: &str, value: f64) -> Vec<[i64; 3]> {
    let mut p = p.clone();
    p.insert(key.to_string(), ParamValue::Length(value));
    let mesh = (spec.build)(&p, 24);
    mesh.positions.iter().map(|v| [v.x, v.y, v.z].map(|c| (c * 1e6).round() as i64)).collect()
}

#[test]
fn every_cap_is_where_the_shape_stops_changing() {
    let mut wrong = Vec::new();
    for spec in REGISTRY {
        for p in setups(spec) {
            for param in spec.params.iter().filter(|param| spec.param_visible(param, &p)) {
                let Some(cap) = spec.cap(param.key, &p) else { continue };
                if cap <= 1e-3 {
                    continue;
                }
                let at = built(spec, &p, param.key, cap);
                if built(spec, &p, param.key, cap * 0.8) == at {
                    wrong.push(format!("{} {} below {cap}: no change", spec.type_id, param.key));
                }
                if built(spec, &p, param.key, cap * 1.5) != at {
                    wrong.push(format!("{} {} past {cap}: still changes", spec.type_id, param.key));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn holding_to_caps_brings_a_corner_radius_down_with_its_box() {
    let spec = lookup("rounded_box").unwrap();
    let mut p = spec.default_params();
    p.insert("corner_radius".into(), ParamValue::Length(8.0));
    p.insert("width".into(), ParamValue::Length(10.0));
    spec.hold_to_caps(&mut p);
    assert_eq!(p.num("corner_radius"), 5.0);
}
