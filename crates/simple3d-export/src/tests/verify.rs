//! The watertightness check, and what an export does when it fails.

use super::*;
use simple3d_geom::primitives;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_valid_mesh_passes_verification() {
    assert!(verify(&plate()).is_empty());
    assert!(signed_volume(&plate().weld()) > 0.0);
}

#[test]
pub(crate) fn inward_winding_is_caught_before_anything_is_written() {
    let mut flipped = plate();
    flipped.flip_winding();
    let problems = verify(&flipped);
    assert!(problems.iter().any(|p| p.contains("wound inward")), "{problems:?}");
}

#[test]
pub(crate) fn an_open_mesh_is_caught() {
    let mut open = plate();
    open.indices.pop();
    let problems = verify(&open);
    assert!(problems.iter().any(|p| p.contains("watertight")), "{problems:?}");
}

#[test]
pub(crate) fn export_refuses_an_invalid_mesh_and_writes_nothing() {
    let mut open = plate();
    open.indices.pop();
    let path = temp_dir().join("invalid.stl");
    let mut cb = no_progress();
    let err = write(&path, &open, &Options { format: Format::StlBinary, ..Default::default() }, &mut cb).unwrap_err();
    assert!(matches!(err, ExportError::Invalid(_)));
    assert!(err.to_string().contains("watertight"));
    assert!(!path.exists(), "a file was written for a mesh that failed verification");
}

#[test]
pub(crate) fn export_anyway_is_possible_once_the_user_has_been_told() {
    let mut open = plate();
    open.indices.pop();
    let path = temp_dir().join("anyway.stl");
    let mut cb = no_progress();
    let options = Options { format: Format::StlBinary, allow_invalid: true, ..Default::default() };
    write(&path, &open, &options, &mut cb).unwrap();
    assert!(path.exists());
    std::fs::remove_file(&path).unwrap();
}

#[test]
pub(crate) fn a_drilled_plate_exports_as_a_watertight_solid() {
    // Spec acceptance criterion 4, read back out of the written file rather
    // than trusted from the mesh that went in: the hole must still be round,
    // in the right place, and the surface closed.
    use simple3d_geom::{evaluate_boolean, BooleanOp};
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 32).translated(Vec3::new(-12.0, 0.0, 0.0));
    let mesh = evaluate_boolean(BooleanOp::Difference, &[plate(), hole]);
    assert!(verify(&mesh.weld()).is_empty(), "{:?}", verify(&mesh.weld()));

    let path = temp_dir().join("drilled.3mf");
    let mut cb = no_progress();
    write(&path, &mesh, &Options::default(), &mut cb).unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 500);
    std::fs::remove_file(&path).unwrap();

    // OBJ for the geometry assertions, because it is the one format this
    // crate writes that can be read back without a zip reader.
    let path = temp_dir().join("drilled.obj");
    write(&path, &mesh, &Options { format: Format::Obj, ..Default::default() }, &mut cb).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();

    let vertices: Vec<[f64; 3]> = text
        .lines()
        .filter(|l| l.starts_with("v "))
        .map(|l| {
            let mut f = l[2..].split_whitespace().map(|v| v.parse::<f64>().unwrap());
            [f.next().unwrap(), f.next().unwrap(), f.next().unwrap()]
        })
        .collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for v in &vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v[a]);
            hi[a] = hi[a].max(v[a]);
        }
    }
    // The plate's own dimensions are untouched by the cut.
    assert_eq!((hi[0] - lo[0], hi[2] - lo[2]), (40.0, 4.0));
    // The bore is open: a 32-segment circumscribed hole's nearest surface is
    // its flats, at radius 3 * cos(pi/32).
    let flat_radius = 3.0 * (std::f64::consts::PI / 32.0).cos();
    for v in &vertices {
        let r = ((v[0] + 12.0).powi(2) + v[1].powi(2)).sqrt();
        assert!(r > flat_radius - 1e-6, "a vertex at {v:?} landed inside the bore");
    }
    // And 12mm from the left edge, measured on the hole's own vertices.
    let bore: Vec<&[f64; 3]> =
        vertices.iter().filter(|v| ((v[0] + 12.0).powi(2) + v[1].powi(2)).sqrt() < 3.5).collect();
    assert!(!bore.is_empty(), "no bore vertices found");
    let bore_centre = bore.iter().map(|v| v[0]).sum::<f64>() / bore.len() as f64;
    assert!(
        (bore_centre - lo[0] - 8.0).abs() < 1e-6,
        "bore centre {bore_centre} is not 8mm from the left edge {}",
        lo[0]
    );

    // Watertight in the file: every undirected edge shared by exactly two faces.
    let mut edges: std::collections::HashMap<(usize, usize), u32> = std::collections::HashMap::new();
    for line in text.lines().filter(|l| l.starts_with("f ")) {
        let face: Vec<usize> =
            line[2..].split_whitespace().map(|f| f.split('/').next().unwrap().parse::<usize>().unwrap()).collect();
        for k in 0..face.len() {
            let (a, b) = (face[k], face[(k + 1) % face.len()]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    assert!(!edges.is_empty());
    assert!(edges.values().all(|&c| c == 2), "the exported surface is not closed");
    std::fs::remove_file(&path).unwrap();
}
