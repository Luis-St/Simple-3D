//! An axis crossing several solids, including several in one mesh.

use super::*;
use crate::raster::Image;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::Vec3;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

#[test]
pub(crate) fn one_mesh_of_two_shapes_is_still_two_solids_to_an_axis() {
    // The viewport hands the renderer the *whole scene* as one mesh, so
    // "the solid an axis runs into" cannot be an item: a box on the origin
    // and a box nowhere near it arrive welded into a single `Renderable`,
    // and taking that as one solid drew all three axes across everything
    // (img2). The bodies are found in the mesh instead.
    let mut req = request(Vec::new(), DisplayMode::Shaded);
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    // One on the origin, and one off to the side but nearer the eye, so it
    // covers a stretch of the axes without touching any of them.
    let (right, up) = req.view.basis();
    let elsewhere =
        primitives::box_mesh(24.0, 24.0, 24.0).translated(req.view.offset_dir() * 40.0 + right * 26.0 + up * 8.0);
    let apart = Renderable::prepare(&elsewhere);
    let scene = simple3d_geom::csg_bsp::union(&primitives::box_mesh(20.0, 20.0, 20.0), &elsewhere);
    let prepared = Renderable::prepare(&scene);
    assert_eq!(prepared.body_count, 2, "the two shapes welded into one body, so this proves nothing");

    let items = vec![Item { renderable: &prepared, style: Style::Solid }];
    let material = axis_material(&items, &req.grid, None);
    for axis in 0..3 {
        let seen = material.through[axis].len();
        assert_eq!(seen, 1, "axis {axis} is seen through {seen} of the two bodies");
    }

    let frame = |axes: [bool; 3], items: Vec<Item<'_>>| {
        let mut this = Request { grid: Grid { axes, ..req.grid }, ..request(Vec::new(), req.mode) };
        this.view = req.view;
        this.items = items;
        render(&this)
    };
    fn solid<'a>(renderable: &'a Renderable) -> Vec<Item<'a>> {
        vec![Item { renderable, style: Style::Solid }]
    }
    // Where each box drew on its own, so a sample can be put behind the far
    // one on purpose -- and away from the near one, which is seen through.
    let apart_only = frame([false; 3], solid(&apart));
    let on_origin = Renderable::prepare(&primitives::box_mesh(20.0, 20.0, 20.0));
    let near_only = frame([false; 3], solid(&on_origin));
    let all = frame([true; 3], solid(&prepared));
    let bare = frame([true; 3], Vec::new());

    // Counted across all three axes: the far box sits on one side, so it can
    // cover the whole approach of one axis while leaving the others clear.
    let (mut behind_the_far_box, mut arriving) = (0, 0);
    for axis in 0..3 {
        let mut axes = [true; 3];
        axes[axis] = false;
        let (without, bare_without) = (frame(axes, solid(&prepared)), frame(axes, Vec::new()));
        let at = |frame: &Image, reference: &Image, at: f64| {
            let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
            let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
            let drawn = (y.saturating_sub(1)..=y + 1).any(|y| {
                (x.saturating_sub(1)..=x + 1).any(|x| {
                    x < frame.width && y < frame.height && {
                        let o = (y * frame.width + x) * 4;
                        frame.color[o..o + 4] != reference.color[o..o + 4]
                    }
                })
            });
            // Off-frame samples report the last pixel, which is background
            // in every one of these renders, so they simply do not qualify.
            let pixel = if x < frame.width && y < frame.height { x + y * frame.width } else { 0 };
            (drawn, pixel)
        };

        // Just outside the box on the origin, on the eye's side, where the
        // line runs up to the surface it goes into: drawn, over that box's
        // own silhouette. Only that arm -- the one leaving through the back
        // is behind the box, and the box hides it.
        let near = -component(req.view.forward(), axis).signum();
        // The samples are found rather than guessed: the far box sits off
        // to one side and may cover any given point of the near arm, so
        // walk out along it and take every point it does not cover.
        for step in 0..40 {
            let sample = (11.0 + step as f64 * 0.5) * near;
            let (drawn, pixel) = at(&all, &without, sample);
            // Only where the far box is not the one in the way: that stretch
            // is its own case, tested below.
            if !is_background(&apart_only, pixel, &req.palette) {
                continue;
            }
            arriving += 1;
            assert!(drawn, "axis {axis} stopped short of the solid it enters, at {sample}");
        }

        // ...and behind the box it never enters: not drawn. The samples are
        // found rather than guessed -- a point on this axis that the far box
        // covers, and that the axis does draw at with nothing in the way.
        for step in 0..80 {
            let sample = 15.0 + step as f64;
            for sample in [sample, -sample] {
                let (drawn_bare, pixel) = at(&bare, &bare_without, sample);
                let behind_far = !is_background(&apart_only, pixel, &req.palette);
                let over_near = !is_background(&near_only, pixel, &req.palette);
                // Behind the far box and clear of the near one, so the far
                // box is what is actually in the way.
                if !drawn_bare || !behind_far || over_near {
                    continue;
                }
                behind_the_far_box += 1;
                assert!(
                    !at(&all, &without, sample).0,
                    "axis {axis} drew through the solid it only passes behind, at {sample}"
                );
            }
        }
    }
    assert!(arriving > 0, "every sample beside the near box was covered by the far one");
    assert!(
        behind_the_far_box >= 2,
        "only {behind_the_far_box} samples landed behind the far box, so nothing was really tested"
    );
}

