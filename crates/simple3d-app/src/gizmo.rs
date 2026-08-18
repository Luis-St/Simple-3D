//! On-screen manipulators (spec section 6.2): move, rotate and resize.
//!
//! The important property, and the one the tests are built around, is that
//! **resize writes dimensions, never a scale factor**. Dragging the right face
//! of a box changes its width parameter and leaves the left face where it was;
//! the property editor then shows the new real width, and the saved project file
//! contains no scale anywhere. Which parameter governs which extent comes from
//! the primitive registry's `axes` declaration, so a handle is only ever offered
//! on an axis some parameter genuinely controls.
//!
//! Modifiers, consistent across all three modes:
//!
//! | Modifier | Effect |
//! |---|---|
//! | none  | snap to the increment (the scene step for move and resize, 15 degrees for rotate) |
//! | Alt   | drag freely, no snapping |
//! | Shift | snap coarsely (ten times the increment) |
//! | Ctrl  | resize about the centre (faces) / preserve proportions (corners) |

use crate::view::View;
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{AxisDriver, ParamValue, Params};
use simple3d_core::scene::{Node, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_core::unit::{format_angle, format_length, Unit};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

/// How far from the origin an axis arrow reaches, in screen pixels. Handles keep
/// a constant on-screen size regardless of zoom (spec section 6.2).
pub const ARM_PIXELS: f64 = 78.0;
/// Where a plane handle's corner sits along each of its two axes.
pub const PLANE_FRACTION: f64 = 0.42;
/// Click tolerance in pixels.
pub const GRAB_PIXELS: f32 = 9.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Move,
    Rotate,
    /// Rewrite the dimension the shape is defined by. Exact, and only offered on
    /// an axis some parameter actually governs.
    Resize,
    /// Multiply what is there by a factor. Works on a group, and on an axis no
    /// parameter governs, because it does not have to know what anything means.
    Scale,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Move, Mode::Rotate, Mode::Resize, Mode::Scale];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Move => "Move",
            Mode::Rotate => "Rotate",
            Mode::Resize => "Resize",
            Mode::Scale => "Scale",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Drag along one axis.
    MoveAxis(usize),
    /// Drag in the plane whose normal is this axis, for two-axis movement.
    MovePlane(usize),
    /// Rotate about one axis.
    RotateRing(usize),
    /// A face of the selection's bounding box: which axis, and which side.
    ResizeFace(usize, bool),
    /// A corner: which side on each axis.
    ResizeCorner([bool; 3]),
}

impl Handle {
    /// The axes this handle affects, for colouring and for the readout.
    pub fn axes(self) -> Vec<usize> {
        match self {
            Handle::MoveAxis(a) | Handle::RotateRing(a) | Handle::ResizeFace(a, _) => vec![a],
            Handle::MovePlane(a) => (0..3).filter(|&x| x != a).collect(),
            Handle::ResizeCorner(_) => vec![0, 1, 2],
        }
    }
}

/// Which modifiers are down. Kept as a plain struct so the drag maths can be
/// tested without an egui context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    /// Drag freely: no snapping at all.
    pub free: bool,
    /// Snap coarsely: ten times the increment.
    pub coarse: bool,
    /// Resize about the centre, or preserve proportions on a corner.
    pub symmetric: bool,
}

impl Mods {
    /// The increment to snap a value to, or `None` for a free drag.
    fn increment(self, base: f64) -> Option<f64> {
        if self.free || base <= 0.0 {
            None
        } else if self.coarse {
            Some(base * 10.0)
        } else {
            Some(base)
        }
    }

    fn snap(self, value: f64, base: f64) -> f64 {
        match self.increment(base) {
            Some(step) => (value / step).round() * step,
            None => value,
        }
    }
}

/// The manipulator for one selected node, positioned in world space.
#[derive(Clone, Debug)]
pub struct Gizmo {
    pub mode: Mode,
    /// The node's origin in world space -- where move and rotate handles centre.
    pub origin: Vec3,
    /// The handle frame's world axes: the node's own axes, or the world axes.
    pub axes: [Vec3; 3],
    /// The node's own frame, for turning local box corners into world points.
    pub own: Xform,
    /// The parent frame, for writing a dragged world position back to
    /// `Node::position`, which lives in the parent's coordinates.
    pub parent: Xform,
    pub local_lo: Vec3,
    pub local_hi: Vec3,
    /// Which parameter governs each local extent. `None` means no resize handle
    /// on that axis, rather than one that silently does nothing.
    pub drivers: [Option<AxisDriver>; 3],
    /// The node's own scale factors.
    pub own_scale: Vec3,
    /// How many world millimetres one local millimetre covers along each of the
    /// node's axes -- its own scale times every ancestor's. What turns a drag
    /// measured on screen into a change to a dimension.
    pub axis_scale: [f64; 3],
}

impl Gizmo {
    pub fn build(scene: &Scene, evaluated: &Evaluated, id: NodeId, mode: Mode, world_frame: bool) -> Option<Gizmo> {
        let node = scene.get(id)?;
        if id == scene.root() {
            return None;
        }
        let parent = *evaluated.node_frames.get(&id)?;
        let own =
            parent.compose(&Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale)));
        let axes = if world_frame {
            [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)]
        } else {
            [own.axis(0), own.axis(1), own.axis(2)]
        };
        let (local_lo, local_hi) = evaluated.node_local_bounds.get(&id).copied().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let drivers = match (node.spec(), node.params()) {
            (Some(spec), Some(params)) => (spec.axes)(params),
            _ => [None, None, None],
        };
        let axis_scale = [0, 1, 2].map(|a| own.axis_vector(a).length().max(1e-9));
        Some(Gizmo {
            mode,
            origin: parent.point(node.position),
            axes,
            own,
            parent,
            local_lo,
            local_hi,
            drivers,
            own_scale: Node::sane_scale(node.scale),
            axis_scale,
        })
    }

    /// The handles to draw and hit-test, in the order they should be tested --
    /// the smaller, more specific handles first, so a corner wins over the face
    /// it sits on.
    pub fn handles(&self, is_group: bool) -> Vec<Handle> {
        match self.mode {
            Mode::Move => {
                let mut out: Vec<Handle> = (0..3).map(Handle::MovePlane).collect();
                out.extend((0..3).map(Handle::MoveAxis));
                out
            }
            Mode::Rotate => (0..3).map(Handle::RotateRing).collect(),
            // Scale asks nothing of the shape underneath it: every axis and
            // every corner, on a group as readily as on a primitive.
            Mode::Scale => {
                let mut out: Vec<Handle> = CORNERS.iter().map(|c| Handle::ResizeCorner(*c)).collect();
                for axis in 0..3 {
                    out.push(Handle::ResizeFace(axis, false));
                    out.push(Handle::ResizeFace(axis, true));
                }
                out
            }
            Mode::Resize => {
                // Resize handles on groups are out of scope for this version:
                // scaling a group truthfully means rewriting every descendant's
                // dimensions and relative positions (spec section 6.2).
                if is_group {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for corner in CORNERS {
                    // A corner is only offered if at least two of its axes are
                    // actually drivable; one is a face handle's job.
                    if (0..3).filter(|&a| self.drivers[a].is_some()).count() >= 2 {
                        out.push(Handle::ResizeCorner(corner));
                    }
                }
                for axis in 0..3 {
                    if self.drivers[axis].is_some() {
                        out.push(Handle::ResizeFace(axis, false));
                        out.push(Handle::ResizeFace(axis, true));
                    }
                }
                out
            }
        }
    }

    /// The arm length in millimetres that keeps the handle a constant size on
    /// screen.
    pub fn arm(&self, view: &View) -> f64 {
        ARM_PIXELS * view.mm_per_pixel_at(self.origin)
    }

    /// Where a handle sits in world space.
    pub fn handle_point(&self, handle: Handle, view: &View) -> Vec3 {
        let arm = self.arm(view);
        match handle {
            Handle::MoveAxis(a) => self.origin + self.axes[a] * arm,
            Handle::MovePlane(a) => {
                let (u, v) = other_axes(a);
                self.origin + (self.axes[u] + self.axes[v]) * (arm * PLANE_FRACTION)
            }
            Handle::RotateRing(a) => self.origin + self.axes[a] * arm,
            Handle::ResizeFace(a, positive) => self.own.point(self.face_centre(a, positive)),
            Handle::ResizeCorner(sides) => self.own.point(self.corner(sides)),
        }
    }

    fn face_centre(&self, axis: usize, positive: bool) -> Vec3 {
        let mid = (self.local_lo + self.local_hi) * 0.5;
        let edge = if positive { self.local_hi } else { self.local_lo };
        let mut p = mid;
        set_axis(&mut p, axis, get_axis(edge, axis));
        p
    }

    fn corner(&self, sides: [bool; 3]) -> Vec3 {
        let mut p = Vec3::ZERO;
        for axis in 0..3 {
            let edge = if sides[axis] { self.local_hi } else { self.local_lo };
            set_axis(&mut p, axis, get_axis(edge, axis));
        }
        p
    }

    /// Points along a rotate ring, for drawing it and for hit-testing it.
    pub fn ring_points(&self, axis: usize, view: &View, count: usize) -> Vec<Vec3> {
        let radius = self.arm(view) * 0.86;
        let (u, v) = other_axes(axis);
        (0..count)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / count as f64;
                self.origin + self.axes[u] * (radius * t.cos()) + self.axes[v] * (radius * t.sin())
            })
            .collect()
    }

    /// The handle under the cursor, if any.
    pub fn hit_test(&self, view: &View, cursor: egui::Pos2, is_group: bool) -> Option<Handle> {
        let mut best: Option<(f32, Handle)> = None;
        for handle in self.handles(is_group) {
            let distance = match handle {
                Handle::RotateRing(axis) => {
                    let points = self.ring_points(axis, view, 72);
                    let mut nearest = f32::MAX;
                    for pair in
                        points.windows(2).chain(std::iter::once([*points.last().unwrap(), points[0]].as_slice()))
                    {
                        let (Some((a, _)), Some((b, _))) = (view.project(pair[0]), view.project(pair[1])) else {
                            continue;
                        };
                        nearest = nearest.min(distance_to_segment(cursor, a, b));
                    }
                    nearest
                }
                other => match view.project(self.handle_point(other, view)) {
                    Some((screen, _)) => (screen - cursor).length(),
                    None => continue,
                },
            };
            if distance <= GRAB_PIXELS && best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, handle));
            }
        }
        best.map(|(_, handle)| handle)
    }
}

