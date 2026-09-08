mod handles;
mod move_drag;
mod nudge;
mod readout;
mod resize;
mod rotate;
mod scale;

use super::axis::*;
use super::build::*;
use super::drag::*;
use super::hit::*;
use super::mode::*;
use super::mods::*;
use super::nudge::*;
use crate::view::View;
use simple3d_core::eval::Evaluated;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::Camera;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::unit::Unit;
use simple3d_geom::Vec3;

struct Fixture {
    pub(super) scene: Scene,
    pub(super) evaluator: Evaluator,
    pub(super) evaluated: Evaluated,
    pub(super) node: NodeId,
    pub(super) view: View,
}

impl Fixture {
    pub(super) fn new(type_id: &str) -> Fixture {
        let mut scene = Scene::new();
        let root = scene.root();
        let node = scene.add_primitive(type_id, root, 0).unwrap();
        scene.camera = Camera { yaw: -55.0, pitch: 28.0, distance: 160.0, ..Camera::default() };
        let mut evaluator = Evaluator::new();
        let evaluated = evaluator.evaluate(&scene, &Cancel::new());
        let view = View::new(scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));
        Fixture { scene, evaluator, evaluated, node, view }
    }

    pub(super) fn reevaluate(&mut self) {
        self.evaluated = self.evaluator.evaluate(&self.scene, &Cancel::new());
    }

    pub(super) fn gizmo(&self, mode: Mode) -> Gizmo {
        Gizmo::build(&self.scene, &self.evaluated, self.node, mode, false).unwrap()
    }

    pub(super) fn param(&self, key: &str) -> f64 {
        self.scene.node(self.node).params().unwrap().num(key)
    }

    pub(super) fn world_bounds(&self) -> (Vec3, Vec3) {
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
