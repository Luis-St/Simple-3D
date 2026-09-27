//! The outlines drawn before anything is cut.

use super::*;

#[test]
pub(crate) fn the_preview_draws_the_grid_at_both_ends_of_the_run() {
    // The loops are drawn on the model, so they sit at the top and bottom faces, and nowhere between
    // without layers.
    let tiling = Tiling { size: 15.0, ..Tiling::default() };
    let (lo, hi) = box_bounds();
    let loops = preview_loops(&tiling, (lo, hi), 1000);
    // Four cells across a 30 mm square at 15 mm, at each of two depths.
    assert_eq!(loops.len(), 8, "the grid was not drawn at both ends");
    let depths: Vec<f64> = loops.iter().map(|l| l[0].z).collect();
    assert!(depths.iter().any(|z| (z - hi.z).abs() < 1e-9), "nothing was drawn on the top face");
    assert!(depths.iter().any(|z| (z - lo.z).abs() < 1e-9), "nothing was drawn on the bottom face");
    assert!(depths.iter().all(|z| (z - hi.z).abs() < 1e-9 || (z - lo.z).abs() < 1e-9), "{depths:?}");
    // Every loop is closed, four-cornered and the stated size.
    for outline in &loops {
        assert_eq!(outline.len(), 4);
        assert!((outline[0] - outline[1]).length() - 15.0 < 1e-9);
    }
}

#[test]
pub(crate) fn layers_put_a_grid_at_every_cut_between_the_ends() {
    // A 10 mm run in 4 mm layers is cut at 4 and 8: four grids in all.
    let tiling = Tiling { size: 15.0, layer: 4.0, ..Tiling::default() };
    let (lo, hi) = box_bounds();
    let loops = preview_loops(&tiling, (lo, hi), 1000);
    let mut depths: Vec<f64> = loops.iter().map(|l| l[0].z).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    depths.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    assert_eq!(depths.len(), 4, "the layer cuts were not drawn: {depths:?}");
    assert!((depths[1] - (lo.z + 4.0)).abs() < 1e-9, "{depths:?}");
    assert!((depths[2] - (lo.z + 8.0)).abs() < 1e-9, "{depths:?}");
}

#[test]
pub(crate) fn the_preview_follows_the_axis_it_is_cut_through() {
    // Cells along X put their grids on the X faces, not flat on the ground.
    let tiling = Tiling { size: 15.0, axis: 0, ..Tiling::default() };
    let (lo, hi) = box_bounds();
    let loops = preview_loops(&tiling, (lo, hi), 1000);
    assert!(!loops.is_empty());
    for outline in &loops {
        let x = outline[0].x;
        assert!((x - lo.x).abs() < 1e-9 || (x - hi.x).abs() < 1e-9, "a loop was drawn at x = {x}");
        // The loop lies in the plane, so every corner shares that x.
        assert!(outline.iter().all(|p| (p.x - x).abs() < 1e-9));
    }
}

#[test]
pub(crate) fn the_preview_is_capped_rather_than_drawing_ten_thousand_loops() {
    // Thousands of cells are capped; the end laid down first survives.
    let tiling = Tiling { size: 0.5, ..Tiling::default() };
    let (lo, hi) = box_bounds();
    let loops = preview_loops(&tiling, (lo, hi), 100);
    assert_eq!(loops.len(), 100);
    assert!(loops.iter().all(|l| (l[0].z - hi.z).abs() < 1e-9), "the cap ate the wrong end");
    assert!(preview_loops(&tiling, (lo, hi), 0).is_empty());
}

#[test]
pub(crate) fn a_tiling_too_fine_to_cut_previews_nothing() {
    // A tiling too fine to cut previews nothing; `refusal` explains why.
    let (lo, hi) = box_bounds();
    assert!(preview_loops(&Tiling { size: 0.0, ..Tiling::default() }, (lo, hi), 1000).is_empty());
}
