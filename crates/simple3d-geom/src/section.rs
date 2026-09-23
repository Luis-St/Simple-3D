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
pub use clip::{clip_by_all, clip_segment, clip_triangle, kept_by_all, kept_segments, Segments};

/// The most sections that cut at once: what the renderer's shaders are sized
/// for, and so what the interface lets be added.
pub const MAX_CUTS: usize = 8;
mod cap;
pub use cap::{cap, fill, loops};
mod window;
pub use window::{clip_polygon, faces, segment_within, triangle_touches, within, Face, Window};
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
///
/// With a [`Window`] the plane is a rectangle rather than the whole plane, and
/// only the material straight behind that rectangle goes: what is cut away is
/// a box, open to infinity on the removed side, and everything round it stays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    /// Where along the normal the plane sits, in millimetres.
    pub offset: f64,
    /// The rectangle of the plane that cuts, or `None` for all of it.
    pub window: Option<Window>,
}

impl Plane {
    pub fn new(normal: Vec3, offset: f64) -> Plane {
        Plane { normal: normal.normalized(), offset, window: None }
    }

    /// The same plane cutting only within `window`.
    pub fn within(self, window: Window) -> Plane {
        Plane { window: Some(window), ..self }
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
            true => Plane { normal: -normal, offset: -offset, window: None },
            false => Plane { normal, offset, window: None },
        }
    }

    /// How far past the plane `p` is: negative on the side that is kept, zero
    /// on the plane itself.
    pub fn depth(&self, p: Vec3) -> f64 {
        self.normal.dot(p) - self.offset
    }

    /// Whether `p` stays in the picture: in front of the plane, or out to the
    /// side of its window.
    pub fn keeps(&self, p: Vec3) -> bool {
        self.walls().iter().any(|wall| wall.depth(p) <= 0.0)
    }

    /// The planes whose far sides together are what is cut away: the plane
    /// itself, and with a window the four sides of the box behind it, each
    /// facing into the box. A point is cut away when it is past every one of
    /// them. Each comes without a window of its own.
    pub fn walls(&self) -> Walls {
        let mut walls = Walls { planes: [Plane::new(Vec3::ZERO, 0.0); 5], count: 1 };
        walls.planes[0] = Plane { window: None, ..*self };
        if let Some(window) = self.window {
            for (axis, half) in [(window.u, window.half[0]), (window.v, window.half[1])] {
                let middle = axis.dot(window.centre);
                // Inside the box is `middle - half < axis . p < middle + half`.
                walls.planes[walls.count] = Plane { normal: axis, offset: middle - half, window: None };
                walls.planes[walls.count + 1] = Plane { normal: -axis, offset: -(middle + half), window: None };
                walls.count += 2;
            }
        }
        walls
    }

    /// A point on the plane: the foot of the normal from the origin. What the
    /// interface hangs the plane's own frame and its grip on.
    pub fn origin(&self) -> Vec3 {
        self.normal * self.offset
    }
}

/// [`Plane::walls`]: one plane, or five.
#[derive(Clone, Copy, Debug)]
pub struct Walls {
    planes: [Plane; 5],
    count: usize,
}

impl std::ops::Deref for Walls {
    type Target = [Plane];
    fn deref(&self) -> &[Plane] {
        &self.planes[..self.count]
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
    /// What a windowed plane leaves, which can be more than two triangles: a
    /// triangle with the box taken out of its middle is a ring. Empty, and so
    /// never allocated, for a plane without a window.
    many: Vec<[Vec3; 3]>,
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
        match self.many.is_empty() {
            true => &self.triangles[..self.count],
            false => &self.many,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.triangles().is_empty()
    }

    fn nothing() -> Clipped {
        Clipped { triangles: [[Vec3::ZERO; 3]; 2], count: 0, many: Vec::new(), cut: None }
    }

    fn whole(world: [Vec3; 3]) -> Clipped {
        Clipped { triangles: [world, [Vec3::ZERO; 3]], count: 1, many: Vec::new(), cut: None }
    }

    pub(crate) fn pieces(many: Vec<[Vec3; 3]>) -> Clipped {
        Clipped { triangles: [[Vec3::ZERO; 3]; 2], count: 0, many, cut: None }
    }
}
