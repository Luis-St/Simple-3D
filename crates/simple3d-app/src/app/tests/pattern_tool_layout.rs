//! Where the tool's own controls sit.

use super::*;

/// The cross that drops a stage ends where the stage's own fields end, and
/// is big enough to aim at.
///
/// Asked for after it sat eight pixels right of every field under it -- a
/// stage's name line ran to the edge of the column while a property row is
/// pinned `EDGE_PAD` inside it -- and at egui's small-button size, 18.5 by
/// 14, which is a small target for the one control in the tool that throws
/// work away.
///
/// Both edges are read back from the frame rather than worked out here:
/// what is being held is that they go on coming from the same place.
#[test]
pub(crate) fn the_cross_that_drops_a_stage_lines_up_with_the_fields_under_it() {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    // The cross is only on the last stage, and only when there is more than
    // one -- the others are what it repeats.
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

    let cross = ctx.read_response(crate::pattern_tool::drop_stage_id()).expect("the cross was not drawn").rect;
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
    // Twice the fourteen pixels egui's small button comes out at, and
    // square: it is a mark rather than a word.
    assert!(
        cross.width() >= 28.0 && cross.height() >= 28.0,
        "the cross is {:?}, which is not twice the small button it was",
        cross.size()
    );
}
