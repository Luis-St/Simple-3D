use super::*;
use crate::primitives::{box_mesh, ellipsoid_mesh};
use crate::shaping::weights;
use crate::topology::{vertex_normals, Topology};
use crate::vec3::Vec3;
use crate::Mesh;

/// A sphere 40 mm across, its topology and normals, and the vertices within `radius` of its top.
fn sphere() -> (Mesh, Topology, Vec<Vec3>) {
    let mesh = ellipsoid_mesh(40.0, 40.0, 40.0, 64).weld();
    let topology = Topology::of(&mesh);
    let normals = vertex_normals(&mesh);
    (mesh, topology, normals)
}

fn region(mesh: &Mesh, centre: Vec3, radius: f64) -> Vec<(u32, f64)> {
    let near = mesh.positions.iter().enumerate().map(|(v, &p)| (v as u32, (p - centre).length()));
    near.filter(|&(_, d)| d <= radius).collect()
}

fn apply(mesh: &mut Mesh, moves: &[(u32, Vec3)]) {
    for &(v, p) in moves {
        mesh.positions[v as usize] = p;
    }
}

#[test]
fn the_falloffs_are_whole_at_the_centre_and_gone_at_the_edge() {
    for falloff in Falloff::ALL {
        assert_eq!(falloff.at(0.0, 5.0), 1.0, "{falloff:?}");
        assert_eq!(falloff.at(5.0, 5.0), 0.0, "{falloff:?}");
        assert_eq!(falloff.at(7.0, 5.0), 0.0, "{falloff:?}");
        let near_edge = falloff.at(4.5, 5.0);
        assert!(near_edge > 0.0 && near_edge < 1.0, "{falloff:?}");
    }
    // The plateau keeps more of its strength halfway out than the hill, the peak less.
    assert!(Falloff::Flat.at(2.5, 5.0) > Falloff::Smooth.at(2.5, 5.0));
    assert!(Falloff::Sharp.at(2.5, 5.0) < Falloff::Smooth.at(2.5, 5.0));
}

#[test]
fn raising_lifts_the_centre_by_a_tenth_of_the_radius_at_full_strength_and_lowering_presses_it_in() {
    let (mesh, topology, normals) = sphere();
    let top = Vec3::new(0.0, 0.0, 20.0);
    let weights = vec![1.0; mesh.positions.len()];
    let surface = Surface {
        positions: &mesh.positions,
        normals: &normals,
        neighbours: &topology.neighbours,
        weights: &weights,
        rest: &|_| None,
    };
    let brush = Brush { radius: 6.0, strength: 1.0, ..Brush::default() };
    let near = region(&mesh, top, brush.radius);
    let pole = near.iter().find(|&&(_, d)| d < 1e-9).expect("the pole is a vertex").0;
    for (mode, sign) in [(Mode::Raise, 1.0), (Mode::Lower, -1.0)] {
        let moves = dab(&brush, mode, &surface, &near);
        let to = moves.iter().find(|&&(v, _)| v == pole).expect("the centre moved").1;
        assert!((to.z - (20.0 + sign * 0.6)).abs() < 1e-9, "{mode:?}: {to:?}");
        // Nothing at or past the edge moves.
        assert!(moves.iter().all(|&(v, _)| (mesh.positions[v as usize] - top).length() < brush.radius));
    }
}

