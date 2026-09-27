//! Cutting one polygon by a plane into the pieces either side of it.

use super::*;
use crate::vec3::Vec3;

/// Reused scratch buffers: `split` runs up to a hundred million times per boolean, and fresh `Vec`s
/// and clones per call were most of a boolean's cost.
#[derive(Default)]
pub(crate) struct Splitter {
    pub(super) types: Vec<i32>,
    pub(super) front: Vec<Vec3>,
    pub(super) back: Vec<Vec3>,
}

impl Splitter {
    /// Split `poly` by `plane` into the four buckets. Takes it by value since three outcomes pass it on
    /// whole without copying.
    pub(super) fn split(
        &mut self,
        plane: &Plane,
        poly: Polygon,
        coplanar_front: &mut Vec<Polygon>,
        coplanar_back: &mut Vec<Polygon>,
        front: &mut Vec<Polygon>,
        back: &mut Vec<Polygon>,
    ) {
        let mut polygon_type = 0;
        self.types.clear();
        for v in &poly.vertices {
            let t = plane.normal.dot(*v) - plane.w;
            let ty = if t < -EPSILON {
                BACK
            } else if t > EPSILON {
                FRONT
            } else {
                COPLANAR
            };
            polygon_type |= ty;
            self.types.push(ty);
        }

        match polygon_type {
            COPLANAR => {
                if plane.normal.dot(poly.plane.normal) > 0.0 {
                    coplanar_front.push(poly);
                } else {
                    coplanar_back.push(poly);
                }
            }
            FRONT => front.push(poly),
            BACK => back.push(poly),
            _ => {
                self.front.clear();
                self.back.clear();
                let n = poly.vertices.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (ti, tj) = (self.types[i], self.types[j]);
                    let (vi, vj) = (poly.vertices[i], poly.vertices[j]);
                    if ti != BACK {
                        self.front.push(vi);
                    }
                    if ti != FRONT {
                        self.back.push(vi);
                    }
                    if (ti | tj) == SPANNING {
                        let denom = plane.normal.dot(vj - vi);
                        let t = (plane.w - plane.normal.dot(vi)) / denom;
                        let v = vi.lerp(vj, t);
                        self.front.push(v);
                        self.back.push(v);
                    }
                }
                if self.front.len() >= 3 {
                    front.push(Polygon::new(self.front.clone(), poly.plane, poly.tag));
                }
                if self.back.len() >= 3 {
                    back.push(Polygon::new(self.back.clone(), poly.plane, poly.tag));
                }
            }
        }
    }
}
