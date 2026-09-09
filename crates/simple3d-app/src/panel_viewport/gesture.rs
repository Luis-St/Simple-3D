//! What a pointer gesture over the viewport means.

use crate::gizmo::Mods;
use crate::view::View;
use simple3d_core::keymap::{MouseButton, NavMap};
use simple3d_core::scene::Camera;

/// What a drag on the viewport means under the current navigation bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Orbit,
    Pan,
}

/// Which navigation gesture a drag is, under `nav`. `held` says which buttons the
/// drag is using, in `MouseButton` order.
///
/// Split out of `navigate` so acceptance criterion 29 -- remapping the orbit
/// button and having navigation follow immediately -- can be asserted. Note that
/// it takes `nav` as an argument and holds no state of its own: that is *why*
/// there is nothing to restart, and a cached copy here is what would break it.
pub fn nav_gesture(nav: &NavMap, held: [bool; 3], ctrl: bool, shift: bool, alt: bool) -> Option<Gesture> {
    for (button, down) in [MouseButton::Left, MouseButton::Middle, MouseButton::Right].into_iter().zip(held) {
        if !down {
            continue;
        }
        // Pan is tested first: it usually carries an extra modifier on top of
        // orbit's binding, and the exact-match rule keeps them apart.
        if nav.pan.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Pan);
        }
        if nav.orbit.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Orbit);
        }
    }
    None
}

/// Apply a navigation gesture to the camera. `view` is only consulted for a pan,
/// which has to know how many millimetres a pixel covers at the target.
pub fn apply_gesture(camera: &mut Camera, gesture: Gesture, delta: egui::Vec2, view: &View) {
    match gesture {
        Gesture::Orbit => {
            // Both ways about, the drag carries the *model* with the pointer:
            // dragging left turns the model left and walks the eye round to its
            // right, and pulling down tips the top of it towards the viewer.
            // The same thing a pan does, and the same thing the cube in the
            // corner does when it is grabbed directly -- one rule for every
            // drag over the picture, whichever of them the hand is on.
            camera.yaw -= delta.x as f64 * 0.4;
            camera.pitch = (camera.pitch + delta.y as f64 * 0.4).clamp(-89.9, 89.9);
        }
        Gesture::Pan => {
            // A viewport with no height has no millimetres per pixel to
            // measure the drag in: `mm_per_pixel_at` divides by a half-height
            // floored at 1e-9 and hands back 1e9, so a three-pixel pan throws
            // the target some 3.5e9 mm away and the model is gone with no way
            // back but Frame all. The zoom guards the same arithmetic the same
            // way; this is the other half of it.
            if !(view.size.x > 0.0 && view.size.y > 0.0) {
                return;
            }
            let (right, up) = view.basis();
            let scale = view.mm_per_pixel_at(camera.target);
            camera.target = camera.target - right * (delta.x as f64 * scale) + up * (delta.y as f64 * scale);
        }
    }
}

pub(crate) fn mods_from(ui: &egui::Ui) -> Mods {
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    Mods { free: alt, coarse: shift, symmetric: ctrl }
}
