use super::*;

/// Regression: a star and two spaces differ in width in the menu's font, so "* Move" and
/// "  Rotate" started their words at different places, and entries with no slot further left still.
#[test]
fn a_marked_and_an_unmarked_entry_start_their_words_at_the_same_place() {
    let ctx = egui::Context::default();
    let mut starts = Vec::new();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for mark in [true, false] {
                let galley = marked(ui, mark, "Move").into_galley(ui, None, f32::INFINITY, egui::TextStyle::Button);
                let glyphs = &galley.rows[0].glyphs;
                starts.push(glyphs.iter().find(|glyph| glyph.chr == 'M').expect("the word was laid out").pos.x);
            }
        });
    });
    assert_eq!(starts[0], starts[1], "the marked word starts elsewhere than the unmarked one");
    assert_eq!(split_mark("* Move"), (Some(true), "Move"));
    assert_eq!(split_mark("  Align and distribute"), (Some(false), "Align and distribute"));
    assert_eq!(split_mark("Undo"), (None, "Undo"));
}
