//! What the caller is told while an export runs, and afterwards.

use super::*;
use simple3d_geom::Mesh;

#[test]
pub(crate) fn an_empty_scene_is_reported_as_such() {
    let path = temp_dir().join("empty.stl");
    let mut cb = no_progress();
    let err = write(&path, &Mesh::new(), &Options::default(), &mut cb).unwrap_err();
    assert_eq!(err, ExportError::Empty);
    assert!(err.to_string().contains("nothing to export"));
}

#[test]
pub(crate) fn progress_runs_from_zero_to_one() {
    let mut seen: Vec<f32> = Vec::new();
    let mut cb = |p: f32| {
        seen.push(p);
        true
    };
    write(&temp_dir().join("progress.obj"), &plate(), &Options { format: Format::Obj, ..Default::default() }, &mut cb)
        .unwrap();
    assert_eq!(seen.first(), Some(&0.0));
    assert_eq!(seen.last(), Some(&1.0));
    assert!(seen.windows(2).all(|w| w[1] >= w[0] - 1e-6), "progress went backwards: {seen:?}");
    std::fs::remove_file(temp_dir().join("progress.obj")).unwrap();
}

#[test]
pub(crate) fn exporting_the_same_mesh_twice_produces_the_same_bytes() {
    for format in Format::ALL {
        let path = temp_dir().join(format!("stable.{}", format.id()));
        let mut cb = no_progress();
        write(&path, &plate(), &Options { format, ..Default::default() }, &mut cb).unwrap();
        let first = std::fs::read(&path).unwrap();
        write(&path, &plate(), &Options { format, ..Default::default() }, &mut cb).unwrap();
        let second = std::fs::read(&path).unwrap();
        assert_eq!(first, second, "{format:?} is not reproducible");
        std::fs::remove_file(&path).unwrap();
    }
}
