//! The brush's strokes on a mesh body (issue 127), driven through the app without a pointer.

use super::*;
use simple3d_core::keymap::{Chord, Command, Keymap, Preset};
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use simple3d_geom::brush::{Brush, Mode};
use simple3d_geom::Vec3;

/// An app holding `mesh` as a mesh body, selected, with the brush picked.
fn app_with(name: &str, mesh: simple3d_geom::Mesh) -> (App, NodeId) {
    let mut app = app_in(temp_config_dir(name));
    let root = app.scene.root();
    let id = app.scene.add_primitive("sphere", root, 0).expect("a sphere");
    assert!(app.scene.convert_to_mesh(id, MeshData::new(mesh)));
    app.select_only(id);
    app.reevaluate_for_test();
    app.history.clear();
    app.run(Command::ModeBrush);
    (app, id)
}

fn sphere(name: &str) -> (App, NodeId) {
    app_with(name, simple3d_geom::primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 64))
}

fn mesh(app: &App, id: NodeId) -> simple3d_geom::Mesh {
    app.scene.node(id).mesh().expect("a mesh").mesh.clone()
}

/// A stroke of `dabs` dabs straight down onto the top of the mesh, along x.
fn stroke(app: &mut App, mode: Mode, dabs: usize) {
    assert!(app.begin_brush_stroke(), "no stroke started");
    let top = app.evaluated.bounds.expect("evaluated").1.z;
    for k in 0..dabs {
        let x = k as f64 * 0.8 - 2.0;
        app.brush_dab(Vec3::new(x, 0.3, top + 50.0), Vec3::new(0.0, 0.0, -1.0), mode).expect("the dab landed");
    }
    app.end_brush_stroke();
    app.reevaluate_for_test();
}

#[test]
fn every_preset_brushes_on_alt_b() {
    for preset in Preset::ALL {
        assert_eq!(Keymap::from_preset(preset).binding(Command::ModeBrush), Some(&Chord::alt("B")), "{preset:?}");
    }
}

#[test]
fn every_mode_changes_the_mesh_and_keeps_it_closed() {
    for mode in Mode::ALL {
        let (mut app, id) = sphere(&format!("brush-mode-{mode:?}"));
        // Something to smooth and flatten first.
        stroke(&mut app, Mode::Raise, 4);
        let before = mesh(&app, id);
        stroke(&mut app, mode, 6);
        let after = mesh(&app, id);
        assert_ne!(before.positions, after.positions, "{mode:?} moved nothing");
        assert_eq!(before.indices, after.indices, "{mode:?} changed the triangles");
        assert!(after.manifold_issue().is_none(), "{mode:?}: {:?}", after.manifold_issue());
        assert_eq!(app.history.undo_len(), 2, "{mode:?}: not one undo step per stroke");
    }
}

#[test]
fn a_kept_base_stays_where_it_is() {
    let (mut app, id) = sphere("brush-base");
    app.settings.last_brush = Brush { keep_base: true, base: 50.0, radius: 15.0, strength: 1.0, ..Brush::default() };
    let before = mesh(&app, id);
    stroke(&mut app, Mode::Raise, 4);
    // The whole sphere is under 50 mm, so nothing may move.
    assert_eq!(mesh(&app, id).positions, before.positions);
}

#[test]
fn adding_detail_splits_a_coarse_box_under_the_stroke_and_keeps_it_closed() {
    let (mut app, id) = app_with("brush-detail", simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0));
    app.settings.last_brush = Brush { detail: true, edge: 1.0, radius: 3.0, strength: 1.0, ..Brush::default() };
    let before = mesh(&app, id);
    stroke(&mut app, Mode::Raise, 5);
    let after = mesh(&app, id);
    assert!(after.indices.len() > before.indices.len() * 10, "only {} triangles", after.indices.len());
    assert!(after.manifold_issue().is_none(), "{:?}", after.manifold_issue());
    let top = before.bounds().expect("a box").1.z;
    assert!(after.positions.iter().any(|p| p.z > top + 0.1), "the top did not rise");
    // Detail only where the brush went: the bottom keeps its two triangles' worth of corners.
    let bottom = before.bounds().expect("a box").0.z;
    assert_eq!(after.positions.iter().filter(|p| p.z == bottom).count(), 4);
    app.run(Command::Undo);
    assert_eq!(mesh(&app, id).indices.len(), before.indices.len());
}

