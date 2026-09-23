//! Whether the plane is cutting anything at all.

use crate::app::App;
use crate::render::Renderable;
use simple3d_core::xform::Xform;
use simple3d_geom::section::{self, Plane};
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

/// A shape drawn somewhere other than where the scene has it, and the move
/// that puts it there.
type Moving = (Arc<Renderable>, Option<Xform>);

/// The planes the viewport cuts with this frame: one per section, less those
/// that have no cut to make.
///
/// None while the tool is off. A section is left out while its plane has been
/// slid or turned clear of every body: a plane that crosses nothing has
/// nothing to show the inside of, and since the side that goes is the side the
/// camera is on (issue 109), the whole model would otherwise vanish the moment
/// it was on that side -- which reads as the model being gone rather than as a
/// plane that needs sliding back. Clear of the model, the model is drawn whole
/// and the frame alone says where the plane is. Each section is asked on its
/// own, so one slid off the model does not take the others' cuts with it.
///
/// A body being dragged is asked about where it is drawn rather than where the
/// last evaluation left it: the evaluation trails the drag, and asking the
/// scene alone switched the cut off and on again as each one landed, which is
/// the model flickering while it was moved into the plane.
pub fn cut(app: &mut App, forward: Vec3) -> Vec<Plane> {
    if !app.scene.settings.section.enabled {
        app.section_crossing.clear();
        return Vec::new();
    }
    // The dragged stretch of the scene, left out of the scene's own answer,
    // and what is drawn in its place, each with the move that puts it there.
    let (skip, moving): (Option<Range<u32>>, Vec<Moving>) = match app.live_csg() {
        Some(csg) => (Some(csg.range), csg.leaves),
        None => match app.live_move() {
            Some((id, part, moved)) => match app.node_renderables.get(&id) {
                Some(body) => (Some(part), vec![(body.clone(), Some(moved))]),
                None => (None, Vec::new()),
            },
            None => (None, Vec::new()),
        },
    };
    let sections: Vec<_> = app.scene.settings.sections().copied().collect();
    let mut held = std::mem::take(&mut app.section_crossing);
    let mut asked = Vec::with_capacity(sections.len());
    let mut planes = Vec::new();
    for section in sections.iter().take(section::MAX_CUTS) {
        let plane = section.cut(app.evaluated.bounds, forward);
        let key = key_of(app.scene_renderable.id, &skip, &plane, section.normal());
        // The still part of the scene is asked once per plane; only what moves
        // is asked every frame.
        let still = match held.iter().position(|&(was, _)| was == key) {
            Some(index) => held.swap_remove(index).1,
            None => {
                let mesh = &app.scene_renderable.mesh;
                // A triangle of the dragged stretch is left to the moving
                // shapes: a part shares no vertex with anything else, so its
                // triangles are the ones whose corners are in its range.
                let still = |tri: &&[u32; 3]| skip.as_ref().is_none_or(|range| !range.contains(&tri[0]));
                touches(mesh.indices.iter().filter(still), &mesh.positions, None, &plane)
            }
        };
        asked.push((key, still));
        let crossing = still
            || moving
                .iter()
                .any(|(body, moved)| touches(body.mesh.indices.iter(), &body.mesh.positions, *moved, &plane));
        if crossing {
            planes.push(plane);
        }
    }
    app.section_crossing = asked;
    planes
}

/// What whether a plane crosses the still scene is remembered by: the scene,
/// the stretch of it being dragged, and the plane as it stands, whichever way
/// it faces -- which side the camera is on changes what goes, never whether
/// the plane crosses a body.
fn key_of(scene: u64, skip: &Option<Range<u32>>, plane: &Plane, axis: Vec3) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scene.hash(&mut hasher);
    skip.hash(&mut hasher);
    let facing = plane.normal.dot(axis).signum();
    let mut numbers =
        vec![plane.normal.x * facing, plane.normal.y * facing, plane.normal.z * facing, plane.offset * facing];
    if let Some(window) = plane.window {
        for at in [window.centre, window.u, window.v] {
            numbers.extend([at.x, at.y, at.z]);
        }
        numbers.extend(window.half);
    }
    for value in numbers {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// Whether the cut runs through any of these triangles -- and, cut down to a
/// rectangle, through one of them inside it. Asked a triangle at a time rather
/// than of the points alone: a plane standing in the gap between two bodies has
/// corners on both sides of it and still cuts neither.
///
/// A rectangle the surface does not cross at all is either wholly outside the
/// model or wholly inside it -- a small pocket opened in the middle of a big
/// face -- and which, is whether its middle is inside: an odd number of the
/// triangles along a ray from it.
fn touches<'a>(
    triangles: impl Iterator<Item = &'a [u32; 3]>,
    positions: &[Vec3],
    moved: Option<Xform>,
    plane: &Plane,
) -> bool {
    let at = |index: u32| {
        let p = positions[index as usize];
        moved.map_or(p, |moved| moved.point(p))
    };
    let middle = plane.window.map(|window| window.centre);
    let mut crossings = 0_usize;
    for tri in triangles {
        let corners = tri.map(at);
        if section::triangle_touches(plane, corners) {
            return true;
        }
        if let Some(middle) = middle {
            crossings += ray_hits(middle, corners) as usize;
        }
    }
    crossings % 2 == 1
}

/// Whether a ray from `from` passes through the triangle. Skewed off every axis
/// so a ray along a box's own edges and faces does not graze them.
fn ray_hits(from: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let dir = Vec3::new(1.0, 0.001_234_5, 0.002_345_6);
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return false;
    }
    let t_vec = from - a;
    let u = t_vec.dot(p) / det;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = t_vec.cross(e1);
    let v = dir.dot(q) / det;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    e2.dot(q) / det > 0.0
}

/// Whether the plane passes through the surface of these triangles.
#[cfg(test)]
pub(crate) fn crosses(positions: &[Vec3], triangles: &[[u32; 3]], plane: &Plane) -> bool {
    touches(triangles.iter(), positions, None, plane)
}
