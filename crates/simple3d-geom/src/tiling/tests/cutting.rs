//! What a cut produces: whole pieces, in the right number, adding up to the shape.

use super::*;
use crate::primitives::box_mesh;
use crate::vec3::Vec3;
use std::f64::consts::PI;

/// Piece volumes add up to the original, whatever the cell shape.
#[test]
pub(crate) fn every_kind_of_cell_adds_back_up_to_the_shape() {
    for kind in CellKind::ALL {
        let tiling = Tiling { kind, size: 9.0, depth: 7.0, ..Tiling::default() };
        let pieces = cut_box(&tiling);
        let total: f64 = pieces.iter().map(volume).sum();
        assert!(pieces.len() > 1, "{kind:?} made {} piece(s)", pieces.len());
        assert!((total - 9000.0).abs() < 1.0, "{kind:?} pieces hold {total} mm3 of the 9000 they were cut from");
    }
}

/// Interior cells skip the kernel, and the shortcut must agree: a 30 mm cube in 10 mm squares is
/// nine columns of exactly 100 mm2 each.
#[test]
pub(crate) fn cells_inside_the_solid_come_out_whole() {
    let pieces = cut_box(&Tiling { size: 10.0, ..Tiling::default() });
    assert_eq!(pieces.len(), 9);
    for piece in &pieces {
        assert!((volume(piece) - 1000.0).abs() < 1e-6, "a column came out at {} mm3", volume(piece));
    }
}

#[test]
pub(crate) fn layers_cut_along_the_axis_as_well() {
    let plain = cut_box(&Tiling { size: 10.0, ..Tiling::default() });
    let layered = cut_box(&Tiling { size: 10.0, layer: 5.0, ..Tiling::default() });
    assert_eq!(plain.len(), 9);
    assert_eq!(layered.len(), 18);
    let total: f64 = layered.iter().map(volume).sum();
    assert!((total - 9000.0).abs() < 1.0);
}

/// The axis is the cells' direction: cutting a 30 x 30 x 10 plate along X gives cells across its
/// 30 x 10 side.
#[test]
pub(crate) fn the_axis_chooses_which_way_the_cells_run() {
    let along_z = cut_box(&Tiling { size: 10.0, axis: 2, ..Tiling::default() });
    let along_x = cut_box(&Tiling { size: 10.0, axis: 0, ..Tiling::default() });
    assert_eq!(along_z.len(), 9);
    assert_eq!(along_x.len(), 3);
    for piece in &along_x {
        assert!((volume(piece) - 3000.0).abs() < 1e-6, "a slab came out at {} mm3", volume(piece));
    }
}

#[test]
pub(crate) fn a_cell_bigger_than_the_shape_leaves_it_in_one_piece() {
    let pieces = cut_box(&Tiling { size: 100.0, ..Tiling::default() });
    assert_eq!(pieces.len(), 1);
    assert!((volume(&pieces[0]) - 9000.0).abs() < 1e-6);
}

#[test]
pub(crate) fn a_count_too_large_is_refused_before_anything_is_built() {
    let bounds = (Vec3::new(-100.0, -100.0, -1.0), Vec3::new(100.0, 100.0, 1.0));
    let tiling = Tiling { size: 0.5, ..Tiling::default() };
    assert!(tiling.cell_count(bounds) > MAX_CELLS);
    assert!(tiling.refusal(bounds).is_some());
    assert!(Tiling { size: 20.0, ..Tiling::default() }.refusal(bounds).is_none());
}

/// Every piece is a solid on its own, even cut from a shape with a hole.
#[test]
pub(crate) fn every_piece_is_a_solid_of_its_own() {
    let plate =
        crate::csg_bsp::subtract(&box_mesh(40.0, 40.0, 6.0), &crate::primitives::cylinder_mesh(14.0, 14.0, 20.0, 48));
    for kind in CellKind::ALL {
        let tiling = Tiling { kind, size: 12.0, depth: 9.0, ..Tiling::default() };
        let pieces = cut(&plate, &tiling, &|| {}, &never).expect("nothing abandoned it");
        assert!(pieces.len() > 3, "{kind:?} made {} piece(s)", pieces.len());
        for (index, piece) in pieces.iter().enumerate() {
            assert!(piece.manifold_issue().is_none(), "{kind:?} piece {index}: {:?}", piece.manifold_issue());
        }
        let total: f64 = pieces.iter().map(volume).sum();
        let expected = 40.0 * 40.0 * 6.0 - PI * 7.0 * 7.0 * 6.0;
        assert!((total - expected).abs() < expected * 0.01, "{kind:?} holds {total} mm3 of {expected}");
    }
}

/// Margin cells are not pieces: a 60 x 40 plate in 10 mm squares is 24 cells, not 48.
#[test]
pub(crate) fn the_plan_counts_the_cells_that_can_hold_a_piece_and_no_others() {
    let bounds = (Vec3::new(-30.0, -20.0, -4.0), Vec3::new(30.0, 20.0, 4.0));
    let tiling = Tiling { size: 10.0, ..Tiling::default() };
    assert_eq!(planned(&tiling, bounds), 24);
    assert_eq!(cell_outlines(&tiling, bounds).len(), 24);
    // The planned cells are the ones the shape is cut into.
    let pieces = cut(&box_mesh(60.0, 40.0, 8.0), &tiling, &|| {}, &never).expect("nothing abandoned it");
    assert_eq!(pieces.len(), 24);
}

#[test]
pub(crate) fn giving_up_hands_back_nothing() {
    let tiling = Tiling { size: 2.0, ..Tiling::default() };
    assert!(cut(&box_mesh(30.0, 30.0, 10.0), &tiling, &|| {}, &|| true).is_none());
}
