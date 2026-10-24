//! Workplanes (issue 125): grids put down on a face, so shapes can be added, moved and snapped in
//! that face's frame rather than the ground's.
//!
//! A workplane is fixed in space once placed: it keeps the frame of the face it was put on, plus a
//! slide along its own X and Y, a lift along its normal and a turn about it, so moving or deleting
//! the body underneath leaves it where it is. The scene's own ground grid is the default and is not
//! stored; with no workplane active everything works on the ground as before. Workplanes live in
//! the scene's settings, so each component keeps its own and they are saved and undone with it.

use super::*;
use crate::xform::Xform;
use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

pub type WorkplaneId = u64;

/// A placed grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workplane {
    pub id: WorkplaneId,
    pub name: String,
    /// The middle of the face it was put down on.
    pub base: Vec3,
    /// The face's turn (X, then Y, then Z in degrees), taking +Z to the face's normal.
    #[serde(default = "Vec3_zero")]
    pub base_rotation: Vec3,
    /// Slide along its own X and Y and lift along its normal, in millimetres.
    #[serde(default)]
    pub offset: [f64; 3],
    /// Turn about its normal in degrees.
    #[serde(default)]
    pub spin: f64,
    /// Its grid's spacing in millimetres.
    pub spacing: f64,
}

impl Workplane {
    /// Its frame in the world: X and Y along the grid, Z out of it, the origin at the grid's centre.
    pub fn frame(&self) -> Xform {
        let face = Xform::from_pos_rot(self.base, self.base_rotation);
        let [x, y, z] = self.offset;
        face.compose(&Xform::from_pos_rot(Vec3::new(x, y, z), Vec3::new(0.0, 0.0, self.spin)))
    }

    /// Where its grid is centred.
    pub fn origin(&self) -> Vec3 {
        self.frame().t
    }

    /// Its three unit axes in the world.
    pub fn axes(&self) -> [Vec3; 3] {
        let frame = self.frame();
        [frame.axis(0), frame.axis(1), frame.axis(2)]
    }

    /// Its turn as a node's rotation in the world, so a shape given it lies on the workplane.
    pub fn rotation(&self) -> Vec3 {
        let [x, y, z] = self.axes();
        Xform::rotation_of_axes(x, y, z)
    }

    /// The spacing, never zero or negative, so a bad file cannot hang the grid.
    pub fn sane_spacing(&self) -> f64 {
        if self.spacing.is_finite() && self.spacing > 1e-6 {
            self.spacing
        } else {
            10.0
        }
    }
}

impl SceneSettings {
    /// The workplane everything works on now, or `None` for the scene's ground.
    pub fn active_workplane(&self) -> Option<&Workplane> {
        self.workplane(self.active_workplane?)
    }

    pub fn workplane(&self, id: WorkplaneId) -> Option<&Workplane> {
        self.workplanes.iter().find(|plane| plane.id == id)
    }

    pub fn workplane_mut(&mut self, id: WorkplaneId) -> Option<&mut Workplane> {
        self.workplanes.iter_mut().find(|plane| plane.id == id)
    }

    /// Add `plane` under a fresh id, which is returned; the id passed in is ignored.
    pub fn add_workplane(&mut self, mut plane: Workplane) -> WorkplaneId {
        let id = self.workplanes.iter().map(|plane| plane.id).max().map_or(1, |most| most + 1);
        plane.id = id;
        self.workplanes.push(plane);
        id
    }

    /// Remove a workplane; the ground takes over if it was the active one.
    pub fn remove_workplane(&mut self, id: WorkplaneId) {
        self.workplanes.retain(|plane| plane.id != id);
        if self.active_workplane == Some(id) {
            self.active_workplane = None;
        }
    }

    /// A name no workplane has yet: "Workplane", then "Workplane 2" and on.
    pub fn free_workplane_name(&self) -> String {
        let taken = |name: &str| self.workplanes.iter().any(|plane| plane.name == name);
        (1..)
            .map(|n| if n == 1 { "Workplane".to_string() } else { format!("Workplane {n}") })
            .find(|name| !taken(name))
            .unwrap_or_default()
    }
}
