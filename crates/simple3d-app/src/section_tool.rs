//! The section plane in the viewport: where it is, and how it is moved
//! (issue 71).
//!
//! The cut itself is the renderer's -- [`simple3d_geom::section`] decides what
//! survives the plane and closes what it opens. What is left is the half a
//! picture cannot show on its own: *where the plane is standing*, especially
//! when it has been slid past the model and is cutting nothing at all, and a
//! way of moving it by hand.
//!
//! Both are one thing here: a frame drawn in the plane, around the model, with
//! a grip in the middle of it that slides the plane along its own axis. It is
//! drawn with the 2D painter over the finished image rather than as geometry in
//! the frame, and that is correct rather than convenient: everything the
//! section keeps is *behind* the plane, so there is nothing the frame could be
//! hidden by.
//!
//! Moving the plane is not an edit. It changes no geometry, so there is no undo
//! step for it and the document is not made dirty by it, exactly as toggling
//! the grid or an axis is not an edit either.

use crate::app::App;
use crate::panel_properties::{component, set_component};
use crate::view::View;

use simple3d_core::scene::SectionView;
use simple3d_core::unit::format_length;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// How much wider than the model the frame is drawn, so it reads as a plane
/// passing through the shape rather than as an outline of it.
const MARGIN: f64 = 0.2;

/// The smallest half-width the frame is drawn at, in millimetres: a section
/// through a 2 mm pin still needs something to grab.
const MIN_HALF: f64 = 10.0;

/// What the frame falls back to with nothing in the scene: a square about the
/// origin, big enough to see and to take hold of.
const EMPTY_HALF: f64 = 25.0;

/// How big the grip is on screen, in points.
const GRIP: f32 = 13.0;

/// Everything about the section that changes the picture, so the viewport
/// redraws when the plane moves and not when anything else does.
pub fn hash_section(section: &SectionView, hasher: &mut impl Hasher) {
    section.enabled.hash(hasher);
    section.axis().hash(hasher);
    section.offset.to_bits().hash(hasher);
    section.flipped.hash(hasher);
}

/// The four corners of the frame, in world space and in order around it.
///
/// It is sized and centred on the model rather than on the origin: a plane
/// through a part that sits 300 mm out would otherwise be drawn in the middle
/// of the grid, nowhere near the thing it is cutting.
pub fn frame(section: &SectionView, bounds: Option<(Vec3, Vec3)>) -> [Vec3; 4] {
    let axis = section.axis();
    // The two directions the plane spans, with the second one upright wherever
    // the plane is upright: the grip hangs off that edge, and on a standing
    // plane it belongs at the top of it rather than off one side.
    let (u, v) = match axis {
        1 => (0, 2),
        _ => ((axis + 1) % 3, (axis + 2) % 3),
    };
    let (lo, hi) = bounds.unwrap_or((Vec3::splat(-EMPTY_HALF), Vec3::splat(EMPTY_HALF)));
    let half = |a: usize| (((component(hi, a) - component(lo, a)) * 0.5) * (1.0 + MARGIN)).max(MIN_HALF);
    let middle = |a: usize| (component(hi, a) + component(lo, a)) * 0.5;
    let corner = |su: f64, sv: f64| {
        let mut at = Vec3::ZERO;
        set_component(&mut at, axis, section.offset);
        set_component(&mut at, u, middle(u) + su * half(u));
        set_component(&mut at, v, middle(v) + sv * half(v));
        at
    };
    [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
}

/// Where the grip sits: the middle of the frame's upper edge.
///
/// Not the middle of the frame, which is where the model is and therefore where
/// the manipulator's own handles are. Two things to take hold of in one place
/// is one too many, and the plane is the one that would win a click meant for
/// the shape.
pub fn grip(corners: &[Vec3; 4]) -> Vec3 {
    (corners[2] + corners[3]) * 0.5
}

/// The way the plane travels, which is always the positive axis: the offset is
/// a coordinate, so flipping which side is cut away must not turn the numbers
/// round as well.
fn travel(axis: usize) -> Vec3 {
    let mut dir = Vec3::ZERO;
    set_component(&mut dir, axis, 1.0);
    dir
}

/// Drag the grip to slide the plane. Returns whether the pointer belongs to it
/// this frame, so a drag that moves the plane does not also select what is
/// behind it.
pub fn interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    let section = app.scene.settings.section;
    if !section.enabled {
        app.section_grab = None;
        return false;
    }
    let axis = section.axis();
    let at = grip(&frame(&section, app.evaluated.mesh.bounds()));
    let Some((screen, _)) = view.project(at) else { return false };
    let response = ui
        .interact(
            egui::Rect::from_center_size(screen, egui::Vec2::splat(GRIP + 5.0)),
            ui.id().with("section-grip"),
            egui::Sense::drag(),
        )
        .on_hover_text(format!("Slide the section along {}", section.axis_label()));

    app.section_hover = response.hovered() || response.dragged();
    if app.section_hover {
        ui.ctx().set_cursor_icon(crate::panel_viewport::slide_cursor(crate::panel_viewport::screen_direction(
            view,
            at,
            travel(axis),
        )));
    }
    // Where the pointer took hold of the plane, kept for the length of the
    // drag: without it the plane jumps so that the point grabbed lands under
    // the pointer, which for a grip in the middle of a large frame is a jump
    // of the whole model.
    if response.drag_started() {
        app.section_grab = ui
            .input(|i| i.pointer.interact_pos())
            .and_then(|cursor| view.ray_axis(cursor, Vec3::ZERO, travel(axis)))
            .map(|under| section.offset - under);
    }
    if response.dragged() {
        let grab = app.section_grab.unwrap_or(0.0);
        if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
            if let Some(under) = view.ray_axis(cursor, Vec3::ZERO, travel(axis)) {
                // Snapped to the document's move step, and freed or coarsened
                // by the same modifiers every other drag answers to.
                let wanted = crate::panel_viewport::mods_from(ui).snap(under + grab, app.move_snap());
                app.set_section_offset(wanted);
            }
        }
    }
    if response.drag_stopped() {
        app.section_grab = None;
    }
    response.dragged() || response.hovered()
}

