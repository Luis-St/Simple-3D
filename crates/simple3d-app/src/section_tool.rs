//! The section plane: where it stands, how it is moved, and the window its
//! settings live in (issues 71, 72).
//!
//! The cut itself is the renderer's -- [`simple3d_geom::section`] decides what
//! survives the plane and closes what it opens. What is left is the half a
//! picture cannot show on its own: *where the plane is standing*, especially
//! when it has been slid past the model and is cutting nothing at all, and a
//! way of moving it by hand.
//!
//! Both are one thing here: a frame drawn in the plane, around the model, with
//! grips on it that slide the plane along its own axis. It is drawn with the 2D
//! painter over the finished image rather than as geometry in the frame, and
//! that is correct rather than convenient: everything the section keeps is
//! *behind* the plane, so there is nothing the frame could be hidden by.
//!
//! There are five of those grips -- the middle of each of the frame's four
//! edges, and the middle of the plane -- because which of them is in reach
//! depends entirely on where the model has been orbited to (issue 72). One grip
//! on the upper edge is behind the shape as soon as the plane is looked at from
//! below, and a control that has to be orbited to before it can be used is one
//! that has to be found again every time.
//!
//! The settings are an [in-place popup](crate::popup) over the viewport rather
//! than a section of the properties panel: the plane is not part of the
//! selection, and the numbers that say where it stands belong beside the frame
//! they move rather than in a panel that is describing something else. The
//! window standing open *is* the section being on -- closing it puts the model
//! back together, and rolling it up by its chevron leaves the cut where it is.
//!
//! Moving the plane is not an edit. It changes no geometry, so there is no undo
//! step for it and the document is not made dirty by it, exactly as toggling
//! the grid or an axis is not an edit either.

use crate::app::App;
use crate::panel_properties::{component, field_row, named, room_left, scalar_field, set_component, Scalar, POINT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use crate::view::View;

use simple3d_core::keymap::Command;
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

/// The five places the plane can be taken hold of: the middle of each of the
/// frame's four edges, and the middle of the plane itself (issue 72).
///
/// All five do the same thing -- slide the plane along its axis -- and they are
/// five rather than one because a single grip is only ever in reach from the
/// side of the model it was put on. Orbit round to look at the cut from
/// underneath and a grip on the upper edge is behind the shape, which leaves
/// the plane movable only by the number in its window.
///
/// The middle one is the one place two controls want the same pixels, and it
/// keeps them: [`interact`] is offered the pointer before the manipulator is,
/// so a press at the middle of the plane slides the plane. That costs the
/// selection nothing, because a manipulator handle only lands at the middle of
/// the shape on screen when it is pointing at the camera -- an arrow, a face or
/// a ring seen end-on, which cannot be dragged in that view whoever gets the
/// press. The one it does cost is a ring seen *nearly* edge-on, which passes
/// close to the middle and is still turnable: it is grabbed anywhere else along
/// its length instead.
pub fn grips(corners: &[Vec3; 4]) -> [Vec3; 5] {
    let middle = |a: usize, b: usize| (corners[a] + corners[b]) * 0.5;
    [middle(0, 1), middle(1, 2), middle(2, 3), middle(3, 0), middle(0, 2)]
}

/// The way the plane travels, which is always the positive axis: the offset is
/// a coordinate, so flipping which side is cut away must not turn the numbers
/// round as well.
fn travel(axis: usize) -> Vec3 {
    let mut dir = Vec3::ZERO;
    set_component(&mut dir, axis, 1.0);
    dir
}

/// Drag any of the grips to slide the plane. Returns whether the pointer
/// belongs to one of them this frame, so a drag that moves the plane does not
/// also select what is behind it.
///
/// Every grip is the same control in a different place, so they share the one
/// drag: whichever is taken hold of, the plane slides along its axis by the
/// same rule, and the others follow the frame while it does.
pub fn interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    let section = app.scene.settings.section;
    if !section.enabled {
        app.section_grab = None;
        app.section_hover = None;
        return false;
    }
    let axis = section.axis();
    let mut owned = false;
    // Which grip the pointer is on, kept for the drawing: the arrows that say
    // which way the plane travels are put on that one alone, so five grips do
    // not become five pairs of arrows over the model.
    let mut live = None;
    for (index, at) in grips(&frame(&section, app.evaluated.mesh.bounds())).into_iter().enumerate() {
        let Some((screen, _)) = view.project(at) else { continue };
        let response = ui
            .interact(
                egui::Rect::from_center_size(screen, egui::Vec2::splat(GRIP + 5.0)),
                ui.id().with(("section-grip", index)),
                egui::Sense::drag(),
            )
            .on_hover_text(format!("Slide the section along {}", section.axis_label()));

        if response.hovered() || response.dragged() {
            live = Some(index);
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
        owned |= response.dragged() || response.hovered();
    }
    app.section_hover = live;
    owned
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
    let live = app.section_grab.is_some() || app.section_hover.is_some();
    let colour = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_LO,
    };
    // The grips are drawn a shade brighter than the frame when they are at
    // rest: they lie over the model as often as over the background, and the
    // frame's grey is the model's own grey -- a hairline in it disappears
    // exactly where the shape is.
    let mark = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_HI,
    };
    for (index, &from) in screen.iter().enumerate() {
        painter.line_segment([from, screen[(index + 1) % screen.len()]], egui::Stroke::new(1.0_f32, colour));
    }
    for (index, at) in grips(&corners).into_iter().enumerate() {
        let Some((middle, _)) = view.project(at) else { continue };
        // Filled, not outlined. Half of what a grip lies over is the model
        // itself, and a hairline square in a grey close to the model's own is
        // invisible exactly where it is most needed -- which is where the shape
        // is.
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
        // An arrow head each way out of the grip under the pointer, along the
        // line the plane travels: the frame says where the plane is, not which
        // way it slides. On that one grip alone, because five sets of arrows
        // over the model is a diagram of the control rather than the model.
        if app.section_hover != Some(index) {
            continue;
        }
        let dir = crate::panel_viewport::screen_direction(view, at, travel(section.axis()));
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
}

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "section-tool";