#[test]
fn a_converted_box_takes_a_stroke_without_adding_detail() {
    // A 100 x 10 x 100 box converted to a mesh has a vertex at each corner only, 100 mm apart: a
    // 5 mm dab reached none of them, so the stroke moved nothing at all.
    let (mut app, id) = app_with("brush-coarse", simple3d_geom::primitives::box_mesh(100.0, 100.0, 10.0));
    assert!(!app.settings.last_brush.detail);
    let before = mesh(&app, id);
    stroke(&mut app, Mode::Raise, 5);
    let after = mesh(&app, id);
    let top = before.bounds().expect("a box").1.z;
    assert!(after.positions.iter().any(|p| p.z > top + 0.05), "the top did not rise");
    assert!(after.manifold_issue().is_none(), "{:?}", after.manifold_issue());
    // Only as fine as the brush needs: no edge under it shorter than about a quarter of the radius.
    assert!(after.indices.len() < 2_000, "{} triangles", after.indices.len());
}

#[test]
fn cancelling_after_detail_was_added_puts_the_coarse_mesh_back() {
    let (mut app, _) = app_with("brush-detail-cancel", simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0));
    app.settings.last_brush = Brush { detail: true, edge: 1.0, radius: 3.0, ..Brush::default() };
    assert!(app.begin_brush_stroke());
    app.brush_dab(Vec3::new(0.0, 0.0, 60.0), Vec3::new(0.0, 0.0, -1.0), Mode::Raise).expect("landed");
    assert!(app.sculpt.as_ref().expect("a session").renderable.mesh.indices.len() > 12);
    app.cancel_stroke();
    assert_eq!(app.sculpt.as_ref().expect("a session").renderable.mesh.indices.len(), 12);
    app.end_brush_stroke();
    assert_eq!(app.history.undo_len(), 0);
}

#[test]
fn the_brush_settings_are_remembered_and_leaving_drops_the_working_copy() {
    let (mut app, _) = sphere("brush-settings");
    let radius = app.settings.last_brush.radius;
    app.run(Command::BrushLarger);
    assert!((app.settings.last_brush.radius - radius * crate::brush_tool::STEP).abs() < 1e-9);
    let dir = app.config_dir().to_path_buf();
    let reopened = App::with_config_dir(&egui::Context::default(), None, dir);
    assert_eq!(reopened.settings.last_brush.radius, app.settings.last_brush.radius, "the radius was not saved");
    stroke(&mut app, Mode::Raise, 2);
    assert!(app.sculpt.is_some());
    app.run(Command::ModeMove);
    assert!(app.sculpt.is_none(), "the working copy outlived the brush");
}

/// One noise stroke of 40 overlapping dabs along x across the top of `mesh`, with or without adding
/// detail; how far the stroke moved the top face's inner vertices off its plane at most.
fn noise_height(name: &str, mesh: simple3d_geom::Mesh, detail: bool) -> f64 {
    let (mut app, _) = app_with(name, mesh);
    app.settings.last_brush =
        Brush { mode: Mode::Noise, detail, edge: 0.5, radius: 4.0, strength: 1.0, scale: 2.0, ..Brush::default() };
    let top = app.evaluated.bounds.expect("evaluated").1.z;
    assert!(app.begin_brush_stroke());
    for k in 0..40 {
        let x = -10.0 + k as f64 * 0.5;
        app.brush_dab(Vec3::new(x, 0.0, top + 50.0), Vec3::new(0.0, 0.0, -1.0), Mode::Noise).expect("landed");
    }
    let copy = &app.sculpt.as_ref().expect("a session").renderable.mesh;
    copy.positions
        .iter()
        .filter(|p| p.z > top - 2.0 && p.x.abs() < 18.0 && p.y.abs() < 18.0)
        .map(|p| (p.z - top).abs())
        .fold(0.0, f64::max)
}

#[test]
fn a_noise_stroke_adding_detail_stays_within_the_noise_height() {
    // The noise mode never goes past a tenth of the radius times the strength (0.4 mm here), measured
    // from where a vertex was when the stroke began. Adding detail remakes the copy (`reshape`) and
    // clears the stroke's record of those places, so every dab that split something made the
    // vertices already moved start again from where they now were: the same stroke reached 1.19 mm.
    let cap = 4.0 * 0.1 * 1.0 + 1e-9;
    let coarse = simple3d_geom::primitives::box_mesh(40.0, 40.0, 10.0);
    let (fine, _) =
        simple3d_geom::shaping::refine_until(&coarse, 0.5, 2_000_000, &simple3d_geom::never).expect("refined");
    let refined = noise_height("brush-noise-refined", fine, false);
    assert!(refined <= cap, "on a mesh already fine enough the stroke reached {refined} mm");
    let detailed = noise_height("brush-noise-detail", coarse, true);
    assert!(detailed <= cap, "adding detail, the same stroke reached {detailed} mm, past the {cap} mm the mode allows");
}