/// The plane's frame and its grip, over the finished image.
pub fn draw(app: &App, painter: &egui::Painter, view: &View) {
    let section = app.scene.settings.section;
    if !section.enabled {
        return;
    }
    let corners = frame(&section, app.evaluated.mesh.bounds());
    let screen: Vec<egui::Pos2> = corners.iter().filter_map(|&at| view.project(at).map(|(p, _)| p)).collect();
    if screen.len() < 4 {
        return;
    }
    // The frame is a reference rather than a selection, so it is drawn in the
    // quiet grey the scene's own bounding box uses -- and brightens to the
    // accent while the plane is under the pointer or being moved.
    let live = app.section_grab.is_some() || app.section_hover;
    let colour = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_LO,
    };
    // The grip is drawn a shade brighter than the frame when it is at rest: it
    // lies over the model as often as over the background, and the frame's grey
    // is the model's own grey -- a hairline in it disappears exactly where the
    // shape is.
    let mark = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_HI,
    };
    for (index, &from) in screen.iter().enumerate() {
        painter.line_segment([from, screen[(index + 1) % screen.len()]], egui::Stroke::new(1.0_f32, colour));
    }
    let Some((middle, _)) = view.project(grip(&corners)) else { return };
    // Filled, not outlined. Half of what the grip lies over is the model
    // itself, and a hairline square in a grey close to the model's own is
    // invisible exactly where it is most needed -- which is where the shape is.
    painter.rect_filled(
        egui::Rect::from_center_size(middle, egui::Vec2::splat(GRIP)),
        2.0,
        crate::theme::token::SURFACE_1,
    );
    painter.rect_stroke(
        egui::Rect::from_center_size(middle, egui::Vec2::splat(GRIP)),
        2.0,
        egui::Stroke::new(1.5_f32, mark),
        egui::StrokeKind::Middle,
    );
    // An arrow head each way out of the grip, along the line the plane travels:
    // the frame says where the plane is, not which way it slides.
    let dir = crate::panel_viewport::screen_direction(view, grip(&corners), travel(section.axis()));
    if dir.length() > 1e-3 {
        let dir = dir / dir.length();
        for way in [1.0_f32, -1.0] {
            painter.line_segment(
                [middle + dir * (GRIP * 0.5 * way), middle + dir * (GRIP * 1.3 * way)],
                egui::Stroke::new(1.5_f32, mark),
            );
        }
    }
}

