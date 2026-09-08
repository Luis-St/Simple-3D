//! The tool's preview, and the camera that looks through it.

use super::*;
use simple3d_geom::Vec3;

/// Reported: "why does the preview change when i change the values? this
/// should not happen, the preview should be under user control."
///
/// It framed itself again whenever the rule started laying its copies out
/// somewhere else, so every number typed moved the camera: a picture turned
/// and zoomed to look at one end of a run jumped back to the whole of it on
/// the next keystroke.
///
/// **This reverses an earlier fix**, whose test this replaces. That one was
/// for "the preview does not show what the stages do" -- the camera was
/// pointed once, when the tool opened, so adding a stage laid copies out
/// past the edges of the picture. Reframing on every change answered it and
/// took the camera off the user to do it. The picture going off the edges is
/// now the expected thing, and the Frame button is the answer to it: it is
/// one click, it says what it does, and it leaves the choice where it
/// belongs. Do not put the automatic reframing back.
#[test]
pub(crate) fn editing_the_rule_leaves_the_previews_camera_alone() {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    app.reevaluate_for_test();

    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let draw = |app: &mut App| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 700.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(app, ui));
        });
    };
    // One frame to let it point itself at the pattern, which is the one
    // time it may.
    draw(&mut app);
    // Then the user looks where they want to look.
    app.pattern_preview_camera.yaw += 40.0;
    app.pattern_preview_camera.pitch -= 15.0;
    app.pattern_preview_camera.distance *= 0.35;
    draw(&mut app);
    let looked = app.pattern_preview_camera;

    // Now the rule lays out a great deal more, the way typing does.
    if let Some(params) = app.scene.get_mut(pattern).and_then(|n| n.params_mut()) {
        params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(3));
    }
    app.reevaluate_for_test();
    draw(&mut app);
    assert_eq!(app.pattern_preview_camera, looked, "editing the rule moved the preview's camera");

    // And Frame is still how the whole of it is asked for back.
    app.reset_pattern_preview();
    draw(&mut app);
    assert_ne!(app.pattern_preview_camera, looked, "Frame left the picture where the user had put it");
    let (lo, hi) = app.evaluated.node_world_bounds[&pattern];
    let rect = ctx.read_response(crate::pattern_tool::preview_id()).expect("the preview was not drawn").rect;
    let view = crate::view::View::new(app.pattern_preview_camera, rect);
    for corner in [
        Vec3::new(lo.x, lo.y, lo.z),
        Vec3::new(hi.x, lo.y, lo.z),
        Vec3::new(lo.x, hi.y, lo.z),
        Vec3::new(hi.x, hi.y, lo.z),
        Vec3::new(lo.x, lo.y, hi.z),
        Vec3::new(hi.x, lo.y, hi.z),
        Vec3::new(lo.x, hi.y, hi.z),
        Vec3::new(hi.x, hi.y, hi.z),
    ] {
        let (at, _) = view.project(corner).expect("an orthographic projection always lands");
        assert!(rect.contains(at), "after Frame the corner {corner:?} is drawn at {at:?}, outside {rect:?}");
    }
}

/// Reported: "the preview does not render anything".
///
/// It rendered nothing because it was never drawn. With `embed_dialogs` on
/// -- the setting that keeps the application off a second window system
/// surface, and the way out of the freeze in issue 64 -- a dialog is an
/// `egui::Window`, and the window was given no size, so it sized itself to
/// what was in it. The tool's body lays itself out against the room it is
/// *given*, and what it was given was the auto-sized default: 362 px, under
/// what the two columns need, at which the tool gives the picture up and
/// draws the numbers alone.
///
/// The second half of the same fault is the height. The buttons were
/// stacked under a body that fills whatever room it has, so once the window
/// had a width worth having, it grew by the height of the button row every
/// frame -- 907 px on an 880 px screen, with the buttons off the bottom of
/// it.
///
/// Asked of the frame rather than of the layout arithmetic: the headless
/// context hands every dialog back as `Embedded`, so running `App::ui`
/// against it draws the dialog the way that setting does, and the context
/// is then asked where the preview and the window itself ended up.
#[test]
pub(crate) fn the_pattern_tools_preview_is_drawn_when_the_dialog_is_embedded() {
    let mut app = headless_app();
    app.open_pattern_tool();
    assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
    app.reevaluate_for_test();

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0));
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    // Several frames: a window that grows to fit its contents does it one
    // frame at a time, and one frame of it looks like a rounding error.
    for _ in 0..4 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    let preview = ctx
        .read_response(crate::pattern_tool::preview_id())
        .expect("the preview was not drawn in the embedded dialog")
        .rect;
    assert!(
        preview.width() >= 220.0 && preview.height() >= 220.0,
        "the preview was drawn at {preview:?}, which is a stamp rather than a viewport"
    );
    let window = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("Custom pattern kind")))
        .expect("the embedded dialog was not drawn");
    assert!(
        screen.contains_rect(window),
        "the embedded dialog is {window:?}, which runs off the {screen:?} it is drawn on"
    );
}

#[test]
pub(crate) fn the_pattern_tools_preview_turns_without_turning_the_viewport() {
    // The preview is a viewport now rather than a scatter of dots, and the
    // point of giving it a camera of its own is that turning the pattern
    // round to look at what a rule made must not move the view behind the
    // window -- which is where the shape is actually being modelled.
    let mut app = headless_app();
    app.open_pattern_tool();
    assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
    app.reevaluate_for_test();
    app.frame_pattern_preview(1.0);

    // It opens looking from where the viewport looks, so the picture in the
    // window and the one behind it agree about which way round the shape is.
    assert_eq!(app.pattern_preview_camera.yaw, app.scene.camera.yaw, "the preview opened at another angle");
    assert_eq!(app.pattern_preview_camera.pitch, app.scene.camera.pitch);

    let viewport_camera = app.scene.camera;
    let view = crate::view::View::new(app.pattern_preview_camera, app.viewport_rect);
    crate::panel_viewport::apply_gesture(
        &mut app.pattern_preview_camera,
        crate::panel_viewport::Gesture::Orbit,
        egui::vec2(60.0, 0.0),
        &view,
    );
    assert_ne!(app.pattern_preview_camera.yaw, viewport_camera.yaw, "the preview did not turn");
    assert_eq!(app.scene.camera, viewport_camera, "turning the preview turned the viewport with it");
}
