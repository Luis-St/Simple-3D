//! The axis handles, and what they drive.

use super::*;

#[test]
pub(crate) fn declared_axis_drivers_match_the_real_bounding_extent() {
    // A resize handle must write a parameter that genuinely governs the
    // extent it is dragging (spec section 6.2), so the declaration and the
    // generated mesh have to agree.
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
    // The invariant a resize handle relies on is that `axes(params)`
    // predicts the real bounding extents for *any* parameter values, not
    // just the defaults -- otherwise a handle would keep tracking the
    // cursor after the parameter it writes stopped governing that extent.
    // Where that can happen (a spherical cap dragged wider than twice its
    // cap height is widest at its rim, not at its equator) the declaration
    // has to withdraw the handle, and this test is what proves it does.
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
    // Every type that declares a sweep has to stay manifold short of a full
    // turn, and stop offering X/Y resize handles there: a quarter cylinder
    // is one radius wide, so a handle writing "diameter" would not track
    // the cursor.
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
            // The Z extent is unaffected by how far round the shape goes,
            // so that handle stays and has to stay truthful.
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
    // The sweep was added to types that already existed; anything that
    // loads without one has to come back a whole turn.
    for spec in REGISTRY {
        let Some(sweep) = spec.param("sweep") else { continue };
        assert_eq!(sweep.default, ParamValue::Angle(360.0), "{}", spec.type_id);
        let migrated = spec.migrate_params(&Params::new());
        assert_eq!(migrated.num("sweep"), 360.0, "{}", spec.type_id);
    }
}
