use super::settle::*;

use super::*;

fn bounds() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(800.0, 600.0))
}

#[test]
fn a_popup_opens_inside_the_viewport_it_belongs_to() {
    // Not in the middle of the screen the way a dialog does: it is drawn in
    // the viewport, and the viewport is not the window.
    let placed = settle(&Placement::default(), 320.0, bounds());
    assert!(bounds().contains(placed), "{placed:?} is outside {:?}", bounds());
    // In the top right corner (issue 103), clear of the edge by the same margin
    // on both sides.
    assert_eq!(placed, egui::pos2(bounds().right() - 320.0 - 16.0, bounds().top() + 16.0));
}

#[test]
fn a_popup_dragged_off_the_edge_keeps_its_title_bar_in_reach() {
    // Every edge, because a window pushed out of any of them is a window
    // that cannot be dragged back -- there is nothing left to grab.
    let bar = egui::vec2(320.0, TITLE_BAR);
    for away in [egui::vec2(-4000.0, 0.0), egui::vec2(4000.0, 0.0), egui::vec2(0.0, -4000.0), egui::vec2(0.0, 4000.0)] {
        let out = clamp_into(bounds().center() + away, bar, bounds());
        assert!(bounds().contains(out), "dragged by {away:?} the bar landed at {out:?}");
        assert!(out.x + bar.x <= bounds().right() + 0.001, "the bar hangs off the right edge at {out:?}");
        assert!(out.y + bar.y <= bounds().bottom() + 0.001, "the bar hangs off the bottom edge at {out:?}");
    }
}

#[test]
fn rolling_a_window_up_leaves_its_title_bar_exactly_where_it_was() {
    // The bug: a tall window near the bottom edge was lifted to fit, and
    // rolling it up removed the reason for the lift -- so the bar dropped
    // back down, out from under the chevron that had just been clicked.
    // Against a 600 px viewport, a 500 px window dropped at y = 400 was
    // lifted to y = 100, and collapsing it put it back at 400.
    let mut placement = Placement { pos: None, collapsed: false, height: 500.0 };
    placement.pos = Some(egui::pos2(300.0, 400.0));
    let open = settle(&placement, 320.0, bounds());
    assert!(open.y < 400.0, "a window taller than the room below it was not lifted to fit");
    placement.pos = Some(open);

    placement.collapsed = true;
    let rolled = settle(&placement, 320.0, bounds());
    assert_eq!(rolled, open, "rolling the window up moved its title bar");

    // And unrolling it does not move it either, because it was already
    // standing somewhere the whole window fits.
    placement.collapsed = false;
    assert_eq!(settle(&placement, 320.0, bounds()), open, "unrolling the window moved its title bar");
}

#[test]
fn a_window_that_fits_where_it_was_left_is_not_moved_at_all() {
    let placement = Placement { pos: Some(egui::pos2(300.0, 200.0)), collapsed: false, height: 180.0 };
    assert_eq!(settle(&placement, 320.0, bounds()), egui::pos2(300.0, 200.0));
}

#[test]
fn a_viewport_smaller_than_the_popup_still_leaves_it_somewhere_to_be() {
    // Dragging the panels out until the viewport is narrower than the
    // window must not produce a negative range to clamp into.
    let tiny = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 20.0));
    let out = clamp_into(egui::pos2(500.0, 500.0), egui::vec2(320.0, TITLE_BAR), tiny);
    assert_eq!(out, tiny.left_top());
}

#[test]
fn a_popup_taller_than_the_viewport_ends_inside_it() {
    // The bug: the room the body was given left out part of the window's own
    // chrome, so a body that had to scroll came out taller than the viewport
    // and its foot hung over the status bar. Against a viewport from y = 54 to
    // y = 855 the rule builder ran on to y = 867.
    for tall in [300.0_f32, 520.0, 801.0] {
        let bounds = egui::Rect::from_min_size(egui::pos2(300.0, 54.0), egui::vec2(780.0, tall));
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut placement = Placement::default();
        for frame in 0..20 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                time: Some(frame as f64 * 0.1),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                let spec = PopupSpec { key: "tall", title: "Tall", width: 430.0 };
                show(ctx, bounds, &mut placement, spec, |ui| {
                    scrolling_body(ui, bounds, |ui| {
                        for row in 0..200 {
                            ui.label(format!("row {row}"));
                        }
                    });
                    action_row(ui, |ui| {
                        let _ = ui.button("Done");
                    });
                });
            });
        }
        let window = ctx.memory(|m| m.area_rect(egui::Id::new(("in-place-popup", "tall")))).unwrap();
        assert!(
            window.top() >= bounds.top() && window.bottom() <= bounds.bottom() + 0.5,
            "at {tall} px the window spans {:?} in a viewport spanning {:?}",
            window.y_range(),
            bounds.y_range()
        );
    }
}
