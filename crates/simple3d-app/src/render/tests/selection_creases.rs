//! The creases of a selected body, and the seams that are not creases.

use super::*;
use crate::raster::Image;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

/// The creases inside the contour carry the selection colour too (issue 89).
///
/// A box seen from a corner shows three faces and nine of its edges: six
/// make the silhouette, and three meet at the near vertical corner. Only
/// the six were coloured, so a selected box read as an orange ring drawn
/// around a grey box rather than as an orange box.
///
/// Both halves are measured, because the obvious fix breaks the second: the
/// three creases at the *far* corner project inside the same contour, and
/// drawing those is how an earlier pass at this scribbled a cage over the
/// model. Shaded is the mode used here deliberately -- it draws no edges of
/// its own, so every accent pixel inside the contour came from the
/// highlight and from nothing else.
#[test]
pub(crate) fn the_creases_facing_the_camera_are_part_of_the_selection() {
    let mesh = primitives::box_mesh(20.0, 20.0, 20.0);
    let (scene, selected) = (Renderable::prepare(&mesh), Renderable::prepare_outlined(&mesh));
    let accent = Palette::dark().selected;
    let items = || {
        vec![Item { renderable: &scene, style: Style::Solid }, Item { renderable: &selected, style: Style::Selected }]
    };

    // The vertical edges nearest to and furthest from the camera, halfway
    // down their run: view-space z grows with distance.
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
    // It really is inside the contour: an assertion about it is an
    // assertion about the creases and not about the silhouette.
    let (x, y) = near;
    assert!(x > left + 4 && x + 4 < right, "the near edge is at column {x}, not between {left} and {right}");
    assert!(y > top + 4 && y + 4 < bottom, "the near edge is at row {y}, not between {top} and {bottom}");

    let lit = |frame: &Image, (x, y): (usize, usize)| {
        (y - 2..=y + 2).any(|row| (x - 2..=x + 2).any(|col| rows_of(frame, col, accent, row..row + 1)))
    };
    assert!(lit(&frame, near), "the edge facing the camera was not part of the highlight");
    // And with the shape's own edges on screen too, where the highlight
    // recolours them rather than adding to them.
    assert!(
        lit(&render(&request(items(), DisplayMode::ShadedWithEdges)), near),
        "the facing crease lost its colour once the edge pass drew it as well"
    );

    // The hidden half cannot be ruled out by looking at a pixel: seen from
    // a corner, a cube's near and far vertical edges project within a few
    // pixels of each other, which is what `far` is worked out here to show.
    // So the lines are counted instead. Six silhouette edges, each drawn
    // twice -- once on the shape and once a pixel outside it -- and three
    // creases drawn once: eighteen would mean the far corner's three had
    // been drawn too, which is the cage an earlier attempt at this left
    // over the model.
    assert!(near.0.abs_diff(far.0) < 8, "the two corners are far enough apart to test by eye after all");
    let lines =
        prepare(&req).iter().filter(|step| matches!(step, Step::Line { colour, .. } if *colour == accent)).count();
    assert_eq!(lines, 6 * 2 + 3, "the highlight is {lines} lines, not six silhouette edges and three creases");
}

/// A selected round shape shows its contours and nothing across them
/// (issue 89, the other half).
///
/// The creases the highlight follows have to be the shape's own corners and
/// not the steps in how it is tessellated. A torus at the stock 32 segments
/// has a sixteen-sided tube, so its rings meet at 22.5 degrees -- real
/// feature edges by the twenty the edge pass draws at, and once they were
/// highlighted a selected torus came back with concentric rings in accent
/// across its whole visible surface. That is the picture an earlier attempt
/// at the outline was rejected for, and it is what this holds back.
///
/// Measured through the middle of the shape both ways: a torus crosses its
/// own outline four times over -- the outside and the hole, on each side --
/// and a sphere twice. Drawn at the tube's own threshold instead, the same
/// two lines crossed it eleven and fourteen times.
///
/// `a_smooth_solid_is_outlined_and_a_creased_one_is_not_scribbled_over`
/// asks the same of the picture as a ratio against what the feature edges
/// draw. This says it as a number of lines instead, which is the form the
/// answer is actually wanted in: four, and which four.
#[test]
pub(crate) fn a_selected_round_shape_shows_its_contours_and_nothing_across_them() {
    let accent = Palette::dark().selected;
    for (name, mesh, crossings) in [
        ("torus", primitives::torus_mesh(30.0, 6.0, 360.0, 32), 4),
        ("sphere", primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 32), 2),
    ] {
        let (scene, selected) = (Renderable::prepare(&mesh), Renderable::prepare_outlined(&mesh));
        // Larger than the other tests draw at: at 160 by 120 a torus is
        // thirty rows tall and its rings run together, which is a picture
        // nothing can be concluded from.
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

/// Where two bodies of one mesh touch, the seam is inside the shape and no
/// part of its outline.
///
/// Four triangles meet along such an edge, which used to be read as "no far
/// side to ask about" and drawn whatever the camera was doing. Every piece
/// of a split touches its neighbours, so selecting an eighty-piece one lit
/// every cut in it and drew a cage over the model.
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

/// A body inside another one is filled with a glow that whatever is in
/// front of it does not hide -- an outline has nothing on screen to draw
/// itself around when the shape it belongs to is buried (issue 82).
#[test]
pub(crate) fn a_glowing_body_is_seen_through_whatever_is_in_front_of_it() {
    let outer = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
    let inner = Renderable::prepare_outlined(&primitives::box_mesh(10.0, 10.0, 10.0));
    // Measured as the difference the buried body makes to the frame rather
    // than as a count of its own colour: the glow is blended over the solid
    // in front of it, so no pixel of it is ever exactly the colour it was
    // drawn in.
    let frame_with = |items: Vec<Item<'_>>| render(&request(items, DisplayMode::Shaded));
    let plain = frame_with(vec![Item { renderable: &outer, style: Style::Solid }]);
    let changed = |style: Style| {
        let frame =
            frame_with(vec![Item { renderable: &outer, style: Style::Solid }, Item { renderable: &inner, style }]);
        (0..frame.width * frame.height)
            .filter(|&i| frame.color[i * 4..i * 4 + 3] != plain.color[i * 4..i * 4 + 3])
            .count()
    };
    // Outlined alone it is invisible: every line of it is behind fifteen
    // millimetres of solid.
    assert_eq!(changed(Style::Selected), 0, "the buried body showed through without being asked to");
    assert!(changed(Style::Glow) > 100, "the glow of the buried body did not reach the frame");
}
