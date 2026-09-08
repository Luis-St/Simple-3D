//! The pattern tool's window, at whatever width it is given.

use super::*;
use simple3d_core::keymap::Command;

#[test]
pub(crate) fn the_pattern_tool_wraps_the_selection_and_the_editor_draws() {
    // Issue 67: the creation tool wraps what is selected in a pattern node
    // that repeats it, selects the pattern, and the property editor's Pattern
    // section draws without panicking.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    assert!(app.scene.node(pat).is_pattern(), "the tool did not make a pattern");
    assert_eq!(app.scene.node(pat).children, vec![plate], "the shape was not put under the pattern");
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // Turning it into a circular pattern and drawing again exercises the
    // choice-gated fields the editor shows per kind.
    app.scene
        .get_mut(pat)
        .unwrap()
        .params_mut()
        .unwrap()
        .insert("kind".into(), simple3d_core::primitive::ParamValue::Choice(2));
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // With nothing selected the tool drops a bare pattern to fill later.
    app.clear_selection();
    app.run(Command::Pattern);
    assert!(app.scene.node(app.primary().unwrap()).is_pattern());
}

/// Issue 67: the tool's window is resizable, so what is in it has to fit
/// whatever width the window is dragged to.
///
/// Its three columns were three fixed widths -- 170 for the shelf, 300 for the
/// stages and whatever was left for the picture -- which is 640-odd pixels of
/// content whether or not the window has them. Dragged narrow, the picture was
/// squeezed to nothing and the rest ran off the right-hand edge. The columns
/// are shares of the room now, and give themselves up in order when there is
/// not enough of it.
///
/// The check is that nothing is laid out wider than the room it was given, at
/// each of the widths where the layout changes its mind and either side of
/// them, and it covers the button row too: that row fills from the right, so
/// what overflows it runs off the *left* edge, which is where "Name" went.
///
/// It stops at the window's own minimum size rather than going down to
/// nothing. Below that there is no layout to find -- two buttons and a field
/// are wider than that on their own -- which is why the window is not allowed
/// to be dragged there.
#[test]
pub(crate) fn the_pattern_tool_fits_whatever_width_its_window_is_given() {
    let mut app = headless_app();
    app.open_pattern_tool();
    assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
    app.reevaluate_for_test();

    for width in [1400.0_f32, 900.0, 820.0, 640.0, 560.0, 470.0, 400.0, 360.0, 330.0, 320.0] {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 520.0))),
            ..Default::default()
        };
        let (mut room, mut used) = (0.0_f32, 0.0_f32);
        let (mut row_room, mut row_used) = (0.0_f32, 0.0_f32);
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                room = ui.max_rect().width();
                crate::pattern_tool::body(&mut app, ui);
                used = ui.min_rect().width();
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    row_room = ui.max_rect().width();
                    crate::pattern_tool::actions(&mut app, ui);
                    row_used = ui.min_rect().width();
                });
            });
        });
        // Half a pixel of slack: a rule and its spacing are rounded, and this
        // is looking for columns that do not fit, not for rounding.
        assert!(used <= room + 0.5, "at {room} px of room the tool laid out {used} px of content");
        assert!(row_used <= row_room + 0.5, "at {row_room} px of room the button row took {row_used} px");
    }
}

/// The split tool's window is resizable too, and it has the same two-column
/// problem: the numbers beside a picture of the cells, and below the width
/// both need, the picture is what gives way (issue 82).
#[test]
pub(crate) fn the_split_tool_fits_whatever_width_its_window_is_given() {
    let mut app = headless_app();
    app.open_split_tool();
    assert!(app.split_tool.is_some(), "the tool did not open, so this measures nothing");

    for kind in simple3d_geom::tiling::CellKind::ALL {
        app.split_tool.as_mut().unwrap().plan.passes[0].kind = kind;
        for width in [1400.0_f32, 900.0, 680.0, 560.0, 480.0, 400.0, 360.0, 320.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 420.0))),
                ..Default::default()
            };
            let (mut room, mut used) = (0.0_f32, 0.0_f32);
            let (mut row_room, mut row_used) = (0.0_f32, 0.0_f32);
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    room = ui.max_rect().width();
                    crate::split_tool::body(&mut app, ui);
                    used = ui.min_rect().width();
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        row_room = ui.max_rect().width();
                        crate::split_tool::actions(&mut app, ui);
                        row_used = ui.min_rect().width();
                    });
                });
            });
            assert!(used <= room + 0.5, "{kind:?} at {room} px of room laid out {used} px of content");
            assert!(row_used <= row_room + 0.5, "at {row_room} px of room the button row took {row_used} px");
        }
    }
    // Drawing the tool must not have cut anything: nothing is cut until
    // Split is pressed.
    assert!(app.primary().is_some_and(|id| !app.scene.node(id).is_split()));
}

#[test]
pub(crate) fn the_pattern_tools_stages_keep_one_width_however_wide_the_window_is() {
    // Widening the window widens the picture and nothing else. The stages
    // are labelled fields with a natural width, and stretching them with the
    // window put a hundred pixels of nothing between every name and its
    // number; the viewport is the half that is worth more the bigger it is.
    let mut app = headless_app();
    app.open_pattern_tool();
    assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
    app.reevaluate_for_test();

    // Every width at which both columns fit, which is where the promise
    // holds; below it the stages give width up rather than the picture
    // vanishing, and that is a different rule.
    let mut measured: Vec<(f32, egui::Rect)> = Vec::new();
    for width in [640.0_f32, 900.0, 1200.0, 1600.0, 2400.0] {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 640.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(&mut app, ui));
        });
        let grip = crate::panel_properties::grip_id("tool:1 Copies");
        let rect = ctx.read_response(grip).expect("the first stage's copies field is drawn").rect;
        measured.push((width, rect));
    }
    let (first_width, first) = measured[0];
    for (width, rect) in &measured {
        assert!(
            (rect.width() - first.width()).abs() < 1.0 && (rect.right() - first.right()).abs() < 1.0,
            "at {width} px the stage field is {:?}, against {first:?} at {first_width} px",
            rect
        );
    }
}
