//! Putting a new shape down on a surface (issue 117): under the "On a surface" placement, a shape
//! just added follows the pointer, resting on whatever face of the model is under it -- turned so
//! its bottom faces it -- or on the ground, until a click puts it down.
//!
//! On a flat face the shape turns along its edges, and with the snap key its middle catches the
//! face's centre, corners and edges (issue 128, `face`).
//!
//! The shape is moved in the document as it follows, as a drag moves a body, and the GPU draws it
//! moved without evaluating each step (`App::live_move`). Escape takes the addition back.

use crate::app::{App, Status};
use crate::snap::{on_grid, SnapMark, DRAG_CATCH_PIXELS};
use crate::view::View;
use simple3d_core::config::Placement;
use simple3d_core::scene::{Anchor, Node, NodeId};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

mod face;
use face::Face;
mod outline;

/// The shape being put down.
#[derive(Clone, Debug)]
pub(crate) struct Placing {
    pub(crate) id: NodeId,
    /// Its box in its own frame, after its anchor and before its scale, measured once here since it
    /// has not been evaluated yet.
    local: (Vec3, Vec3),
    /// The frame its parent puts it in.
    frame: Xform,
    /// The undo history's revision just after the addition, so Escape only takes back that.
    revision: u64,
    /// The face whose snap position was caught last frame, which keeps its catch a little longer.
    held: Option<Face>,
}

impl App {
    /// Let a just-added shape follow the pointer, under the "On a surface" placement. Nothing that
    /// has no shape yet (an empty group or pattern) follows.
    pub(crate) fn start_placing(&mut self, id: NodeId) {
        if self.settings.placement == Placement::OnSurface {
            self.start_placing_now(id);
        }
    }

    /// Let `id` follow the pointer whatever the document's placement, as the add popup asks for.
    pub(crate) fn start_placing_now(&mut self, id: NodeId) {
        let Some(node) = self.scene.get(id) else { return };
        // In its own frame: it may start at the 3D cursor, and that offset would be added twice.
        let Some((mut lo, mut hi)) = simple3d_core::eval::baked_mesh(&self.scene, id).bounds() else { return };
        if node.anchor == Anchor::Base {
            (lo.z, hi.z) = (0.0, hi.z - lo.z);
        }
        let frame = match node.parent.and_then(|parent| self.scene.get(parent).map(|node| (parent, node))) {
            Some((parent, group)) => {
                let outer = self.evaluated.node_frames.get(&parent).copied().unwrap_or(Xform::IDENTITY);
                outer.compose(&Xform::from_pos_rot_scale(group.position, group.rotation, Node::sane_scale(group.scale)))
            }
            None => Xform::IDENTITY,
        };
        let name = node.name.clone();
        self.placing = Some(Placing { id, local: (lo, hi), frame, revision: self.history.revision(), held: None });
        self.status = Status::Info(format!("Click a surface to put {name} down; Escape takes it back"));
    }

    /// Put the shape down where it is now.
    pub(crate) fn finish_placing(&mut self) {
        let Some(id) = self.placing.as_ref().map(|placing| placing.id) else { return };
        // Still drawn where it was put until that is evaluated, as a released drag is.
        let drawn = self.live_move_of(id);
        self.placing = None;
        self.snap_indicator = None;
        self.settling = Some(id).filter(|_| drawn);
        self.touch();
        let name = self.scene.get(id).map(|node| node.name.clone()).unwrap_or_default();
        self.status = Status::Info(format!("Put {name} down"));
    }

    /// Take the addition back, unless something else was done since.
    pub(crate) fn cancel_placing(&mut self) {
        let Some(placing) = self.placing.take() else { return };
        self.snap_indicator = None;
        if self.history.revision() == placing.revision {
            self.undo();
            self.status = Status::Info("Took the new shape back".into());
        }
    }

    /// Rest the shape being put down on what is under `cursor`.
    pub(crate) fn follow_pointer(&mut self, view: &View, cursor: egui::Pos2, free: bool) {
        let Some(placing) = self.placing.as_ref() else { return };
        let id = placing.id;
        let Some(Rest { position, rotation, mark, held }) = resting(self, placing, view, cursor, free) else { return };
        self.snap_indicator = mark;
        if let Some(placing) = self.placing.as_mut() {
            placing.held = held;
        }
        let node = self.scene.node(id);
        if node.position != position || node.rotation != rotation {
            if let Some(node) = self.scene.get_mut(id) {
                node.position = position;
                node.rotation = rotation;
            }
            self.touch();
        }
    }