/// How wide the window is: three axis chips across it, the offset field and its
/// label, and no wider. A popup lives over the model, so every pixel of it is a
/// pixel of the thing being cut that cannot be seen.
const WIDTH: f32 = 300.0;

/// The section's window, drawn over the viewport once a frame while the plane
/// is out (issue 72).
///
/// It stands open for exactly as long as the section is on -- there is no
/// separate switch for the window, because a section with its settings put away
/// and a section that is off are the same picture. The chevron is what puts the
/// window out of the way while the cut stays.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.scene.settings.section.enabled {
        return;
    }
    let bounds = app.viewport_rect;
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Section", width: WIDTH }, |ui| {
            // Four rows and a hint is a short window until the rows stack on a
            // narrow one, and a viewport can be short: the body scrolls rather
            // than pushing the button off the bottom of the screen.
            let (area, restore) = theme::list_scroll_area(ui);
            area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
                ui.set_style(restore);
                body(app, ui);
            });
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross means the same thing the button in the row means: the plane is
    // put away and the model is whole again.
    if event == PopupEvent::Closed && app.scene.settings.section.enabled {
        app.run(Command::ToggleSection);
    }
}

/// The plane's settings: the axis it stands perpendicular to, where along that
/// axis it sits, and which side of it is cut away (issues 71, 72).
///
/// The offset is a scalar field like any other, so it can be typed exactly and
/// scrubbed with the pointer -- and the grips in the viewport slide the same
/// number. A plane that can only be dragged cannot be put at 12.5 mm, and one
/// that can only be typed cannot be swept through a part to find where the wall
/// gets thin.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    field_row(ui, "Plane", "The axis the section plane stands perpendicular to", |ui| {
        for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
            let showing = app.scene.settings.section.axis() == axis;
            if theme::choice(ui, showing, name).clicked() && !showing {
                app.scene.settings.section.axis = axis;
                // The old offset is a place on a different axis, so the plane
                // goes back to the middle of the model rather than to wherever
                // that number happens to land on this one.
                let middle = middle_of(app.evaluated.mesh.bounds(), axis);
                app.set_section_offset(middle);
            }
        }
    });
    field_row(ui, &named("At", unit.suffix()), "Where the plane sits along its axis", |ui| {
        let step = unit.from_mm(app.move_snap()).max(1e-6);
        let width = room_left(ui).max(40.0);
        let id = ui.id().with("section-offset");
        ui.scope(|ui| {
            ui.set_width(width);
            let field = Scalar { grip: "Section", id, kind: POINT, current: app.scene.settings.section.offset, step };
            // No undo step: moving the plane is not an edit -- see the module
            // header.
            scalar_field(app, ui, field, |app, mm, _| app.set_section_offset(mm));
        });
    });
    field_row(ui, "Keeps", "Which side of the plane stays in the picture", |ui| {
        // Named by the coordinate, not by the camera: which side is nearer
        // depends on where the model has been orbited to.
        for (flipped, label) in [(false, "Below"), (true, "Above")] {
            let showing = app.scene.settings.section.flipped == flipped;
            if theme::choice(ui, showing, label).clicked() && !showing {
                app.scene.settings.section.flipped = flipped;
                app.status = crate::app::Status::Info(readout(app));
            }
        }
    });
    ui.add(
        egui::Label::new(theme::hint(
            "The cut is on screen only: what is exported is the whole model. Drag any of the five marks on the \
             frame to slide the plane.",
        ))
        .selectable(false),
    );
}

