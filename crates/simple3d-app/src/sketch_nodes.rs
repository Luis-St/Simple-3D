//! Kept sketches in the viewport (issue 74): their curves drawn over the picture while they are
//! shown, and the closed region under the pointer, which push/pull and the revolve tool take as a
//! profile. A sketch makes no solid, so nothing of it is in the evaluated mesh to pick.

use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_core::scene::{sketch_frame, NodeId};
use simple3d_core::xform::Xform;
use simple3d_geom::push_pull::Outline;
use simple3d_geom::Vec3;
use simple3d_sketch::regions::Regions;
use simple3d_sketch::{sample_curve, RegionPick, Sketch};
use std::collections::HashMap;
use std::sync::Arc;

/// Pieces a full circle is found and drawn with, as the features build it by default.
pub(crate) const SEGMENTS: u32 = 64;

/// A closed region of a kept sketch.
#[derive(Clone)]
pub(crate) struct RegionHit {
    pub(crate) node: NodeId,
    pub(crate) sketch: Arc<Sketch>,
    /// The sketch plane in the world.
    pub(crate) frame: Xform,
    pub(crate) pick: RegionPick,
    /// The region's outline in the sketch plane.
    pub(crate) outline: Arc<Outline>,
    /// Where the pointer's ray met it, in the world.
    pub(crate) at: Vec3,
}

/// The regions found per sketch node, kept while the node's sketch is the same allocation.
#[derive(Default)]
pub(crate) struct SketchRegions(std::cell::RefCell<HashMap<NodeId, (usize, Arc<Regions>)>>);

impl App {
    /// The sketch nodes drawn now, with their sketch and world plane; not the one open in the editor.
    pub(crate) fn shown_sketches(&self) -> Vec<(NodeId, Arc<Sketch>, Xform)> {
        let editing = self.sketch_tool.as_ref().and_then(|tool| tool.editing);
        let frames = &self.evaluated.node_frames;
        self.scene
            .ids()
            .into_iter()
            .filter(|&id| Some(id) != editing && self.scene.is_shown(id))
            .filter_map(|id| {
                let sketch = self.scene.node(id).sketch()?.clone();
                Some((id, sketch, sketch_frame(&self.scene, id, frames)?))
            })
            .collect()
    }

    /// The closed regions of sketch node `id`.
    pub(crate) fn regions_of(&self, id: NodeId, sketch: &Arc<Sketch>) -> Arc<Regions> {
        let key = Arc::as_ptr(sketch) as usize;
        let mut cache = self.sketch_regions.0.borrow_mut();
        if let Some((_, regions)) = cache.get(&id).filter(|(at, _)| *at == key) {
            return regions.clone();
        }
        let regions = Arc::new(simple3d_sketch::regions::find(sketch, SEGMENTS));
        cache.insert(id, (key, regions.clone()));
        regions
    }

    /// The region of a shown sketch under `cursor`: the nearest one along the ray, unless the model
    /// is in front of it. A sketch on a face wins over the face it lies on.
    pub(crate) fn region_under(&self, view: &View, cursor: egui::Pos2) -> Option<RegionHit> {
        let (origin, direction) = view.ray(cursor);
        let along = |p: Vec3| (p - origin).dot(direction) / direction.dot(direction).max(1e-12);
        let model = crate::pick::ray_mesh_triangle(&self.evaluated.mesh, origin, direction).map(|(t, _)| t);
        let mut best: Option<(f64, RegionHit)> = None;
        for (node, sketch, frame) in self.shown_sketches() {
            let Some(at) = view.ray_plane_ahead(cursor, frame.t, frame.axis(2)) else { continue };
            let t = along(at);
            if model.is_some_and(|m| t > m + 1e-3 * (1.0 + m.abs())) || best.as_ref().is_some_and(|(b, _)| *b <= t) {
                continue;
            }
            let local = frame.inverse().point(at);
            let regions = self.regions_of(node, &sketch);
            let Some(index) = regions.at([local.x, local.y]) else { continue };
            let Some(outline) = regions.outlines(&[index]).into_iter().next() else { continue };
            let pick = regions.pick(index, [local.x, local.y]);
            best = Some((t, RegionHit { node, sketch, frame, pick, outline: Arc::new(outline), at }));
        }
        best.map(|(_, hit)| hit)
    }
}

