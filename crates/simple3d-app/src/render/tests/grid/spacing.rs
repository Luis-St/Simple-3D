//! The spacing the grid draws at as the view zooms.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_core::scene::Camera;

#[test]
pub(crate) fn zooming_out_brings_the_next_grid_decade_in_without_a_step() {
    // Issue 25: the grid stepped up a whole decade at one particular zoom,
    // and the picture lurched -- every cell tenfold wider from one wheel
    // notch to the next, and the ground's extent with it. Sweeping the zoom
    // continuously, neither the coarse spacing nor the extent may ever jump.
    let mut previous: Option<(f64, f64)> = None;
    let mut distance = 20.0_f64;
    while distance < 200_000.0 {
        let camera = Camera { distance, ..Camera::default() };
        let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
        let (fine, coarse, strength) = grid_levels(&view, 1.0);
        let radius = grid_radius(&view);
        if let Some((previous_coarse, previous_radius)) = previous {
            // The extent follows the zoom itself, so one step of it moves
            // the extent by one step.
            assert!(
                radius / previous_radius < 1.05,
                "the ground's extent jumped at distance {distance}: {previous_radius} to {radius}"
            );
            // Nothing to check where the level that stepped up was the
            // document's own spacing: there was no finer decade under it to
            // be faded, and it was being drawn in full.
            if coarse != previous_coarse {
                // A decade may only arrive by taking over from the one below
                // it, and that one has to still be drawn at full strength as
                // it hands over -- so the frame after the step looks like the
                // frame before it.
                assert!((coarse - previous_coarse * 10.0).abs() < 1e-9, "the grid skipped a decade");
                assert!((fine - previous_coarse).abs() < 1e-9, "the level that stepped up is not the old coarse");
                assert!(
                    strength > 0.98,
                    "the grid stepped up while the decade below it was already faded to {strength}"
                );
            }
        }
        previous = Some((coarse, radius));
        distance *= 1.01;
    }
}

#[test]
pub(crate) fn hiding_the_grid_hides_it() {
    let empty = Renderable::empty();
    let mut req = request(vec![Item { renderable: &empty, style: Style::Solid }], DisplayMode::Shaded);
    req.grid = Grid { visible: false, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let frame = render(&req);
    assert_eq!(frame.color.chunks_exact(4).filter(|p| *p == req.palette.grid).count(), 0);
    // The axes are not part of the grid toggle.
    assert!(frame.color.chunks_exact(4).any(|p| *p == req.palette.axis_x));
}

#[test]
pub(crate) fn grid_spacing_steps_up_so_a_fine_grid_stays_legible() {
    let mut v = view(800, 600);
    v.camera.distance = 50.0;
    assert_eq!(effective_grid_spacing(&v, 10.0), 10.0);
    // Zoomed far out, a 1mm grid would be sub-pixel, so it coarsens.
    v.camera.distance = 100_000.0;
    let spacing = effective_grid_spacing(&v, 1.0);
    assert!(spacing >= 100.0, "spacing stayed at {spacing}");
    assert!(spacing * v.pixels_per_mm() >= 6.0);
}

#[test]
pub(crate) fn the_light_and_dark_palettes_differ_in_every_role() {
    let (dark, light) = (Palette::dark(), Palette::light());
    assert_ne!(dark.background, light.background);
    assert_ne!(dark.background_low, light.background_low);
    // The gradient runs one way only: the sky is never darker than the floor.
    assert!(dark.background[0] < dark.background_low[0]);
    assert_ne!(dark.solid, light.solid);
    assert_ne!(dark.grid, light.grid);
    assert_eq!(Palette::for_dark_mode(true).background, dark.background);
    assert_eq!(Palette::for_dark_mode(false).background, light.background);
    // A dark background needs a light model and vice versa, or nothing reads.
    assert!(dark.background[0] < dark.solid[0]);
    assert!(light.background[0] > light.solid[0]);
}
