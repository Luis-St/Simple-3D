//! The creases of a selected body, and the seams that are not creases.

use super::*;
use crate::raster::Image;
use simple3d_core::config::DisplayMode;
use simple3d_geom::primitives;
use simple3d_geom::Vec3;

/// The creases inside the contour carry the selection colour too (issue 89), but not the
/// hidden ones at the far corner, which once drew a cage. Shaded mode, so every accent pixel
/// inside the contour comes from the highlight.
#[test]
pub(crate) fn the_creases_facing_the_camera_are_part_of_the_selection() {
    let mesh = primitives::box_mesh(20.0, 20.0, 20.0);
    let (scene, selected) = (Renderable::prepare(&mesh), Renderable::prepare_outlined(&mesh));
    let accent = Palette::dark().selected;
    let items = || {
        vec![Item { renderable: &scene, style: Style::Solid }, Item { renderable: &selected, style: Style::Selected }]
    };

    // The nearest and furthest vertical edges, halfway down: view-space z grows with distance.
    let req = request(items(), DisplayMode::Shaded);
    let corners = |far: bool| {
        [(-10.0, -10.0), (-10.0, 10.0), (10.0, -10.0), (10.0, 10.0)]
            .into_iter()
            .map(|(x, y)| Vec3::new(x, y, 0.0))
            .max_by(|a, b| {
                let (a, b) = (req.view.to_view(*a).z, req.view.to_view(*b).z);
                if far {
                    a.total_cmp(&b)
                } else {
                    b.total_cmp(&a)
                }
            })
            .expect("four corners")
    };
    let at = |point: Vec3| {
        let p = req.view.project(point).expect("the box is in frame").0;
        (p.x.round() as usize, p.y.round() as usize)
    };
    let (near, far) = (at(corners(false)), at(corners(true)));

    let frame = render(&req);
    let (left, right) = accent_span(&frame, accent);
    let (top, bottom) = accent_rows(&frame, accent);
    // Inside the contour, so this tests creases rather than the silhouette.
    let (x, y) = near;
    assert!(x > left + 4 && x + 4 < right, "the near edge is at column {x}, not between {left} and {right}");
    assert!(y > top + 4 && y + 4 < bottom, "the near edge is at row {y}, not between {top} and {bottom}");

    let lit = |frame: &Image, (x, y): (usize, usize)| {
        (y - 2..=y + 2).any(|row| (x - 2..=x + 2).any(|col| rows_of(frame, col, accent, row..row + 1)))
    };
    assert!(lit(&frame, near), "the edge facing the camera was not part of the highlight");
    // With the shape's own edges drawn too, the highlight recolours them.
    assert!(
        lit(&render(&request(items(), DisplayMode::ShadedWithEdges)), near),
        "the facing crease lost its colour once the edge pass drew it as well"
    );

    // Near and far edges project too close to test by pixel, so lines are counted: six silhouette
    // edges and three near creases, each drawn twice (issue 99); twenty-four would mean the far ones
    // were drawn.
    assert!(near.0.abs_diff(far.0) < 8, "the two corners are far enough apart to test by eye after all");
    let lines =
        prepare(&req).iter().filter(|step| matches!(step, Step::Line { colour, .. } if *colour == accent)).count();
    assert_eq!(lines, (6 + 3) * 2, "the highlight is {lines} lines, not six silhouette edges and three creases");
}

/// A selected round shape shows its contours and nothing across them (issue 89).
///
/// A 32-segment torus's rings meet at 22.5 degrees, which the highlight must not treat as
/// creases. Lines through the middle cross the outline four times (torus) and twice (sphere);
/// at the tube's own threshold they crossed eleven and fourteen times.
#[test]
pub(crate) fn a_selected_round_shape_shows_its_contours_and_nothing_across_them() {
    let accent = Palette::dark().selected;
    for (name, mesh, crossings) in [
        ("torus", primitives::torus_mesh(30.0, 6.0, 360.0, 32), 4),
        ("sphere", primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 32), 2),
    ] {
        let (scene, selected) = (Renderable::prepare(&mesh), Renderable::prepare_outlined(&mesh));
        // Larger than other tests, since at 160 by 120 the torus rings run together.
        let req = Request {
            view: view(400, 300),
            size: [400, 300],
            ..request(
                vec![
                    Item { renderable: &scene, style: Style::Solid },
                    Item { renderable: &selected, style: Style::Selected },
                ],
                DisplayMode::Shaded,
            )
        };
        let frame = render(&req);
        let (top, bottom) = accent_rows(&frame, accent);
        let (left, right) = accent_span(&frame, accent);
        let across = runs(&frame, accent, (left..right).map(|x| (x, (top + bottom) / 2)));
        let down = runs(&frame, accent, (top..bottom).map(|y| ((left + right) / 2, y)));
        assert_eq!(across, crossings, "a line across the middle of the {name} met its outline {across} times");
        assert_eq!(down, crossings, "a line down the middle of the {name} met its outline {down} times");
    }
}

/// A seam where two bodies of one mesh touch is not part of the outline; it once drew a cage
/// over a selected split.
#[test]
pub(crate) fn a_seam_between_two_touching_bodies_is_not_part_of_the_outline() {
    let mut both = shifted_box(-10.0);
    both.append(&shifted_box(10.0));
    let prepared = Renderable::prepare_outlined(&both);
    assert!(prepared.outline.iter().any(|edge| edge.junction), "the seam was not seen as a junction at all");

    let frame = straight_on(vec![
        Item { renderable: &prepared, style: Style::Solid },
        Item { renderable: &prepared, style: Style::Selected },
    ]);
    let accent = Palette::dark().selected;
    let (first, last) = accent_span(&frame, accent);
    let seam = (first + last) / 2;
    let (top, bottom) = accent_rows(&frame, accent);
    let (from, to) = (top + 4, bottom - 4);
    assert!(
        !(seam.saturating_sub(1)..=seam + 1).any(|x| column_has_between(&frame, x, accent, from, to)),
        "the seam inside the shape was drawn as part of its outline"
    );
    assert!(last - first > 60, "the two boxes were not outlined as one shape");
}

