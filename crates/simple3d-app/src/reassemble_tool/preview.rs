//! Keeping the answer in step with the numbers, and drawing it on the model.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::reassemble::{Assembly, Shape};
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

impl App {
    /// Keep the open tool honest against a document that can change underneath
    /// it, and keep what is drawn in the viewport in step with the numbers in
    /// the window.
    ///
    /// Four things can have happened since the last frame: the mesh can have
    /// gone, which closes the tool; it can have been changed by something else
    /// -- an undo, a paste -- which makes *that* the mesh to take apart; a run
    /// can have finished; and a number can have been turned, which starts the
    /// next run. The usual case is none of them, and costs two comparisons.
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
            // Something outside the tool changed the geometry, and the tool
            // follows it rather than arguing with it: an undo while the window
            // is open is a perfectly reasonable thing to do, and what it undid
            // is now what there is to take apart.
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

    /// Take a finished run, so the window can say what it found and the
    /// viewport can draw it.
    fn take_reassemble_run(&mut self, tool: &mut ReassembleTool) {
        let Some(job) = tool.job.as_ref() else { return };
        let Some(found) = job.poll() else { return };
        let plan = job.plan;
        tool.job = None;
        // An abandoned run has no answer, and the numbers it was abandoned for
        // are already on screen: the next frame starts the run that replaces it.
        let Some(assembly) = found else { return };
        // Drawn once, here, rather than on every frame: what the lines are does
        // not change while the numbers do not, and the mesh being moved about
        // under the window changes only where they are put.
        let outline = outline(&assembly);
        let drawn = outline.len() <= PREVIEW_LOOPS;
        tool.found = Some(Found { plan, assembly: Arc::new(assembly), outline, drawn });
    }

    /// Start the run the numbers on screen are asking for, if it is not the one
    /// already showing or already running.
    fn start_reassemble_run(&mut self, tool: &mut ReassembleTool) {
        if tool.found.as_ref().is_some_and(|found| found.plan == tool.plan) {
            return;
        }
        match tool.job.as_ref() {
            // One at a time. A scrubbed field asks for a new answer on every
            // frame it moves, and what is wanted is the newest of them: the run
            // in flight is told to stop, and the next frame -- once it has --
            // starts the one that is actually wanted.
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

/// What was found, in the mesh's own frame (issue 108).
///
/// A recognised body is drawn as the shape it was recognised as: that is the
/// claim being made, and drawing anything else -- a box around it, a label --
/// would be a picture of the claim rather than the claim itself. A body that
/// was not recognised is drawn as the box it will be kept in, which says the
/// other thing that has to be said: this one stays a mesh.
///
/// The shape's *edges*, not its triangles. The triangulation is not what is
/// being claimed -- it is how a curve happens to be approximated -- and drawing
/// it puts a diagonal across every flat face of every box, which is a great
/// deal of line for a picture whose whole job is to say "that one is a box".
/// What is left at [`DEFAULT_SHARP`](simple3d_geom::simplify::DEFAULT_SHARP)
/// is the shape's own corners: twelve edges for a box, two rims for a
/// cylinder, and every facet of a hexagonal prism, because on a prism the
/// facets *are* the shape.
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

/// What was found, in world space, for the renderer to draw over the mesh.
///
/// They go to the renderer rather than to the 2D painter so the depth buffer
/// can have them: a shape on the far side of the model is behind it, and lines
/// drawn through the solid read as floating in front of it.
pub(crate) fn preview_loops(app: &App) -> Vec<Vec<Vec3>> {
    match app.reassemble_tool.as_ref() {
        Some(tool) => loops_for(tool),
        None => Vec::new(),
    }
}

/// The same, from the tool itself -- which is how the window asks, since it
/// holds the tool while it is drawing its own contents.
///
/// The mesh stands in the document, so the lines are carried out through the
/// node's own frame to land on it rather than beside it.
pub(crate) fn loops_for(tool: &ReassembleTool) -> Vec<Vec<Vec3>> {
    let Some(found) = tool.found.as_ref() else { return Vec::new() };
    if !tool.outlines || !found.drawn {
        return Vec::new();
    }
    found.outline.iter().map(|line| line.iter().map(|&p| tool.placement.point(p)).collect()).collect()
}

/// The edges of a shape where its surface turns a corner, each as a line of two
/// points.
///
/// An edge two triangles share is drawn only where those two triangles face
/// meaningfully different ways; an edge only one triangle has is drawn always,
/// since there is nothing on the other side of it to be flat with.
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
    // Gathered out of a map, so put back in an order that does not change from
    // one run to the next: the renderer draws them in the order they arrive,
    // and a preview that reshuffles itself every frame flickers.
    out.sort_by(|a, b| {
        let key = |line: &Vec<Vec3>| [line[0].x, line[0].y, line[0].z, line[1].x, line[1].y, line[1].z];
        key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// The twelve edges of a box, as six lines -- the two faces square to Z and the
/// four uprights between them.
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
