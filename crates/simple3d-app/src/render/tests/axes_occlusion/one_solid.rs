//! An axis meeting a single solid.

use super::*;
use crate::raster::Image;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use simple3d_geom::primitives;

#[test]
pub(crate) fn an_axis_arrives_at_the_solid_it_enters_and_stays_behind_it_on_the_way_out() {
    // Issue 47, and the rule the three earlier passes all missed. What
    // hides an axis on the way *in* is the material it runs through, not
    // the depth buffer: a shape merely standing in front of the line is no
    // reason to drop it, or the stretch arriving at that shape goes missing
    // and only the point where the line meets the surface is left.
    //
    // On the way out it is the other way round. Past the far surface the
    // line has left the solid and is simply behind it, so the depth buffer
    // is exactly the right question -- and answering it the same way as the
    // approach drew the arm across the face of a box it had already come
    // out of, which reads as a line inside the object.
    //
    // Sampled at points on the line itself, in the world, and measured as
    // the difference switching that one axis off makes -- a line is faded
    // and alpha-blended, so the pixel is never the axis colour exactly, and
    // counting coloured pixels is what let every earlier version of this
    // pass while being wrong.
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let frame = render(&req);

    for axis in 0..3 {
        let mut without = Request { grid: Grid { axes: [true; 3], ..req.grid }, ..request(Vec::new(), req.mode) };
        without.grid.axes[axis] = false;
        without.items = vec![Item { renderable: &prepared, style: Style::Solid }];
        let without = render(&without);

        // Whether this axis put anything on the frame at a point on it.
        let drawn_at = |at: f64| {
            let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
            let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
            // A line is a pixel wide and the projection rounds, so the
            // neighbourhood is what is asked, not the single pixel.
            (y.saturating_sub(1)..=y + 1).any(|y| {
                (x.saturating_sub(1)..=x + 1).any(|x| {
                    if x >= frame.width || y >= frame.height {
                        return false;
                    }
                    let o = (y * frame.width + x) * 4;
                    frame.color[o..o + 4] != without.color[o..o + 4]
                })
            })
        };

        // Which way this axis runs into the frame: the arm on the eye's
        // side is the one that arrives at a surface, and the other one
        // leaves through the back.
        let near = -component(req.view.forward(), axis).signum();

        // Inside the box, which spans -15..15: nothing of the line.
        for at in [-12.0, -6.0, 0.0, 6.0, 12.0] {
            assert!(!drawn_at(at), "axis {axis} drew inside the solid, at {at}");
        }
        // The near arm is there unbroken right up to the surface it goes
        // into -- including where it is still over the box's own
        // silhouette, which is the stretch the depth test used to eat.
        for at in [16.0, 18.0, 24.0] {
            assert!(drawn_at(at * near), "axis {axis} left a gap arriving at the solid, at {at}");
        }
        // The far arm is behind the box, so the box hides it like anything
        // else: nothing while it is over the silhouette...
        for at in [16.0, 18.0, 22.0] {
            assert!(!drawn_at(at * -near), "axis {axis} drew behind the solid, at {at}");
        }
        // ...and the line again once it is clear of it.
        assert!(drawn_at(40.0 * -near), "axis {axis} never came out from behind the solid");
    }
}

#[test]
pub(crate) fn a_solid_an_axis_does_not_run_through_hides_it_like_anything_else() {
    // Issue 47, the other half: only the solid an axis actually goes into
    // is seen through. A shape standing in front of the origin covers the
    // axes behind it, and a shape a *different* axis runs through covers
    // them just the same -- the exception is per axis and per solid, which
    // is why it cannot be one flag on the item.
    let mut req = request(Vec::new(), DisplayMode::Shaded);
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    // Between the eye and the origin, and square in front of it: the axes
    // cross its silhouette without touching the solid itself.
    let between = primitives::box_mesh(30.0, 30.0, 30.0).translated(req.view.offset_dir() * 40.0);
    let prepared = Renderable::prepare(&between);
    let items = vec![Item { renderable: &prepared, style: Style::Solid }];
    assert!(
        axis_material(&items, &req.grid, None).inside.iter().all(|spans| spans.is_empty()),
        "the solid was placed on an axis, so this proves nothing"
    );

    // The same request, with or without the solid in the way and with one
    // axis switched off, so what the axis drew can be measured as the
    // difference switching it off makes.
    let frame = |axes: [bool; 3], in_the_way: bool| {
        let mut this = Request { grid: Grid { axes, ..req.grid }, ..request(Vec::new(), req.mode) };
        this.view = req.view;
        if in_the_way {
            this.items = vec![Item { renderable: &prepared, style: Style::Solid }];
        }
        render(&this)
    };
    let bare = frame([true; 3], false);
    let covered = frame([true; 3], true);

    for axis in 0..3 {
        let mut axes = [true; 3];
        axes[axis] = false;
        let without = frame(axes, false);
        let covered_without = frame(axes, true);

        let drawn_at = |frame: &Image, reference: &Image, at: f64| {
            let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
            let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
            (y.saturating_sub(1)..=y + 1).any(|y| {
                (x.saturating_sub(1)..=x + 1).any(|x| {
                    if x >= frame.width || y >= frame.height {
                        return false;
                    }
                    let o = (y * frame.width + x) * 4;
                    frame.color[o..o + 4] != reference.color[o..o + 4]
                })
            })
        };

        for at in [-6.0, -3.0, 3.0, 6.0] {
            // The control: with nothing in the way the axis draws here...
            assert!(drawn_at(&bare, &without, at), "axis {axis} does not draw at {at} even with nothing in the way");
            // ...and with the solid in front of it, it does not.
            assert!(
                !drawn_at(&covered, &covered_without, at),
                "axis {axis} drew through a solid in front of it, at {at}"
            );
        }
    }
}
