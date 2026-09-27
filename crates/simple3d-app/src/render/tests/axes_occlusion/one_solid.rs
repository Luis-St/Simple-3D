//! An axis meeting a single solid.

use super::*;
use crate::raster::Image;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::{primitives, Vec3};

#[test]
pub(crate) fn an_axis_arrives_at_the_solid_it_enters_and_stays_behind_it_on_the_way_out() {
    // Issue 47: on the way in, an axis is hidden only by the material it runs through, not by the
    // depth buffer, or the stretch arriving at a shape goes missing. On the way out the line is
    // behind the solid, and the depth test applies. Measured as the difference switching the axis
    // off makes, since faded, blended lines never match the axis colour exactly.
    let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    let frame = render(&req);

    for axis in 0..3 {
        let mut without = Request { grid: Grid { axes: [true; 3], ..req.grid }, ..request(Vec::new(), req.mode) };
        without.grid.axes[axis] = false;
        without.items = vec![Item { renderable: &prepared, style: Style::Solid }];
        let without = render(&without);

        // Whether this axis drew anything at a point on it.
        let drawn_at = |at: f64| {
            let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
            let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
            // Lines are a pixel wide and projection rounds, so the neighbourhood is checked.
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

        // The arm on the eye's side arrives at a surface; the other leaves through the back.
        let near = -component(req.view.forward(), axis).signum();

        // Inside the box (-15..15): nothing of the line.
        for at in [-12.0, -6.0, 0.0, 6.0, 12.0] {
            assert!(!drawn_at(at), "axis {axis} drew inside the solid, at {at}");
        }
        // The near arm is unbroken up to the surface, including over the box's silhouette.
        for at in [16.0, 18.0, 24.0] {
            assert!(drawn_at(at * near), "axis {axis} left a gap arriving at the solid, at {at}");
        }
        // The far arm is hidden while behind the box...
        for at in [16.0, 18.0, 22.0] {
            assert!(!drawn_at(at * -near), "axis {axis} drew behind the solid, at {at}");
        }
        // ...and visible again once clear of it.
        assert!(drawn_at(40.0 * -near), "axis {axis} never came out from behind the solid");
    }
}

#[test]
pub(crate) fn a_solid_an_axis_does_not_run_through_hides_it_like_anything_else() {
    // Issue 47: the see-through exception is per axis and per solid; other solids hide axes normally.
    let mut req = request(Vec::new(), DisplayMode::Shaded);
    req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
    // Between the eye and the origin: the axes cross its silhouette without touching it.
    let between = primitives::box_mesh(30.0, 30.0, 30.0).translated(req.view.offset_dir() * 40.0);
    let prepared = Renderable::prepare(&between);
    let items = vec![Item { renderable: &prepared, style: Style::Solid }];
    assert!(
        axis_material(&items, &req.grid, &[]).inside.iter().all(|spans| spans.is_empty()),
        "the solid was placed on an axis, so this proves nothing"
    );

    // With or without the solid and with one axis off, so the axis is measured as a difference.
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
            // Control: with nothing in the way the axis draws here...
            assert!(drawn_at(&bare, &without, at), "axis {axis} does not draw at {at} even with nothing in the way");
            // ...and with the solid in front it does not.
            assert!(
                !drawn_at(&covered, &covered_without, at),
                "axis {axis} drew through a solid in front of it, at {at}"
            );
        }
    }
}

#[test]
pub(crate) fn the_axis_in_a_hole_through_a_body_is_behind_the_wall_in_front_of_it() {
    // A block drilled through, with the X axis in the hole: one body, two stretches of material on
    // the line. Regression: the hole's stretch was drawn over the near wall; it should show only
    // where the hole is open to the eye.
    let block = primitives::box_mesh(13.3333, 20.0, 25.0).translated(Vec3::new(21.0, 0.0, 0.0));
    let hole = primitives::cylinder_mesh(8.0, 8.0, 60.0, 32).translated(Vec3::new(21.0, 0.0, 0.0));
    let prepared = Renderable::prepare(&simple3d_geom::csg_bsp::subtract(&block, &hole));

    for (yaw, pitch, open) in [(-90.0, 0.0, false), (-55.0, 28.0, false), (-90.0, 90.0, true)] {
        let camera = Camera { yaw, pitch, distance: 120.0, target: Vec3::new(10.0, 0.0, 0.0), ..Camera::default() };
        let (w, h) = (500usize, 400usize);
        let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32)));
        let frame = |axes: [bool; 3]| {
            let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
            req.view = view;
            req.size = [w, h];
            req.grid = Grid { visible: true, spacing: 10.0, axes, style: AxisStyle::Grid, plane_marks: false };
            render(&req)
        };
        let (with, without) = (frame([true; 3]), frame([false, true, true]));
        let drawn_at = |at: f64| {
            let (pos, _) = view.project(along(0, at)).expect("the sample is in front of the camera");
            let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
            let o = (y * w + x) * 4;
            with.color[o..o + 4] != without.color[o..o + 4]
        };
        // Control: the line is drawn arriving at the block (+X in all three views).
        assert!(drawn_at(32.0), "no axis arriving at the block at yaw {yaw}, pitch {pitch}");
        // Before the fix, the front view drew all of 18..24.
        for at in [19.0, 21.0, 23.0] {
            assert_eq!(
                drawn_at(at),
                open,
                "the axis in the hole at {at}, at yaw {yaw}, pitch {pitch}: drawn {}, and the hole is {}",
                drawn_at(at),
                if open { "open to the eye" } else { "behind a wall" }
            );
        }
    }
}