/// Every shown sketch's curves over the picture, the selected ones in the accent colour.
pub(crate) fn draw(app: &App, painter: &egui::Painter, view: &View) {
    for (id, sketch, frame) in app.shown_sketches() {
        let colour = if app.is_selected(id) { token::ACCENT } else { token::TEXT_HI.gamma_multiply(0.7) };
        let screen = |p: [f64; 2]| view.project(frame.point(Vec3::new(p[0], p[1], 0.0))).map(|(s, _)| s);
        for curve in &sketch.curves {
            let samples = sample_curve(&sketch, curve, SEGMENTS);
            let mut points: Vec<egui::Pos2> = samples.points.iter().filter_map(|&p| screen(p)).collect();
            if samples.closed && !points.is_empty() {
                points.push(points[0]);
            }
            let stroke = egui::Stroke::new(1.5_f32, colour);
            if curve.construction {
                let faint = egui::Stroke::new(stroke.width, colour.gamma_multiply(0.6));
                painter.extend(egui::Shape::dashed_line(&points, faint, 6.0, 4.0));
            } else {
                painter.add(egui::Shape::line(points, stroke));
            }
        }
    }
}

/// A region of a kept sketch tinted, with its edge drawn, as push/pull tints a face.
pub(crate) fn tint(painter: &egui::Painter, view: &View, hit: &RegionHit, colour: egui::Color32, edge: bool) {
    let screen = |p: [f64; 2]| view.project(hit.frame.point(Vec3::new(p[0], p[1], 0.0))).map(|(s, _)| s);
    fill_outline(painter, &hit.outline, colour, screen);
    if edge {
        for points in std::iter::once(&hit.outline.outer).chain(hit.outline.holes.iter()) {
            let line: Vec<egui::Pos2> = points.iter().filter_map(|&p| screen(p)).collect();
            if line.len() == points.len() && line.len() > 2 {
                painter.add(egui::Shape::closed_line(line, egui::Stroke::new(1.5_f32, token::ACCENT)));
            }
        }
    }
}

/// An outline filled on screen, holes left open, by triangulating it; `screen` places its points.
pub(crate) fn fill_outline(
    painter: &egui::Painter,
    outline: &Outline,
    colour: egui::Color32,
    screen: impl Fn([f64; 2]) -> Option<egui::Pos2>,
) {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut loops: Vec<Vec<u32>> = Vec::new();
    for points in std::iter::once(&outline.outer).chain(outline.holes.iter()) {
        let start = positions.len() as u32;
        loops.push((0..points.len() as u32).map(|k| start + k).collect());
        positions.extend(points.iter().map(|p| Vec3::new(p[0], p[1], 0.0)));
    }
    let Some(triangles) = simple3d_geom::planar::triangulate_loops(&positions, Vec3::new(0.0, 0.0, 1.0), loops) else {
        return;
    };
    let mut mesh = egui::Mesh::default();
    for p in &positions {
        mesh.colored_vertex(screen([p.x, p.y]).unwrap_or(egui::Pos2::ZERO), colour);
    }
    for [a, b, c] in triangles {
        mesh.add_triangle(a, b, c);
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl App {
    /// Add `feature` on `plane` as one undo step named `undo`, beside or cut out of the owner as
    /// push/pull's solids go, select it, and hide the sketch node it was made from, if any, as its
    /// lines would otherwise run over the solid. Tried on a copy, so a refusal leaves no empty undo
    /// step behind.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_feature_node(
        &mut self,
        undo: &str,
        name: &str,
        feature: simple3d_core::scene::Feature,
        params: simple3d_core::primitive::Params,
        plane: &Xform,
        owner: Option<(NodeId, simple3d_core::scene::Placing)>,
        sketch: Option<NodeId>,
    ) -> Option<NodeId> {
        let mut scene = self.scene.clone();
        let id = scene.add_feature(name, feature, params, plane, owner, &self.evaluated.node_frames)?;
        if let Some(node) = sketch.and_then(|id| scene.get_mut(id)).filter(|node| node.is_sketch()) {
            node.visible = false;
        }
        self.edit(undo, None);
        self.scene = scene;
        self.select_only(id);
        Some(id)
    }
}
