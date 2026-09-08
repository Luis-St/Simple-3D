//! What a field shows over a selection, and what typing into it does.

use super::*;
use crate::app::App;
use crate::ui::{self};
use simple3d_core::primitive::ParamValue;
use simple3d_core::scene::NodeId;
use simple3d_core::unit::{format_length, Unit};

/// A view centre is shown to a hundredth of a millimetre, not to the four
/// decimals a measurement gets.
///
/// The camera's target is wherever a drag happened to stop, so it carries
/// all four of those places nearly all of the time -- and `-8.2888` in a
/// field sized for `40` is the number that would not fit. Two decimals at
/// every magnitude, so the readout keeps its shape as the view travels.
#[test]
pub(crate) fn a_view_centre_is_shown_to_a_hundredth_of_a_millimetre() {
    let shown = |mm: f64| format_length(shown_view_centre(mm), Unit::Millimetre);
    assert_eq!(shown(-8.288812), "-8.29");
    assert_eq!(shown(39.236851), "39.24");
    // A rounding, not a truncation.
    assert_eq!(shown_view_centre(0.005), 0.01);
    assert_eq!(shown_view_centre(-0.005), -0.01);
    // It never lengthens a number that was already short.
    assert_eq!(shown(40.0), "40");
    assert_eq!(shown(0.0), "0");
    // And the two decimals stay on however far out the camera is taken.
    assert_eq!(shown(-12345.678912), "-12345.68");
    assert_eq!(shown(-6248194.994), "-6248194.99");
    // Never more than two, which is the whole point: four made every value
    // a pan left behind too long for the field.
    for mm in [-8.288812, 39.236851, -6248194.994, 1234.5678, -0.0051] {
        let text = shown(mm);
        let decimals = text.split_once('.').map_or(0, |(_, rest)| rest.len());
        assert!(decimals <= 2, "{mm} shows as {text}, which has {decimals} decimals");
    }
}

#[test]
pub(crate) fn a_field_over_a_multi_selection_shows_the_shared_value_or_an_em_dash() {
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    let unit = app.unit();
    let shown = |app: &App, ids: &[NodeId]| {
        ui::shared_text(
            ids.iter().map(|t| ui::show_param(param_value(app, *t, "width", ParamValue::Length(0.0)), unit)),
        )
    };
    assert_eq!(shown(&app, &[a, b]), ui::MIXED, "two different widths must not claim to be one");
    assert_eq!(shown(&app, &[a]), "40");
    set_param(&mut app, b, "width", ParamValue::Length(40.0));
    assert_eq!(shown(&app, &[a, b]), "40", "two equal widths are one value, not a dash");
}

#[test]
pub(crate) fn an_absolute_value_applies_to_the_whole_selection_and_a_delta_applies_per_node() {
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    let unit = app.unit();
    let param = width_spec();
    let field = egui::Id::new("width-field");

    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "25".into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (25.0, 25.0), "an absolute value is one value for all");

    set_param(&mut app, a, "width", ParamValue::Length(40.0));
    set_param(&mut app, b, "width", ParamValue::Length(60.0));
    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "+2".into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (42.0, 62.0), "a delta is relative to each node's own value");

    // And the field's other tricks reach the model the same way.
    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "4cm".into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 40.0), "a value in another unit did not convert");
    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "12+8".into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (20.0, 20.0), "an expression was not evaluated");
}

#[test]
pub(crate) fn an_em_dash_left_alone_edits_nothing() {
    // Tabbing through a panel of mixed values must not flatten them.
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    let unit = app.unit();
    let param = width_spec();
    set_shared_param(&mut app, &[a, b], param, param.kind, unit, egui::Id::new("f"), ui::MIXED.into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0));
}

#[test]
pub(crate) fn a_value_that_cannot_be_read_leaves_every_node_alone_and_marks_the_field() {
    // Acceptance criterion 14, and the design's rule that the typed text
    // stays put: it is the thing the user has to correct.
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    let unit = app.unit();
    let param = width_spec();
    let field = egui::Id::new("width-field");
    let before = app.history.revision();

    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "wide-ish".into());
    assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "a rejected entry changed the model");
    assert_eq!(app.history.revision(), before, "a rejected entry took an undo step");
    assert!(app.fields.is_rejected(field), "the field was not marked");

    // Correcting it clears the mark.
    set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "30".into());
    assert!(!app.fields.is_rejected(field));
    assert_eq!((width_of(&app, a), width_of(&app, b)), (30.0, 30.0));
}