#[test]
pub(crate) fn the_spans_an_axis_is_inside_are_the_solids_it_passes_through() {
    // Pairs of crossings, per solid: in at one face, out at the other.
    let centred = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let beside = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 0.0, 0.0)));
    let items =
        vec![Item { renderable: &centred, style: Style::Solid }, Item { renderable: &beside, style: Style::Solid }];
    let grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let material = axis_material(&items, &grid, None);

    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let spans = &material.inside[0];
    assert_eq!(spans.len(), 2, "one span per solid the X axis passes through, got {spans:?}");
    assert!(spans.iter().any(|&(a, b)| near(a, -15.0) && near(b, 15.0)), "{spans:?}");
    assert!(spans.iter().any(|&(a, b)| near(a, 35.0) && near(b, 45.0)), "{spans:?}");
    // The second box is off the Y axis entirely, so only the first is on it.
    assert_eq!(material.inside[1].len(), 1);
    // Both boxes are run through by X, so neither may hide it...
    assert_eq!(material.through[0].len(), 2, "{:?}", material.through[0]);
    // ...while for Y only the one it goes into is seen through, which is
    // the whole of img2: the other box hides Y like anything else.
    assert_eq!(material.through[1].len(), 1, "{:?}", material.through[1]);
    // The furthest corner of the further box, which the arms have to clear.
    assert!(material.reach > 45.0, "reach was {}", material.reach);

    // A solid the axes miss is an ordinary occluder, and cuts nothing.
    let away = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 40.0, 0.0)));
    let items = vec![Item { renderable: &away, style: Style::Solid }];
    let material = axis_material(&items, &grid, None);
    assert!(material.inside.iter().all(|spans| spans.is_empty()), "a solid off the axes cut one of them");
    assert!(
        material.through.iter().all(|bodies| bodies.is_empty()),
        "a solid the axes never enter was marked see-through, so it would not hide them"
    );

    // A ghost hides nothing: it is see-through, and so is the axis in it.
    let ghosted = vec![Item { renderable: &centred, style: Style::Ghost }];
    let material = axis_material(&ghosted, &grid, None);
    assert!(material.through.iter().all(|bodies| bodies.is_empty()), "a ghost was marked see-through");
    assert!(material.inside[0].is_empty(), "a ghost cut the axis");

    // An axis that is switched off is not looked for at all.
    let items = vec![Item { renderable: &centred, style: Style::Solid }];
    let off = Grid { axes: [false, true, true], ..grid };
    assert!(axis_material(&items, &off, None).inside[0].is_empty(), "a switched-off axis was cut out of the model");
}

#[test]
pub(crate) fn a_stretch_of_axis_is_cut_at_every_boundary_it_crosses() {
    let spans = [(-15.0, 15.0), (35.0, 45.0)];
    // Wholly clear, wholly inside, and straddling one edge.
    assert_eq!(clip_spans(20.0, 30.0, &spans), vec![(20.0, 30.0, false)]);
    assert_eq!(clip_spans(-10.0, 10.0, &spans), vec![(-10.0, 10.0, true)]);
    assert_eq!(clip_spans(10.0, 20.0, &spans), vec![(10.0, 15.0, true), (15.0, 20.0, false)]);
    // Across a whole span: three pieces, in the order they were asked for.
    assert_eq!(clip_spans(30.0, 50.0, &spans), vec![(30.0, 35.0, false), (35.0, 45.0, true), (45.0, 50.0, false)]);
    // Backwards, for the arm that runs the other way: the pieces come back
    // in that direction too, so the fade along the arm stays put.
    assert_eq!(clip_spans(50.0, 30.0, &spans), vec![(50.0, 45.0, false), (45.0, 35.0, true), (35.0, 30.0, false)]);
}
