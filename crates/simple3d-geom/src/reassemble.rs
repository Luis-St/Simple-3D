//! Putting a mesh back into objects and groups (issue 108).
//!
//! A mesh is separated into its connected bodies; each is compared against the shapes the
//! registry can build, and the result is a group of real objects where the triangles stood.
//! Bodies are recovered, not history: a drilled plate stays one mesh. Unrecognised bodies stay
//! meshes, and touching bodies are grouped as an assembly.
//!
//! A fit is judged by generating the candidate shape and measuring the body against it
//! ([`Reassemble::tolerance`]). Triangle centroids are measured as well as corners, since a
//! drilled plate's corners all lie on its bounding box.

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

/// Reassembly settings (issue 108): tolerance and recognition decide what a shape is, grouping
/// and the cap decide how much structure to make.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reassemble {
    /// How far a body's surface may sit from the fitted shape, in millimetres, and still be that shape.
    pub tolerance: f64,
    /// Whether to fit shapes at all; off, the tool only separates bodies.
    pub recognise: bool,
    /// Whether touching bodies are grouped as an assembly.
    pub group_touching: bool,
    /// The most objects to place. Past this the biggest bodies become objects and the rest stay
    /// together in one mesh, so a lattice of thousands of bodies stays workable.
    pub max_objects: u32,
}

impl Default for Reassemble {
    fn default() -> Reassemble {
        Reassemble { tolerance: DEFAULT_TOLERANCE, recognise: true, group_touching: true, max_objects: 200 }
    }
}

/// The default tolerance: under any printer's layer height, yet above a round trip through an
/// inch STL.
pub const DEFAULT_TOLERANCE: f64 = 0.1;

/// What a mesh was found to be made of.
pub struct Assembly {
    /// The bodies, biggest first (see [`reassemble_until`]).
    pub parts: Vec<Part>,
    /// Which parts stand together, as indices into `parts`; one is a lone object, several a group.
    pub groups: Vec<Vec<usize>>,
    /// Everything past the cap as one mesh in the input's frame; `None` if the cap was not reached.
    pub rest: Option<Mesh>,
    /// How many bodies are in that mesh.
    pub rest_bodies: usize,
}

impl Assembly {
    /// How many nodes this puts in the document, including the leftover mesh but not groups.
    pub fn objects(&self) -> usize {
        self.parts.len() + usize::from(self.rest.is_some())
    }

    /// How many bodies were recognised as rebuildable shapes.
    pub fn recognised(&self) -> usize {
        self.parts.iter().filter(|part| part.shape != Shape::Mesh).count()
    }

    /// Whether this changes nothing: one unrecognised body and no leftover.
    pub fn is_nothing(&self) -> bool {
        self.rest.is_none() && self.parts.len() == 1 && self.parts[0].shape == Shape::Mesh
    }
}

/// Take a mesh apart. See [`Reassemble`] for what the settings mean.
pub fn reassemble(mesh: &Mesh, plan: &Reassemble) -> Assembly {
    reassemble_until(mesh, plan, &crate::never).expect("a run that is never abandoned finishes")
}

/// The same, abandoned when `give_up` says so.
///
/// Bodies come back biggest first and the cap applies in that order, so a bracket with hundreds
/// of support spatters keeps the bracket.
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

/// A body's size for cap ordering: its box diagonal, not volume or triangle count.
fn span(mesh: &Mesh) -> f64 {
    match mesh.bounds() {
        Some((lo, hi)) => (hi - lo).length(),
        None => 0.0,
    }
}

/// The box of a set of points, or nothing for no points.
fn bounds_of(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
    let mut it = points.iter();
    let first = *it.next()?;
    Some(it.fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p))))
}
