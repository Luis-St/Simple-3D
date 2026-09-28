//! The outline goes all the way round, and does not scribble over the body.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_core::scene::Camera;
use simple3d_geom::primitives;

#[test]
pub(crate) fn a_smooth_solid_is_outlined_and_a_creased_one_is_not_scribbled_over() {
    // Compared with the old feature-edge outline (`prepare` alone): a 32-segment sphere had no feature
    // edges and drew nothing, a torus scribbled rings across its surface.
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
    // The hole is mid-frame, so the inner silhouette crosses it; creases covered it three times over.
    assert!(
        middle * 3 <= creased_middle,
        "the outline still scribbles over the torus: {middle} pixels in the middle against {creased_middle}"
    );
    assert!(outlined < creased, "the outline is no smaller than the creases: {outlined} against {creased}");
}

#[test]
pub(crate) fn the_selection_outline_goes_all_the_way_round() {
    // Silhouettes need a second pass one pixel out, since depth tests fail at the rim (a single pass
    // left 17 of 180 sectors empty). Which side is out is judged across the edge: against the shift's
    // axis alone, the bottom of the rim went bare at bearing 92 (issue 99). Checked sector by sector, since a dotted rim barely changes the
    // pixel count. The frame is large enough for two-degree sectors to be meaningful.
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
            live: Live::default(),
            section: Vec::new(),
        }
    }
    let ball = primitives::ellipsoid_mesh(50.0, 50.0, 50.0, 32);
    let prepared = Renderable::prepare_outlined(&ball);
    let plain = render(&wide(vec![Item { renderable: &prepared, style: Style::Solid }]));
    let outlined = render(&wide(vec![
        Item { renderable: &prepared, style: Style::Solid },
        Item { renderable: &prepared, style: Style::Selected },
    ]));

    // Every pixel the outline changed, as a difference, since blended lines never match exactly.
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