pub const CORNERS: [[bool; 3]; 8] = [
    [false, false, false],
    [true, false, false],
    [false, true, false],
    [true, true, false],
    [false, false, true],
    [true, false, true],
    [false, true, true],
    [true, true, true],
];

fn other_axes(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

pub fn get_axis(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

pub fn set_axis(v: &mut Vec3, axis: usize, value: f64) {
    match axis {
        0 => v.x = value,
        1 => v.y = value,
        _ => v.z = value,
    }
}

fn distance_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-9 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// An in-progress drag. Everything needed to reproduce the node's pre-drag state
/// exactly, so Escape restores it and the whole drag is one undo step.
#[derive(Clone, Debug)]
pub struct Drag {
    pub node: NodeId,
    pub handle: Handle,
    /// The manipulator **as it stood when the handle was grabbed**.
    ///
    /// A drag must be measured against a frame that does not move, and the live
    /// gizmo is rebuilt every frame from the very node the drag is moving. Using
    /// it here meant the reference slid along with the result: once the node had
    /// moved 10 mm, `gizmo.origin` had moved 10 mm too, the cursor measured 10 mm
    /// nearer, and the next frame wrote the node back to where it started. The
    /// position then flipped between the two on alternate frames, and where a
    /// drag finished came down to which frame the button happened to come up on.
    gizmo: Gizmo,
    pub start_position: Vec3,
    pub start_rotation: Vec3,
    pub start_scale: Vec3,
    pub start_params: Params,
    pub start_local_lo: Vec3,
    pub start_local_hi: Vec3,
    /// The scalar the handle was grabbed at, in whatever the handle measures.
    grab: f64,
    /// Where a plane handle was grabbed, in world space.
    grab_point: Vec3,
    /// Accumulated rotation angle, unwrapped across the +/-180 degree seam.
    last_angle: f64,
    turns: f64,
    /// The last value shown at the cursor.
    pub readout: String,
}

impl Drag {
    pub fn begin(
        scene: &Scene,
        gizmo: &Gizmo,
        node: NodeId,
        handle: Handle,
        view: &View,
        cursor: egui::Pos2,
    ) -> Option<Drag> {
        let n = scene.get(node)?;
        let mut drag = Drag {
            node,
            handle,
            gizmo: gizmo.clone(),
            start_position: n.position,
            start_rotation: n.rotation,
            start_scale: Node::sane_scale(n.scale),
            start_params: n.params().cloned().unwrap_or_default(),
            start_local_lo: gizmo.local_lo,
            start_local_hi: gizmo.local_hi,
            grab: 0.0,
            grab_point: Vec3::ZERO,
            last_angle: 0.0,
            turns: 0.0,
            readout: String::new(),
        };
        match handle {
            Handle::MoveAxis(a) => {
                drag.grab = view.ray_axis(cursor, gizmo.origin, gizmo.axes[a])?;
            }
            Handle::MovePlane(a) => {
                drag.grab_point = view.ray_plane(cursor, gizmo.origin, gizmo.axes[a])?;
            }
            Handle::RotateRing(a) => {
                drag.last_angle = ring_angle(gizmo, a, view, cursor)?;
                drag.grab = drag.last_angle;
            }
            Handle::ResizeFace(a, positive) => {
                let anchor = gizmo.own.point(gizmo.face_centre(a, positive));
                drag.grab = view.ray_axis(cursor, anchor, gizmo.axes[a])?;
            }
            Handle::ResizeCorner(sides) => {
                let anchor = gizmo.own.point(gizmo.corner(sides));
                // A corner is dragged in the plane facing the camera, and each
                // axis takes the component of that movement along itself.
                drag.grab_point = view.ray_plane(cursor, anchor, view.forward())?;
            }
        }
        Some(drag)
    }

    /// Apply the drag for the current cursor position. Called every frame, so the
    /// property editor tracks the handle live -- one source of truth, both
    /// directions (spec section 6.2).
    pub fn update(
        &mut self,
        scene: &mut Scene,
        view: &View,
        cursor: egui::Pos2,
        mods: Mods,
        move_snap: f64,
        rotate_snap: f64,
        unit: Unit,
    ) {
        // Everything below is measured in the frame the drag began in, never the
        // live one -- see the note on `Drag::gizmo`.
        let gizmo = &self.gizmo.clone();
        match self.handle {
            Handle::MoveAxis(axis) => {
                let Some(along) = view.ray_axis(cursor, gizmo.origin, gizmo.axes[axis]) else { return };
                let delta = mods.snap(along - self.grab, move_snap);
                let world_delta = gizmo.axes[axis] * delta;
                self.write_position(scene, world_delta);
                self.readout = format!("{} {}", axis_name(axis), signed_length(delta, unit));
            }
            Handle::MovePlane(axis) => {
                let Some(point) = view.ray_plane(cursor, gizmo.origin, gizmo.axes[axis]) else { return };
                let raw = point - self.grab_point;
                let (u, v) = other_axes(axis);
                let du = mods.snap(raw.dot(gizmo.axes[u]), move_snap);
                let dv = mods.snap(raw.dot(gizmo.axes[v]), move_snap);
                let world_delta = gizmo.axes[u] * du + gizmo.axes[v] * dv;
                self.write_position(scene, world_delta);
                self.readout = format!(
                    "{} {}  {} {}",
                    axis_name(u),
                    signed_length(du, unit),
                    axis_name(v),
                    signed_length(dv, unit)
                );
            }
            Handle::RotateRing(axis) => {
                let Some(angle) = ring_angle(gizmo, axis, view, cursor) else { return };
                // Unwrap across the seam so a full turn keeps counting up.
                let mut step = angle - self.last_angle;
                if step > 180.0 {
                    step -= 360.0;
                } else if step < -180.0 {
                    step += 360.0;
                }
                self.turns += step;
                self.last_angle = angle;
                let delta = mods.snap(self.turns, rotate_snap);
                let mut rotation = self.start_rotation;
                set_axis(&mut rotation, axis, get_axis(self.start_rotation, axis) + delta);
                if let Some(node) = scene.get_mut(self.node) {
                    node.rotation = rotation;
                }
                self.readout = format!("{} {}deg", axis_name(axis), format_angle(delta));
            }
            Handle::ResizeFace(axis, positive) => {
                let anchor = gizmo.own.point(gizmo.face_centre(axis, positive));
                let Some(along) = view.ray_axis(cursor, anchor, gizmo.axes[axis]) else { return };
                let outward = mods.snap(along - self.grab, move_snap) * if positive { 1.0 } else { -1.0 };
                let applied = self.size_axis(scene, axis, outward, mods.symmetric, positive);
                self.readout = match applied {
                    Some(extent) => format!("{} {}", axis_name(axis), format_length(extent, unit)),
                    None => "not resizable on this axis".to_string(),
                };
            }
            Handle::ResizeCorner(sides) => {
                let anchor = gizmo.own.point(gizmo.corner(sides));
                let Some(point) = view.ray_plane(cursor, anchor, view.forward()) else { return };
                let raw = point - self.grab_point;
                let mut ratio: Option<f64> = None;
                if mods.symmetric {
                    // Preserve proportions: take the axis that moved most, as a
                    // fraction of its own starting extent, and apply that same
                    // fraction to the others.
                    let mut best = 0.0;
                    for axis in 0..3 {
                        if !self.sizeable(axis) {
                            continue;
                        }
                        let extent = self.start_extent(axis);
                        if extent <= 0.0 {
                            continue;
                        }
                        let outward = raw.dot(gizmo.axes[axis]) * if sides[axis] { 1.0 } else { -1.0 };
                        if outward.abs() > best {
                            best = outward.abs();
                            ratio = Some((extent + outward) / extent);
                        }
                    }
                }
                let mut parts: Vec<String> = Vec::new();
                for axis in 0..3 {
                    if !self.sizeable(axis) {
                        continue;
                    }
                    let outward = match ratio {
                        Some(r) => self.start_extent(axis) * (r - 1.0),
                        None => mods.snap(raw.dot(gizmo.axes[axis]), move_snap) * if sides[axis] { 1.0 } else { -1.0 },
                    };
                    if let Some(extent) = self.size_axis(scene, axis, outward, false, sides[axis]) {
                        parts.push(format!("{} {}", axis_name(axis), format_length(extent, unit)));
                    }
                }
                self.readout = parts.join("  ");
            }
        }
    }

    fn drivable(&self, axis: usize) -> Option<AxisDriver> {
        self.gizmo.drivers[axis]
    }

    /// Whether this axis can be sized at all under the current mode. Resize
    /// needs a parameter to write; scale needs nothing.
    fn sizeable(&self, axis: usize) -> bool {
        self.gizmo.mode == Mode::Scale || self.drivable(axis).is_some()
    }

    /// The node's extent along one of its own axes **in world millimetres** at
    /// the moment the drag began -- the local extent times every scale between
    /// it and the world.
    ///
    /// World, because that is what a drag measures. The conversion back into a
    /// dimension, or into a factor, happens in one place, in `size_axis`.
    fn start_extent(&self, axis: usize) -> f64 {
        let local = get_axis(self.start_local_hi, axis) - get_axis(self.start_local_lo, axis);
        local * self.gizmo.axis_scale[axis]
    }

    /// Grow the given axis by `outward` world millimetres on the given side, and
    /// return the extent actually achieved.
    ///
    /// The two sizing modes part company only in what they write. **Resize**
    /// solves the driver's `extent = value * factor` for the value that produces
    /// the extent asked for -- that is the whole of "resize writes dimensions",
    /// and no scale factor is involved. **Scale** multiplies the node's own
    /// factor by however much bigger the extent got, which needs nothing of the
    /// shape underneath and so works on a group too.
    ///
    /// Both then shift the node by half the change, in the direction of the face
    /// being dragged, so the opposite face stays exactly where it was.
    fn size_axis(&self, scene: &mut Scene, axis: usize, outward: f64, symmetric: bool, positive: bool) -> Option<f64> {
        let scaling = self.gizmo.mode == Mode::Scale;
        let start_extent = self.start_extent(axis);
        // A dimension cannot go to zero or negative; clamp rather than let the
        // generator produce inverted geometry.
        let target_extent = (start_extent + outward * if symmetric { 2.0 } else { 1.0 }).max(MIN_EXTENT);

        if scaling {
            // Nothing to be a factor *of*: an axis with no extent cannot be
            // scaled into one.
            if start_extent <= MIN_EXTENT {
                return None;
            }
            let factor = target_extent / start_extent;
            let mut scale = self.start_scale;
            let grown = (get_axis(self.start_scale, axis) * factor).max(Node::MIN_SCALE);
            set_axis(&mut scale, axis, grown);
            scene.get_mut(self.node)?.scale = scale;
        } else {
            let driver = self.drivable(axis)?;
            if driver.factor.abs() < 1e-12 {
                return None;
            }
            let local_extent = target_extent / self.gizmo.axis_scale[axis];
            let params = scene.get_mut(self.node)?.params_mut()?;
            params.insert(driver.param.to_string(), ParamValue::Length(local_extent / driver.factor));
        }

        let node = scene.get_mut(self.node)?;
        if symmetric {
            node.position = self.start_position;
        } else {
            // `position` is in the parent's frame, so the shift is measured
            // there: the world change divided by whatever the ancestors scale
            // by, which is everything in `axis_scale` except this node's own.
            let ancestor = (self.gizmo.axis_scale[axis] / get_axis(self.start_scale, axis)).max(1e-9);
            let change = (target_extent - start_extent) / ancestor;
            let mut local_shift = Vec3::ZERO;
            set_axis(&mut local_shift, axis, change / 2.0 * if positive { 1.0 } else { -1.0 });
            let parent_shift = Xform::from_pos_rot(Vec3::ZERO, self.start_rotation).vector(local_shift);
            node.position = self.start_position + parent_shift;
        }
        Some(target_extent)
    }

    /// Write a world-space movement back to `Node::position`, which lives in the
    /// parent's frame.
    fn write_position(&self, scene: &mut Scene, world_delta: Vec3) {
        let parent = self.gizmo.parent;
        let target = parent.point(self.start_position) + world_delta;
        if let Some(node) = scene.get_mut(self.node) {
            node.position = parent.inverse().point(target);
        }
    }

    /// Escape during a drag restores the pre-drag values exactly (spec section 6.2).
    pub fn cancel(&self, scene: &mut Scene) {
        if let Some(node) = scene.get_mut(self.node) {
            node.position = self.start_position;
            node.rotation = self.start_rotation;
            node.scale = self.start_scale;
            if let Some(params) = node.params_mut() {
                *params = self.start_params.clone();
            }
        }
    }
}

/// The cursor's angle around a ring, in degrees, measured from the ring's first
/// in-plane axis.
fn ring_angle(gizmo: &Gizmo, axis: usize, view: &View, cursor: egui::Pos2) -> Option<f64> {
    let point = view.ray_plane(cursor, gizmo.origin, gizmo.axes[axis])?;
    let d = point - gizmo.origin;
    let (u, v) = other_axes(axis);
    let (x, y) = (d.dot(gizmo.axes[u]), d.dot(gizmo.axes[v]));
    if x.hypot(y) < 1e-9 {
        return None;
    }
    Some(y.atan2(x).to_degrees())
}

pub fn axis_name(axis: usize) -> &'static str {
    ["X", "Y", "Z"][axis]
}

/// Axis colours, matching the origin axes so a handle's meaning is obvious.
pub fn axis_colour(axis: usize) -> egui::Color32 {
    match axis {
        0 => egui::Color32::from_rgb(226, 92, 92),
        1 => egui::Color32::from_rgb(112, 200, 112),
        _ => egui::Color32::from_rgb(104, 152, 245),
    }
}

fn signed_length(mm: f64, unit: Unit) -> String {
    let text = format_length(mm.abs(), unit);
    let sign = if mm < 0.0 { "-" } else { "+" };
    format!("{sign}{text}{}", unit.suffix())
}

/// Nudging with the keyboard (spec section 6.2): the arrow keys act along the two
/// axes most closely aligned with the screen, and a further pair handles the
/// third. Returns the handle-frame axis index for screen-right, screen-up and
/// the remaining axis.
pub fn screen_aligned_axes(gizmo: &Gizmo, view: &View) -> [usize; 3] {
    let (right, up) = view.basis();
    let mut remaining: Vec<usize> = vec![0, 1, 2];
    let pick = |remaining: &mut Vec<usize>, direction: Vec3| -> usize {
        let (best_index, _) = remaining
            .iter()
            .enumerate()
            .max_by(|(_, &a), (_, &b)| {
                let sa = gizmo.axes[a].dot(direction).abs();
                let sb = gizmo.axes[b].dot(direction).abs();
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("at least one axis remains");
        remaining.remove(best_index)
    };
    let horizontal = pick(&mut remaining, right);
    let vertical = pick(&mut remaining, up);
    [horizontal, vertical, remaining[0]]
}

/// Whether the given handle-frame axis points right/up on screen, so a nudge
/// "left" really goes left.
pub fn axis_screen_sign(gizmo: &Gizmo, view: &View, axis: usize, vertical: bool) -> f64 {
    let (right, up) = view.basis();
    let direction = if vertical { up } else { right };
    if gizmo.axes[axis].dot(direction) < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// The smallest extent a resize nudge will leave behind, so a shape cannot be
/// nudged to zero or inside out.
const MIN_EXTENT: f64 = 1e-3;

/// What one press of a nudge key does, in the terms the caller has to write back
/// (spec section 6.2, acceptance criterion 26).
///
/// This lives here rather than in `App::nudge` so the arithmetic -- which axis,
/// which direction, and how far -- can be asserted from a test. `App` needs a
/// live `egui::Context` to construct, which makes anything inside it unreachable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nudge {
    /// Move the node this far in world space.
    Move { axis: usize, world_delta: Vec3 },
    /// Add this many degrees to the node's rotation about `axis`.
    Rotate { axis: usize, degrees: f64 },
    /// Rewrite `driver`'s parameter so the extent along `axis` becomes `extent`.
    Resize { axis: usize, driver: AxisDriver, extent: f64 },
    /// Set the node's own scale factor on `axis` to `factor`.
    Scale { axis: usize, factor: f64 },
    /// A resize nudge on an axis no parameter governs: nothing to write, and the
    /// caller says so rather than silently doing nothing.
    NoDimension { axis: usize },
}

impl Nudge {
    /// Write the step into the scene. `gizmo` must be the one it was computed
    /// from -- the move case needs its parent frame to turn a world delta back
    /// into `Node::position`, which lives in the parent's coordinates.
    ///
    /// Caller records the undo snapshot first, with `nudge_coalesce_key`.
    pub fn apply(self, gizmo: &Gizmo, scene: &mut Scene, id: NodeId) {
        match self {
            Nudge::Move { world_delta, .. } => {
                let Some(node) = scene.get(id) else { return };
                let target = gizmo.parent.point(node.position) + world_delta;
                let local = gizmo.parent.inverse().point(target);
                if let Some(node) = scene.get_mut(id) {
                    node.position = local;
                }
            }
            Nudge::Rotate { axis, degrees } => {
                if let Some(node) = scene.get_mut(id) {
                    let mut rotation = node.rotation;
                    let turned = get_axis(rotation, axis) + degrees;
                    set_axis(&mut rotation, axis, turned);
                    node.rotation = rotation;
                }
            }
            Nudge::Resize { driver, extent, .. } => {
                if let Some(params) = scene.get_mut(id).and_then(|n| n.params_mut()) {
                    params.insert(driver.param.to_string(), ParamValue::Length(extent / driver.factor));
                }
            }
            Nudge::Scale { axis, factor } => {
                if let Some(node) = scene.get_mut(id) {
                    let mut scale = Node::sane_scale(node.scale);
                    set_axis(&mut scale, axis, factor);
                    node.scale = scale;
                }
            }
            // Nothing this axis can be resized by. The caller says so; silently
            // doing nothing would look like a dropped keypress.
            Nudge::NoDimension { .. } => {}
        }
    }
}

/// The signed handle-frame axis a nudge command acts on: the axis index and
/// `±1`, already corrected against the view so "left" really goes left.
pub fn nudge_axis(gizmo: &Gizmo, view: &View, command: Command) -> Option<(usize, f64)> {
    let [horizontal, vertical, third] = screen_aligned_axes(gizmo, view);
    let (axis, mut sign) = match command {
        Command::NudgeLeft => (horizontal, -1.0),
        Command::NudgeRight => (horizontal, 1.0),
        Command::NudgeDown => (vertical, -1.0),
        Command::NudgeUp => (vertical, 1.0),
        Command::NudgeToward => (third, -1.0),
        Command::NudgeAway => (third, 1.0),
        _ => return None,
    };
    // The third axis has no screen direction to match, so it is left alone.
    let vertical_key = matches!(command, Command::NudgeUp | Command::NudgeDown);
    if vertical_key || matches!(command, Command::NudgeLeft | Command::NudgeRight) {
        sign *= axis_screen_sign(gizmo, view, axis, vertical_key);
    }
    Some((axis, sign))
}

/// One press of a nudge key. `move_snap` is the scene step, which governs both
/// the move step and the resize step; `rotate_snap_deg` governs rotation.
pub fn nudge_step(gizmo: &Gizmo, view: &View, command: Command, move_snap: f64, rotate_snap_deg: f64) -> Option<Nudge> {
    let (axis, sign) = nudge_axis(gizmo, view, command)?;
    Some(match gizmo.mode {
        Mode::Move => Nudge::Move { axis, world_delta: gizmo.axes[axis] * (move_snap * sign) },
        Mode::Rotate => Nudge::Rotate { axis, degrees: rotate_snap_deg * sign },
        Mode::Resize => match gizmo.drivers[axis] {
            Some(driver) => {
                let extent = get_axis(gizmo.local_hi, axis) - get_axis(gizmo.local_lo, axis);
                Nudge::Resize { axis, driver, extent: (extent + move_snap * sign).max(MIN_EXTENT) }
            }
            None => Nudge::NoDimension { axis },
        },
        // A scale nudge moves the face by the step, the same distance a resize
        // nudge would -- expressed as the factor that produces it, since that is
        // what a scale writes.
        Mode::Scale => {
            let local = get_axis(gizmo.local_hi, axis) - get_axis(gizmo.local_lo, axis);
            let world = local * gizmo.axis_scale[axis];
            if world <= MIN_EXTENT {
                Nudge::NoDimension { axis }
            } else {
                let factor = ((world + move_snap * sign).max(MIN_EXTENT)) / world;
                let grown = (get_axis(gizmo.own_scale, axis) * factor).max(Node::MIN_SCALE);
                Nudge::Scale { axis, factor: grown }
            }
        }
    })
}

/// The undo-coalescing key for a nudge. Every press within the coalescing window
/// that carries the same key extends one undo step, so holding an arrow key down
/// undoes in one (acceptance criterion 26). It deliberately does *not* mention
/// the direction: a run of left presses followed by right presses is still one
/// gesture, but nudging a different node, or in a different mode, is not.
pub fn nudge_coalesce_key(id: NodeId, mode: Mode) -> String {
    format!("nudge:{id}:{mode:?}")
}

/// The pointer facts the viewport panel reads off an `egui::Response` each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerState {
    /// Escape was pressed this frame.
    pub escape: bool,
    /// The drag button came up.
    pub released: bool,
    /// A drag started this frame.
    pub started: bool,
    /// The cursor is over a manipulator handle.
    pub on_handle: bool,
    /// There is a cursor position at all -- the pointer may be off the window.
    pub have_cursor: bool,
}

/// What the pointer is asking the manipulator to do this frame.
///
/// Split out of `panel_viewport::manipulate` so the begin/continue/finish
/// bookkeeping can be asserted. An `egui::Response` cannot be built outside a
/// running frame, and that is what kept acceptance criterion 23's last clause --
/// "a completed drag undoes in one step" -- untested: the single undo record
/// happens on `Begin` and on no other phase, which is the whole mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    /// Escape during a drag: put the pre-drag values back exactly.
    Cancel,
    /// The button came up: the drag is over.
    Finish,
    /// A drag is running: follow the cursor.
    Continue,
    /// A handle was grabbed: open one undo step and start.
    Begin,
    /// Nothing for the manipulator to do.
    Idle,
}

/// Which phase a frame is in, given whether a drag is already running.
///
/// The ordering is the part that matters. Escape beats release, so a cancel is
/// never mistaken for a completed drag; and `Begin` requires that no drag is
/// running, which is what stops a second undo step opening mid-gesture.
pub fn drag_phase(dragging: bool, pointer: PointerState) -> DragPhase {
    if dragging {
        if pointer.escape {
            return DragPhase::Cancel;
        }
        if pointer.released {
            return DragPhase::Finish;
        }
        return if pointer.have_cursor { DragPhase::Continue } else { DragPhase::Idle };
    }
    if pointer.started && pointer.on_handle && pointer.have_cursor {
        DragPhase::Begin
    } else {
        DragPhase::Idle
    }
}

/// One press of a nudge key, all the way through: work out the step, open or
/// extend the undo run it belongs to, and write it into the scene.
///
/// This is `App::nudge` minus the parts that need a live `egui::Context` -- the
/// selection, the status line and the field cache. Keeping the undo record here
/// rather than at the call site is what lets criterion 26's "the whole repeat run
/// is a single undo step" be asserted against the code that actually runs.
/// Returns the step taken, so the caller can report an axis with no dimension.
pub fn apply_nudge(
    history: &mut History,
    scene: &mut Scene,
    gizmo: &Gizmo,
    view: &View,
    id: NodeId,
    command: Command,
    move_snap: f64,
    rotate_snap_deg: f64,
) -> Option<Nudge> {
    let step = nudge_step(gizmo, view, command, move_snap, rotate_snap_deg)?;
    // Record before mutating, under a key stable across the whole held run.
    history.record(scene, "Nudge", Some(&nudge_coalesce_key(id, gizmo.mode)));
    step.apply(gizmo, scene, id);
    Some(step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::eval::{Cancel, Evaluator};
    use simple3d_core::primitive::ParamsExt;
    use simple3d_core::scene::Camera;

    struct Fixture {
        scene: Scene,
        evaluator: Evaluator,
        evaluated: Evaluated,
        node: NodeId,
        view: View,
    }

    impl Fixture {
        fn new(type_id: &str) -> Fixture {
            let mut scene = Scene::new();
            let root = scene.root();
            let node = scene.add_primitive(type_id, root, 0).unwrap();
            scene.camera = Camera { yaw: -55.0, pitch: 28.0, distance: 160.0, ..Camera::default() };
            let mut evaluator = Evaluator::new();
            let evaluated = evaluator.evaluate(&scene, &Cancel::new());
            let view = View::new(scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));
            Fixture { scene, evaluator, evaluated, node, view }
        }

        fn reevaluate(&mut self) {
            self.evaluated = self.evaluator.evaluate(&self.scene, &Cancel::new());
        }

        fn gizmo(&self, mode: Mode) -> Gizmo {
            Gizmo::build(&self.scene, &self.evaluated, self.node, mode, false).unwrap()
        }

        fn param(&self, key: &str) -> f64 {
            self.scene.node(self.node).params().unwrap().num(key)
        }

        fn world_bounds(&self) -> (Vec3, Vec3) {
            self.evaluated.node_meshes[&self.node].bounds().unwrap()
        }
    }

    /// Drag a handle from where it sits to where the given world point projects.
    fn drag_to(f: &mut Fixture, handle: Handle, target: Vec3, mods: Mods, snap: f64) -> Drag {
        let mode = match handle {
            Handle::MoveAxis(_) | Handle::MovePlane(_) => Mode::Move,
            Handle::RotateRing(_) => Mode::Rotate,
            _ => Mode::Resize,
        };
        drag_in(f, mode, handle, target, mods, snap)
    }

    /// The same, in a mode the handle does not imply: a face handle is a resize
    /// in one mode and a scale in the other.
    fn drag_in(f: &mut Fixture, mode: Mode, handle: Handle, target: Vec3, mods: Mods, snap: f64) -> Drag {
        let gizmo = f.gizmo(mode);
        let from = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
        let to = f.view.project(target).unwrap().0;
        let mut drag = Drag::begin(&f.scene, &gizmo, f.node, handle, &f.view, from).unwrap();
        drag.update(&mut f.scene, &f.view, to, mods, snap, 15.0, Unit::Millimetre);
        f.reevaluate();
        drag
    }

    #[test]
    fn a_move_axis_drag_moves_only_that_axis() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let handle = Handle::MoveAxis(0);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(30.0, 0.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
        let position = f.scene.node(f.node).position;
        assert!((position.x - 30.0).abs() < 0.2, "{position:?}");
        assert!(position.y.abs() < 1e-6 && position.z.abs() < 1e-6, "other axes moved: {position:?}");
    }

    #[test]
    fn a_move_drag_snaps_to_the_increment_and_a_modifier_frees_it() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let handle = Handle::MoveAxis(0);
        let start = gizmo.handle_point(handle, &f.view);
        let target = start + Vec3::new(23.0, 0.0, 0.0);

        drag_to(&mut f, handle, target, Mods::default(), 10.0);
        assert!(
            (f.scene.node(f.node).position.x - 20.0).abs() < 1e-9,
            "snapped to {:?}",
            f.scene.node(f.node).position
        );

        let mut fresh = Fixture::new("box");
        drag_to(&mut fresh, handle, target, Mods { free: true, ..Default::default() }, 10.0);
        let free = fresh.scene.node(fresh.node).position.x;
        assert!((free - 23.0).abs() < 0.2 && (free - 20.0).abs() > 1.0, "free drag snapped anyway: {free}");

        let mut coarse = Fixture::new("box");
        drag_to(
            &mut coarse,
            handle,
            start + Vec3::new(63.0, 0.0, 0.0),
            Mods { coarse: true, ..Default::default() },
            10.0,
        );
        assert!((coarse.scene.node(coarse.node).position.x - 100.0).abs() < 1e-9, "coarse snap");
    }

    #[test]
    fn a_plane_handle_moves_two_axes_and_leaves_the_third() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let handle = Handle::MovePlane(2); // the XY plane
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(20.0, 30.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
        let p = f.scene.node(f.node).position;
        assert!((p.x - 20.0).abs() < 0.3 && (p.y - 30.0).abs() < 0.3, "{p:?}");
        assert!(p.z.abs() < 1e-6, "the plane's normal axis moved: {p:?}");
    }

    #[test]
    fn escape_restores_the_pre_drag_state_exactly() {
        // Spec acceptance criterion 23.
        let mut f = Fixture::new("box");
        let before_position = f.scene.node(f.node).position;
        let before_params = f.scene.node(f.node).params().cloned().unwrap();
        let gizmo = f.gizmo(Mode::Move);
        let handle = Handle::MoveAxis(1);
        let start = gizmo.handle_point(handle, &f.view);
        let drag = drag_to(&mut f, handle, start + Vec3::new(0.0, 37.0, 0.0), Mods::default(), 10.0);
        assert_ne!(f.scene.node(f.node).position, before_position);
        drag.cancel(&mut f.scene);
        assert_eq!(f.scene.node(f.node).position, before_position);
        assert_eq!(f.scene.node(f.node).params().unwrap(), &before_params);
    }

    #[test]
    fn dragging_the_right_face_changes_the_width_and_leaves_the_left_face() {
        // Spec acceptance criterion 24, the central one for this module.
        let mut f = Fixture::new("box");
        let (lo_before, hi_before) = f.world_bounds();
        assert!((hi_before.x - lo_before.x - 20.0).abs() < 1e-9);

        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeFace(0, true);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(10.0, 0.0, 0.0), Mods::default(), 10.0);

        assert!((f.param("width") - 30.0).abs() < 1e-9, "width is {}", f.param("width"));
        let (lo_after, hi_after) = f.world_bounds();
        assert!((lo_after.x - lo_before.x).abs() < 1e-9, "the left face moved: {} -> {}", lo_before.x, lo_after.x);
        assert!((hi_after.x - hi_before.x - 10.0).abs() < 1e-9, "the right face did not follow");
        // Nothing else changed.
        assert!((hi_after.y - lo_after.y - 20.0).abs() < 1e-9);
        assert!((hi_after.z - lo_after.z - 20.0).abs() < 1e-9);
    }

    #[test]
    fn dragging_the_left_face_leaves_the_right_face() {
        let mut f = Fixture::new("box");
        let (_, hi_before) = f.world_bounds();
        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeFace(0, false);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(-10.0, 0.0, 0.0), Mods::default(), 10.0);
        assert!((f.param("width") - 30.0).abs() < 1e-9, "width is {}", f.param("width"));
        let (lo_after, hi_after) = f.world_bounds();
        assert!((hi_after.x - hi_before.x).abs() < 1e-9, "the right face moved");
        assert!((hi_after.x - lo_after.x - 30.0).abs() < 1e-9);
    }

    #[test]
    fn the_symmetry_modifier_grows_about_the_centre() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeFace(0, true);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(
            &mut f,
            handle,
            start + Vec3::new(10.0, 0.0, 0.0),
            Mods { symmetric: true, ..Default::default() },
            10.0,
        );
        assert!((f.param("width") - 40.0).abs() < 1e-9, "width is {}", f.param("width"));
        let (lo, hi) = f.world_bounds();
        assert!(((lo.x + hi.x) / 2.0).abs() < 1e-9, "the centre moved: {lo:?} {hi:?}");
        assert_eq!(f.scene.node(f.node).position, Vec3::ZERO);
    }

    #[test]
    fn a_corner_drag_with_the_proportions_modifier_keeps_the_ratio() {
        // Spec acceptance criterion 25.
        let mut f = Fixture::new("plate");
        let ratio_before = f.param("width") / f.param("depth");
        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeCorner([true, true, true]);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(
            &mut f,
            handle,
            start + Vec3::new(15.0, 8.0, 0.0),
            Mods { symmetric: true, free: true, ..Default::default() },
            10.0,
        );
        let ratio_after = f.param("width") / f.param("depth");
        assert!((ratio_after - ratio_before).abs() < 1e-6, "{ratio_before} -> {ratio_after}");
        assert!(f.param("width") > 40.0, "the corner drag did nothing: {}", f.param("width"));
        // The third dimension scaled by the same ratio too.
        let scale = f.param("width") / 40.0;
        assert!((f.param("thickness") - 4.0 * scale).abs() < 1e-6);
    }

    #[test]
    fn a_scale_drag_writes_a_factor_and_leaves_the_dimension_alone() {
        // The difference between the two sizing tools, in one assertion: resize
        // rewrites the number the shape is defined by, scale multiplies what is
        // there and leaves that number where it was.
        let mut f = Fixture::new("box");
        let width = f.param("width");
        let (lo, hi) = f.world_bounds();
        let handle = Handle::ResizeFace(0, true);
        let gizmo = f.gizmo(Mode::Scale);
        let face = gizmo.handle_point(handle, &f.view);

        // Out along +X by the width again: twice as wide.
        drag_in(&mut f, Mode::Scale, handle, face + Vec3::new(width, 0.0, 0.0), Mods::default(), 1.0);

        assert!((f.param("width") - width).abs() < 1e-9, "scaling rewrote the dimension");
        let scale = f.scene.node(f.node).scale;
        assert!((scale.x - 2.0).abs() < 0.05, "the X factor came out {}", scale.x);
        assert_eq!((scale.y, scale.z), (1.0, 1.0), "one axis was dragged and three were scaled");

        let (nlo, nhi) = f.world_bounds();
        assert!((nhi.x - nlo.x - (hi.x - lo.x) * 2.0).abs() < 0.2, "it is not twice as wide on screen");
        // The face that was not dragged did not move.
        assert!((nlo.x - lo.x).abs() < 0.05, "the opposite face moved: {nlo:?} vs {lo:?}");
    }

    #[test]
    fn a_group_can_be_scaled_although_it_cannot_be_resized() {
        // Resize refuses a group -- scaling one truthfully would mean rewriting
        // every descendant. A factor needs none of that, and it is the only way
        // to make an assembly a proportion of what it was.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(simple3d_core::scene::GroupOp::Union, root, 0);
        let child = scene.add_primitive("box", group, 0).unwrap();
        scene.get_mut(child).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
        scene.camera = Camera { yaw: -55.0, pitch: 28.0, distance: 160.0, ..Camera::default() };
        let mut evaluator = Evaluator::new();
        let evaluated = evaluator.evaluate(&scene, &Cancel::new());
        let view = View::new(scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));

        assert!(
            Gizmo::build(&scene, &evaluated, group, Mode::Resize, false).unwrap().handles(true).is_empty(),
            "resize offered a group handles"
        );
        let gizmo = Gizmo::build(&scene, &evaluated, group, Mode::Scale, false).unwrap();
        assert!(!gizmo.handles(true).is_empty(), "scale offered a group no handles");

        let handle = Handle::ResizeFace(0, true);
        let face = gizmo.handle_point(handle, &view);
        let from = view.project(face).unwrap().0;
        let to = view.project(face + Vec3::new(20.0, 0.0, 0.0)).unwrap().0;
        let mut drag = Drag::begin(&scene, &gizmo, group, handle, &view, from).unwrap();
        drag.update(&mut scene, &view, to, Mods { free: true, ..Default::default() }, 1.0, 15.0, Unit::Millimetre);

        assert!(scene.node(group).scale.x > 1.5, "the group was not scaled: {:?}", scene.node(group).scale);
        let after = evaluator.evaluate(&scene, &Cancel::new());
        let (lo, hi) = after.node_meshes[&child].bounds().unwrap();
        assert!((hi.x - lo.x) > 12.0, "the child did not grow with the group it is in");
    }

    #[test]
    fn a_resize_on_a_scaled_node_still_lands_on_the_dimension_that_was_dragged_to() {
        // A drag is measured on screen, and the screen shows the node *after*
        // its scale. Writing the on-screen distance straight into the dimension
        // would resize by the factor again.
        let mut f = Fixture::new("box");
        f.scene.get_mut(f.node).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);
        f.reevaluate();
        let handle = Handle::ResizeFace(0, true);
        let gizmo = f.gizmo(Mode::Resize);
        let face = gizmo.handle_point(handle, &f.view);
        let width = f.param("width");

        // Ten world millimetres out is five of the node's own, at a factor of 2.
        drag_in(
            &mut f,
            Mode::Resize,
            handle,
            face + Vec3::new(10.0, 0.0, 0.0),
            Mods { free: true, ..Default::default() },
            1.0,
        );
        assert!((f.param("width") - (width + 5.0)).abs() < 0.2, "the width came out {}", f.param("width"));
        assert_eq!(f.scene.node(f.node).scale, Vec3::new(2.0, 1.0, 1.0), "the resize touched the scale");
    }

    #[test]
    fn a_scale_cannot_be_dragged_to_zero_or_through_it() {
        let mut f = Fixture::new("box");
        let handle = Handle::ResizeFace(0, true);
        let gizmo = f.gizmo(Mode::Scale);
        let face = gizmo.handle_point(handle, &f.view);
        // Far past the opposite face, which would invert the solid.
        drag_in(
            &mut f,
            Mode::Scale,
            handle,
            face - Vec3::new(500.0, 0.0, 0.0),
            Mods { free: true, ..Default::default() },
            1.0,
        );
        assert!(f.scene.node(f.node).scale.x >= Node::MIN_SCALE, "{:?}", f.scene.node(f.node).scale);
        assert!(f.world_bounds().1.x > f.world_bounds().0.x, "the solid was turned inside out");
    }

    #[test]
    fn escape_puts_a_scale_back_where_a_drag_found_it() {
        let mut f = Fixture::new("box");
        f.scene.get_mut(f.node).unwrap().scale = Vec3::new(1.5, 1.0, 1.0);
        f.reevaluate();
        let handle = Handle::ResizeFace(0, true);
        let target = f.gizmo(Mode::Scale).handle_point(handle, &f.view) + Vec3::new(25.0, 0.0, 0.0);
        let drag = drag_in(&mut f, Mode::Scale, handle, target, Mods::default(), 1.0);
        assert_ne!(f.scene.node(f.node).scale.x, 1.5);
        drag.cancel(&mut f.scene);
        assert_eq!(f.scene.node(f.node).scale, Vec3::new(1.5, 1.0, 1.0));
    }

    #[test]
    fn resizing_never_writes_a_scale_factor_into_the_project() {
        // The other half of criterion 24: the saved file has no scale anywhere.
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeFace(2, true);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(0.0, 0.0, 20.0), Mods::default(), 10.0);
        let text = simple3d_core::project::to_string(&f.scene);
        assert!(!text.to_lowercase().contains("scale"), "a scale factor reached the project file");
        assert!(text.contains("\"height\""));
    }

    #[test]
    fn no_resize_handle_is_offered_on_an_axis_no_parameter_governs() {
        // A torus's X and Y extents are its ring and tube diameters together, so
        // the registry withdraws those handles rather than offer one that lies.
        let f = Fixture::new("torus");
        let gizmo = f.gizmo(Mode::Resize);
        let handles = gizmo.handles(false);
        assert!(handles.contains(&Handle::ResizeFace(2, true)), "the Z handle should be offered");
        assert!(!handles.contains(&Handle::ResizeFace(0, true)), "an X handle was offered on a torus");
        assert!(!handles.contains(&Handle::ResizeFace(1, false)));
        // With only one drivable axis there is nothing for a corner to do either.
        assert!(!handles.iter().any(|h| matches!(h, Handle::ResizeCorner(_))));
    }

    #[test]
    fn a_polyhedron_offers_no_resize_handles_at_all() {
        let f = Fixture::new("icosahedron");
        assert!(f.gizmo(Mode::Resize).handles(false).is_empty());
        // But it can still be moved and rotated.
        assert_eq!(f.gizmo(Mode::Move).handles(false).len(), 6);
        assert_eq!(f.gizmo(Mode::Rotate).handles(false).len(), 3);
    }

    #[test]
    fn groups_get_move_and_rotate_but_not_resize() {
        // Spec section 6.2: resize handles on groups are out of scope.
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        assert!(gizmo.handles(true).is_empty());
        assert!(!gizmo.handles(false).is_empty());
        let move_gizmo = f.gizmo(Mode::Move);
        assert_eq!(move_gizmo.handles(true).len(), 6);
        assert_eq!(f.gizmo(Mode::Rotate).handles(true).len(), 3);
    }

    #[test]
    fn a_rotate_drag_snaps_to_fifteen_degrees_by_default() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Rotate);
        let ring = gizmo.ring_points(2, &f.view, 72);
        let from = f.view.project(ring[0]).unwrap().0;
        // A little over 30 degrees around the ring.
        let to = f.view.project(ring[7]).unwrap().0;
        let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
        drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
        let z = f.scene.node(f.node).rotation.z;
        assert!((z % 15.0).abs() < 1e-9, "not snapped to 15 degrees: {z}");
        assert!(z > 0.0, "rotated the wrong way: {z}");
        assert!(drag.readout.contains("deg"), "{}", drag.readout);
    }

    #[test]
    fn a_free_rotate_drag_is_not_snapped() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Rotate);
        let ring = gizmo.ring_points(2, &f.view, 72);
        let from = f.view.project(ring[0]).unwrap().0;
        let to = f.view.project(ring[7]).unwrap().0;
        let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
        drag.update(&mut f.scene, &f.view, to, Mods { free: true, ..Default::default() }, 10.0, 15.0, Unit::Millimetre);
        let z = f.scene.node(f.node).rotation.z;
        assert!(z > 20.0 && z < 45.0, "{z}");
        assert!((z % 15.0).abs() > 1e-6, "a free drag snapped anyway: {z}");
    }

    #[test]
    fn handles_keep_a_constant_screen_size_as_the_camera_pulls_back() {
        let mut f = Fixture::new("box");
        let near_arm = {
            let gizmo = f.gizmo(Mode::Move);
            let point = gizmo.handle_point(Handle::MoveAxis(0), &f.view);
            (f.view.project(point).unwrap().0 - f.view.project(gizmo.origin).unwrap().0).length()
        };
        f.scene.camera.distance = 900.0;
        f.view = View::new(f.scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));
        f.reevaluate();
        let far_arm = {
            let gizmo = f.gizmo(Mode::Move);
            let point = gizmo.handle_point(Handle::MoveAxis(0), &f.view);
            (f.view.project(point).unwrap().0 - f.view.project(gizmo.origin).unwrap().0).length()
        };
        assert!((near_arm - far_arm).abs() < 2.0, "{near_arm} vs {far_arm} pixels");
    }

    #[test]
    fn hit_testing_finds_the_handle_under_the_cursor_and_nothing_far_from_one() {
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        for handle in [Handle::MoveAxis(0), Handle::MoveAxis(2), Handle::MovePlane(1)] {
            let screen = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
            assert_eq!(gizmo.hit_test(&f.view, screen, false), Some(handle), "{handle:?}");
        }
        assert_eq!(gizmo.hit_test(&f.view, egui::pos2(5.0, 5.0), false), None);
    }

    #[test]
    fn a_corner_wins_over_the_face_it_sits_on() {
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        let corner = Handle::ResizeCorner([true, true, true]);
        let screen = f.view.project(gizmo.handle_point(corner, &f.view)).unwrap().0;
        assert_eq!(gizmo.hit_test(&f.view, screen, false), Some(corner));
    }

    #[test]
    fn a_rotate_ring_is_grabbable_along_its_whole_circumference() {
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Rotate);
        let points = gizmo.ring_points(1, &f.view, 24);
        for (i, point) in points.iter().enumerate() {
            let screen = f.view.project(*point).unwrap().0;
            let hit = gizmo.hit_test(&f.view, screen, false);
            assert!(matches!(hit, Some(Handle::RotateRing(_))), "{point:?} grabbed {hit:?}");
            // The three rings genuinely cross where they meet an axis, and there
            // the nearest one legitimately wins; away from those crossings it must
            // be this ring.
            let near_axis = i % 6 <= 1 || i % 6 >= 5;
            if !near_axis {
                assert_eq!(hit, Some(Handle::RotateRing(1)), "{point:?}");
            }
        }
    }

    #[test]
    fn the_handle_frame_follows_the_nodes_own_rotation_or_the_world() {
        let mut f = Fixture::new("box");
        f.scene.get_mut(f.node).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
        f.reevaluate();
        let object = Gizmo::build(&f.scene, &f.evaluated, f.node, Mode::Move, false).unwrap();
        let world = Gizmo::build(&f.scene, &f.evaluated, f.node, Mode::Move, true).unwrap();
        // The node's local X now points along world +Y.
        assert!((object.axes[0] - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-9, "{:?}", object.axes[0]);
        assert!((world.axes[0] - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn a_move_drag_on_a_rotated_child_writes_parent_frame_coordinates() {
        let mut f = Fixture::new("box");
        let root = f.scene.root();
        let group = f.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
        f.scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
        f.scene.reparent(f.node, group, 0).unwrap();
        f.reevaluate();

        let gizmo = f.gizmo(Mode::Move);
        // Drag along the child's local X, which the group has turned into world +Y.
        let handle = Handle::MoveAxis(0);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(0.0, 30.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
        let position = f.scene.node(f.node).position;
        // Stored in the parent's frame, so it reads as +30 on X, not on Y.
        assert!((position.x - 30.0).abs() < 0.3, "{position:?}");
        assert!(position.y.abs() < 0.3, "{position:?}");
        // And the geometry really moved along world +Y.
        let (lo, hi) = f.world_bounds();
        assert!(((lo.y + hi.y) / 2.0 - 30.0).abs() < 0.3, "{lo:?} {hi:?}");
    }

    #[test]
    fn the_screen_aligned_axes_are_a_permutation_matched_to_the_view() {
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let [horizontal, vertical, third] = screen_aligned_axes(&gizmo, &f.view);
        let mut sorted = [horizontal, vertical, third];
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2], "not a permutation");
        // From the default isometric-ish view, Z is the most vertical axis.
        assert_eq!(vertical, 2, "expected Z to be the screen-vertical axis");
        let (right, _) = f.view.basis();
        assert!(
            gizmo.axes[horizontal].dot(right).abs() > gizmo.axes[third].dot(right).abs(),
            "the horizontal axis is not the most horizontal one"
        );
    }

    #[test]
    fn a_nudge_left_really_goes_left() {
        let f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let [horizontal, vertical, _] = screen_aligned_axes(&gizmo, &f.view);
        let (right, up) = f.view.basis();
        let h_sign = axis_screen_sign(&gizmo, &f.view, horizontal, false);
        assert!((gizmo.axes[horizontal] * h_sign).dot(right) > 0.0);
        let v_sign = axis_screen_sign(&gizmo, &f.view, vertical, true);
        assert!((gizmo.axes[vertical] * v_sign).dot(up) > 0.0);
    }

    /// Spec acceptance criterion 26: nudge with the arrow keys, hold to repeat,
    /// the step matches the snap increment, and the whole repeat run is a single
    /// undo step.
    ///
    /// The run goes through `apply_nudge`, which is the whole of what `App::nudge`
    /// does apart from the status line, so this asserts the real path -- undo
    /// record included -- rather than a re-implementation of it.
    #[test]
    fn a_held_nudge_run_steps_by_the_snap_and_undoes_in_one() {
        const SNAP: f64 = 2.5;
        const PRESSES: usize = 12;

        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let start = f.scene.node(f.node).position;

        let mut history = History::new();
        // Something before the run, so "one step" is distinguishable from
        // "undo emptied the stack".
        history.record(&f.scene, "Before", None);

        // Holding the key down: the key repeat delivers the same command over
        // and over, and each press goes through the whole production path.
        for _ in 0..PRESSES {
            let step =
                apply_nudge(&mut history, &mut f.scene, &gizmo, &f.view, f.node, Command::NudgeRight, SNAP, 15.0)
                    .expect("a nudge command");
            let Nudge::Move { axis, world_delta } = step else { panic!("move mode gave {step:?}") };
            assert!(
                (world_delta.length() - SNAP).abs() < 1e-9,
                "one press moved {} mm, not the {SNAP} mm snap increment",
                world_delta.length()
            );
            assert!((world_delta * (1.0 / SNAP) - gizmo.axes[axis]).length() < 1e-9, "the step left its axis");
        }

        // Every press moved: the run really is a run, not one press repeated
        // into the same place.
        let after = f.scene.node(f.node).position;
        let travelled = (gizmo.parent.point(after) - gizmo.parent.point(start)).length();
        assert!(
            (travelled - SNAP * PRESSES as f64).abs() < 1e-6,
            "{PRESSES} presses travelled {travelled} mm, expected {}",
            SNAP * PRESSES as f64
        );

        // ... and all of it undoes at once.
        assert_eq!(history.undo(&mut f.scene).as_deref(), Some("Nudge"));
        assert_eq!(f.scene.node(f.node).position, start, "one undo did not restore the pre-run position");
        assert_eq!(history.undo_label(), Some("Before"), "the run left more than one undo step behind");

        // Redo puts the whole run back, also in one.
        assert_eq!(history.redo(&mut f.scene).as_deref(), Some("Nudge"));
        assert_eq!(f.scene.node(f.node).position, after);
    }

    /// The coalesce key is what makes the run one step, so what it does and does
    /// not merge is worth pinning down: changing direction mid-run is still one
    /// gesture, but a different node or a different mode starts a new step.
    #[test]
    fn a_nudge_coalesces_across_directions_but_not_across_nodes_or_modes() {
        let mut f = Fixture::new("box");
        let root = f.scene.root();
        let other = f.scene.add_primitive("box", root, 1).unwrap();
        f.reevaluate();

        let key = nudge_coalesce_key(f.node, Mode::Move);
        assert_eq!(key, nudge_coalesce_key(f.node, Mode::Move), "the key is not stable across presses");
        assert_ne!(key, nudge_coalesce_key(other, Mode::Move), "two nodes share a coalesce key");
        assert_ne!(key, nudge_coalesce_key(f.node, Mode::Rotate), "two modes share a coalesce key");

        let move_gizmo = f.gizmo(Mode::Move);
        let rotate_gizmo = f.gizmo(Mode::Rotate);
        let other_gizmo = Gizmo::build(&f.scene, &f.evaluated, other, Mode::Move, false).unwrap();

        // Left then right then left: one gesture, whatever the direction.
        let mut history = History::new();
        for command in [Command::NudgeLeft, Command::NudgeRight, Command::NudgeLeft] {
            apply_nudge(&mut history, &mut f.scene, &move_gizmo, &f.view, f.node, command, 1.0, 15.0).unwrap();
        }
        // Switching mode, and switching node, each start a step of their own.
        apply_nudge(&mut history, &mut f.scene, &rotate_gizmo, &f.view, f.node, Command::NudgeUp, 1.0, 15.0).unwrap();
        apply_nudge(&mut history, &mut f.scene, &other_gizmo, &f.view, other, Command::NudgeUp, 1.0, 15.0).unwrap();

        let mut steps = 0;
        while history.undo(&mut f.scene).is_some() {
            steps += 1;
        }
        assert_eq!(steps, 3, "expected the three same-key presses to merge and nothing else to");
    }

    /// Criterion 26's "step matches the snap increment" for the other two modes:
    /// rotate steps by the rotation snap, resize by the scene step.
    #[test]
    fn a_nudge_steps_by_the_snap_in_rotate_and_resize_too() {
        let mut f = Fixture::new("box");

        let gizmo = f.gizmo(Mode::Rotate);
        let step = nudge_step(&gizmo, &f.view, Command::NudgeUp, 2.5, 15.0).expect("a nudge command");
        let Nudge::Rotate { axis, degrees } = step else { panic!("rotate mode gave {step:?}") };
        assert_eq!(degrees.abs(), 15.0, "rotate did not step by the rotation snap");
        let before = get_axis(f.scene.node(f.node).rotation, axis);
        step.apply(&gizmo, &mut f.scene, f.node);
        assert!((get_axis(f.scene.node(f.node).rotation, axis) - (before + degrees)).abs() < 1e-9);

        // A fresh box: the rotation above would leave the world bounds an AABB
        // around a turned solid, which is not the extent being asserted.
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        let (axis, _) = nudge_axis(&gizmo, &f.view, Command::NudgeRight).unwrap();
        let extent = get_axis(gizmo.local_hi, axis) - get_axis(gizmo.local_lo, axis);
        let step = nudge_step(&gizmo, &f.view, Command::NudgeRight, 2.5, 15.0).expect("a nudge command");
        let Nudge::Resize { extent: target, driver, .. } = step else { panic!("resize mode gave {step:?}") };
        assert!((target - (extent + 2.5)).abs() < 1e-9, "resize did not step by the scene step");
        let before = f.param(driver.param);
        step.apply(&gizmo, &mut f.scene, f.node);
        f.reevaluate();
        // The box is unrotated, so its local axes are the world ones.
        let (lo, hi) = f.world_bounds();
        let measured = get_axis(hi, axis) - get_axis(lo, axis);
        assert!((measured - (extent + 2.5)).abs() < 1e-6, "the measured extent {measured} did not follow the nudge");
        // Criterion 24's rule holds for the keyboard too: a dimension changed,
        // not a scale factor.
        assert!((f.param(driver.param) - (before + 2.5 / driver.factor)).abs() < 1e-9, "resizing wrote no dimension");
    }

    /// A resize nudge on an axis no parameter governs reports itself rather than
    /// looking like a dropped keypress -- and changes nothing.
    #[test]
    fn a_resize_nudge_on_an_ungoverned_axis_says_so() {
        let mut f = Fixture::new("sphere");
        let gizmo = f.gizmo(Mode::Resize);
        // A sphere's one radius drives all three axes, so pick a primitive that
        // genuinely lacks one if this fixture does not.
        let ungoverned = (0..3).find(|&a| gizmo.drivers[a].is_none());
        let Some(axis) = ungoverned else { return };
        let before = f.scene.node(f.node).params().unwrap().clone();
        Nudge::NoDimension { axis }.apply(&gizmo, &mut f.scene, f.node);
        assert_eq!(f.scene.node(f.node).params().unwrap(), &before, "an ungoverned axis still wrote something");
    }

    #[test]
    fn the_readout_is_in_the_display_unit() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Move);
        let handle = Handle::MoveAxis(0);
        let from = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
        let to = f.view.project(gizmo.handle_point(handle, &f.view) + Vec3::new(20.0, 0.0, 0.0)).unwrap().0;
        let mut drag = Drag::begin(&f.scene, &gizmo, f.node, handle, &f.view, from).unwrap();
        drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Centimetre);
        assert_eq!(drag.readout, "X +2cm", "{}", drag.readout);
        drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
        assert_eq!(drag.readout, "X +20mm");
    }

    #[test]
    fn a_dimension_cannot_be_dragged_to_zero_or_negative() {
        let mut f = Fixture::new("box");
        let gizmo = f.gizmo(Mode::Resize);
        let handle = Handle::ResizeFace(0, true);
        let start = gizmo.handle_point(handle, &f.view);
        drag_to(&mut f, handle, start + Vec3::new(-500.0, 0.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
        assert!(f.param("width") > 0.0, "width went to {}", f.param("width"));
        let (lo, hi) = f.world_bounds();
        assert!(hi.x >= lo.x, "the shape inverted");
    }

    #[test]
    fn the_gizmo_is_not_offered_for_the_scene_root() {
        let f = Fixture::new("box");
        assert!(Gizmo::build(&f.scene, &f.evaluated, f.scene.root(), Mode::Move, false).is_none());
    }

    #[test]
    fn modifier_snapping_is_what_the_table_in_the_doc_comment_says() {
        assert_eq!(Mods::default().snap(23.0, 10.0), 20.0);
        assert_eq!(Mods { free: true, ..Default::default() }.snap(23.0, 10.0), 23.0);
        assert_eq!(Mods { coarse: true, ..Default::default() }.snap(63.0, 10.0), 100.0);
        // A zero increment cannot snap, and must not divide by zero.
        assert_eq!(Mods::default().snap(23.0, 0.0), 23.0);
    }
}
