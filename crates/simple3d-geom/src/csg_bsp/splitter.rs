//! Cutting one polygon by a plane into the pieces either side of it.

use super::*;
use crate::vec3::Vec3;

/// Scratch buffers for splitting, so the hot loop allocates nothing.
///
/// `clip_polygons` calls `split` once per polygon per node it descends. For a
/// dense convex operand that is a chain one node deep per face, and the count
/// runs into the hundred million: the three `Vec`s this used to build afresh
/// inside every call, and the clone every polygon that did not actually need
/// splitting was given, were between them most of the cost of a boolean.
#[derive(Default)]
pub(crate) struct Splitter {
    pub(super) types: Vec<i32>,
    pub(super) front: Vec<Vec3>,
    pub(super) back: Vec<Vec3>,
}

impl Splitter {
    /// Split `poly` by `plane`, appending results into the four buckets.
    ///
    /// Takes the polygon *by value*: three of the four outcomes hand it on
    /// whole, and moving it there costs nothing where copying it cost an
    /// allocation and a walk over its vertices.
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
