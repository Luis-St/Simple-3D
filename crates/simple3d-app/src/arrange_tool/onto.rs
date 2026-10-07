//! Putting objects onto a picked face (issue 118): turned so their bottom faces it, if asked, and
//! lowered until their lowest point rests on it. They move as one block, keeping their layout.

use super::*;
use simple3d_core::xform::Xform;

/// The objects resting on the tool's face, or why they cannot yet.
pub(super) fn onto_face(tool: &ArrangeTool, subjects: &[Subject]) -> Result<Vec<Placement>, String> {
    let Some(landing) = tool.landing else {
        return Err("Click a face in the viewport to put the objects onto".into());
    };
    let normal = landing.normal.normalized();
    let turn = if tool.turn { Xform::turn_z_to(normal) } else { Xform::IDENTITY };
    let (lo, hi) =
        subjects.iter().skip(1).fold(subjects[0].bounds, |(lo, hi), s| (lo.min(s.bounds.0), hi.max(s.bounds.1)));
    let centre = (lo + hi) * 0.5;
    // How far the lowest point sits below the middle along the normal, once turned about the middle.
    let lowest =
        subjects.iter().flat_map(points).map(|p| turn.vector(p - centre).dot(normal)).fold(f64::INFINITY, f64::min);
    let at = match tool.onto {
        OntoAt::Centre => landing.centre,
        OntoAt::Clicked => landing.at,
    };
    let middle = at + normal * (tool.lift - lowest);
    let world = Xform { m: turn.m, t: middle - turn.vector(centre) };
    // Already resting there: nothing moves, so no template.
    let still = (world.t.length() < 1e-9) && turn == Xform::IDENTITY;
    Ok(subjects.iter().map(|subject| carried(subject, if still { Xform::IDENTITY } else { world })).collect())
}

/// Every point of a subject's body in world space, or its box's corners without one.
fn points(subject: &Subject) -> Vec<Vec3> {
    match &subject.body {
        Some((mesh, frame)) => mesh.positions.iter().map(|&p| frame.point(p)).collect(),
        None => {
            let (lo, hi) = subject.bounds;
            (0..8)
                .map(|i| {
                    Vec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    )
                })
                .collect()
        }
    }
}

/// A subject carried by the world move `world`, as a position and rotation in its parent's frame.
fn carried(subject: &Subject, world: Xform) -> Placement {
    let local = subject.parent.inverse().compose(&world).compose(&subject.parent);
    let position = local.point(subject.position);
    let rotation = if local.m == Xform::IDENTITY.m {
        subject.rotation
    } else {
        let turned = local.compose(&Xform::from_pos_rot(Vec3::ZERO, subject.rotation));
        Xform::rotation_of_axes(turned.axis(0), turned.axis(1), turned.axis(2))
    };
    Placement { source: subject.id, copy: false, position, rotation, world }
}
