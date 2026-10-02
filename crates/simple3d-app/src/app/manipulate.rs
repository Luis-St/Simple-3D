//! One step of a manipulator drag, and the nudge keys that do the same by keyboard.

use super::*;
use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Nudge the selection along the two axes most aligned with the screen; in rotate and resize
    /// modes the keys rotate and resize instead (spec section 6.2).
    pub(super) fn nudge(&mut self, command: Command) {
        let Some(id) = self.primary() else { return };
        let Some(gizmo) = self.gizmo_for(id) else { return };
        let view = self.current_view();
        let snap = self.move_snap();
        let rotate_snap = self.settings.rotate_snap_deg;
        // The undo key is stable across a held run so it coalesces into one step. It records its own
        // step rather than using `edit`, so it lifts a tool's preview itself (`App::lift_preview`).
        let lifted = self.lift_preview();
        let step =
            gizmo::apply_nudge(&mut self.history, &mut self.scene, &gizmo, &view, id, command, snap, rotate_snap);
        self.drop_preview_back(lifted);
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

    /// One frame of a manipulator drag, independent of a running frame so tests can drive it.
    /// Undo is recorded only on `Begin`, making a drag one step (spec acceptance criterion 23).
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
                self.settling = None;
                if let Some(drag) = self.drag.take() {
                    drag.cancel(&mut self.scene);
                    // The Begin snapshot equals the restored state, so drop it rather than leave a dead undo step.
                    self.history.discard_last();
                    self.touch();
                    self.fields.clear();
                    self.status = Status::Info("Drag cancelled".into());
                }
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Finish => {
                // Still drawn where released until that is evaluated.
                let drawn = self.live_drag().is_some() || self.live_csg().is_some();
                self.settling = self.drag.as_ref().filter(|_| drawn).map(|drag| drag.node);
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
                // Ctrl held for face snapping is not also a symmetric resize request.
                let mut mods = mods;
                let pulling_face = matches!(self.drag.as_ref().map(|d| d.handle), Some(Handle::ResizeFace(..)));
                if pulling_face && self.snap_requested && self.snap_holds_ctrl() {
                    mods.symmetric = false;
                }
                let handle = match self.drag.as_mut() {
                    Some(drag) => {
                        drag.update(&mut self.scene, view, cursor, mods, snap, rotate_snap, unit);
                        drag.handle
                    }
                    None => return,
                };
                // Geometry snapping (issue 68) on top of the grid drag: moves and face resizes only, since an
                // angle or a corner has no single feature to land on.
                if self.snap_requested && matches!(handle, Handle::ResizeFace(..)) {
                    let caught = self.apply_resize_snap(id, view, cursor, mods);
                    let extent = caught.as_ref().map(|&(_, extent)| extent);
                    self.snap_indicator = caught.map(|(mark, _)| mark);
                    // The grid resize's readout describes the extent the snap just overrode.
                    if let (Some(extent), Handle::ResizeFace(axis, _)) = (extent, handle) {
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
                    // Restate the readout from the snapped move, which the grid drag's text no longer describes.
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
                // The rest of the selection follows, after any snap so it is carried too.
                if let Some(drag) = self.drag.as_ref() {
                    drag.carry(&mut self.scene);
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
                // The other selected nodes come along, as a Transform field applies to all of them. Never a
                // node inside or around the dragged one, which would then move twice.
                let others: Vec<Drag> = self
                    .top_level_selection()
                    .into_iter()
                    .filter(|&o| o != id && !self.scene.is_ancestor_of(o, id) && !self.scene.is_ancestor_of(id, o))
                    .filter_map(|o| Drag::carried(&self.scene, &self.gizmo_for(o)?, o, handle))
                    .collect();
                if let Some(drag) = self.drag.as_mut() {
                    drag.others = others;
                }
                // Before anything moves, while evaluated meshes and live positions agree.
                self.snap_sources = self.drag_snap_sources(id);
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Idle => {}
        }
    }
}

impl App {
    /// The body a drag carries, when the GPU can draw it moved without re-evaluation
    /// ([`crate::render::Live`]): its scene range and the move since the last evaluation. Only for
    /// move, turn or scale (not resize), and only for bodies that passed through evaluation untouched.
    pub(crate) fn live_drag(&self) -> Option<(NodeId, std::ops::Range<u32>, simple3d_core::xform::Xform)> {
        let drag = self.drag.as_ref()?;
        // Several bodies moving at once are left to evaluation.
        if !drag.others.is_empty() {
            return None;
        }
        let node = self.scene.get(drag.node)?;
        if node.params().cloned().unwrap_or_default() != drag.start_params {
            return None;
        }
        self.moved_since_evaluated(drag.node)
    }

    /// [`App::live_drag`], or a just-released body still awaiting its evaluation.
    pub(crate) fn live_move(&self) -> Option<(NodeId, std::ops::Range<u32>, simple3d_core::xform::Xform)> {
        match self.drag {
            Some(_) => self.live_drag(),
            None => self.moved_since_evaluated(self.settling?),
        }
    }

    /// Forget a released body once the evaluation shows it where it was dropped.
    pub(crate) fn settle(&mut self) {
        let Some(id) = self.settling else { return };
        let caught_up =
            self.scene.get(id).is_none_or(|node| self.evaluated.placements.get(&id) == Some(&placement(node)));
        if caught_up {
            self.settling = None;
            self.invalidate_image();
        }
    }

    /// `id`'s scene range and its move since the last evaluation, when the GPU draws and the range is its own.
    fn moved_since_evaluated(&self, id: NodeId) -> Option<(NodeId, std::ops::Range<u32>, simple3d_core::xform::Xform)> {
        self.gpu.as_ref()?;
        let part = self.scene_renderable.parts.get(&id)?;
        self.node_renderables.get(&id)?;
        Some((id, part.clone(), self.moved_by(id)?))
    }

    /// How far `id` has moved since the last evaluation, in world space.
    pub(crate) fn moved_by(&self, id: NodeId) -> Option<simple3d_core::xform::Xform> {
        let node = self.scene.get(id)?;
        let parent = self.evaluated.node_frames.get(&id)?;
        let then = self.evaluated.placements.get(&id)?;
        Some(parent.compose(&placement(node)).compose(&then.inverse()).compose(&parent.inverse()))
    }

    /// Whether a GPU-drawn drag is under way, evaluated once at its end.
    pub(crate) fn drag_drawn_live(&self) -> bool {
        self.drag.is_some() && (self.live_drag().is_some() || self.live_csg().is_some())
    }
}

/// A node's own placement in its parent's frame, as `Evaluated::placements` records it.
fn placement(node: &simple3d_core::scene::Node) -> simple3d_core::xform::Xform {
    simple3d_core::xform::Xform::from_pos_rot_scale(
        node.position,
        node.rotation,
        simple3d_core::scene::Node::sane_scale(node.scale),
    )
}
