//! The origin axes: where they lie and how they are styled.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::Vec3;
use simple3d_core::scene::Camera;

#[test]
pub(crate) fn along_the_grid_the_axes_travel_with_the_view_and_pinned_ones_do_not() {
    // Issue 14: along the grid, X and Y are the grid's lines through zero and always visible; the
    // pinned cross still shows where the origin is.
    let far =
        Camera { target: Vec3::new(4000.0, 0.0, 0.0), yaw: -55.0, pitch: 28.0, distance: 120.0, ..Camera::default() };
    let drawn = |style: AxisStyle| {
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.view = View::new(far, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));
        req.grid = Grid { visible: false, spacing: 10.0, axes: [true; 3], style, plane_marks: false };
        let frame = render(&req);
        pixels_of(&frame, req.palette.axis_x)
    };
    assert_eq!(drawn(AxisStyle::Origin), 0, "a pinned X axis should be long gone at this distance");
    assert!(drawn(AxisStyle::Grid) > 0, "the X axis should follow the grid and still be drawn");
}

#[test]
pub(crate) fn the_z_axis_stays_at_the_origin_in_both_styles() {
    // Z has no ground line, so neither style moves it off zero.
    let drawn = |style: AxisStyle| {
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.grid = Grid { visible: false, spacing: 10.0, axes: [false, false, true], style, plane_marks: false };
        let frame = render(&req);
        let mut columns: Vec<usize> = Vec::new();
        for i in 0..frame.width * frame.height {
            let o = i * 4;
            if frame.color[o..o + 3] == req.palette.axis_z[..3] {
                columns.push(i % frame.width);
            }
        }
        columns
    };
    for style in AxisStyle::ALL {
        let columns = drawn(style);
        assert!(!columns.is_empty(), "{style:?}: the Z axis did not draw");
        // The camera looks at the origin, so Z runs down the frame's middle in either style.
        let (lo, hi) = (*columns.iter().min().unwrap(), *columns.iter().max().unwrap());
        assert!(lo >= 78 && hi <= 82, "{style:?}: the Z axis is not at the origin: columns {lo}..{hi}");
    }
}

#[test]
pub(crate) fn the_two_axis_styles_are_two_different_pictures() {
    // Issue 23: the two styles looked identical at the origin; pinned is now a short bounded cross,
    // grid style runs the ground's width. Reach counts any non-background pixel, since axes fade.
    let reach = |style: AxisStyle| {
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.grid = Grid { visible: false, spacing: 10.0, axes: [true, false, false], style, plane_marks: false };
        let frame = render(&req);
        let centre = ((frame.width / 2) as f64, (frame.height / 2) as f64);
        (0..frame.width * frame.height)
            .filter(|&i| !is_background(&frame, i, &req.palette))
            .map(|i| ((i % frame.width) as f64 - centre.0).hypot((i / frame.width) as f64 - centre.1))
            .fold(0.0_f64, f64::max)
    };
    let pinned = reach(AxisStyle::Origin);
    let along = reach(AxisStyle::Grid);
    assert!(pinned > 0.0 && along > 0.0, "both styles should draw the X axis: {pinned} pinned, {along} along");
    assert!(along > pinned * 1.5, "the two styles look the same: {pinned} pinned against {along} along the grid");
}

#[test]
pub(crate) fn the_axes_lie_along_the_grid_lines_rather_than_across_them() {
    // Issue 20: the X axis could run between grid lines. Both are whole multiples of the spacing, so
    // zero is a grid line; checked from an off-grid target.
    let camera = Camera { target: Vec3::new(37.3, -12.8, 0.0), distance: 300.0, ..Camera::default() };
    let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));
    let spacing = effective_grid_spacing(&view, 10.0);
    for target in [camera.target.x, camera.target.y] {
        let centre = (target / spacing).round() * spacing;
        let remainder = (centre / spacing) - (centre / spacing).round();
        assert!(remainder.abs() < 1e-9, "the grid centre is not a whole multiple of the spacing");
        // Zero is one of the grid's lines, which is the axis.
        assert!((centre / spacing).abs().fract() < 1e-9);
    }
}

#[test]
pub(crate) fn each_origin_axis_can_be_turned_off_on_its_own() {
    // Three independent switches.
    let colours = |axes: [bool; 3]| {
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.grid = Grid { visible: false, spacing: 10.0, axes, style: AxisStyle::Origin, plane_marks: false };
        let frame = render(&req);
        let mut found: Vec<Rgba> = (0..frame.width * frame.height)
            .filter(|&i| !is_background(&frame, i, &req.palette))
            .map(|i| {
                let o = i * 4;
                [frame.color[o], frame.color[o + 1], frame.color[o + 2], 255]
            })
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    };

    assert!(colours([false, false, false]).is_empty(), "an axis was drawn with all three turned off");
    let all = colours([true; 3]);
    assert!(all.len() >= 3, "the three axes should be three colours, got {all:?}");
    for axis in 0..3 {
        let mut only = [false; 3];
        only[axis] = true;
        let drawn = colours(only);
        assert!(!drawn.is_empty(), "axis {axis} drew nothing when it was the one turned on");
        let mut without = [true; 3];
        without[axis] = false;
        let rest = colours(without);
        for colour in &drawn {
            assert!(!rest.contains(colour), "axis {axis} was still drawn after being turned off");
        }
    }
}

#[test]
pub(crate) fn the_grid_and_axes_draw_in_their_own_colours() {
    let empty = Renderable::empty();
    let mut req = request(vec![Item { renderable: &empty, style: Style::Solid }], DisplayMode::Shaded);
    // Close enough that a 10 mm grid is at full strength, with lines either side of the axes.
    req.view = req.view.with_camera(Camera { distance: 30.0, ..req.view.camera() });
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let frame = render(&req);
    for (name, colour) in
        [("X axis", req.palette.axis_x), ("Y axis", req.palette.axis_y), ("Z axis", req.palette.axis_z)]
    {
        let count = frame.color.chunks_exact(4).filter(|p| *p == colour).count();
        assert!(count > 0, "{name} did not draw");
    }
    // Counted as what the grid adds to the frame, since it fades and the axes cover its zero lines.
    let mut bare = req;
    bare.grid.visible = false;
    let bare = render(&bare);
    assert!(
        count_non_background(&frame, &Palette::dark()) > count_non_background(&bare, &Palette::dark()),
        "the grid did not draw"
    );
}
