//! What one body of a mesh turned out to be, and where it stands.

use super::*;
use crate::primitives as gen;

/// A shape a body was recognised as, with the parameters it is rebuilt from.
///
/// Deliberately a short list. Every one of these is a shape whose surface a
/// handful of numbers describes completely, so a fit either measures within the
/// tolerance or it does not, and there is no third answer to argue about. A
/// torus or a rounded box would each need a search over one more parameter
/// before that question could even be asked, and a wrong answer there costs
/// more than the right one gains: what a body is *not* recognised as is simply
/// kept, exactly as it came in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Nothing was recognised: the body keeps its triangles.
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
    /// A regular prism, measured across its corners.
    ///
    /// The same triangles a cylinder of that many segments has -- the registry
    /// builds both by extruding a regular polygon -- and told apart from one by
    /// nothing but how many sides there are, which is the one judgement in the
    /// recognition that no measurement can settle.
    Prism {
        sides: u32,
        diameter: f64,
        height: f64,
    },
}

impl Shape {
    /// The kind of thing this is, for counting them up in a sentence.
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

    /// The plural of it, for the same sentence.
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

    /// How many segments the shape was tessellated with, for the round ones
    /// that have a count at all.
    ///
    /// The node has to carry it. A cylinder of twelve segments rebuilt at the
    /// document's default of thirty-two is a different solid from the one the
    /// triangles described -- wider across the corners by the sagitta the
    /// coarse tessellation cost -- and a part that was reassembled to be
    /// measured would measure wrong.
    pub fn segments(self) -> Option<u32> {
        match self {
            Shape::Sphere { segments, .. } | Shape::Cylinder { segments, .. } | Shape::Cone { segments, .. } => {
                Some(segments)
            }
            // A prism's sides are its own parameter, and a box and a mesh have
            // no tessellation to keep.
            Shape::Prism { .. } | Shape::Box { .. } | Shape::Mesh => None,
        }
    }

    /// The shape as triangles, in its own frame -- what the fit is measured
    /// against, and what the registry builds from the same numbers.
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

/// One body of the mesh: what it is, where it stands, and the triangles it was.
pub struct Part {
    /// The body's own triangles, moved into its own frame so that the node
    /// carrying them stands where the body stands.
    ///
    /// Kept whatever was recognised, not only for a body that stayed a mesh:
    /// it is what the preview draws for a part nothing was found in, and it is
    /// what the tool would have to put back if a fit were ever undone.
    pub mesh: Mesh,
    pub shape: Shape,
    /// Where the shape's own centre sits, in the frame the whole mesh was in.
    pub centre: Vec3,
    /// How it is turned, in degrees, in the application's X-then-Y-then-Z
    /// order -- straight into a node's `rotation`.
    pub rotation: Vec3,
    /// How far the body's surface is from the shape fitted to it, in
    /// millimetres. Zero for a body that kept its triangles, which is not an
    /// approximation of anything.
    pub deviation: f64,
    /// The body's box, in the frame the whole mesh was in, for deciding what
    /// touches what.
    pub bounds: (Vec3, Vec3),
}

impl Part {
    /// The shape as it stands: its triangles, turned and moved into the frame
    /// the whole mesh was in.
    ///
    /// What the preview draws, and the thing the fit was measured against.
    pub fn placed(&self) -> Mesh {
        match self.shape {
            Shape::Mesh => self.mesh.translated(self.centre),
            shape => shape.mesh().transformed(self.centre, self.rotation),
        }
    }
}
