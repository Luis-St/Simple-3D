//! Cutting the model with a plane so its inside can be seen (issue 71).
//!
//! Nothing here changes a model. A section is a way of *looking* at one: the
//! material on the far side of a plane is left out of the picture, and the
//! opening that leaves is closed with a cap, so a wall reads as a wall with a
//! thickness rather than as a hollow shell seen from inside.
//!
//! The two halves of that are here because both are geometry and both have to
//! answer the same way. [`clip_triangle`] is what the renderer walks the model
//! with, one triangle at a time; [`loops`] finds the closed outlines the plane
//! leaves in a mesh and [`cap`] fills them. The outlines are worth having on
//! their own -- they are a 2D section of the model, in world space -- which is
//! what pulling a drawing back out of a cut would start from.

mod clip;
pub use clip::{clip_segment, clip_triangle};
mod cap;
pub use cap::{cap, fill, loops};
#[cfg(test)]
mod tests;

use crate::vec3::Vec3;

/// The plane the model is cut with, as a half-space: what lies on the `normal`
/// side of it is not drawn.
///
/// `normal` points at the material that goes away, so a plane with a `+Z`
/// normal takes the top off. It need not be a unit vector for the sign tests to
/// work, but [`cap`] and the winding rule below take directions from it, so
/// [`Plane::new`] normalises it once and everything downstream can rely on that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    /// Where along the normal the plane sits, in millimetres.
    pub offset: f64,
}

impl Plane {
    pub fn new(normal: Vec3, offset: f64) -> Plane {
        Plane { normal: normal.normalized(), offset }
    }

    /// The plane perpendicular to one of the three axes, `offset` along it.
    pub fn on_axis(axis: usize, offset: f64, flipped: bool) -> Plane {
        let normal = match axis {
            0 => Vec3::new(1.0, 0.0, 0.0),
            1 => Vec3::new(0.0, 1.0, 0.0),
            _ => Vec3::new(0.0, 0.0, 1.0),
        };
        // Flipping keeps the plane where it is and swaps which side of it
        // survives, so the offset is negated with the normal.
        match flipped {
            true => Plane { normal: -normal, offset: -offset },
            false => Plane { normal, offset },
        }
    }

    /// How far past the plane `p` is: negative on the side that is kept, zero
    /// on the plane itself.
    pub fn depth(&self, p: Vec3) -> f64 {
        self.normal.dot(p) - self.offset
    }

    pub fn keeps(&self, p: Vec3) -> bool {
        self.depth(p) <= 0.0
    }

    /// A point on the plane: the foot of the normal from the origin. What the
    /// interface hangs the plane's own frame and its grip on.
    pub fn origin(&self) -> Vec3 {
        self.normal * self.offset
    }
}

/// What is left of one triangle once the plane has had it, and the edge the cut
/// left behind.
///
/// At most two triangles: a triangle with one corner on the kept side comes
/// back as itself shrunk, and one with two comes back as a quad, which is two.
/// They are held in an array rather than a `Vec` because this is called once
/// per triangle of the whole scene on every frame the picture changes, and an
/// allocation there is the difference between a section costing nothing and
/// costing the frame.
pub struct Clipped {
    triangles: [[Vec3; 3]; 2],
    count: usize,
    /// The cut edge, wound for the *cap* -- see [`loops`] for what that means
    /// and why the direction matters.
    pub cut: Option<[Vec3; 2]>,
}

impl Clipped {
    /// A triangle no section touched, so that a caller drawing with the section
    /// off and one drawing with it on can walk the same answer.
    pub fn untouched(world: [Vec3; 3]) -> Clipped {
        Clipped::whole(world)
    }

    pub fn triangles(&self) -> &[[Vec3; 3]] {
        &self.triangles[..self.count]
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn nothing() -> Clipped {
        Clipped { triangles: [[Vec3::ZERO; 3]; 2], count: 0, cut: None }
    }

    fn whole(world: [Vec3; 3]) -> Clipped {
        Clipped { triangles: [world, [Vec3::ZERO; 3]], count: 1, cut: None }
    }
}
