//! The section plane: what it takes out, and the cap it leaves.

use super::*;
use crate::raster::{Image, Rgba};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

/// The plane through the origin perpendicular to `axis`, cutting the
/// material past it away.
pub(crate) fn section_at(axis: usize, offset: f64) -> Plane {
    Plane::on_axis(axis, offset, false)
}

/// How many pixels the cap was drawn on: it is filled flat, in one colour
/// worked out once, so counting that colour is counting the cut.
pub(crate) fn cap_pixels(frame: &Image, palette: &Palette, plane: &Plane, view: &View) -> usize {
    let colour = shade(palette.cut, plane.normal, view.forward(), 255);
    (0..frame.width * frame.height).filter(|i| frame.color[i * 4..i * 4 + 4] == colour[..]).count()
}

#[test]
pub(crate) fn a_section_takes_the_material_past_the_plane_out_of_the_picture() {
    // Two boxes far enough apart to project to their own corners of the
    // frame, so what is drawn where can be asked of one of them at a time.
    let mut mesh = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(-60.0, 0.0, 0.0));
    mesh.append(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(60.0, 0.0, 0.0)));
    let prepared = Renderable::prepare(&mesh);
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.section = Some(section_at(0, 0.0));
    // The axes off: both boxes are centred on X, and an axis drawn through
    // where one of them was would answer the question for it.
    req.grid.axes = [false; 3];
    let frame = render(&req);
    let palette = Palette::dark();

    let pixel = |at: Vec3| {
        let (screen, _) = req.view.project(at).expect("the box is in the frame");
        (screen.y as usize) * frame.width + screen.x as usize
    };
    assert!(!is_background(&frame, pixel(Vec3::new(-60.0, 0.0, 0.0)), &palette), "the kept box is not drawn");
    assert!(
        is_background(&frame, pixel(Vec3::new(60.0, 0.0, 0.0)), &palette),
        "the box past the plane is still in the picture"
    );
}

#[test]
pub(crate) fn the_cut_is_capped_so_a_sectioned_solid_still_reads_as_solid() {
    let mesh = primitives::box_mesh(40.0, 40.0, 40.0);
    let prepared = Renderable::prepare(&mesh);
    let plane = section_at(2, 0.0);
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.section = Some(plane);
    let frame = render(&req);
    let palette = Palette::dark();
    assert!(
        cap_pixels(&frame, &palette, &plane, &req.view) > 200,
        "the cut was left open: the inside of the box is a hole in the picture"
    );
}

#[test]
pub(crate) fn a_wall_reads_as_a_wall_and_not_as_a_full_face() {
    // What the feature is for. A tube cut across shows a ring of material
    // and a hole through the middle of it, so the cap has to cover less
    // than the solid cylinder of the same size would.
    let plane = section_at(2, 0.0);
    let solid = Renderable::prepare(&primitives::cylinder_mesh(40.0, 40.0, 40.0, 48));
    let tube = Renderable::prepare(&primitives::tube_mesh(40.0, 28.0, 40.0, 48));
    let palette = Palette::dark();
    let covered = |item: &Renderable| {
        let mut req = request(vec![Item { renderable: item, style: Style::Solid }], DisplayMode::Shaded);
        req.section = Some(plane);
        let frame = render(&req);
        cap_pixels(&frame, &palette, &plane, &req.view)
    };
    let (whole, walled) = (covered(&solid), covered(&tube));
    assert!(walled > 0, "the tube was not capped at all");
    assert!(walled < whole, "the tube's cut covers {walled} pixels, the same as a solid rod's {whole}");
}

#[test]
pub(crate) fn a_wireframe_section_says_where_the_shape_was_cut() {
    // Wireframe fills nothing, so a cut with no line round it is a shape
    // whose edges simply stop in mid-air. The probe is the middle of one
    // side of the cut, where the box has no edge of its own: it is drawn
    // only if the cut brought its own outline.
    let prepared = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let items = || vec![Item { renderable: &prepared, style: Style::Solid }];
    let palette = Palette::dark();
    let probe = Vec3::new(20.0, 0.0, 0.0);
    let drawn = |section: Option<Plane>| {
        let mut req = request(items(), DisplayMode::Wireframe);
        req.grid.axes = [false; 3];
        req.section = section;
        let frame = render(&req);
        let (at, _) = req.view.project(probe).expect("the box is in the frame");
        let index = (at.y as usize) * frame.width + at.x as usize;
        !is_background(&frame, index, &palette)
    };
    assert!(!drawn(None), "the box already draws a line at the middle of its side, so the probe proves nothing");
    assert!(drawn(Some(section_at(2, 0.0))), "a wireframe section left no line where the shape was cut");
}

