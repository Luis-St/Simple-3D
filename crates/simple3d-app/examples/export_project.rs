//! Writes 3MF files from a project along the app's own export path (evaluate, refuse on errors,
//! `simple3d_export::write`), since the portal file dialog cannot be driven on the nested X server.
//!
//! usage: export_project <project.simple3d> <out-dir> [group-name ...]

use simple3d_core::{eval, project, scene::Scene};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn main() {
    let mut args = std::env::args().skip(1);
    let project_path = PathBuf::from(args.next().expect("project path"));
    let out_dir = PathBuf::from(args.next().expect("output directory"));
    let wanted: Vec<String> = args.collect();

    let text = std::fs::read_to_string(&project_path).expect("read project");
    let scene: Scene = project::from_str(&text).expect("parse project");
    let mut evaluator = eval::Evaluator::new();
    let cancel = eval::Cancel::new();
    let evaluated = evaluator.evaluate(&scene, &cancel);
    assert!(evaluated.errors.is_empty(), "scene has evaluation errors: {:?}", evaluated.errors);

    std::fs::create_dir_all(&out_dir).expect("create output directory");

    write_mesh(&out_dir.join("showcase-all.3mf"), &evaluated.mesh);

    // One file per named top-level group, evaluated like an exported selection.
    for id in scene.node(scene.root()).children.clone() {
        let name = scene.node(id).name.clone();
        if !wanted.is_empty() && !wanted.contains(&name) {
            continue;
        }
        let mesh = eval::selection_mesh(&scene, &[id], &evaluated.node_frames);
        let file = format!("{}.3mf", name.to_lowercase().replace(' ', "-"));
        write_mesh(&out_dir.join(file), &Arc::new(mesh));
    }
}

fn write_mesh(path: &Path, mesh: &Arc<simple3d_geom::Mesh>) {
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        scale: 1.0,
        unit: simple3d_export::Unit3mf::Millimeter,
        allow_invalid: false,
        bodies: simple3d_export::BodyMode::One,
        compress: true,
    };
    let mut progress = |_: f32| true;
    match simple3d_export::write(path, mesh, &options, &mut progress) {
        Ok(()) => println!("wrote {} ({} triangles)", path.display(), mesh.triangle_count()),
        Err(e) => println!("FAILED {}: {e}", path.display()),
    }
}
