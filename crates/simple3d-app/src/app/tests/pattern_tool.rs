//! The pattern tool's window, at whatever width it is given.

use super::*;
use simple3d_core::keymap::Command;

#[test]
pub(crate) fn the_pattern_tool_wraps_the_selection_and_the_editor_draws() {
    // Issue 67: the tool wraps the selection in a selected pattern, and the editor section draws.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    assert!(app.scene.node(pat).is_pattern(), "the tool did not make a pattern");
    assert_eq!(app.scene.node(pat).children, vec![plate], "the shape was not put under the pattern");
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // A circular pattern exercises the per-kind gated fields.
    app.scene
        .get_mut(pat)
        .unwrap()
        .params_mut()
        .unwrap()
        .insert("kind".into(), simple3d_core::primitive::ParamValue::Choice(2));
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // With nothing selected the tool drops a bare pattern.
    app.clear_selection();
    app.run(Command::Pattern);
    assert!(app.scene.node(app.primary().unwrap()).is_pattern());
}

/// The tool's contents fit any room they are given, well below the requested width (issues 67,
/// 96), including the right-to-left button row, which overflows off the left edge.
#[test]
pub(crate) fn the_pattern_tool_fits_whatever_width_its_window_is_given() {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool did not open, so this measures nothing");
    // Past the opening question, so the stages are measured too.
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
        // Half a pixel of slack for rounding.
        assert!(used <= room + 0.5, "at {room} px of room the tool laid out {used} px of content");
        assert!(row_used <= row_room + 0.5, "at {row_room} px of room the button row took {row_used} px");
    }
}

/// The split tool's resizable window has the same two-column problem; the picture gives way (issue 82).
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
    // Drawing the tool cuts nothing; only Split does.
    assert!(app.primary().is_some_and(|id| !app.scene.node(id).is_split()));
}
