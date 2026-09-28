pub mod aabb;
pub mod csg_bsp;
pub mod hull;
pub mod mesh;
pub mod number;
pub mod path;
pub mod planar;
pub mod polyhedra;
pub mod primitives;
pub mod ray;
pub mod reassemble;
pub mod repair;
pub mod revolve;
pub mod section;
pub mod simplify;
pub mod tiling;
pub mod vec3;

mod tests;

mod bench;

pub use mesh::{colour_tag, tag_colour, Mesh};
pub use vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
    Hull,
}

type Bounds = (Vec3, Vec3);

/// Whether two boxes overlap, with enough tolerance that touching shapes still go through the
/// kernel, which must handle coincident faces.
fn boxes_overlap(a: Bounds, b: Bounds) -> bool {
    let ((alo, ahi), (blo, bhi)) = (a, b);
    const SLACK: f64 = 1e-6;
    alo.x - SLACK <= bhi.x
        && blo.x - SLACK <= ahi.x
        && alo.y - SLACK <= bhi.y
        && blo.y - SLACK <= ahi.y
        && alo.z - SLACK <= bhi.z
        && blo.z - SLACK <= ahi.z
}

fn merged_bounds(a: Bounds, b: Bounds) -> Bounds {
    (a.0.min(b.0), a.1.max(b.1))
}

fn meshes_overlap(a: &Mesh, b: &Mesh) -> bool {
    match (a.bounds(), b.bounds()) {
        (Some(a), Some(b)) => boxes_overlap(a, b),
        _ => false,
    }
}

/// A difference's cutters that reach the base, packed into as few layers of mutually disjoint
/// cutters as possible, each layer one mesh.
///
/// One pass per layer instead of per cutter: a sphere drilled twenty times went from 3.4 s to
/// 0.27 s. Overlapping cutters go into later layers rather than being unioned first, which cost
/// more than the subtraction on a ring of sixty. Filled in child order for deterministic results.
fn disjoint_layers(base: &Mesh, cutters: &[Mesh]) -> Vec<Mesh> {
    let Some(base_bounds) = base.bounds() else { return Vec::new() };
    let reaching: Vec<&Mesh> =
        cutters.iter().filter(|cutter| cutter.bounds().is_some_and(|b| boxes_overlap(base_bounds, b))).collect();
    layers_of(&reaching)
}

/// `operands` packed into layers of mutually disjoint meshes, in order (see `disjoint_layers`).
fn layers_of(operands: &[&Mesh]) -> Vec<Mesh> {
    let mut layers: Vec<(Mesh, Vec<Bounds>)> = Vec::new();
    for operand in operands {
        let Some(bounds) = operand.bounds() else { continue };
        match layers.iter_mut().find(|(_, taken)| taken.iter().all(|&other| !boxes_overlap(other, bounds))) {
            Some((mesh, taken)) => {
                mesh.append(operand);
                taken.push(bounds);
            }
            None => layers.push(((*operand).clone(), vec![bounds])),
        }
    }
    layers.into_iter().map(|(mesh, _)| mesh).collect()
}

