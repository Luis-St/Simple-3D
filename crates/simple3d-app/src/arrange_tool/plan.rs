//! Where each object goes: the arithmetic, apart from the application so it can be tested alone.

use super::*;
use simple3d_core::unit::wrap_degrees;
use simple3d_core::xform::Xform;
use simple3d_geom::path::SAME_POINT;

/// One object as the plan sees it.
#[derive(Clone, Copy, Debug)]
pub struct Subject {
    pub id: NodeId,
    /// Its world-space box.
    pub bounds: (Vec3, Vec3),
    /// Its parent's world frame, which turns world moves into positions.
    pub parent: Xform,
    pub position: Vec3,
    pub rotation: Vec3,
}

impl Subject {
    fn centre(&self) -> Vec3 {
        (self.bounds.0 + self.bounds.1) * 0.5
    }
}

/// Where one object ends up: an original moved, or a copy of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub source: NodeId,
    pub copy: bool,
    pub position: Vec3,
    pub rotation: Vec3,
    /// What carries the source's body to this place in world space, for its template.
    pub world: Xform,
}

impl Placement {
    /// An original left exactly where it is needs no template.
    pub fn moves(&self) -> bool {
        self.copy || self.world != Xform::IDENTITY
    }
}

/// Every placement the tool's settings come to, originals first, or why there are none.
pub fn plan(tool: &ArrangeTool, subjects: &[Subject]) -> Result<Vec<Placement>, String> {
    if subjects.is_empty() {
        return Err("None of the objects has a shape to arrange yet".into());
    }
    match tool.mode {
        Arrange::Align => align(tool, subjects),
        Arrange::Distribute => distribute(tool, subjects),
        Arrange::Path => along_path(tool, subjects),
    }
}

/// A subject moved by `delta` in world space.
fn moved(subject: &Subject, delta: Vec3) -> Placement {
    Placement {
        source: subject.id,
        copy: false,
        position: subject.position + subject.parent.inverse().vector(delta),
        rotation: subject.rotation,
        world: if delta == Vec3::ZERO { Xform::IDENTITY } else { Xform::from_translation(delta) },
    }
}

fn align(tool: &ArrangeTool, subjects: &[Subject]) -> Result<Vec<Placement>, String> {
    if tool.align.iter().all(Option::is_none) {
        return Err("Choose a side to line up on at least one axis".into());
    }
    let key = subjects.iter().find(|s| tool.to_key && Some(s.id) == tool.key);
    if subjects.len() < 2 {
        return Err("Select two or more objects to line them up".into());
    }
    let reference = match key {
        Some(key) => key.bounds,
        None => subjects.iter().fold(subjects[0].bounds, |(lo, hi), s| (lo.min(s.bounds.0), hi.max(s.bounds.1))),
    };
    Ok(subjects
        .iter()
        .map(|subject| {
            let mut delta = Vec3::ZERO;
            for (axis, side) in tool.align.iter().enumerate() {
                if let Some(side) = side {
                    delta.set(axis, side.of(reference, axis) - side.of(subject.bounds, axis));
                }
            }
            moved(subject, delta)
        })
        .collect())
}

