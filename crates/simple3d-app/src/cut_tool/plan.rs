//! Which objects the plane runs through, and the plane in each one's own frame.

use super::*;
use crate::app::App;
use simple3d_core::scene::Visibility;
use simple3d_geom::plane_cut::cross_section_points;
use std::hash::{Hash, Hasher};

impl App {
    /// What Cut would cut, worked out again only when the face, the area or the evaluation changed:
    /// baking every object is too slow to repeat each frame.
    pub(crate) fn cut_planned(&mut self) -> Arc<Vec<Planned>> {
        let Some(tool) = self.cut_tool.as_ref() else { return Arc::default() };
        let key = self.cut_key(tool);
        if let Some((held, planned)) = &tool.planned {
            if *held == key {
                return planned.clone();
            }
        }
        let planned = Arc::new(self.plan_cut(tool));
        if let Some(tool) = self.cut_tool.as_mut() {
            tool.planned = Some((key, planned.clone()));
        }
        planned
    }

    fn cut_key(&self, tool: &CutTool) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.evaluation_generation.hash(&mut hasher);
        tool.limited.hash(&mut hasher);
        let face = tool.face.map(|face| [face.point, face.normal]);
        for p in face.iter().flatten() {
            [p.x, p.y, p.z].map(f64::to_bits).hash(&mut hasher);
        }
        tool.size.map(f64::to_bits).hash(&mut hasher);
        hasher.finish()
    }

    /// Every candidate the plane runs through, with its whole cross-section in the area.
    fn plan_cut(&self, tool: &CutTool) -> Vec<Planned> {
        let Some(face) = tool.face else { return Vec::new() };
        let mut planned = Vec::new();
        for id in self.cut_candidates() {
            // A box test first, so objects far off the plane are never baked.
            let Some(&(lo, hi)) = self.evaluated.node_world_bounds.get(&id) else { continue };
            let depths = (0..8).map(|corner| face.depth(simple3d_geom::aabb::box_corner(lo, hi, corner)));
            let (least, most) = depths.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), d| (a.min(d), b.max(d)));
            if least >= 0.0 || most <= 0.0 {
                continue;
            }
            let (mesh, placement) = self.bake_for_split(id);
            let plane = local_plane(&placement, &face);
            let section = cross_section_points(&mesh, &plane);
            if section.is_empty() || !section.iter().all(|&p| tool.within(placement.point(p))) {
                continue;
            }
            let outline = section_segments(&mesh, &plane, &placement);
            planned.push(Planned { node: id, mesh: Arc::new(mesh), plane, bounds: (lo, hi), outline });
        }
        planned
    }

    /// What the plane may cut: the document's shown objects, but no cutters, and in place of a split
    /// its pieces, so cutting a cut object again halves the piece the plane runs through.
    pub(crate) fn cut_candidates(&self) -> Vec<NodeId> {
        let solid = |id: NodeId| {
            self.scene.get(id).is_some_and(|node| node.visibility() == Visibility::Visible) && !self.cuts(id)
        };
        let mut out = Vec::new();
        for &id in &self.scene.node(self.scene.root()).children {
            if !solid(id) {
                continue;
            }
            let node = self.scene.node(id);
            if node.is_split() {
                out.extend(node.children.iter().copied().filter(|&piece| solid(piece)));
            } else {
                out.push(id);
            }
        }
        out
    }
}

/// `face` in the frame `placement` takes to the world: a point carried back, and the normal by the
/// transpose of the frame's turn, which keeps it square to the plane under a stretch.
pub(crate) fn local_plane(placement: &Xform, face: &CutPlane) -> CutPlane {
    let point = placement.inverse().point(face.point);
    let (m, n) = (placement.m, face.normal);
    let normal = Vec3::new(
        m[0][0] * n.x + m[1][0] * n.y + m[2][0] * n.z,
        m[0][1] * n.x + m[1][1] * n.y + m[2][1] * n.z,
        m[0][2] * n.x + m[1][2] * n.y + m[2][2] * n.z,
    );
    CutPlane { point, normal: normal.normalized() }
}

/// Where the plane crosses an object's triangles, in the world: its cut's outline.
fn section_segments(mesh: &Mesh, plane: &CutPlane, placement: &Xform) -> Vec<[Vec3; 2]> {
    let depth: Vec<f64> = mesh.positions.iter().map(|&p| plane.depth(p)).collect();
    let mut segments = Vec::new();
    for triangle in &mesh.indices {
        let mut ends = Vec::with_capacity(2);
        for k in 0..3 {
            let (a, b) = (triangle[k] as usize, triangle[(k + 1) % 3] as usize);
            if (depth[a] > 0.0) != (depth[b] > 0.0) {
                let t = depth[a] / (depth[a] - depth[b]);
                ends.push(placement.point(mesh.positions[a] + (mesh.positions[b] - mesh.positions[a]) * t));
            }
        }
        if let [a, b] = ends[..] {
            segments.push([a, b]);
        }
    }
    segments
}
