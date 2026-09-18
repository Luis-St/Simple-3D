//! Putting a mesh back into objects and groups (issue 108).
//!
//! A mesh that came in from a printer file is one bag of triangles, however
//! many separate things are in it and however plainly each of them is a box or
//! a pin. Everything the application is good at -- editing a dimension,
//! painting one part, subtracting one body from another -- needs those things
//! to be *nodes*, and converting to a mesh is a door that only opens one way.
//! This is the way back: the bag is separated into the bodies it holds, each
//! body is compared against the shapes the registry can build, and what comes
//! out is a group of real objects standing exactly where the triangles stood.
//!
//! ## What it can and cannot recover
//!
//! It recovers *bodies*, not history. A mesh does not remember that it was a
//! plate with four holes drilled through it, and nothing here invents that: a
//! drilled plate is one connected surface and comes back as one mesh. What a
//! mesh does still say plainly is where one solid ends and the next begins --
//! two bodies of one mesh share no vertex -- and what shape each of those
//! solids is, which is a question that can be answered by fitting and
//! measuring. So the recovery is:
//!
//! * every connected body of the mesh becomes a node of its own,
//! * a body that measures as a box, a cylinder, a prism, a cone or a sphere
//!   becomes that shape, with its own parameters back,
//! * a body that measures as nothing keeps its triangles and stays a mesh,
//! * bodies that touch are gathered into a group, because a thing that was
//!   modelled as several solids in contact was an assembly.
//!
//! ## Why a fit is measured rather than classified
//!
//! There is no test on a triangle that says "this is part of a cylinder". What
//! there is instead is a shape the application can *build*, and the honest
//! question is how far the body is from the one that was built for it: fit the
//! candidate to the body, generate it exactly as the registry would, and
//! measure. A body is that shape when nothing on it is further than the
//! tolerance from the shape, and it is not when something is -- see
//! [`Reassemble::tolerance`] for the number.
//!
//! Measuring the *centroids* of the body's triangles as well as its corners is
//! what makes the answer mean anything. A plate with a hole through it has
//! every corner sitting on the surface of its own bounding box, so a test on
//! corners alone calls it a box; the triangles lining the hole are in the
//! middle of the box, and a test that asks about them does not.

mod fit;
mod frame;
mod group;
mod shape;
pub use shape::{Part, Shape};
mod shells;
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use crate::Abandon;
use serde::{Deserialize, Serialize};

/// How hard to look, and how far to go (issue 108).
///
/// Two of the four are about *what a shape is* and two are about *how much
/// structure to make*, and they are worth keeping apart: the tolerance decides
/// whether a body is a cylinder, and the cap decides whether the document gets
/// four hundred nodes it cannot be navigated with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reassemble {
    /// How far a body's surface may sit from the shape fitted to it, in
    /// millimetres, and still be called that shape.
    ///
    /// Zero would mean nothing but a shape this application generated itself,
    /// down to the last bit of the last vertex, and the default is loose enough
    /// for a shape that has been through a file format and a unit conversion
    /// while still being far tighter than the difference between a cylinder and
    /// something merely round.
    pub tolerance: f64,
    /// Whether to fit shapes at all. With it off every body keeps its
    /// triangles, and what the tool does is separate the mesh into the bodies
    /// it is in -- which is worth having on its own for a mesh of parts that
    /// are not primitives.
    pub recognise: bool,
    /// Whether bodies that touch are gathered into a group.
    ///
    /// Two solids in contact were an assembly before somebody flattened them,
    /// and a group says so -- it moves as one, paints as one and exports as
    /// one. Bodies standing apart are left apart, because nothing about the
    /// mesh says they belong together.
    pub group_touching: bool,
    /// The most objects to place.
    ///
    /// The setting the whole feature needs: a mesh of a printed lattice is
    /// forty thousand separate bodies, and forty thousand nodes is not a
    /// document anybody can work in -- it is a slower way of holding the mesh
    /// that was already there. Past this many the biggest bodies are the ones
    /// that become objects and the rest are kept, together, as one mesh.
    pub max_objects: u32,
}

