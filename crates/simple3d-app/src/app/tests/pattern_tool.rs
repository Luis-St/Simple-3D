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

/// The tool's window has one width and takes whatever height its contents come
/// to, but the popup it lives in is dragged around a viewport of any size and
/// its body scrolls -- so what is in it has to fit the room it is given, and go
/// on fitting well below the width the window actually asks for (issues 67, 96).
///
/// The check is that nothing is laid out wider than the room it was given, and
/// it covers the button row too: that row fills from the right, so what
/// overflows it runs off the *left* edge, which is where the name field goes.
#[test]
pub(crate) fn the_pattern_tool_fits_whatever_width_its_window_is_given() {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool did not open, so this measures nothing");
    // Past the question the window opens on, so the stages are measured too.
    app.start_rule_from(pattern, 0);
    app.reevaluate_for_test();

    for width in [1400.0_f32, 900.0, 640.0, 470.0, 400.0, 360.0, 320.0, 300.0, 280.0] {
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
