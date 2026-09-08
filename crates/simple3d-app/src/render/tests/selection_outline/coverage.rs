//! The outline goes all the way round, and does not scribble over the body.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_core::scene::Camera;
use simple3d_geom::primitives;

#[test]
pub(crate) fn a_smooth_solid_is_outlined_and_a_creased_one_is_not_scribbled_over() {
    // Both halves of one bug, measured against the picture the feature
    // edges used to draw -- which is still what `prepare` alone gives, so
    // the old and the new can be rendered side by side.
    //
    // A 32-segment sphere creases at 11.25 degrees, under the 20-degree
    // feature-edge threshold, so it had no feature edges at all: selecting
    // one drew nothing, and only the manipulator said what was selected. A
    // torus at the same segment count creases past the threshold around its
    // tube, so selecting one scribbled concentric rings across the surface.
    let ball = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 32);
    let (creased, _) = selection_coverage_of(&Renderable::prepare(&ball));
    assert_eq!(creased, 0, "the sphere is only interesting because the creases drew nothing");
    let (outlined, middle) = selection_coverage_of(&Renderable::prepare_outlined(&ball));
    assert!(outlined > 50, "a smooth sphere got no selection outline: {outlined} pixels");
    assert_eq!(middle, 0, "the outline ran across the middle of the sphere: {middle} pixels");

    let ring = primitives::torus_mesh(40.0, 14.0, 360.0, 32);
    let (creased, creased_middle) = selection_coverage_of(&Renderable::prepare(&ring));
    let (outlined, middle) = selection_coverage_of(&Renderable::prepare_outlined(&ring));
    assert!(outlined > 50, "the torus got no selection outline: {outlined} pixels");
    // The hole is in the middle of the frame at this camera, so the inner
    // silhouette does cross it; the creases covered it three times over.
    assert!(
        middle * 3 <= creased_middle,
        "the outline still scribbles over the torus: {middle} pixels in the middle against {creased_middle}"
    );
    assert!(outlined < creased, "the outline is no smaller than the creases: {outlined} against {creased}");
}

#[test]
pub(crate) fn the_selection_outline_goes_all_the_way_round() {
    // A silhouette edge is the one line a depth test cannot draw: it lies
    // exactly where the surface turns away from the eye, so the face beside
    // it is nearly edge-on and its depth changes by more across one pixel
    // than a bias can cover. Drawn once, the outline of a 32-segment sphere
    // came out as a row of dots -- seventeen of these hundred and eighty
    // sectors with nothing in them at all, and raising the bias tenfold
    // still left six. The second pass, one pixel out from the shape, is over
    // background rather than over the shape and has nothing to win from.
    //
    // Asked as "is the rim drawn all the way round" rather than "how many
    // pixels are orange", because a dotted line and a solid one differ by
    // very little on a pixel count and by everything to look at.
    // A frame big enough for the question: at the stock test size the rim is
    // forty pixels across and a two-degree sector is less than one of them,
    // so bare sectors would say nothing about the drawing.
    fn wide<'a>(items: Vec<Item<'a>>) -> Request<'a> {
        Request {
            view: View::new(
                Camera { yaw: -55.0, pitch: 28.0, distance: 150.0, ..Camera::default() },
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0)),
            ),
            size: [640, 480],
            mode: DisplayMode::Shaded,
            palette: Palette::dark(),
            grid: Grid {
                visible: false,
                spacing: 10.0,
                axes: [false; 3],
                style: AxisStyle::Origin,
                plane_marks: false,
            },
            items,
            preview: Vec::new(),
            section: None,
        }
    }
    let ball = primitives::ellipsoid_mesh(50.0, 50.0, 50.0, 32);
    let prepared = Renderable::prepare_outlined(&ball);
    let plain = render(&wide(vec![Item { renderable: &prepared, style: Style::Solid }]));
    let outlined = render(&wide(vec![
        Item { renderable: &prepared, style: Style::Solid },
        Item { renderable: &prepared, style: Style::Selected },
    ]));

    // Every pixel the outline changed, measured as the difference selecting
    // makes: an alpha-blended line is never exactly its own colour.
    let changed: Vec<usize> = (0..plain.width * plain.height)
        .filter(|&i| plain.color[i * 4..i * 4 + 4] != outlined.color[i * 4..i * 4 + 4])
        .collect();
    assert!(changed.len() > 100, "the sphere got no outline at all: {} pixels", changed.len());

    let (width, sum) = (plain.width, changed.len() as f64);
    let cx = changed.iter().map(|i| (i % width) as f64).sum::<f64>() / sum;
    let cy = changed.iter().map(|i| (i / width) as f64).sum::<f64>() / sum;
    let mut sectors = [false; 180];
    for &i in &changed {
        let (x, y) = ((i % width) as f64 - cx, (i / width) as f64 - cy);
        let degrees = y.atan2(x).to_degrees().rem_euclid(360.0);
        sectors[(degrees / 2.0) as usize % 180] = true;
    }
    let bare: Vec<usize> = (0..180).filter(|&s| !sectors[s]).map(|s| s * 2).collect();
    assert!(bare.is_empty(), "the outline is broken at these bearings: {bare:?}");
}
