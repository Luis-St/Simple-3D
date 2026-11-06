use super::*;
use crate::primitives::{box_mesh, ellipsoid_mesh};
use crate::vec3::Vec3;

fn edges_at_most(mesh: &Mesh, edge: f64) -> bool {
    mesh.indices.iter().all(|tri| {
        (0..3).all(|k| {
            let (a, b) = (mesh.positions[tri[k] as usize], mesh.positions[tri[(k + 1) % 3] as usize]);
            (b - a).length() <= edge + 1e-9
        })
    })
}

fn plan(function: Function) -> Shaping {
    Shaping { function, refine: false, ..Shaping::default() }
}

#[test]
fn refining_a_cube_leaves_no_long_edge_and_keeps_it_closed_and_whole() {
    let mut cube = box_mesh(10.0, 10.0, 10.0);
    cube.set_tag(7);
    cube.set_source(3);
    let (fine, capped) = refine_until(&cube, 2.0, MAX_TRIANGLES, &crate::never).expect("not abandoned");
    assert!(!capped);
    assert!(edges_at_most(&fine, 2.0));
    assert!(fine.triangle_count() > 12 * 16);
    assert_eq!(fine.manifold_issue(), None);
    assert!((fine.signed_volume() - 1000.0).abs() < 1e-6);
    assert!(fine.tags.iter().all(|&t| t == 7) && fine.sources.iter().all(|&s| s == 3));
    assert_eq!(fine.sources.len(), fine.triangle_count());
}

#[test]
fn refining_stops_at_its_budget_and_says_so() {
    let (fine, capped) = refine_until(&box_mesh(10.0, 10.0, 10.0), 0.1, 1_000, &crate::never).expect("not abandoned");
    assert!(capped);
    assert!(fine.triangle_count() <= 1_000);
    assert_eq!(fine.manifold_issue(), None);
}

#[test]
fn a_seed_gives_the_same_noise_every_time_and_another_seed_different_noise() {
    let (a, b, c) = (Noise::new(1), Noise::new(1), Noise::new(2));
    let points: Vec<Vec3> = (0..200).map(|i| Vec3::new(i as f64 * 0.37, i as f64 * 0.11, -(i as f64) * 0.23)).collect();
    assert!(points.iter().all(|&p| a.perlin(p) == b.perlin(p) && a.cells(p) == b.cells(p)));
    assert!(points.iter().any(|&p| (a.perlin(p) - c.perlin(p)).abs() > 0.1));
    for &p in &points {
        assert!(a.perlin(p).abs() <= 1.1, "{}", a.perlin(p));
        assert!((0.0..=1.0).contains(&a.cells(p)));
    }
    assert_eq!(a.perlin(Vec3::new(3.0, -4.0, 5.0)), 0.0, "zero on the lattice");
}

#[test]
fn noise_moves_a_sphere_by_at_most_its_amount_and_keeps_it_closed() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 32).weld();
    let outcome = shape(&ball, &Shaping { amount: 2.0, scale: 8.0, ..plan(Function::Noise) });
    assert!(outcome.moved > 0.5 && outcome.moved <= 2.0 * 1.1, "moved {}", outcome.moved);
    assert_eq!(outcome.mesh.positions.len(), ball.positions.len());
    assert_eq!(outcome.mesh.manifold_issue(), None);
}

#[test]
fn every_patterned_function_runs_and_ridges_and_cells_only_push_outwards() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 24).weld();
    for function in [Function::Ridges, Function::Cells] {
        let outcome = shape(&ball, &Shaping { amount: 1.0, scale: 6.0, ..plan(function) });
        assert!(outcome.moved > 0.1, "{function:?} did nothing");
        for (after, before) in outcome.mesh.positions.iter().zip(&ball.positions) {
            assert!(after.length() >= before.length() - 1e-9, "{function:?} pushed a vertex in");
        }
    }
}

#[test]
fn inflating_a_sphere_grows_its_radius_by_the_amount() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 32).weld();
    let outcome = shape(&ball, &Shaping { amount: 1.5, ..plan(Function::Inflate) });
    for p in &outcome.mesh.positions {
        assert!((p.length() - 21.5).abs() < 0.05, "{}", p.length());
    }
}

#[test]
fn moving_along_an_axis_changes_only_that_coordinate() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 16).weld();
    let outcome = shape(&ball, &Shaping { direction: Direction::Z, ..plan(Function::Noise) });
    for (after, before) in outcome.mesh.positions.iter().zip(&ball.positions) {
        assert_eq!((after.x, after.y), (before.x, before.y));
    }
}

#[test]
fn smoothing_takes_noise_back_out_without_shrinking_the_shape() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 32).weld();
    let rough = shape(&ball, &Shaping { amount: 1.5, scale: 3.0, layers: 1, ..plan(Function::Noise) }).mesh;
    let smooth = shape(&rough, &Shaping { layers: 20, ..plan(Function::Smooth) }).mesh;
    let spread = |mesh: &Mesh| {
        let radii: Vec<f64> = mesh.positions.iter().map(|p| p.length()).collect();
        let mean = radii.iter().sum::<f64>() / radii.len() as f64;
        // The mean departure from a sphere: one stubborn vertex says little about a surface.
        radii.iter().map(|r| (r - mean).abs()).sum::<f64>() / radii.len() as f64
    };
    assert!(spread(&smooth) < spread(&rough) * 0.5, "{} against {}", spread(&smooth), spread(&rough));
    let (before, after) = (ball.signed_volume(), smooth.signed_volume());
    assert!((after - before).abs() / before < 0.03, "volume {before} became {after}");
}

#[test]
fn the_base_and_a_holes_rim_stay_where_they_are() {
    let ball = ellipsoid_mesh(40.0, 40.0, 40.0, 32).weld();
    let based = shape(&ball, &Shaping { keep_base: true, base: 5.0, ..plan(Function::Inflate) }).mesh;
    for (after, before) in based.positions.iter().zip(&ball.positions) {
        if before.z < -20.0 + 5.0 {
            assert_eq!(after, before);
        }
    }
    let mut open = ball.clone();
    open.indices.truncate(open.indices.len() - 40);
    let rim = crate::topology::Topology::of(&open).boundary_vertices();
    let kept = shape(&open, &plan(Function::Inflate)).mesh;
    for (v, &on_rim) in rim.iter().enumerate() {
        if on_rim {
            assert_eq!(kept.positions[v], open.positions[v]);
        }
    }
}

#[test]
fn refining_first_gives_a_coarse_box_enough_vertices_to_show_the_noise() {
    let cube = box_mesh(20.0, 20.0, 20.0);
    let outcome = shape(&cube, &Shaping { edge: 1.0, scale: 4.0, ..Shaping::default() });
    assert!(outcome.mesh.positions.len() > 2000);
    assert!(outcome.moved > 0.3);
    assert_eq!(outcome.mesh.manifold_issue(), None);
}