/// Union operands while keeping the result as mutually disjoint islands, each with its own box.
///
/// Folding `union` would grow one box over the whole plate and push every later operand through
/// the kernel against everything so far; per-island boxes limit the kernel to islands an operand
/// can touch. Merging can grow a box into a neighbour, so the search restarts until none overlap.
///
/// Operands that stay islands of their own are copied untouched and reported ([`Traced`]).
/// Islands are found from boxes first, then built a disjoint layer at a time
/// (`disjoint_layers`), since merging one by one re-ran the whole island per operand.
fn union_all(children: &[Mesh], give_up: Abandon<'_>) -> Traced {
    // Each island: its operands in child order, and its box.
    let mut islands: Vec<(Vec<usize>, Bounds)> = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Some(mut bounds) = child.bounds() else { continue };
        let mut members = vec![index];
        while let Some(i) = islands.iter().position(|(_, b)| boxes_overlap(*b, bounds)) {
            let (other, other_bounds) = islands.remove(i);
            members.extend(other);
            bounds = merged_bounds(bounds, other_bounds);
        }
        members.sort_unstable();
        islands.push((members, bounds));
    }
    let mut out = Mesh::new();
    let mut untouched = Vec::new();
    for (members, _) in &islands {
        if let [alone] = members[..] {
            untouched.push((alone, out.positions.len() as u32));
            out.append(&children[alone]);
            continue;
        }
        let (first, rest) = members.split_first().expect("an island has an operand");
        let rest: Vec<&Mesh> = rest.iter().map(|&index| &children[index]).collect();
        let mut acc = children[*first].clone();
        for layer in layers_of(&rest) {
            if give_up() {
                return (Mesh::new(), Vec::new());
            }
            acc = csg_bsp::union_until(&acc, &layer, give_up);
        }
        out.append(&acc);
    }
    (out, untouched)
}

/// A boolean's result, with the operands that passed through untouched: each one's index and the
/// result vertex where it starts; its vertices and triangles follow in order. Lets the viewport
/// move such a body without re-running the boolean.
pub type Traced = (Mesh, Vec<(usize, u32)>);

/// Polled wherever the kernel can safely stop: is this answer still wanted?
///
/// Booleans once ignored cancellation, so a big scatter could not be interrupted and every later
/// edit queued behind it. A callback rather than the core crate's `Cancel`, since this crate sits
/// below it.
pub type Abandon<'a> = &'a dyn Fn() -> bool;

/// Never gives up, for callers with nothing to cancel against.
pub fn never() -> bool {
    false
}

/// Combine evaluated child meshes by a group's boolean operation; `Difference` subtracts every
/// later mesh from the first, per the spec.
///
/// Operands with non-overlapping boxes skip the BSP kernel (concatenation, no-op, or empty).
/// This is what makes the 200-primitive target take a fraction of a second rather than eleven;
/// the result is identical.
pub fn evaluate_boolean(op: BooleanOp, children: &[Mesh]) -> Mesh {
    evaluate_boolean_until(op, children, &never)
}

/// The same, abandoned when `give_up` says so. The result is then never used.
pub fn evaluate_boolean_until(op: BooleanOp, children: &[Mesh], give_up: Abandon<'_>) -> Mesh {
    evaluate_boolean_traced(op, children, give_up).0
}

/// The same, reporting untouched operands ([`Traced`]): in a union those meeting no other, in a
/// difference the base when nothing is removed.
pub fn evaluate_boolean_traced(op: BooleanOp, children: &[Mesh], give_up: Abandon<'_>) -> Traced {
    match op {
        BooleanOp::Union => union_all(children, give_up),
        BooleanOp::Difference => {
            let Some((first, cutters)) = children.split_first() else { return (Mesh::new(), Vec::new()) };
            let layers = disjoint_layers(first, cutters);
            if layers.is_empty() {
                return (first.clone(), vec![(0, 0)]);
            }
            let mut result = first.clone();
            for layer in layers {
                if give_up() {
                    return (Mesh::new(), Vec::new());
                }
                result = csg_bsp::subtract_until(&result, &layer, give_up);
            }
            (result, Vec::new())
        }
        BooleanOp::Intersection => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return (Mesh::new(), Vec::new()) };
            let result = iter.fold(first.clone(), |acc, m| {
                if give_up() || !meshes_overlap(&acc, m) {
                    Mesh::new()
                } else {
                    csg_bsp::intersect_until(&acc, m, give_up)
                }
            });
            (result, Vec::new())
        }
        BooleanOp::Hull => {
            let points: Vec<Vec3> = children.iter().flat_map(|m| m.positions.iter().copied()).collect();
            (hull::convex_hull(&points), Vec::new())
        }
    }
}
