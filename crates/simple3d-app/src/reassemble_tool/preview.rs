//! Keeping the answer in step with the numbers, and drawing it on the model.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::reassemble::{Assembly, Shape};
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

impl App {
    /// Sync the open tool with the document and the window's numbers: close if the mesh is gone,
    /// follow outside changes to it, take finished runs, and start new ones.
    pub(crate) fn refresh_reassemble_tool(&mut self) {
        let Some(mut tool) = self.reassemble_tool.take() else { return };
        let held = self.scene.get(tool.target).and_then(|node| node.mesh()).cloned();
        let Some(held) = held else {
            if let Some(job) = &tool.job {
                job.cancel();
            }
            self.status = Status::Warning("The mesh being reassembled is no longer there".into());
            return;
        };
        if !Arc::ptr_eq(&held, &tool.mesh) {
            // Changed outside the tool (an undo, a paste): follow it and take the new mesh apart.
            if let Some(job) = &tool.job {
                job.cancel();
            }
            tool = ReassembleTool { mesh: held, found: None, job: None, ..tool };
        }
        if tool.generation != self.evaluation_generation {
            tool.generation = self.evaluation_generation;
            tool.placement = self.reassemble_placement(tool.target);
        }
        self.take_reassemble_run(&mut tool);
        self.start_reassemble_run(&mut tool);
        self.reassemble_tool = Some(tool);
    }

    /// Take a finished run, so the window and the viewport can show it.
    fn take_reassemble_run(&mut self, tool: &mut ReassembleTool) {
        let Some(job) = tool.job.as_ref() else { return };
        let Some(found) = job.poll() else { return };
        let plan = job.plan;
        tool.job = None;
        // An abandoned run has no answer; the next frame starts its replacement.
        let Some(assembly) = found else { return };
        // Computed once here: the lines only change with the numbers.
        let outline = outline(&assembly);
        let drawn = outline.len() <= PREVIEW_LOOPS;
        tool.found = Some(Found { plan, assembly: Arc::new(assembly), outline, drawn });
    }

    /// Start the run the numbers on screen ask for, unless it is already showing or running.
    fn start_reassemble_run(&mut self, tool: &mut ReassembleTool) {
        if tool.found.as_ref().is_some_and(|found| found.plan == tool.plan) {
            return;
        }
        match tool.job.as_ref() {
            // One at a time: a scrub asks every frame, so the running job is stopped and the next frame
            // starts the newest.
            Some(job) if job.plan == tool.plan => return,
            Some(job) => {
                job.cancel();
                return;
            }
            None => {}
        }
        tool.job = Some(ReassembleJob::spawn(tool.mesh.clone(), tool.plan));
    }
}

/// What was found, in the mesh's frame (issue 108): recognised bodies as their shape's edges at
/// [`DEFAULT_SHARP`](simple3d_geom::simplify::DEFAULT_SHARP) (not the triangulation), and
/// unrecognised ones as the box they stay in.
fn outline(assembly: &Assembly) -> Vec<Vec<Vec3>> {
    let mut out = Vec::new();
    for part in &assembly.parts {
        match part.shape {
            Shape::Mesh => out.extend(box_lines(part.bounds)),
            _ => out.extend(creases(&part.placed())),
        }
        if out.len() > PREVIEW_LOOPS {
            return out;
        }
    }
    if let Some(bounds) = assembly.rest.as_ref().and_then(Mesh::bounds) {
        out.extend(box_lines(bounds));
    }
    out
}

/// What was found, in world space, drawn by the renderer so it is depth-tested.
pub(crate) fn preview_loops(app: &App) -> Vec<Vec<Vec3>> {
    match app.reassemble_tool.as_ref() {
        Some(tool) => loops_for(tool),
        None => Vec::new(),
    }
}

/// The same, from the tool itself, carried through the node's frame onto the model.
pub(crate) fn loops_for(tool: &ReassembleTool) -> Vec<Vec<Vec3>> {
    let Some(found) = tool.found.as_ref() else { return Vec::new() };
    if !tool.outlines || !found.drawn {
        return Vec::new();
    }
    found.outline.iter().map(|line| line.iter().map(|&p| tool.placement.point(p)).collect()).collect()
}

/// A shape's corner edges as two-point lines: shared edges where the faces differ enough, and
/// every boundary edge.
fn creases(mesh: &Mesh) -> Vec<Vec<Vec3>> {
    let welded = mesh.weld();
    let mut edges: HashMap<(u32, u32), (usize, usize)> = HashMap::new();
    for (i, tri) in welded.indices.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            let met = edges.entry(key).or_insert((usize::MAX, usize::MAX));
            if met.0 == usize::MAX {
                met.0 = i;
            } else {
                met.1 = i;
            }
        }
    }
    let sharp = simple3d_geom::simplify::DEFAULT_SHARP.to_radians().cos();
    let mut out = Vec::new();
    for (&(a, b), &(first, second)) in &edges {
        let keep = second == usize::MAX
            || welded.triangle_normal(welded.indices[first]).dot(welded.triangle_normal(welded.indices[second]))
                < sharp;
        if keep {
            out.push(vec![welded.positions[a as usize], welded.positions[b as usize]]);
        }
    }
    // No corners at all (a sphere): draw three rings instead, since nothing looks like a missed
    // body and the full tessellation looks like a blob.
    if out.is_empty() {
        if let Some(bounds) = welded.bounds() {
            out.extend(rings(bounds));
        }
    }
    // Sorted for a stable order, since a reshuffled preview flickers.
    out.sort_by(|a, b| {
        let key = |line: &Vec<Vec3>| [line[0].x, line[0].y, line[0].z, line[1].x, line[1].y, line[1].z];
        key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Three axis-aligned rings sized to a body's box, for a shape with no corners.
fn rings((lo, hi): (Vec3, Vec3)) -> Vec<Vec<Vec3>> {
    let centre = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    let at = |u: f64, v: f64, axis: usize| match axis {
        0 => Vec3::new(0.0, u * half.y, v * half.z),
        1 => Vec3::new(u * half.x, 0.0, v * half.z),
        _ => Vec3::new(u * half.x, v * half.y, 0.0),
    };
    (0..3)
        .map(|axis| {
            (0..RING_POINTS)
                .map(|i| {
                    let (sin, cos) = (std::f64::consts::TAU * i as f64 / RING_POINTS as f64).sin_cos();
                    centre + at(cos, sin, axis)
                })
                .collect()
        })
        .collect()
}

/// Points per ring: round enough, and cheap enough for a hundred of them.
const RING_POINTS: usize = 32;

/// A box's twelve edges as six lines: the two Z faces and the four uprights.
fn box_lines((lo, hi): (Vec3, Vec3)) -> Vec<Vec<Vec3>> {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z })
    };
    let ring =
        |z: bool| vec![corner(false, false, z), corner(true, false, z), corner(true, true, z), corner(false, true, z)];
    let mut out = vec![ring(false), ring(true)];
    for (x, y) in [(false, false), (true, false), (true, true), (false, true)] {
        out.push(vec![corner(x, y, false), corner(x, y, true)]);
    }
    out
}
