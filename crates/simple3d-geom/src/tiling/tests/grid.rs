//! Where the grid falls: turned, evenly divided, and offset.

use super::*;

/// Turning the grid moves cuts without gaining or losing material.
#[test]
pub(crate) fn a_turned_grid_still_covers_the_shape() {
    let pieces = cut_box(&Tiling { size: 9.0, angle: 30.0, ..Tiling::default() });
    let total: f64 = pieces.iter().map(volume).sum();
    assert!((total - 9000.0).abs() < 1.0, "a turned grid holds {total} mm3 of 9000");
}

/// The grid is centred on what it cuts, so a 30 mm square in 15 mm cells gives four, not nine.
#[test]
pub(crate) fn an_even_division_falls_evenly() {
    let pieces = cut_box(&Tiling { size: 15.0, ..Tiling::default() });
    assert_eq!(pieces.len(), 4);
    for piece in &pieces {
        assert!((volume(piece) - 2250.0).abs() < 1e-6, "a quarter came out at {} mm3", volume(piece));
    }
}

#[test]
pub(crate) fn the_offset_moves_where_the_cuts_fall() {
    let centred = cut_box(&Tiling { size: 15.0, ..Tiling::default() });
    let shifted = cut_box(&Tiling { size: 15.0, offset: [7.5, 7.5], ..Tiling::default() });
    // Half a cell off, the same grid cuts nine unequal pieces.
    assert_eq!(centred.len(), 4);
    assert_eq!(shifted.len(), 9);
    for pieces in [&centred, &shifted] {
        let total: f64 = pieces.iter().map(volume).sum();
        assert!((total - 9000.0).abs() < 1.0);
    }
}
