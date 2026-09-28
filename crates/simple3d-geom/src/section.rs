//! Cutting the model with a plane so its inside can be seen (issue 71).
//!
//! A view-only cut: material past the plane is left out of the picture and the opening is capped.
//! [`clip_triangle`] clips triangles for the renderer; [`loops`] finds the plane's outlines in a
//! mesh and [`cap`] fills them.

mod clip;
pub use clip::{clip_by_all, clip_segment, clip_triangle, kept_by_all, kept_segments, Segments};

/// The most sections that cut at once, as the shaders are sized.
pub const MAX_CUTS: usize = 8;
mod cap;
pub use cap::{cap, fill, loops, tagged_loops, CutTags};
mod window;
pub use window::{clip_polygon, faces, segment_within, triangle_touches, within, Face, Window};
#[cfg(test)]
mod tests;

use crate::vec3::Vec3;

/// The cutting plane as a half-space: what lies on the `normal` side is not drawn.
///
/// [`Plane::new`] normalises `normal`, which [`cap`] and the winding rule rely on. With a
/// [`Window`], only the material straight behind that rectangle goes: a box open on the removed side.
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
        // Flipping keeps the plane in place and swaps the kept side, so the offset is negated too.
        match flipped {
            true => Plane { normal: -normal, offset: -offset, window: None },
            false => Plane { normal, offset, window: None },
        }
    }

    /// How far past the plane `p` is: negative on the kept side, zero on the plane.
    pub fn depth(&self, p: Vec3) -> f64 {
        self.normal.dot(p) - self.offset
    }

    /// Whether `p` stays in the picture: in front of the plane, or beside its window.
    pub fn keeps(&self, p: Vec3) -> bool {
        self.walls().iter().any(|wall| wall.depth(p) <= 0.0)
    }

    /// The planes whose far sides together are cut away: the plane, plus with a window the four box
    /// sides facing inwards. A point is cut when past all of them.
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

    /// The foot of the normal from the origin, where the interface hangs the plane's frame and grip.
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

/// What is left of one triangle after the cut, and the cut edge. At most two triangles, held in
/// an array since this runs per triangle per frame and must not allocate.
pub struct Clipped {
    triangles: [[Vec3; 3]; 2],
    count: usize,
    /// What a windowed plane leaves, possibly more than two triangles (a ring); never allocated
    /// without a window.
    many: Vec<[Vec3; 3]>,
    /// The cut edge, wound for the cap (see [`loops`]).
    pub cut: Option<[Vec3; 2]>,
}

impl Clipped {
    /// A triangle no section touched, so section-on and section-off callers share one path.
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
