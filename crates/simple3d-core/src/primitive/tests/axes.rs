//! The axis handles, and what they drive.

use super::*;

#[test]
pub(crate) fn declared_axis_drivers_match_the_real_bounding_extent() {
    // A resize handle must write a parameter that governs the dragged extent (spec section 6.2).
    for spec in REGISTRY {
        let params = spec.default_params();
        let mesh = (spec.build)(&params, 64);
        let (lo, hi) = mesh.bounds().unwrap();
        let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
        for (axis, driver) in (spec.axes)(&params).iter().enumerate() {
            let Some(driver) = driver else { continue };
            let expected = params.num(driver.param) * driver.factor;
            assert!(
                (expected - size[axis]).abs() < 1e-6,
                "{}: axis {axis} driver {} predicts {expected}, mesh measures {}",
                spec.type_id,
                driver.param,
                size[axis]
            );
        }
    }
}

#[test]
pub(crate) fn axis_drivers_stay_truthful_after_a_parameter_changes() {
    // `axes(params)` must predict extents for any parameters, withdrawing a handle where the parameter
    // stops governing it (a wide spherical cap is widest at its rim).
    for spec in REGISTRY {
        let base = spec.default_params();
        for p in spec.params {
            if !matches!(p.kind, ParamKind::Length { .. }) {
                continue;
            }
            for scale in [0.25, 0.5, 1.5, 3.0] {
                let mut edited = base.clone();
                let value = (base.num(p.key) * scale).max(1e-3);
                edited.insert(p.key.to_string(), ParamValue::Length(value));
                let (lo, hi) = (spec.build)(&edited, 64).bounds().unwrap();
                let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
                for (axis, driver) in (spec.axes)(&edited).iter().enumerate() {
                    let Some(driver) = driver else { continue };
                    let predicted = edited.num(driver.param) * driver.factor;
                    assert!(
                        (predicted - size[axis]).abs() < 1e-6,
                        "{}: with {}={value}, axis {axis} driver {} predicts {predicted} but the mesh measures {}",
                        spec.type_id,
                        p.key,
                        driver.param,
                        size[axis]
                    );
                }
            }
        }
    }
}

#[test]
pub(crate) fn a_partly_swept_primitive_is_still_a_solid_and_withdraws_its_width_handles() {
    // Partial sweeps stay manifold and withdraw X/Y handles, since a quarter cylinder is one radius wide.
    for spec in REGISTRY {
        if spec.param("sweep").is_none() {
            continue;
        }
        for sweep in [45.0, 90.0, 200.0, 359.0] {
            let mut params = spec.default_params();
            params.insert("sweep".into(), ParamValue::Angle(sweep));
            let mesh = (spec.build)(&params, 32);
            assert!(mesh.triangle_count() > 0, "{} at {sweep} degrees: empty mesh", spec.type_id);
            assert!(
                mesh.manifold_issue().is_none(),
                "{} at {sweep} degrees: {}",
                spec.type_id,
                mesh.manifold_issue().unwrap()
            );
            let axes = (spec.axes)(&params);
            assert!(axes[0].is_none() && axes[1].is_none(), "{} kept a width handle at {sweep}", spec.type_id);
            // The Z extent is unaffected by the sweep, so that handle stays truthful.
            let (lo, hi) = mesh.bounds().expect("a solid has bounds");
            if let Some(driver) = axes[2] {
                let predicted = params.num(driver.param) * driver.factor;
                assert!(
                    (predicted - (hi.z - lo.z)).abs() < 1e-6,
                    "{} at {sweep} degrees: Z driver predicts {predicted}, mesh measures {}",
                    spec.type_id,
                    hi.z - lo.z
                );
            }
        }
    }
}

#[test]
pub(crate) fn a_full_sweep_is_the_default_so_existing_projects_are_unchanged() {
    // Types that gained a sweep load without one as a full turn.
    for spec in REGISTRY {
        let Some(sweep) = spec.param("sweep") else { continue };
        assert_eq!(sweep.default, ParamValue::Angle(360.0), "{}", spec.type_id);
        let migrated = spec.migrate_params(&Params::new());
        assert_eq!(migrated.num("sweep"), 360.0, "{}", spec.type_id);
    }
}
