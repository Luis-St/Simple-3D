//! Running a [`Shaping`] over a mesh.

use super::*;
use crate::topology::{vertex_normals, Topology};
use crate::vec3::Vec3;
use crate::Abandon;

/// Run `plan` over `mesh`. See [`Shaping`] for what the settings mean.
pub fn shape(mesh: &Mesh, plan: &Shaping) -> Outcome {
    shape_until(mesh, plan, &crate::never).expect("a run that is never abandoned finishes")
}

/// The same, abandoned when `give_up` says the answer is no longer wanted.
pub fn shape_until(mesh: &Mesh, plan: &Shaping, give_up: Abandon<'_>) -> Option<Outcome> {
    let (mut mesh, capped) = match plan.refine {
        true => refine_until(mesh, plan.edge, MAX_TRIANGLES, give_up)?,
        false => (mesh.weld(), false),
    };
    let topology = Topology::of(&mesh);
    let weights = weights(&mesh, &topology, plan);
    let before = mesh.positions.clone();
    if plan.function == Function::Smooth {
        smooth::taubin(&mut mesh.positions, &topology.neighbours, &weights, plan.layers.max(1), give_up)?;
    } else {
        // Inflating moves the faces, not the vertices, out by the amount (`offset_directions`).
        let normals = match plan.function {
            Function::Inflate => super::offset::offset_directions(&mesh),
            _ => vertex_normals(&mesh),
        };
        let noise = Noise::new(plan.seed);
        let scale = plan.scale.max(1e-6);
        let positions = &mesh.positions;
        let offsets = in_parallel(positions.len(), Vec3::ZERO, |v| {
            if weights[v] == 0.0 {
                return Vec3::ZERO;
            }
            let along = match plan.direction {
                Direction::Normal => normals[v],
                Direction::X => Vec3::axis(0),
                Direction::Y => Vec3::axis(1),
                Direction::Z => Vec3::axis(2),
            };
            along * (value(&noise, plan, positions[v] / scale) * plan.amount * weights[v])
        });
        if give_up() {
            return None;
        }
        for (p, offset) in mesh.positions.iter_mut().zip(offsets) {
            *p = *p + offset;
        }
    }
    let moved = mesh.positions.iter().zip(&before).map(|(&a, &b)| (a - b).length()).fold(0.0, f64::max);
    Some(Outcome { mesh, capped, moved })
}

/// The function's value at `p` (already divided by the scale), as a share of `amount`: -1 to 1 for
/// Noise, 0 to 1 for the rest.
fn value(noise: &Noise, plan: &Shaping, p: Vec3) -> f64 {
    match plan.function {
        Function::Noise => noise.layered(p, plan.layers, Noise::perlin),
        Function::Ridges => noise.layered(p, plan.layers, |n, q| {
            // Folding the noise at zero makes a crease, sharpened by squaring.
            let ridge = 1.0 - n.perlin(q).abs();
            ridge * ridge
        }),
        Function::Cells => noise.layered(p, plan.layers, Noise::cells),
        Function::Inflate => 1.0,
        Function::Smooth => 0.0,
    }
}

/// `f` for every index below `count`, split over the cores.
fn in_parallel<T: Copy + Send>(count: usize, fill: T, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let mut out = vec![fill; count];
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = count.div_ceil(cores).max(4096);
    std::thread::scope(|scope| {
        for (index, part) in out.chunks_mut(chunk).enumerate() {
            let f = &f;
            scope.spawn(move || {
                for (offset, slot) in part.iter_mut().enumerate() {
                    *slot = f(index * chunk + offset);
                }
            });
        }
    });
    out
}
