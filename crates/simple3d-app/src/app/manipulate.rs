//! One step of a manipulator drag, and the nudge keys that do the same by
//! keyboard.

use super::*;
use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Nudge the selection along the two axes most closely aligned with the
    /// screen. In rotate and resize modes the same keys rotate and resize
    /// instead (spec section 6.2).
    pub(super) fn nudge(&mut self, command: Command) {
        let Some(id) = self.primary() else { return };
        let Some(gizmo) = self.gizmo_for(id) else { return };
        let view = self.current_view();
        let snap = self.move_snap();
        let rotate_snap = self.settings.rotate_snap_deg;
        // Records the undo step under a key stable across a held run, so the
        // whole run coalesces into one.
        let step =
            gizmo::apply_nudge(&mut self.history, &mut self.scene, &gizmo, &view, id, command, snap, rotate_snap);
        let Some(step) = step else { return };
        self.touch();
        self.nudging = true;

        if let gizmo::Nudge::NoDimension { axis } = step {
            self.status = Status::Info(format!(
                "{} has no dimension on the {} axis",
                self.scene.node(id).name,
                gizmo::axis_name(axis)
            ));
        }
        self.fields.clear();
    }

    /// Carry out one frame of a manipulator drag. `panel_viewport::manipulate`
    /// reads the pointer off an `egui::Response`, works out the phase with
    /// `gizmo::drag_phase`, and hands the result here; nothing about this depends
    /// on a running frame, so a whole gesture can be driven from a test.
    ///
    /// The undo record happens on `Begin` and nowhere else -- that is what makes
    /// a completed drag one undo step (spec acceptance criterion 23). The frames
    /// in between call `touch`, which marks the scene dirty without snapshotting.
    pub fn manipulate_step(
        &mut self,
        gizmo: &Gizmo,
        view: &crate::view::View,
        id: NodeId,
        phase: gizmo::DragPhase,
        handle: Option<Handle>,
        cursor: Option<egui::Pos2>,
        mods: gizmo::Mods,
    ) {
        match phase {
            gizmo::DragPhase::Cancel => {
                if let Some(drag) = self.drag.take() {
                    drag.cancel(&mut self.scene);
                    // The snapshot taken at Begin describes exactly the state the
                    // cancel just restored, so keeping it would leave a dead undo
                    // step behind a drag the user explicitly abandoned.
                    self.history.discard_last();
                    self.touch();
                    self.fields.clear();
                    self.status = Status::Info("Drag cancelled".into());
                }
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Finish => {
                self.drag = None;
                self.history.close();
                self.fields.clear();
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Continue => {
                let snap = self.move_snap();
                let Some(cursor) = cursor else { return };
                let rotate_snap = self.settings.rotate_snap_deg;
                let unit = self.scene.settings.unit;
                let handle = match self.drag.as_mut() {
                    Some(drag) => {
                        drag.update(&mut self.scene, view, cursor, mods, snap, rotate_snap, unit);
                        drag.handle
                    }
                    None => return,
                };
                // Geometry snapping (issue 68) rides on top of the grid drag: a
                // move puts one of the carried body's own features on a body
                // under the pointer, and a face resize puts the face it is
                // pulling there. A rotation and a corner resize keep the grid --
                // an angle has no feature to land on, and a corner has no single
                // face to place.
                if self.snap_requested && matches!(handle, Handle::ResizeFace(..)) {
                    let caught = self.apply_resize_snap(id, view, cursor, mods);
                    self.snap_indicator = caught.map(|(at, _)| at);
                    // The readout the grid resize wrote describes the extent the
                    // cursor asked for, which the snap has just overridden.
                    if let (Some((_, extent)), Handle::ResizeFace(axis, _)) = (caught, handle) {
                        let unit = self.scene.settings.unit;
                        if let Some(drag) = self.drag.as_mut() {
                            drag.readout = format!(
                                "snap {} {}",
                                gizmo::axis_name(axis),
                                simple3d_core::unit::format_length(extent, unit)
                            );
                        }
                    }
                } else if self.snap_requested && matches!(handle, Handle::MoveAxis(_) | Handle::MovePlane(_)) {
                    self.snap_indicator = self.apply_geometry_snap(id, gizmo, handle, view, cursor);
                    // The readout beside the cursor is written by the grid drag
                    // and describes the move the snap has just overridden -- it
                    // read "X +32mm" while the body had actually gone to
                    // (53, -10, 10). Restate it from what really happened.
                    if self.snap_indicator.is_some() {
                        let unit = self.scene.settings.unit;
                        let moved =
                            self.scene.node(id).position - self.drag.as_ref().map_or(Vec3::ZERO, |d| d.start_position);
                        if let Some(drag) = self.drag.as_mut() {
                            drag.readout = format!(
                                "snap {}, {}, {} {}",
                                simple3d_core::unit::format_length(moved.x, unit),
                                simple3d_core::unit::format_length(moved.y, unit),
                                simple3d_core::unit::format_length(moved.z, unit),
                                unit.suffix()
                            );
                        }
                    }
                } else {
                    self.snap_indicator = None;
                }
                // The property editor tracks the handle live, and the preview follows.
                self.fields.clear();
                self.touch();
            }
            gizmo::DragPhase::Begin => {
                let (Some(cursor), Some(handle)) = (cursor, handle) else { return };
                // One snapshot for the whole drag, so it undoes in a single step.
                self.edit(
                    match self.mode {
                        Mode::Move => "Move",
                        Mode::Rotate => "Rotate",
                        Mode::Resize => "Resize",
                        Mode::Scale => "Scale",
                    },
                    None,
                );
                self.drag = Drag::begin(&self.scene, gizmo, id, handle, view, cursor);
                // Before anything has moved, while the evaluated meshes and the
                // live positions still agree.
                self.snap_sources = self.drag_feature_offsets(id);
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Idle => {}
        }
    }
}