fn distribute(tool: &ArrangeTool, subjects: &[Subject]) -> Result<Vec<Placement>, String> {
    let axis = tool.axis;
    let fewest = if tool.spacing == Spacing::Fixed { 2 } else { 3 };
    if subjects.len() < fewest {
        return Err(match tool.spacing {
            Spacing::Fixed => "Select two or more objects to space them out".into(),
            _ => "Select three or more objects: the outer two stay, the ones between are spread".into(),
        });
    }
    let mut order: Vec<&Subject> = subjects.iter().collect();
    order.sort_by(|a, b| a.centre().get(axis).total_cmp(&b.centre().get(axis)));
    let n = order.len();
    let lo = |s: &Subject| s.bounds.0.get(axis);
    let hi = |s: &Subject| s.bounds.1.get(axis);
    let mut starts: Vec<f64> = Vec::with_capacity(n);
    match tool.spacing {
        Spacing::Centres => {
            let (first, last) = (order[0].centre().get(axis), order[n - 1].centre().get(axis));
            for (i, subject) in order.iter().enumerate() {
                let centre = first + (last - first) * i as f64 / (n - 1) as f64;
                starts.push(centre - (hi(subject) - lo(subject)) / 2.0);
            }
        }
        Spacing::Gaps | Spacing::Fixed => {
            let sizes: f64 = order.iter().map(|s| hi(s) - lo(s)).sum();
            let gap = match tool.spacing {
                Spacing::Fixed => tool.gap,
                _ => (hi(order[n - 1]) - lo(order[0]) - sizes) / (n - 1) as f64,
            };
            let mut at = lo(order[0]);
            for subject in &order {
                starts.push(at);
                at += hi(subject) - lo(subject) + gap;
            }
        }
    }
    // Back in selection order, so the result lists objects as they were given.
    Ok(subjects
        .iter()
        .map(|subject| {
            let slot = order.iter().position(|s| s.id == subject.id).expect("every subject is in the order");
            let mut delta = Vec3::ZERO;
            delta.set(axis, starts[slot] - lo(subject));
            moved(subject, delta)
        })
        .collect())
}

fn along_path(tool: &ArrangeTool, subjects: &[Subject]) -> Result<Vec<Placement>, String> {
    let path = tool.path();
    if path.length() <= SAME_POINT {
        return Err(match tool.source {
            PathSource::Edges => "Click body edges in the viewport to make the path".into(),
            PathSource::Drawn => "Click two or more points in the viewport to draw the path".into(),
        });
    }
    // Objects keep the order they already have along the path, so a rough layout is only tidied.
    let mut order: Vec<&Subject> = subjects.iter().collect();
    order.sort_by(|a, b| path.distance_along(a.centre()).total_cmp(&path.distance_along(b.centre())));
    let count = tool.count.max(order.len());
    let spots: Vec<(Vec3, Vec3)> = path.spread(count).into_iter().filter_map(|s| path.at(s)).collect();
    let Some(&(_, first)) = spots.first() else { return Err("The path has no length to spread along".into()) };
    Ok(spots
        .iter()
        .enumerate()
        .map(|(index, &(at, heading))| {
            let subject = order[index % order.len()];
            // The first object keeps its turn and the rest turn as far as the path has since.
            let turn = if tool.follow { heading_of(subject, heading) - heading_of(subject, first) } else { 0.0 };
            placed(subject, at, turn, index >= order.len())
        })
        .collect())
}

/// Which way `direction` points around the parent's Z, in degrees; level with nothing, straight up.
fn heading_of(subject: &Subject, direction: Vec3) -> f64 {
    let local = subject.parent.inverse().vector(direction);
    if local.x.hypot(local.y) < 1e-9 {
        return 0.0;
    }
    local.y.atan2(local.x).to_degrees()
}

/// A subject with its box's centre on `at`, turned `turn` degrees about its parent's Z. Worked in
/// the parent's frame, where a turn about Z is exactly what adding to the Z rotation does.
fn placed(subject: &Subject, at: Vec3, turn: f64, copy: bool) -> Placement {
    let inverse = subject.parent.inverse();
    let spin = Xform::from_pos_rot(Vec3::ZERO, Vec3::new(0.0, 0.0, turn));
    let centre = inverse.point(subject.centre());
    let position = inverse.point(at) - spin.vector(centre - subject.position);
    let mut rotation = subject.rotation;
    if turn != 0.0 {
        rotation.z = wrap_degrees(rotation.z + turn);
    }
    // In the parent's frame a point goes round the old position and lands relative to the new one.
    let local = Xform { m: spin.m, t: position - spin.vector(subject.position) };
    let world = subject.parent.compose(&local.compose(&inverse));
    Placement { source: subject.id, copy, position, rotation, world }
}
