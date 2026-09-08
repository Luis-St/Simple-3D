use super::app_icon::*;
use super::glyph::*;
use crate::theme::token;

#[test]
fn the_window_icon_is_the_box_in_the_accent_colour_on_a_rounded_tile() {
    // Issue 18: the window wore eframe's egui logo, because nothing set an
    // icon. What matters is that this produces one at all, and that it is
    // the drawing the packaged icon files carry rather than a blank square.
    let icon = app_icon(64);
    assert_eq!((icon.width, icon.height), (64, 64));
    assert_eq!(icon.rgba.len(), 64 * 64 * 4);

    let pixel = |x: usize, y: usize| {
        let o = (y * 64 + x) * 4;
        (icon.rgba[o], icon.rgba[o + 1], icon.rgba[o + 2], icon.rgba[o + 3])
    };
    // The corners are rounded, so they are transparent; the middle is not.
    assert_eq!(pixel(0, 0).3, 0, "the tile's corner is not rounded");
    assert_eq!(pixel(32, 32).3, 255, "the middle of the tile is not opaque");

    let accent = token::ACCENT;
    let ink = icon
        .rgba
        .chunks_exact(4)
        .filter(|p| p[3] > 0 && p[0].abs_diff(accent.r()) < 24 && p[2].abs_diff(accent.b()) < 24)
        .count();
    assert!(ink > 200, "the box is not drawn in the accent colour: {ink} pixels of it");
    let ground = icon.rgba.chunks_exact(4).filter(|p| p[3] == 255 && p[0] == token::SURFACE_0.r()).count();
    assert!(ground > ink, "the tile is more line than ground: {ink} against {ground}");
}

#[test]
fn the_icon_is_the_same_drawing_at_every_size() {
    // The same shape, scaled: the proportion of it that is ink cannot move
    // much between one size and the next, or the strokes are not scaling
    // with the tile.
    let inked = |size: usize| {
        let icon = app_icon(size);
        let accent = token::ACCENT;
        let ink = icon
            .rgba
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[0].abs_diff(accent.r()) < 24 && p[2].abs_diff(accent.b()) < 24)
            .count() as f64;
        ink / (size * size) as f64
    };
    let (small, large) = (inked(32), inked(256));
    assert!((small - large).abs() < 0.05, "the drawing does not scale: {small} against {large}");
}
use simple3d_core::primitive;

#[test]
fn every_primitive_in_the_registry_has_a_silhouette() {
    // A tile with no glyph would be an unclickable blank in the palette, so
    // check the mapping covers the registry rather than falling back.
    for spec in primitive::REGISTRY {
        let glyph = Glyph::for_primitive(spec.type_id);
        if spec.type_id != "box" {
            assert_ne!(glyph, Glyph::Box, "{} fell back to the box silhouette", spec.type_id);
        }
    }
}

#[test]
fn an_unknown_type_still_gets_something_to_draw() {
    assert_eq!(Glyph::for_primitive("not-a-primitive"), Glyph::Box);
}