/// The buttons along the foot: put the plane away, or stand it back in the
/// middle of the model.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.run(Command::ToggleSection);
    }
    // At the other end of the row, the way the measure tool's Clear is: a plane
    // swept out past the model shows an uncut shape and no sign of what to do
    // about it, and orbiting round to find the frame again is the long way back.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui
            .add(egui::Button::new("Back to the middle"))
            .on_hover_text("Stand the plane in the middle of the model again")
            .clicked()
        {
            let middle = middle_of(app.evaluated.mesh.bounds(), app.scene.settings.section.axis());
            app.set_section_offset(middle);
        }
    });
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
    fn the_plane_is_held_at_each_edge_and_in_the_middle() {
        // Five places rather than one, so which of them the model is hiding
        // depends on where it has been orbited to and never on all of them at
        // once (issue 72).
        let bounds = Some((Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 20.0)));
        for axis in 0..3 {
            let corners = frame(&section(axis, 5.0), bounds);
            let middle = (corners[0] + corners[2]) * 0.5;
            let held = grips(&corners);

            // The last one is the middle of the plane, and the four before it
            // are the middles of its edges -- each one on the line between two
            // corners, and none of them a corner.
            assert!((held[4] - middle).length() < 1e-9, "the fifth grip is not the middle of the plane on axis {axis}");
            for (index, &at) in held[..4].iter().enumerate() {
                let (from, to) = (corners[index], corners[(index + 1) % 4]);
                assert!((at - (from + to) * 0.5).length() < 1e-9, "grip {index} is not on the middle of its edge");
                assert!((at - from).length() > 1e-6 && (at - to).length() > 1e-6, "grip {index} landed on a corner");
            }

            // And each of the four stands off the middle, so the one that is
            // over the model is never the only one there is.
            assert!(
                held[..4].iter().all(|&at| (at - middle).length() > 10.0),
                "an edge grip sits on top of the model on axis {axis}"
            );

            // Every one of them is in the plane: a grip that slides the cut has
            // to be where the cut is, or it would be pointing at the wrong
            // coordinate.
            assert!(
                held.iter().all(|&at| (component(at, axis) - 5.0).abs() < 1e-9),
                "a grip left the plane on axis {axis}"
            );
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
