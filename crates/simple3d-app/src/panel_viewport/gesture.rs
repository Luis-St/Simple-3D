//! What a pointer gesture over the viewport means.

use crate::gizmo::Mods;
use crate::view::View;
use simple3d_core::keymap::{MouseButton, NavMap};
use simple3d_core::scene::Camera;

/// What a viewport drag means under the current navigation bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Orbit,
    Pan,
}

/// Which navigation gesture a drag is under `nav`; `held` is buttons in `MouseButton` order.
/// Stateless, taking `nav` each call, so rebinding applies at once (acceptance criterion 29).
pub fn nav_gesture(nav: &NavMap, held: [bool; 3], ctrl: bool, shift: bool, alt: bool) -> Option<Gesture> {
    for (button, down) in [MouseButton::Left, MouseButton::Middle, MouseButton::Right].into_iter().zip(held) {
        if !down {
            continue;
        }
        // Pan first: it usually adds a modifier to orbit's binding, and exact matching separates them.
        if nav.pan.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Pan);
        }
        if nav.orbit.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Orbit);
        }
    }
    None
}

/// Apply a navigation gesture to the camera; `view` is only needed for a pan's mm per pixel.
pub fn apply_gesture(camera: &mut Camera, gesture: Gesture, delta: egui::Vec2, view: &View) {
    match gesture {
        Gesture::Orbit => {
            // The drag carries the model with the pointer both ways, like pan and the view cube.
            camera.yaw -= delta.x as f64 * 0.4;
            camera.pitch = (camera.pitch + delta.y as f64 * 0.4).clamp(-90.0, 90.0);
        }
        Gesture::Pan => {
            // A zero-height viewport gives 1e9 mm per pixel and would throw the target away; guarded like zoom.
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
