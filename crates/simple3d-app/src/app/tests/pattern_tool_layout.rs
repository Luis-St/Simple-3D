//! Where the tool's own controls sit.

use super::*;

/// The stage-drop cross ends where the stage's fields do and is big enough to aim at. Regression: it
/// sat eight pixels right of the fields at egui's small-button size. Both edges are read from the frame.
#[test]
pub(crate) fn the_cross_that_drops_a_stage_lines_up_with_the_fields_under_it() {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    app.start_rule_from(pattern, 0);
    // With several stages each has a cross; the second is measured against its own field.
    if let Some(params) = app.scene.get_mut(pattern).and_then(|n| n.params_mut()) {
        params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
    }
    app.reevaluate_for_test();

    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(&mut app, ui));
    });

    let cross = ctx.read_response(crate::pattern_tool::drop_stage_id(1)).expect("the cross was not drawn").rect;
    let field = ctx
        .read_response(crate::panel_properties::grip_id("tool:2 Copies"))
        .expect("the stage's own field was not drawn")
        .rect;
    assert!(
        (cross.right() - field.right()).abs() < 0.5,
        "the cross ends at {} and the field under it at {}",
        cross.right(),
        field.right()
    );
    // Twice egui's small button height, and square.
    assert!(
        cross.width() >= 28.0 && cross.height() >= 28.0,
        "the cross is {:?}, which is not twice the small button it was",
        cross.size()
    );
}
