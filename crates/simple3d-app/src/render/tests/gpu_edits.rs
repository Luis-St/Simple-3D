//! A renderable edited in place (the brush's working copy, issue 127) sent to the card piece by
//! piece draws what uploading it whole draws. On the real GPU only (see `gpu.rs`).

use super::gpu::{gpu_for, headless, read_back};
use super::*;
use crate::sculpt::Sculpt;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use std::sync::Arc;

/// A converted sphere's working copy with a stroke started.
fn sculpt() -> Sculpt {
    let mesh = Arc::new(MeshData::new(primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 64)));
    let mut sculpt = Sculpt::new(1, mesh, Xform::IDENTITY, None);
    sculpt.begin();
    sculpt
}

/// Bumps raised along the normals, far enough apart to make several stretches on the card.
fn dabs(sculpt: &mut Sculpt) {
    // On the sphere (30 mm across), on the side the camera sees.
    for towards in [Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.7, -0.6, 0.4), Vec3::new(-0.5, -0.8, -0.2)] {
        let centre = towards.normalized() * 15.0;
        let moves: Vec<(u32, Vec3)> = (0..sculpt.normals.len() as u32)
            .filter_map(|v| {
                let at = sculpt.renderable.mesh.positions[v as usize];
                let d = (at - centre).length();
                (d < 5.0).then(|| (v, at + sculpt.normals[v as usize] * (3.0 * (1.0 - d / 5.0))))
            })
            .collect();
        sculpt.move_vertices(&moves);
    }
}

fn frame<'a>(sculpt: &'a Sculpt, edits: bool) -> Request<'a> {
    let items = vec![
        Item { renderable: &sculpt.renderable, style: Style::Solid },
        Item { renderable: &sculpt.renderable, style: Style::Selected },
    ];
    let mut req = request(items, DisplayMode::ShadedWithEdges);
    req.size = [480, 360];
    req.view = View::new(
        Camera { yaw: -80.0, pitch: 15.0, distance: 70.0, ..Camera::default() },
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480.0, 360.0)),
    );
    req.grid.axes = [false; 3];
    if edits {
        req.live.edits.push((sculpt.renderable.id, &sculpt.edits));
    }
    req
}

fn apart(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) > 2)).count()
}

#[test]
fn a_partial_upload_draws_what_a_full_upload_draws() {
    let Some(headless) = headless() else { return };
    let mut sculpt = sculpt();
    let (mut kept, texture) = gpu_for(&headless);
    kept.render(&frame(&sculpt, true)).expect("the first frame");
    let untouched = read_back(&headless, texture, [480, 360]);
    sculpt.edits.sent();
    dabs(&mut sculpt);
    kept.render(&frame(&sculpt, true)).expect("the edited frame");
    let partial = read_back(&headless, texture, [480, 360]);
    let (mut fresh, fresh_texture) = gpu_for(&headless);
    fresh.render(&frame(&sculpt, false)).expect("the whole frame");
    let whole = read_back(&headless, fresh_texture, [480, 360]);
    assert!(apart(&untouched, &whole) > 2_000, "the dabs did not show, so the comparison says nothing");
    assert_eq!(apart(&partial, &whole), 0, "the partial upload drew something else");
}

/// The check above can fail: without the list of changes the card keeps the stale copy.
#[test]
fn without_the_changes_the_card_keeps_drawing_the_old_copy() {
    let Some(headless) = headless() else { return };
    let mut sculpt = sculpt();
    let (mut kept, texture) = gpu_for(&headless);
    kept.render(&frame(&sculpt, true)).expect("the first frame");
    let untouched = read_back(&headless, texture, [480, 360]);
    dabs(&mut sculpt);
    kept.render(&frame(&sculpt, false)).expect("the stale frame");
    assert_eq!(apart(&read_back(&headless, texture, [480, 360]), &untouched), 0);
}