#[test]
fn smoothing_takes_a_bump_down_and_flattening_pulls_the_surface_onto_a_plane() {
    let (mut mesh, topology, _) = sphere();
    let top = Vec3::new(0.0, 0.0, 20.0);
    let raise = Brush { radius: 4.0, strength: 1.0, falloff: Falloff::Sharp, ..Brush::default() };
    let weights = vec![1.0; mesh.positions.len()];
    for _ in 0..5 {
        let normals = vertex_normals(&mesh);
        let surface = Surface {
            positions: &mesh.positions,
            normals: &normals,
            neighbours: &topology.neighbours,
            weights: &weights,
            rest: &|_| None,
        };
        let moves = dab(&raise, Mode::Raise, &surface, &region(&mesh, top, 4.0));
        apply(&mut mesh, &moves);
    }
    let peak = |mesh: &Mesh| mesh.positions.iter().map(|p| p.z).fold(f64::MIN, f64::max);
    let bumped = peak(&mesh);
    // Less than five full lifts: the dab stays at the old top while the surface rises out from under it.
    assert!(bumped > 21.2, "the raise reached only {bumped}");
    let soften = Brush { radius: 8.0, strength: 1.0, ..Brush::default() };
    for _ in 0..10 {
        let normals = vertex_normals(&mesh);
        let surface = Surface {
            positions: &mesh.positions,
            normals: &normals,
            neighbours: &topology.neighbours,
            weights: &weights,
            rest: &|_| None,
        };
        let moves = dab(&soften, Mode::Smooth, &surface, &region(&mesh, top, 8.0));
        apply(&mut mesh, &moves);
    }
    assert!(peak(&mesh) < bumped - 0.5, "smoothing left the bump at {} from {bumped}", peak(&mesh));
    let flat = Brush { radius: 10.0, strength: 1.0, falloff: Falloff::Flat, ..Brush::default() };
    let spread = |mesh: &Mesh| {
        let zs: Vec<f64> = region(mesh, top, 5.0).iter().map(|&(v, _)| mesh.positions[v as usize].z).collect();
        zs.iter().fold(f64::MIN, |a, &b| a.max(b)) - zs.iter().fold(f64::MAX, |a, &b| a.min(b))
    };
    let before = spread(&mesh);
    let normals = vertex_normals(&mesh);
    let surface = Surface {
        positions: &mesh.positions,
        normals: &normals,
        neighbours: &topology.neighbours,
        weights: &weights,
        rest: &|_| None,
    };
    let moves = dab(&flat, Mode::Flatten, &surface, &region(&mesh, top, 10.0));
    apply(&mut mesh, &moves);
    assert!(spread(&mesh) < before * 0.3, "the top did not flatten: {before} -> {}", spread(&mesh));
}

#[test]
fn a_kept_rim_and_base_do_not_move() {
    // A box with its top removed: an open rim at the top, a base at the bottom.
    let mut mesh = box_mesh(20.0, 20.0, 20.0);
    mesh.indices.retain(|tri| tri.iter().any(|&v| mesh.positions[v as usize].z < 19.0));
    let mesh = crate::shaping::refine_until(&mesh, 2.0, 1_000_000, &|| false).expect("not abandoned").0;
    let topology = Topology::of(&mesh);
    let normals = vertex_normals(&mesh);
    let brush = Brush { radius: 50.0, strength: 1.0, keep_base: true, base: 2.0, ..Brush::default() };
    let weights = weights(&mesh, &topology, &brush.keeping());
    let surface = Surface {
        positions: &mesh.positions,
        normals: &normals,
        neighbours: &topology.neighbours,
        weights: &weights,
        rest: &|_| None,
    };
    let all: Vec<(u32, f64)> = (0..mesh.positions.len() as u32).map(|v| (v, 0.0)).collect();
    let moved = dab(&brush, Mode::Raise, &surface, &all);
    let rim = topology.boundary_vertices();
    let floor = mesh.bounds().expect("a box").0.z + brush.base;
    for &(v, _) in &moved {
        assert!(!rim[v as usize], "a rim vertex moved");
        assert!(mesh.positions[v as usize].z > floor, "a base vertex moved");
    }
    assert!(!moved.is_empty());
}

#[test]
fn adding_detail_splits_only_near_the_dab_and_keeps_the_mesh_closed() {
    let mesh = box_mesh(20.0, 20.0, 20.0).weld();
    let refined = refine_within(&mesh, 1.0, Vec3::new(0.0, 0.0, 10.0), 3.0, 1_000_000).expect("the top was split").0;
    assert!(refined.manifold_issue().is_none(), "{:?}", refined.manifold_issue());
    assert_eq!(refined.positions[..8], mesh.positions[..], "the old vertices were renumbered");
    // Fine near the dab, coarse far from it.
    let longest_near = |at: Vec3, within: f64| {
        let topology = Topology::of(&refined);
        topology
            .edges
            .iter()
            .filter(|(ends, _)| ends.iter().all(|&v| (refined.positions[v as usize] - at).length() < within))
            .map(|(ends, _)| (refined.positions[ends[0] as usize] - refined.positions[ends[1] as usize]).length())
            .fold(0.0, f64::max)
    };
    assert!(longest_near(Vec3::new(0.0, 0.0, 10.0), 2.0) <= 1.0);
    assert!(refined.indices.len() < 20_000, "the whole box was refined: {}", refined.indices.len());
    assert!(refine_within(&refined, 1.0, Vec3::new(0.0, 0.0, 10.0), 2.0, 1_000_000).is_none());
}

