use super::apply::*;
use super::controls::*;
use super::palette::*;
use super::text::*;
use egui::Vec2;

/// Draw one frame of `contents` with this palette and return every rectangle painted.
fn rects(contents: impl FnOnce(&mut egui::Ui)) -> Vec<egui::epaint::RectShape> {
    let ctx = egui::Context::default();
    apply(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(300.0, 200.0))),
        ..Default::default()
    };
    // `run` may call the closure repeatedly, but the contents are drawn once, on the first frame.
    let mut contents = Some(contents);
    let output = ctx.run(input, move |ctx| {
        if let Some(contents) = contents.take() {
            egui::CentralPanel::default().show(ctx, contents);
        }
    });
    let mut found = Vec::new();
    fn walk(shape: &egui::Shape, found: &mut Vec<egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(rect) => found.push(rect.clone()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, found)),
            _ => {}
        }
    }
    for clipped in &output.shapes {
        walk(&clipped.shape, &mut found);
    }
    found
}

/// An off chip is still a chip: egui's unselected `toggle_value` paints nothing, leaving bare words.
#[test]
fn a_toggle_that_is_off_still_draws_a_frame() {
    let ours = rects(|ui| {
        let mut off = false;
        toggle(ui, &mut off, "Lock");
    });
    let framed =
        ours.iter().any(|r| r.fill == token::SURFACE_2 && r.stroke.width > 0.0 && r.stroke.color == token::SURFACE_3);
    assert!(framed, "an off chip drew no filled, outlined frame: {ours:#?}");

    // egui's own toggle draws only the panel's rectangle.
    let egui_own = rects(|ui| {
        let mut off = false;
        ui.toggle_value(&mut off, "Lock");
    });
    assert!(
        !egui_own.iter().any(|r| r.fill == token::SURFACE_2),
        "egui learnt to frame an unselected toggle; this helper may no longer be needed"
    );
}

/// An on chip keeps egui's selected accent tint.
#[test]
fn a_chosen_option_keeps_the_accent_it_always_had() {
    let ours = rects(|ui| {
        choice(ui, true, "Along the grid");
    });
    let egui_own = rects(|ui| {
        let _ = ui.selectable_label(true, "Along the grid");
    });
    // The panel background is the one rectangle both draw.
    let chip_of = |shapes: &[egui::epaint::RectShape]| {
        shapes.iter().find(|r| r.fill != token::SURFACE_1).map(|r| (r.fill, r.stroke)).expect("no chip was painted")
    };
    assert_eq!(
        chip_of(&ours),
        chip_of(&egui_own),
        "a chosen chip no longer looks the way egui draws a selected button"
    );
}

#[test]
fn a_panel_header_reads_as_spaced_capitals() {
    let text = header_text("Dimensions");
    assert_eq!(text.text().replace('\u{2009}', ""), "DIMENSIONS");
    assert!(text.text().contains('\u{2009}'), "header lost its tracking");
    assert!(!text.text().ends_with('\u{2009}'), "trailing space would offset a right-aligned header");
}

#[test]
fn every_axis_has_its_own_colour() {
    let colours = [axis_colour(0), axis_colour(1), axis_colour(2)];
    for (i, a) in colours.iter().enumerate() {
        for b in &colours[i + 1..] {
            assert_ne!(a, b, "two axes share a colour");
        }
        // The selection accent must never collide with an axis colour; hence amber, not blue.
        assert_ne!(*a, token::ACCENT, "an axis colour collides with the selection accent");
    }
}
