//! What one body of a mesh turned out to be, and where it stands.

use super::*;
use crate::primitives as gen;

/// A shape a body was recognised as, with its rebuild parameters. Deliberately short: each is
/// fully described by a few numbers, so a fit clearly passes or fails; anything else stays a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Nothing recognised: the body keeps its triangles.
    Mesh,
    Box {
        width: f64,
        depth: f64,
        height: f64,
    },
    /// An ellipsoid, which is what the registry's sphere is.
    Sphere {
        diameter_x: f64,
        diameter_y: f64,
        diameter_z: f64,
        segments: u32,
    },
    Cylinder {
        diameter: f64,
        height: f64,
        segments: u32,
    },
    Cone {
        bottom_diameter: f64,
        top_diameter: f64,
        height: f64,
        segments: u32,
    },
    /// A regular prism, measured across its corners. Same triangles as a cylinder of that many
    /// segments; told apart only by side count.
    Prism {
        sides: u32,
        diameter: f64,
        height: f64,
    },
}

impl Shape {
    /// The kind of shape, for a summary sentence.
    pub fn label(self) -> &'static str {
        match self {
            Shape::Mesh => "mesh",
            Shape::Box { .. } => "box",
            Shape::Sphere { .. } => "sphere",
            Shape::Cylinder { .. } => "cylinder",
            Shape::Cone { .. } => "cone",
            Shape::Prism { .. } => "prism",
        }
    }

    /// The plural, for the same sentence.
    pub fn plural(self) -> &'static str {
        match self {
            Shape::Mesh => "meshes",
            Shape::Box { .. } => "boxes",
            Shape::Sphere { .. } => "spheres",
            Shape::Cylinder { .. } => "cylinders",
            Shape::Cone { .. } => "cones",
            Shape::Prism { .. } => "prisms",
        }
    }

    /// The tessellation segment count for round shapes, which the node must carry or the rebuilt
    /// solid would differ from the triangles.
    pub fn segments(self) -> Option<u32> {
        match self {
            Shape::Sphere { segments, .. } | Shape::Cylinder { segments, .. } | Shape::Cone { segments, .. } => {
                Some(segments)
            }
            // A prism's sides are its own parameter; boxes and meshes have no tessellation.
            Shape::Prism { .. } | Shape::Box { .. } | Shape::Mesh => None,
        }
    }

    /// The shape as triangles in its own frame, as the registry builds it.
    pub fn mesh(self) -> Mesh {
        match self {
            Shape::Mesh => Mesh::new(),
            Shape::Box { width, depth, height } => gen::box_mesh(width, depth, height),
            Shape::Sphere { diameter_x, diameter_y, diameter_z, segments } => {
                gen::ellipsoid_mesh(diameter_x, diameter_y, diameter_z, segments)
            }
            Shape::Cylinder { diameter, height, segments } => gen::cylinder_mesh(diameter, diameter, height, segments),
            Shape::Cone { bottom_diameter, top_diameter, height, segments } => {
                gen::cone_mesh(bottom_diameter, top_diameter, height, segments)
            }
            Shape::Prism { sides, diameter, height } => gen::regular_prism_mesh(sides, diameter, height, false),
        }
    }
}

/// One body of the mesh: what it is, where it stands, and its triangles.
pub struct Part {
    /// The body's triangles in its own frame, kept whatever was recognised, for the preview.
    pub mesh: Mesh,
    pub shape: Shape,
    /// The shape's centre, in the whole mesh's frame.
    pub centre: Vec3,
    /// The rotation in degrees, X then Y then Z, ready for a node's `rotation`.
    pub rotation: Vec3,
    /// The surface's distance from the fitted shape, in millimetres; zero for a kept mesh.
    pub deviation: f64,
    /// The body's box in the whole mesh's frame, for deciding what touches what.
    pub bounds: (Vec3, Vec3),
}

impl Part {
    /// The shape as placed in the whole mesh's frame: what the preview draws and the fit was measured
    /// against.
    pub fn placed(&self) -> Mesh {
        match self.shape {
            Shape::Mesh => self.mesh.translated(self.centre),
            shape => shape.mesh().transformed(self.centre, self.rotation),
        }
    }
}