/// Going over one spot again and again grows the noise in but never past a tenth of the radius,
/// measured from where the surface was when the stroke began.
#[test]
fn noise_never_piles_up_past_its_height() {
    let (mut mesh, topology, _) = sphere();
    let start = mesh.positions.clone();
    let start_normals = vertex_normals(&mesh);
    let top = Vec3::new(0.0, 0.0, 20.0);
    let brush = Brush { radius: 6.0, strength: 1.0, scale: 1.5, falloff: Falloff::Flat, ..Brush::default() };
    let weights = vec![1.0; mesh.positions.len()];
    let near = region(&mesh, top, brush.radius);
    for _ in 0..50 {
        let normals = vertex_normals(&mesh);
        let rest = |v: u32| Some((start[v as usize], start_normals[v as usize]));
        let surface = Surface {
            positions: &mesh.positions,
            normals: &normals,
            neighbours: &topology.neighbours,
            weights: &weights,
            rest: &rest,
        };
        let moves = dab(&brush, Mode::Noise, &surface, &near);
        apply(&mut mesh, &moves);
    }
    let furthest = mesh.positions.iter().zip(&start).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
    assert!(furthest > 0.2, "the noise hardly showed: {furthest}");
    assert!(furthest <= 0.6 + 1e-9, "the noise piled up to {furthest}");
}

/// The largest angle between neighbouring triangles, in degrees.
fn sharpest_crease(mesh: &Mesh, topology: &Topology) -> f64 {
    let normal = |face: u32| mesh.triangle_normal(mesh.indices[face as usize]);
    topology
        .edges
        .iter()
        .filter_map(|(_, faces)| match faces {
            [a, b] => Some(normal(*a).dot(normal(*b)).clamp(-1.0, 1.0).acos().to_degrees()),
            _ => None,
        })
        .fold(0.0, f64::max)
}

/// Strokes raised across a converted sphere's pole bend the surface but never fold it. Along each
/// vertex's own normal they did: the pole's fan of thin triangles has normals spreading every way,
/// and five passes folded triangles over (179 degrees between neighbours), a starburst of lines in
/// the viewport. Along the mean normal the sharpest bend is 34 degrees, where the 3 mm bump's crest
/// runs over the fan's 1 mm triangles.
#[test]
fn raising_across_a_pole_again_and_again_folds_nothing() {
    let mut mesh = ellipsoid_mesh(50.0, 50.0, 50.0, 160).weld();
    let topology = Topology::of(&mesh);
    let before = sharpest_crease(&mesh, &topology);
    let brush = Brush { radius: 5.0, strength: 0.3, ..Brush::default() };
    let weights = vec![1.0; mesh.positions.len()];
    for _ in 0..5 {
        for k in 0..30 {
            let centre = Vec3::new(-10.0 + 20.0 * k as f64 / 29.0, 0.0, 25.0);
            // Onto the surface under that point, as the pointer's ray lands.
            let down = Vec3::new(0.0, 0.0, -1.0);
            let from = centre + Vec3::new(0.0, 0.0, 50.0);
            let t = mesh
                .indices
                .iter()
                .filter_map(|&tri| crate::ray::line_triangle(from, down, mesh.corners(tri), 1e-9).filter(|&t| t > 0.0))
                .fold(f64::INFINITY, f64::min);
            let landed = from + down * t;
            let normals = vertex_normals(&mesh);
            let surface = Surface {
                positions: &mesh.positions,
                normals: &normals,
                neighbours: &topology.neighbours,
                weights: &weights,
                rest: &|_| None,
            };
            let moves = dab(&brush, Mode::Raise, &surface, &region(&mesh, landed, brush.radius));
            apply(&mut mesh, &moves);
        }
    }
    let after = sharpest_crease(&mesh, &topology);
    assert!(after < 60.0, "a crease of {after:.1} degrees (the sphere had {before:.1})");
    assert!(mesh.positions.iter().map(|p| p.z).fold(f64::MIN, f64::max) > 26.0, "the stroke did not rise");
}

/// Detail is added by longest sides, so the triangles keep their shape. Split by any long edge
/// reaching into the dab, a box's top turned into needles fanning out to its far corners, which a
/// raised stroke showed as a scratch of creases (2026-11-08).
#[test]
fn added_detail_makes_no_slivers() {
    let mesh = box_mesh(40.0, 40.0, 20.0).weld();
    let refined = refine_within(&mesh, 0.6, Vec3::new(-3.0, 1.0, 10.0), 4.0, 1_000_000).expect("split").0;
    let smallest = refined
        .indices
        .iter()
        .map(|&tri| {
            let c = refined.corners(tri);
            (0..3)
                .map(|k| {
                    let (a, b) = (c[(k + 1) % 3] - c[k], c[(k + 2) % 3] - c[k]);
                    a.cross(b).length().atan2(a.dot(b)).to_degrees()
                })
                .fold(f64::MAX, f64::min)
        })
        .fold(f64::MAX, f64::min);
    assert!(smallest > 20.0, "a triangle with a {smallest:.2} degree corner");
}