#[test]
pub(crate) fn what_the_cut_removed_no_longer_hides_an_origin_axis() {
    // The axis rule asks the model where it runs through material. With the
    // top of a box cut away, the stretch of Z that ran through it is in
    // open air and the line has to be drawn there again.
    let mesh = primitives::box_mesh(40.0, 40.0, 40.0);
    let prepared = Renderable::prepare(&mesh);
    let items = vec![Item { renderable: &prepared, style: Style::Solid }];
    let grid = Grid { visible: false, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let whole = axis_material(&items, &grid, None);
    let cut = axis_material(&items, &grid, Some(section_at(2, 0.0)));
    let top = |material: &AxisMaterial| material.inside[2].iter().fold(f64::MIN, |m: f64, span| m.max(span.1));
    assert!((top(&whole) - 20.0).abs() < 1e-6, "the box fills Z up to 20, got {}", top(&whole));
    assert!((top(&cut)).abs() < 1e-6, "the axis is still blocked up to {} above the cut", top(&cut));
}

#[test]
pub(crate) fn a_line_of_the_model_is_cut_with_the_faces() {
    // Edges, outlines and marks all go through one place, so one of them
    // standing for the rest is fair -- what would break is a path that
    // forgot to ask at all, and this is the check that it asked.
    let plane = section_at(2, 0.0);
    assert_eq!(kept_line(Some(plane), Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 9.0)), None);
    let kept = kept_line(Some(plane), Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0)).expect("half of it");
    assert!((kept.1.z).abs() < 1e-9, "the line was not trimmed at the plane: {kept:?}");
    // And with no section, every line is left exactly as it was.
    let (a, b) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(kept_line(None, a, b), Some((a, b)));
}

#[test]
pub(crate) fn a_shape_the_ground_plane_cuts_is_marked_where_it_cuts_it() {
    // Issue 16: how much of a shape is below the build plate is a real
    // dimension, and it is invisible until something marks it.
    let straddling = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let clear = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0).translated(Vec3::new(0.0, 0.0, 60.0)));
    let marks = |renderable: &Renderable, on: bool| {
        let mut req = request(vec![Item { renderable, style: Style::Solid }], DisplayMode::Shaded);
        req.grid =
            Grid { visible: false, spacing: 10.0, axes: [false, false, true], style: AxisStyle::Grid, plane_marks: on };
        pixels_of(&render(&req), req.palette.axis_z)
    };
    // Measured as the difference the switch makes, because the Z axis
    // itself is drawn in the same colour wherever the box does not hide it.
    assert!(
        marks(&straddling, true) > marks(&straddling, false),
        "the ground plane cuts this box and nothing said where"
    );
    assert_eq!(marks(&clear, true), marks(&clear, false), "a box clear of the plane has nothing to mark");
}

#[test]
pub(crate) fn a_plane_mark_follows_the_switch_of_the_axis_it_is_drawn_as() {
    // Issue 75. A mark is recognised by its colour and by nothing else, and
    // X's and Y's are exchanged on purpose, so the switch has to follow the
    // exchange too: the X box shows and hides the mark drawn in X's red,
    // whichever plane happens to leave it.
    let renderable = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let pixels = |marked: bool, colour: Rgba| {
        let mut req = request(vec![Item { renderable: &renderable, style: Style::Solid }], DisplayMode::Shaded);
        req.grid = Grid {
            visible: false,
            spacing: 10.0,
            axes: [true, false, false],
            style: AxisStyle::Grid,
            plane_marks: marked,
        };
        pixels_of(&render(&req), colour)
    };
    // Measured as the difference the marks make, because the X axis line is
    // drawn in the same red wherever the box does not hide it.
    let palette = Palette::dark();
    assert!(
        pixels(true, palette.axis_x) > pixels(false, palette.axis_x),
        "the X switch is on and no mark was drawn in X's colour"
    );
    assert_eq!(pixels(true, palette.axis_y), 0, "a mark the Y switch governs was drawn with Y turned off");
    assert_eq!(pixels(true, palette.axis_z), 0, "a mark the Z switch governs was drawn with Z turned off");
}