    fn live_move_of(&self, id: NodeId) -> bool {
        self.live_move().is_some_and(|(moved, _, _)| moved == id)
    }
}

/// The viewport's pointer while a shape is being put down: it follows, a click puts it down, Escape
/// takes it back. Orbiting and panning go on as usual.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let Some(placing) = app.placing.clone() else { return };
    if !app.scene.contains(placing.id) {
        app.placing = None;
        return;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.cancel_placing();
        return;
    }
    let free = ui.input(|i| i.modifiers.alt);
    if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|_| response.hovered()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        app.follow_pointer(view, cursor, free);
    }
    if response.clicked_by(egui::PointerButton::Primary) {
        app.finish_placing();
    }
}

/// Where the shape rests, and what it caught.
struct Rest {
    position: Vec3,
    rotation: Vec3,
    mark: Option<SnapMark>,
    /// The face whose snap position it caught, held on to the next frame.
    held: Option<Face>,
}

/// How far the pointer may stray from a caught snap position before the shape lets go, in pixels:
/// further than a new one is caught, so it does not flicker on and off at the edge of the reach.
const RELEASE_PIXELS: f32 = DRAG_CATCH_PIXELS * 1.6;

/// Where the shape rests on the surface under `cursor`, or on the ground away from the model, its
/// middle over the point. Unless `free` (Alt), on a flat face it turns along the face's edges. Only
/// while snapping is asked for (issue 126) does its middle catch a point: on a face the face's
/// centre, corners and edges (issue 128), the face caught last frame keeping its catch a little
/// further out, also with the pointer just past it; off a face whatever else is in reach. Otherwise
/// it sits on the grid unless `free`, where the surface is square to an axis.
fn resting(app: &App, placing: &Placing, view: &View, cursor: egui::Pos2, free: bool) -> Option<Rest> {
    let up = Vec3::new(0.0, 0.0, 1.0);
    let id = placing.id;
    let surface = app.surface_hit_under(view, cursor, &[id]);
    let hit = surface
        .map(|(at, _, normal)| (at, normal))
        .or_else(|| Some((view.ray_plane_ahead(cursor, Vec3::ZERO, up)?, up)))?;
    let face = surface.filter(|_| !free).and_then(|(at, triangle, normal)| app.face_at(at, triangle, normal, &[id]));
    let snapping = app.snap_requested && !free;
    let held = placing.held.as_ref().filter(|_| snapping);
    let on_face = match (&face, held) {
        (Some(face), Some(held)) if face.same(held) => face.catch(view, cursor, RELEASE_PIXELS),
        (Some(face), _) if snapping => face.catch(view, cursor, DRAG_CATCH_PIXELS),
        _ => None,
    }
    .zip(face.clone())
    .or_else(|| held.and_then(|held| Some((held.catch(view, cursor, RELEASE_PIXELS)?, held.clone()))));
    // Off a face what else is in reach is caught, since on one only the face's own positions count.
    let caught = match (&on_face, &face) {
        (None, None) if snapping => app.snap_catch(view, cursor, &[id]),
        _ => None,
    };
    let turn = match (&on_face, &face) {
        (Some(((_, how), face)), _) => face.aligned(Some(how)),
        (None, Some(face)) => face.aligned(None),
        (None, None) => Xform::turn_z_to(hit.1),
    };
    let (lo, hi) = placing.local;
    let scale = Node::sane_scale(app.scene.node(id).scale);
    let (at, mark) = match (&on_face, caught) {
        (Some(((at, _), face)), _) => (*at, Some(face.mark(*at))),
        (None, Some((point, mark))) => (point.at, Some(mark.unwrap_or_else(|| SnapMark::at(point.at)))),
        (None, None) => (on_grid(hit.0, Some(hit.1), if free { 0.0 } else { app.move_snap() }), None),
    };
    // From the point on the surface back to the origin: under the middle, down to the bottom.
    let centre = (lo + hi) * 0.5;
    let offset = Vec3::new(-centre.x * scale.x, -centre.y * scale.y, -lo.z * scale.z);
    let world = at + turn.vector(offset);
    let inverse = placing.frame.inverse();
    let local = inverse.compose(&turn);
    let rotation = Xform::rotation_of_axes(local.axis(0), local.axis(1), local.axis(2));
    Some(Rest { position: inverse.point(world), rotation, mark, held: on_face.map(|(_, face)| face) })
}
