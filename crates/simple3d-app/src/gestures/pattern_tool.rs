//! The custom pattern tool, driven by pointer.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::primitive::ParamsExt;

#[test]
pub(crate) fn a_custom_pattern_offers_the_tool_in_place_of_its_stages() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application. A custom kind's rule is thirty-odd
    // numbered fields -- "1 Copies", "3 Radius per copy" -- and the Pattern
    // section listed every one of them under the kind row, where they say
    // nothing about the rule they make and are only a wall to scroll past on the
    // way to the button that opens the tool. The tool is where they are edited,
    // and it draws each stage beside what it lays down.
    let mut harness = harness("pattern-custom-rows");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A fixed kind still shows its numbers: this is about the custom one only.
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Copies")).is_some(),
        "a linear pattern lost the numbers that place it"
    );
    // And it is offered no way into the tool. The button used to sit under every
    // kind reading "Custom kind...", which made becoming custom something that
    // happened on the way to opening a tool rather than a kind the user picked:
    // the Kind row is where that is said.
    assert!(harness.query_by_label("Edit kind").is_none(), "a linear pattern offers the custom kind's tool");

    let custom = simple3d_core::pattern::CUSTOM;
    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(custom));
    harness.step();

    // The stages are gone, all four of them.
    for stage in 0..simple3d_core::pattern::MAX_STAGES {
        for key in simple3d_core::pattern::stage_keys(stage) {
            let spec = simple3d_core::pattern::PARAMS.iter().find(|p| p.key == key).expect("a stage key with no spec");
            assert!(
                harness.ctx.read_response(crate::panel_properties::grip_id(spec.label)).is_none(),
                "the Pattern section still draws {}",
                spec.label
            );
        }
    }
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Stages")).is_none(),
        "the Pattern section still draws the stage count"
    );
    // What is left is the way in to the tool, the kind itself, and the line
    // saying what the pattern makes.
    assert!(harness.query_by_label("Edit kind").is_some(), "the way into the tool went with the stages");
    assert!(harness.query_by_label("Custom").is_some(), "the kind row went with the stages");
    assert!(
        harness.query_by_label_contains("Put shapes under this pattern").is_some(),
        "the line saying what the pattern makes went with the stages"
    );
}

#[test]
pub(crate) fn a_custom_pattern_picks_a_saved_kind_from_the_panel() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application: a kind saved from the tool is
    // meant to be used again, and reaching one should not mean opening the tool
    // -- picking it is a single click on a name. The shelf the tool keeps is
    // offered on the pattern itself, beside the button that edits it.
    let mut harness = harness("pattern-saved-kind-panel");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A rule on the shelf: two stages, twelve copies -- the same shape as the
    // one the tool's own test saves, and nothing a fixed kind lays out.
    let mut rule = simple3d_core::pattern::default_params();
    rule.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
    rule.insert("stage1_count".to_string(), simple3d_core::primitive::ParamValue::Count(3));
    rule.insert("stage2_count".to_string(), simple3d_core::primitive::ParamValue::Count(4));
    rule.insert("stage2_turn".to_string(), simple3d_core::primitive::ParamValue::Angle(90.0));
    let config = harness.state().config_dir().to_path_buf();
    simple3d_core::pattern_library::save(&config, "Turned row", &rule).expect("the shelf could not be written");
    harness.state_mut().refresh_pattern_kinds();

    // The shelf is offered on a custom pattern only: a linear one has a kind of
    // its own and its numbers to say it.
    harness.step();
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_none(),
        "a linear pattern offers the custom shelf"
    );

    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(simple3d_core::pattern::CUSTOM));
    harness.step();

    // Nothing is picked yet, so the box says so rather than naming a kind.
    let box_rect = rect_of(&harness, crate::panel_properties::saved_kind_id());
    press(&mut harness, box_rect.center());
    release(&mut harness, box_rect.center());
    harness.step();
    harness.step();
    harness.get_by_label("Turned row").click();
    harness.step();
    harness.step();

    let applied = harness.state().scene.node(id).params().cloned().expect("a pattern has parameters");
    assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(
        simple3d_core::pattern::instance_count(&applied).1,
        12,
        "the saved rule was not put on the pattern: {applied:?}"
    );
    // And the pattern is named after what was picked, which is what the box
    // reads back -- the same thing the tool's own shelf shows.
    assert_eq!(harness.state().scene.node(id).name, "Turned row", "the pattern was not named after the kind");
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_some(),
        "the box went once a kind was on the pattern"
    );
}

/// The Frame button puts the preview back the way it opened -- the angle and
/// the zoom with the rest, not just where the camera is pointed.
///
/// Asked for after the button was found to re-centre an orbited picture and
/// leave it orbited, which is half a reset and reads as a button that half
/// works. The automatic reframing, when a rule starts laying its copies out
/// somewhere else, still leaves the angle alone: that one happens under the
/// user's hands, and turning the picture mid-orbit is not what they asked for.
///
/// Driven through the button itself rather than through
/// `App::reset_pattern_preview`, because what is being checked is the wiring:
/// the dialog is `Embedded` against a headless context, so the tool is drawn
/// where the harness can click it.
#[test]
pub(crate) fn the_pattern_tools_frame_button_puts_the_angle_and_the_zoom_back() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-preview-frame");
    harness.state_mut().open_pattern_tool();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the tool did not open");

    // The picture opens at the viewport's angle. Turn it away and pull it out,
    // the way looking round a pattern does.
    let opened = harness.state().pattern_preview_camera;
    assert_eq!(opened.yaw, harness.state().scene.camera.yaw, "the preview did not open at the viewport's angle");
    {
        let camera = &mut harness.state_mut().pattern_preview_camera;
        camera.yaw += 73.0;
        camera.pitch += 21.0;
        camera.distance *= 4.0;
    }
    harness.step();

    harness.get_by_label("Frame").click();
    harness.step();
    harness.step();

    let now = harness.state().pattern_preview_camera;
    let viewport = harness.state().scene.camera;
    assert_eq!(now.yaw, viewport.yaw, "Frame left the picture turned away from the viewport's angle");
    assert_eq!(now.pitch, viewport.pitch, "Frame left the picture pitched away from the viewport's angle");
    assert!(
        (now.distance - opened.distance).abs() < 1e-6,
        "Frame left the picture at {} rather than the {} it opened at",
        now.distance,
        opened.distance
    );
    assert_eq!(harness.state().scene.camera, viewport, "framing the preview moved the viewport behind it");
}