/// What the plane is doing, for the status line: where it stands and which side
/// of it is being kept.
pub fn readout(app: &App) -> String {
    let section = app.scene.settings.section;
    let unit = app.unit();
    format!(
        "Section at {} = {} {}, keeping what is {} it",
        section.axis_label(),
        format_length(section.offset, unit),
        unit.suffix(),
        // Said as the coordinate rather than as the camera sees it: which side
        // is the near one depends on where the model has been orbited to, and
        // the plane does not move when it is.
        if section.flipped { "above" } else { "below" }
    )
}

/// Put the plane back in the middle of the model, along the axis it is on.
///
/// What "on" means for a section that has just been switched on: an offset of
/// zero cuts nothing at all for a part that does not straddle the origin, and a
/// section that appears to do nothing reads as a broken one.
pub fn middle_of(bounds: Option<(Vec3, Vec3)>, axis: usize) -> f64 {
    match bounds {
        Some((lo, hi)) => (component(lo, axis) + component(hi, axis)) * 0.5,
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(axis: usize, offset: f64) -> SectionView {
        SectionView { enabled: true, axis, offset, flipped: false }
    }

    #[test]
    fn the_frame_lies_in_the_plane_and_covers_the_model() {
        let bounds = Some((Vec3::new(-10.0, -20.0, 0.0), Vec3::new(10.0, 20.0, 30.0)));
        let corners = frame(&section(2, 12.0), bounds);
        assert!(corners.iter().all(|c| (c.z - 12.0).abs() < 1e-9), "a corner left the plane: {corners:?}");
        // Wider than the model in both of the directions it spans, so the plane
        // reads as passing through the shape.
        let reach = corners.iter().fold(Vec3::splat(0.0), |m, c| m.max(Vec3::new(c.x.abs(), c.y.abs(), 0.0)));
        assert!(reach.x > 10.0 && reach.y > 20.0, "the frame is inside the model: {reach:?}");
    }

    #[test]
    fn the_frame_follows_a_model_that_is_nowhere_near_the_origin() {
        let bounds = Some((Vec3::new(300.0, 300.0, 0.0), Vec3::new(320.0, 320.0, 10.0)));
        let corners = frame(&section(2, 5.0), bounds);
        let middle = (corners[0] + corners[2]) * 0.5;
        assert!(
            (middle.x - 310.0).abs() < 1e-9 && (middle.y - 310.0).abs() < 1e-9,
            "the frame is centred at {middle:?}"
        );
    }

    #[test]
    fn the_grip_hangs_off_the_frame_rather_than_sitting_on_the_model() {
        // The middle of the frame is where the model is, and the manipulator's
        // own handles are there: two things to take hold of in one place is one
        // too many.
        let bounds = Some((Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 20.0)));
        for axis in 0..3 {
            let corners = frame(&section(axis, 5.0), bounds);
            let middle = (corners[0] + corners[2]) * 0.5;
            let held = grip(&corners);
            assert!((held - middle).length() > 10.0, "the grip is on top of the model on axis {axis}");
            // Upright wherever the plane is upright, so it is reached for over
            // the shape rather than beside it.
            if axis != 2 {
                assert!(held.z > middle.z, "the grip on axis {axis} is not at the top of the frame");
            }
        }
    }

    #[test]
    fn a_thin_model_still_gets_a_frame_worth_grabbing() {
        let bounds = Some((Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 40.0)));
        let corners = frame(&section(2, 5.0), bounds);
        assert!(corners.iter().any(|c| c.x.abs() >= MIN_HALF), "the frame is too small to take hold of");
    }

    #[test]
    fn the_plane_starts_in_the_middle_of_what_it_is_cutting() {
        let bounds = Some((Vec3::new(0.0, 0.0, 4.0), Vec3::new(10.0, 10.0, 24.0)));
        assert!((middle_of(bounds, 2) - 14.0).abs() < 1e-9);
        // Nothing in the scene: the origin, which is where a first shape lands.
        assert_eq!(middle_of(None, 1), 0.0);
    }
}
