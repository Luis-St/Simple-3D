//! Time per brush dab on a large converted sphere, end to end, before and after the working copy
//! (issue 127). Not a check: run by hand with
//! `cargo test --release -p simple3d-app sculpt_bench -- --ignored --nocapture`.
//! The GPU parts are timed only where `gpu.rs` finds a hardware GPU.

use super::gpu::{gpu_for, headless};
use super::*;
use crate::sculpt::Sculpt;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::Scene;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Sphere segments: 1000 gives about a million triangles.
const SEGMENTS: u32 = 1000;
const DABS: usize = 10;
/// The dab's radius and how far it raises, in millimetres, on a 100 mm sphere.
const RADIUS: f64 = 4.0;
const LIFT: f64 = 0.2;

#[test]
#[ignore]
fn sculpt_bench() {
    let headless = headless();
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_primitive("sphere", root, 0).expect("a sphere");
    let mesh = MeshData::new(primitives::ellipsoid_mesh(100.0, 100.0, 100.0, SEGMENTS));
    eprintln!("{} triangles, {} vertices", mesh.triangle_count(), mesh.mesh.positions.len());
    assert!(scene.convert_to_mesh(id, mesh));
    let mut evaluator = Evaluator::new();
    let evaluated = evaluator.evaluate(&scene, &Cancel::new());
    let base = scene.node(id).mesh().expect("a mesh").clone();
    let view = |req: &mut Request| {
        req.size = [1280, 800];
        req.view = View::new(
            Camera { yaw: -80.0, pitch: 15.0, distance: 260.0, ..Camera::default() },
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
        );
    };
    let centre = |dab: usize| Vec3::new(dab as f64 * 1.5 - 7.0, -100.0, dab as f64 - 5.0);
    let ray = |dab: usize| (centre(dab), Vec3::new(0.0, 1.0, 0.0));

    // Before: every dab a new mesh in the document, evaluated, prepared, its tree built and uploaded.
    let mut gpu = headless.as_ref().map(gpu_for);
    let mut positions = base.mesh.positions.clone();
    let normals = simple3d_geom::topology::vertex_normals(&base.mesh);
    let mut parts = [Duration::ZERO; 5];
    for dab in 0..DABS {
        let start = Instant::now();
        let hit = crate::pick::ray_mesh(&evaluated.node_meshes[&id], ray(dab).0, ray(dab).1);
        let at = ray(dab).0 + ray(dab).1 * hit.expect("the ray hits");
        for (p, n) in positions.iter_mut().zip(&normals) {
            let d = (*p - at).length();
            if d < RADIUS {
                *p = *p + *n * (LIFT * (1.0 - d / RADIUS));
            }
        }
        let mesh = simple3d_geom::Mesh { positions: positions.clone(), ..base.mesh.clone() };
        scene.set_mesh(id, Arc::new(MeshData::new(mesh)));
        let t0 = Instant::now();
        let evaluated = evaluator.evaluate(&scene, &Cancel::new());
        let t1 = Instant::now();
        let whole = Renderable::prepare_scene(&evaluated.mesh, &evaluated.ranges, &evaluated.smooth);
        let own = Renderable::prepare_node(&evaluated.node_meshes[&id], true, &evaluated.smooth);
        let t2 = Instant::now();
        crate::pick::ray_mesh(&evaluated.node_meshes[&id], ray(dab).0, ray(dab).1);
        let t3 = Instant::now();
        if let Some((gpu, _)) = gpu.as_mut() {
            let mut req = request(
                vec![
                    Item { renderable: &whole, style: Style::Solid },
                    Item { renderable: &own, style: Style::Selected },
                ],
                DisplayMode::ShadedWithEdges,
            );
            view(&mut req);
            gpu.render(&req).expect("a frame");
            unsafe { eframe::glow::HasContext::finish(&*headless.as_ref().expect("a gpu").gl) };
        }
        let t4 = Instant::now();
        for (part, took) in parts.iter_mut().zip([t0 - start, t1 - t0, t2 - t1, t3 - t2, t4 - t3]) {
            *part += took;
        }
    }
    let per = |d: Duration| d.as_secs_f64() * 1000.0 / DABS as f64;
    let total: Duration = parts.iter().sum();
    eprintln!(
        "before: {:.1} ms per dab (mesh copy {:.1}, evaluation {:.1}, renderables {:.1}, tree and cast {:.1}, gpu {:.1})",
        per(total),
        per(parts[0]),
        per(parts[1]),
        per(parts[2]),
        per(parts[3]),
        per(parts[4])
    );

    // After: the working copy, its caches updated around the dab, and only the changes uploaded.
    let start = Instant::now();
    let mut sculpt = Sculpt::new(id, base.clone(), Xform::IDENTITY, None);
    eprintln!("session built once in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    sculpt.begin();
    let whole = Renderable::prepare_scene(&evaluated.mesh, &evaluated.ranges, &evaluated.smooth);
    let part = whole.parts.get(&id).cloned().expect("the sphere is its own stretch");
    let frame = |gpu: &mut crate::gpu::Gpu, sculpt: &Sculpt| {
        let mut req = request(
            vec![
                Item { renderable: &whole, style: Style::Solid },
                Item { renderable: &sculpt.renderable, style: Style::Solid },
                Item { renderable: &sculpt.renderable, style: Style::Selected },
            ],
            DisplayMode::ShadedWithEdges,
        );
        view(&mut req);
        req.live.hidden.push((whole.id, part.clone()));
        req.live.edits.push((sculpt.renderable.id, &sculpt.edits));
        gpu.render(&req).expect("a frame");
        unsafe { eframe::glow::HasContext::finish(&*headless.as_ref().expect("a gpu").gl) };
    };
    if let Some((gpu, _)) = gpu.as_mut() {
        let first = Instant::now();
        frame(gpu, &sculpt);
        sculpt.edits.sent();
        eprintln!("first stroke frame (whole upload) {:.0} ms", first.elapsed().as_secs_f64() * 1000.0);
    }
    let mut parts = [Duration::ZERO; 3];
    for dab in 0..DABS {
        let start = Instant::now();
        let (t, triangle) = sculpt.ray(ray(dab).0, ray(dab).1).expect("the ray hits");
        let at = ray(dab).0 + ray(dab).1 * t;
        let moves: Vec<(u32, Vec3)> = sculpt
            .within(triangle, at, RADIUS)
            .into_iter()
            .map(|(v, d)| {
                (
                    v,
                    sculpt.renderable.mesh.positions[v as usize]
                        + sculpt.normals[v as usize] * (LIFT * (1.0 - d / RADIUS)),
                )
            })
            .collect();
        let t0 = Instant::now();
        sculpt.move_vertices(&moves);
        let t1 = Instant::now();
        if let Some((gpu, _)) = gpu.as_mut() {
            frame(gpu, &sculpt);
        }
        sculpt.edits.sent();
        let t2 = Instant::now();
        for (part, took) in parts.iter_mut().zip([t0 - start, t1 - t0, t2 - t1]) {
            *part += took;
        }
    }
    let total: Duration = parts.iter().sum();
    eprintln!(
        "after: {:.2} ms per dab (cast and region {:.2}, move and update {:.2}, gpu {:.2})",
        per(total),
        per(parts[0]),
        per(parts[1]),
        per(parts[2])
    );
    if let Some((gpu, _)) = gpu.as_mut() {
        let start = Instant::now();
        for _ in 0..DABS {
            frame(gpu, &sculpt);
        }
        eprintln!("the same frame with nothing changed: {:.2} ms", per(start.elapsed()));
    }
    let start = Instant::now();
    let result = sculpt.result();
    eprintln!(
        "writing the stroke back {:.0} ms ({} triangles)",
        start.elapsed().as_secs_f64() * 1000.0,
        result.triangle_count()
    );
}