#[test]
pub(crate) fn a_patterns_kind_is_clicked_from_a_row_and_its_fields_are_all_one_width() {
    use egui_kittest::kittest::Queryable;

    // Two layout changes to the pattern editor, both checked against the real
    // panel because both are about where things are drawn.
    //
    // The kind and axis options flow across their row instead of taking a line
    // each -- so the first thing to prove is that they are still *buttons* after
    // being lifted out of the vertical layout they used to sit in.
    //
    // And the unit is in the row's name, in brackets, rather than after the
    // field. That is what makes every field one width: a count carries no unit
    // and a step does, and the two used to end at different places down the same
    // column.
    let mut harness = harness_configured("pattern-panel", |app| {
        app.run(simple3d_core::keymap::Command::Pattern);
    });
    let pattern = harness.state().primary().expect("the tool leaves the pattern selected");
    let kind = |harness: &Harness<'_, App>| harness.state().scene.node(pattern).params().unwrap().int("kind");
    assert_eq!(kind(&harness), 0, "a fresh pattern is linear");

    // Every kind is on the row, and clicking one chooses it.
    for (index, name) in ["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral"].iter().enumerate() {
        assert!(harness.query_by_label(name).is_some(), "{name} is not on the kind row");
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        assert_eq!(kind(&harness), index as u32, "clicking {name} did not choose it");
    }

    // A circular pattern is the case the width rule is about: Copies has no
    // unit, Span is in degrees and Radius in millimetres, and all three fields
    // have to start and end together.
    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();
    //
    // Checked across the kinds, not just this one: the longest names a pattern
    // has are a spiral's, and "Radius per copy (mm)" is the one a bracketed unit
    // could have pushed out of the label column and into the field beside it.
    let mut fields: Vec<(&str, egui::Rect)> = Vec::new();
    for (kind, names) in [
        ("Circular", &["Copies", "Span", "Radius"][..]),
        ("Spiral", &["Copies", "Angle per copy", "Start radius", "Radius per copy", "Rise per copy"][..]),
    ] {
        harness.get_by_label(kind).click();
        harness.step();
        harness.step();
        for name in names {
            fields.push((name, rect_of(&harness, crate::panel_properties::grip_id(name))));
        }
    }
    for pair in fields.windows(2) {
        let ((a_name, a), (b_name, b)) = (pair[0], pair[1]);
        assert!(
            (a.width() - b.width()).abs() < 0.5 && (a.right() - b.right()).abs() < 0.5,
            "{a_name} and {b_name} are different sizes: {a:?} and {b:?}"
        );
    }

    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();

    // And the axis options are buttons on their own row, the same as the kinds.
    for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        let axis = harness.state().scene.node(pattern).params().unwrap().int("circ_axis");
        assert_eq!(axis, index as u32, "clicking axis {name} did not choose it");
    }
}

/// Deleting a pattern from the tree took the window with it.
///
/// The row list is taken before any of it is drawn, and the context menu's
/// Delete removes nodes while the loop over that list is still running. A
/// pattern is not a *group*, so deleting one asks nothing and goes at once --
/// and `Scene::remove` takes the whole subtree, which is exactly the rows that
/// come next. The next row drawn then asked the scene for a node that was no
/// longer in it, and `Scene::node` panics on that.
///
/// Driven through the real tree because that is the only place the fault
/// existed: every piece of it -- the list, the menu, the delete -- is correct on
/// its own, and it is the order they run in that was wrong.
#[test]
pub(crate) fn deleting_a_pattern_from_the_tree_does_not_take_the_window_with_it() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-delete-pattern");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    // A pattern *of* the plate, so the tree is Scene > Pattern > Plate and the
    // rows below the one being deleted are the ones that go with it.
    harness.state_mut().run(simple3d_core::keymap::Command::Pattern);
    harness.step();
    let pattern = harness.state().primary().expect("the plate is now inside a pattern");
    assert!(harness.state().scene.node(pattern).is_pattern());
    assert_eq!(harness.state().scene.node(pattern).children, vec![plate], "the plate is not in the pattern");

    let row = rect_of(&harness, crate::panel_outliner::row_id(pattern));
    let at = row.center();
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    harness.step();

    // The menu's Delete carries its binding after a tab, so it is found by what
    // it starts with rather than by the whole label.
    let delete = harness
        .query_all_by_label_contains("Delete")
        .next()
        .expect("the tree's context menu has no Delete on a pattern");
    delete.click();
    // The frame the deletion happens on is the frame that used to panic: the
    // rows after the pattern are its children, and they are gone by then.
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(pattern), "the pattern is still there");
    assert!(!harness.state().scene.contains(plate), "the pattern went and left its child behind");
    assert!(harness.state().selection.is_empty(), "the deleted node is still selected");
}