impl Default for Reassemble {
    fn default() -> Reassemble {
        Reassemble { tolerance: DEFAULT_TOLERANCE, recognise: true, group_touching: true, max_objects: 200 }
    }
}

/// How far a surface may be from the shape fitted to it, by default.
///
/// A tenth of a millimetre is under the layer height of every printer this is
/// for, so a body that is a cylinder to within this was a cylinder; and it is
/// far enough above what a round trip through an STL in inches costs that a
/// shape does not stop being itself for having been saved.
pub const DEFAULT_TOLERANCE: f64 = 0.1;

/// What a mesh was found to be made of.
pub struct Assembly {
    /// The bodies, biggest first -- see [`reassemble_until`] for why that
    /// order.
    pub parts: Vec<Part>,
    /// Which parts stand together, as indices into `parts`. A run of one is an
    /// object on its own; a run of several is a group.
    pub groups: Vec<Vec<usize>>,
    /// Everything past the cap, in one mesh, in the frame the whole mesh was
    /// in. `None` when the cap was never reached.
    pub rest: Option<Mesh>,
    /// How many bodies are in that mesh.
    pub rest_bodies: usize,
}

impl Assembly {
    /// How many nodes this would put in the document, the leftover mesh
    /// included but not the groups holding them.
    pub fn objects(&self) -> usize {
        self.parts.len() + usize::from(self.rest.is_some())
    }

    /// How many bodies were recognised as a shape the application can rebuild.
    pub fn recognised(&self) -> usize {
        self.parts.iter().filter(|part| part.shape != Shape::Mesh).count()
    }

    /// Whether this is worth doing at all: one body, recognised as nothing and
    /// with nothing left over, is the mesh that went in.
    pub fn is_nothing(&self) -> bool {
        self.rest.is_none() && self.parts.len() == 1 && self.parts[0].shape == Shape::Mesh
    }
}

/// Take a mesh apart. See [`Reassemble`] for what the settings mean.
pub fn reassemble(mesh: &Mesh, plan: &Reassemble) -> Assembly {
    reassemble_until(mesh, plan, &crate::never).expect("a run that is never abandoned finishes")
}

/// The same, abandoned part-way when `give_up` says the answer is no longer
/// wanted -- the tool asks for an answer on every change of a number, and the
/// one before last is of no interest the moment the next is asked for.
///
/// The bodies come back biggest first, and the cap is applied in that order.
/// That is the whole of what makes a cap useful rather than arbitrary: a mesh
/// of one bracket and nine hundred spatters of support material has one body
/// worth having a node for, and taking the first two hundred bodies *as they
/// happen to be indexed* would be two hundred spatters and no bracket.
pub fn reassemble_until(mesh: &Mesh, plan: &Reassemble, give_up: Abandon<'_>) -> Option<Assembly> {
    let welded = mesh.weld();
    let mut bodies = shells::shells(&welded, give_up)?;
    bodies.sort_by(|a, b| span(b).total_cmp(&span(a)));
    let cap = (plan.max_objects.max(1) as usize).min(bodies.len());
    let mut parts = Vec::with_capacity(cap);
    for body in bodies.iter().take(cap) {
        if give_up() {
            return None;
        }
        parts.push(fit::part_of(body, plan, give_up)?);
    }
    let rest_bodies = bodies.len() - cap;
    let rest = (rest_bodies > 0).then(|| {
        let mut out = Mesh::new();
        for body in &bodies[cap..] {
            out.append(body);
        }
        out
    });
    let groups = group::gather(&parts, plan.group_touching);
    Some(Assembly { parts, groups, rest, rest_bodies })
}

/// How big a body is, for the order the cap is applied in: the diagonal of its
/// box.
///
/// Not its volume, which would rank a sheet of card below a grain of sand, and
/// not its triangle count, which is a fact about how finely it was tessellated
/// rather than about the part.
fn span(mesh: &Mesh) -> f64 {
    match mesh.bounds() {
        Some((lo, hi)) => (hi - lo).length(),
        None => 0.0,
    }
}

/// The box a set of points sits in, or nothing for no points.
fn bounds_of(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
    let mut it = points.iter();
    let first = *it.next()?;
    Some(it.fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p))))
}