/// A buried body is filled with a glow that nothing in front hides (issue 82).
#[test]
pub(crate) fn a_glowing_body_is_seen_through_whatever_is_in_front_of_it() {
    let outer = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let inner = Renderable::prepare_outlined(&primitives::box_mesh(10.0, 10.0, 10.0));
    // Measured as the change the body makes, since the blended glow is never exactly its colour.
    let frame_with = |items: Vec<Item<'_>>| render(&request(items, DisplayMode::Shaded));
    let plain = frame_with(vec![Item { renderable: &outer, style: Style::Solid }]);
    let changed = |style: Style| {
        let frame =
            frame_with(vec![Item { renderable: &outer, style: Style::Solid }, Item { renderable: &inner, style }]);
        (0..frame.width * frame.height)
            .filter(|&i| frame.color[i * 4..i * 4 + 3] != plain.color[i * 4..i * 4 + 3])
            .count()
    };
    // Outlined alone it is invisible, behind 15 mm of solid.
    assert_eq!(changed(Style::Selected), 0, "the buried body showed through without being asked to");
    assert!(changed(Style::Glow) > 100, "the glow of the buried body did not reach the frame");
}

/// A 20 mm box whose top is folded 45 degrees across its diagonal, by lifting a top corner off it,
/// with the fold's middle on screen.
fn folded_box() -> (simple3d_geom::Mesh, Vec3) {
    let mut mesh = primitives::box_mesh(20.0, 20.0, 20.0).weld();
    let top: Vec<[u32; 3]> =
        mesh.indices.iter().copied().filter(|t| t.iter().all(|&v| mesh.positions[v as usize].z > 9.0)).collect();
    let [first, second] = top[..] else { panic!("a box top is two triangles") };
    let lifted = first.into_iter().find(|v| !second.contains(v)).expect("a corner off the diagonal");
    let [a, b] = first.into_iter().filter(|v| second.contains(v)).collect::<Vec<_>>()[..] else {
        panic!("one diagonal")
    };
    // Half the diagonal's length up from 14.1 mm away tilts that half of the top by 45 degrees.
    mesh.positions[lifted as usize].z += 20.0 / 2f64.sqrt();
    let middle = (mesh.positions[a as usize] + mesh.positions[b as usize]) * 0.5;
    (mesh, middle)
}

/// A brushed mesh body bends past the 35-degree corner limit between coarse triangles all over its
/// folds, and the highlight drew broken strokes along them that came and went with the angle; the
/// fold is shaded there anyway. Between smooth triangles only a sharper corner is lined, a box's
/// still is, and a flat body's 45-degree fold keeps its line. On both renderers.
#[test]
pub(crate) fn a_smooth_bodys_gentle_folds_are_not_lined_but_its_corners_are() {
    let (mesh, middle) = folded_box();
    let mut smooth_mesh = mesh.clone();
    smooth_mesh.set_source(7);
    let flat = Renderable::prepare_outlined(&mesh);
    let smooth = Renderable::prepare_node(&smooth_mesh, true, &[7].into());
    let accent = Palette::dark().selected;
    let req = |selected| {
        request(
            vec![
                Item { renderable: selected, style: Style::Solid },
                Item { renderable: selected, style: Style::Selected },
            ],
            DisplayMode::Shaded,
        )
    };
    let at = |point: Vec3| {
        let p = req(&flat).view.project(point).expect("the box is in frame").0;
        (p.x.round() as usize, p.y.round() as usize)
    };
    // The fold's middle, and the middle of the vertical edge nearest the camera.
    let fold = at(middle);
    let near = [(-10.0, -10.0), (-10.0, 10.0), (10.0, -10.0), (10.0, 10.0)]
        .map(|(x, y)| Vec3::new(x, y, 0.0))
        .into_iter()
        .min_by(|a, b| req(&flat).view.to_view(*a).z.total_cmp(&req(&flat).view.to_view(*b).z))
        .expect("four corners");
    let near = at(near);
    let lit = |pixels: &[u8], width: usize, (x, y): (usize, usize)| {
        (y - 1..=y + 1).any(|row| {
            (x - 1..=x + 1).any(|col| {
                let o = (row * width + col) * 4;
                pixels[o..o + 3] == accent[..3]
            })
        })
    };
    let cpu = |renderable| {
        let frame = render(&req(renderable));
        (lit(&frame.color, frame.width, fold), lit(&frame.color, frame.width, near))
    };
    assert_eq!(cpu(&flat), (true, true), "a flat body's fold or corner lost its line");
    assert_eq!(cpu(&smooth), (false, true), "a smooth body's gentle fold was lined, or its corner was not");
    if let Some(headless) = super::gpu::headless() {
        let gpu = |renderable| {
            let request = req(renderable);
            let pixels = super::gpu::gpu_render(&headless, &request);
            (lit(&pixels, request.size[0], fold), lit(&pixels, request.size[0], near))
        };
        assert_eq!(gpu(&flat), (true, true), "on the GPU, a flat body's fold or corner lost its line");
        assert_eq!(gpu(&smooth), (false, true), "on the GPU, a smooth body's gentle fold was lined");
    }
}
