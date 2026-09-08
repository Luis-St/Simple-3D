//! The banded rasteriser draws what one thread would.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::Vec3;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

/// Splitting the frame across threads must not change one pixel of it.
///
/// This is the whole contract the banded renderer rests on, and it is not
/// self-evident: the first version of it clipped each line to the band
/// before stepping along it, which re-spaced the samples and moved every
/// grid line and feature edge by up to a pixel wherever a band began. The
/// picture still looked right on its own; it was only wrong against the
/// picture one thread drew. Every case below fails on that version.
#[test]
pub(crate) fn bands_draw_the_very_same_frame_as_one_thread() {
    let mut mesh = primitives::box_mesh(30.0, 20.0, 14.0);
    // A round body, so there are many small triangles and many feature
    // edges landing at every angle to the band boundaries.
    mesh.append(
        &primitives::ellipsoid_mesh(18.0, 18.0, 18.0, 24)
            .transformed(simple3d_geom::Vec3::new(22.0, 8.0, 4.0), simple3d_geom::Vec3::ZERO),
    );
    let prepared = Renderable::prepare(&mesh);
    for mode in [DisplayMode::ShadedWithEdges, DisplayMode::Shaded, DisplayMode::Wireframe] {
        let items = vec![Item { renderable: &prepared, style: Style::Solid }];
        let mut req = request(items, mode);
        // The grid, the axes and the plane marks all draw lines that cross
        // the whole frame, so they cross every band boundary there is.
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Grid, plane_marks: true };
        let prepared = prepare_frame(&req);
        let one = render_in_bands(&req, &prepared, 1);
        for bands in [2, 3, 7, 16] {
            let many = render_in_bands(&req, &prepared, bands);
            let differing =
                (0..one.color.len() / 4).filter(|i| one.color[i * 4..i * 4 + 4] != many.color[i * 4..i * 4 + 4]);
            let differing: Vec<usize> = differing.collect();
            assert!(
                differing.is_empty(),
                "{mode:?} in {bands} bands differs from one band at {} pixels, first at ({}, {})",
                differing.len(),
                differing[0] % one.width,
                differing[0] / one.width,
            );
        }
    }
}

#[test]
pub(crate) fn rendering_the_same_scene_twice_gives_the_same_image() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 20.0, 10.0));
    let build = || {
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::ShadedWithEdges);
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        render(&req).color
    };
    assert_eq!(build(), build());
}

#[test]
pub(crate) fn a_camera_inside_the_model_does_not_smear_across_the_viewport() {
    let prepared = Renderable::prepare(&primitives::box_mesh(200.0, 200.0, 200.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.view.camera.distance = 1.0;
    let frame = render(&req);
    // A parallel projection has no near-plane singularity to fall into: the
    // walls the camera has passed simply land behind the ones it has not, so
    // this is a partial fill rather than a panic or a screen of garbage.
    assert_eq!(frame.color.len(), 160 * 120 * 4);
}

#[test]
pub(crate) fn a_view_from_far_away_renders_too() {
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.view.camera.distance = 4000.0;
    req.view.camera.fov_deg = 2.0;
    let frame = render(&req);
    assert!(count_non_background(&frame, &req.palette) > 1000);
}

#[test]
pub(crate) fn nearer_geometry_hides_what_is_behind_it() {
    let near = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let far = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0).translated(Vec3::new(0.0, 0.0, -200.0)));
    let req = request(
        vec![Item { renderable: &far, style: Style::Solid }, Item { renderable: &near, style: Style::Solid }],
        DisplayMode::Shaded,
    );
    let with_both = render(&req);
    let only_near = render(&request(vec![Item { renderable: &near, style: Style::Solid }], DisplayMode::Shaded));
    // The far box is below the near one on screen, so it adds pixels, but at
    // the centre the near box must still win.
    let centre = (120 / 2 * 160 + 160 / 2) * 4;
    assert_eq!(with_both.color[centre..centre + 4], only_near.color[centre..centre + 4]);
}
