//! What a snap caught, as drawn over the picture: the point it landed on, and the corners and edges
//! of every element it caught there (issue 87). A face shows its outline, an edge itself, and both
//! show their corners, not just the one point.

use super::*;
use simple3d_geom::Vec3;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct SnapMark {
    /// Where the snap landed.
    pub at: Vec3,
    pub corners: Vec<Vec3>,
    pub edges: Vec<(Vec3, Vec3)>,
}

impl SnapMark {
    /// Only the point landed on.
    pub fn at(at: Vec3) -> SnapMark {
        SnapMark { at, corners: Vec::new(), edges: Vec::new() }
    }

    /// The mark for `features` caught at `at`, each with its body's face outlines
    /// ([`features_and_faces`]). Shared corners and edges are kept once.
    pub fn of<'a>(at: Vec3, features: impl IntoIterator<Item = (&'a Feature, &'a [Vec<(Vec3, Vec3)>])>) -> SnapMark {
        let mut mark = SnapMark::at(at);
        let mut corners = HashSet::new();
        let mut edges = HashSet::new();
        for (feature, faces) in features {
            let spans: &[(Vec3, Vec3)] = match (&feature.span, feature.face) {
                (Some(span), _) => std::slice::from_ref(span),
                (None, Some(face)) => faces.get(face as usize).map_or(&[], Vec::as_slice),
                (None, None) => &[],
            };
            for &(a, b) in spans {
                let (ka, kb) = (point_key(a), point_key(b));
                if edges.insert(if ka <= kb { (ka, kb) } else { (kb, ka) }) {
                    mark.edges.push((a, b));
                }
                for end in [a, b] {
                    if corners.insert(point_key(end)) {
                        mark.corners.push(end);
                    }
                }
            }
            if feature.kind == FeatureKind::Vertex && corners.insert(point_key(feature.point)) {
                mark.corners.push(feature.point);
            }
        }
        mark
    }
}

/// A point to a micrometre, which tells coincident corners apart from distinct ones in any model.
pub fn point_key(p: Vec3) -> (i64, i64, i64) {
    ((p.x * 1e3).round() as i64, (p.y * 1e3).round() as i64, (p.z * 1e3).round() as i64)
}
