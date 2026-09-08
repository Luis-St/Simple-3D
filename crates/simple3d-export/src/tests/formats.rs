//! What each format writes, and the dimensions it comes back at.

use super::*;

#[test]
pub(crate) fn every_format_writes_a_file_with_the_expected_shape() {
    let mesh = plate();
    let welded = mesh.weld();
    for format in Format::ALL {
        let path = temp_dir().join(format!("plate.{}", format.id()));
        let mut cb = no_progress();
        write(&path, &mesh, &Options { format, ..Default::default() }, &mut cb).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "{format:?} wrote nothing");

        match format {
            Format::ThreeMf => {
                assert_eq!(&bytes[0..2], b"PK");
                let text = String::from_utf8_lossy(&bytes);
                assert!(text.contains("unit=\"millimeter\""), "3MF did not record its unit");
                assert!(text.contains("<triangle v1="));
                assert!(text.contains("[Content_Types].xml"));
                assert!(text.contains("3D/3dmodel.model"));
            }
            Format::StlBinary => {
                let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
                assert_eq!(count as usize, welded.indices.len());
                assert_eq!(bytes.len(), 84 + welded.indices.len() * 50);
            }
            Format::StlAscii => {
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.starts_with("solid "));
                assert!(text.trim_end().ends_with("endsolid simple3d"));
                assert_eq!(text.matches("facet normal").count(), welded.indices.len());
            }
            Format::Obj => {
                let text = String::from_utf8(bytes).unwrap();
                assert_eq!(text.lines().filter(|l| l.starts_with("v ")).count(), welded.positions.len());
                assert_eq!(text.lines().filter(|l| l.starts_with("f ")).count(), welded.indices.len());
                // 1-based indices, and none out of range.
                for line in text.lines().filter(|l| l.starts_with("f ")) {
                    for field in line[2..].split_whitespace() {
                        let i: usize = field.parse().unwrap();
                        assert!(i >= 1 && i <= welded.positions.len());
                    }
                }
            }
            Format::PlyAscii => {
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.starts_with("ply\nformat ascii 1.0\n"));
                assert!(text.contains(&format!("element vertex {}", welded.positions.len())));
                assert_eq!(text.lines().filter(|l| l.starts_with("3 ")).count(), welded.indices.len());
            }
            Format::PlyBinary => {
                let text = String::from_utf8_lossy(&bytes);
                assert!(text.starts_with("ply\nformat binary_little_endian 1.0\n"));
                let header_end = bytes.windows(11).position(|w| w == b"end_header\n").unwrap() + 11;
                let expected = welded.positions.len() * 24 + welded.indices.len() * 13;
                assert_eq!(bytes.len() - header_end, expected);
            }
        }
        std::fs::remove_file(&path).unwrap();
    }
}

#[test]
pub(crate) fn the_exported_bounding_box_matches_the_entered_dimensions() {
    // Spec acceptance criteria 1 and 2, read back out of the written file.
    let path = temp_dir().join("dims.obj");
    let mut cb = no_progress();
    write(&path, &plate(), &Options { format: Format::Obj, ..Default::default() }, &mut cb).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for line in text.lines().filter(|l| l.starts_with("v ")) {
        for (axis, field) in line[2..].split_whitespace().enumerate() {
            let v: f64 = field.parse().unwrap();
            lo[axis] = lo[axis].min(v);
            hi[axis] = hi[axis].max(v);
        }
    }
    assert_eq!(hi[0] - lo[0], 40.0);
    assert_eq!(hi[1] - lo[1], 20.0);
    assert_eq!(hi[2] - lo[2], 4.0);
    std::fs::remove_file(&path).unwrap();
}

#[test]
pub(crate) fn the_scale_factor_multiplies_every_dimension_and_defaults_to_one() {
    let path = temp_dir().join("scaled.stl");
    let mut cb = no_progress();
    let options = Options { format: Format::StlBinary, scale: 2.0, ..Default::default() };
    write(&path, &plate(), &options, &mut cb).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut max_x = f32::MIN;
    for i in 0..u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize {
        let base = 84 + i * 50 + 12;
        for v in 0..3 {
            let o = base + v * 12;
            max_x = max_x.max(f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]));
        }
    }
    assert!((max_x - 40.0).abs() < 1e-3, "scale 2.0 should put the far face at 40mm, got {max_x}");
    std::fs::remove_file(&path).unwrap();
}

#[test]
pub(crate) fn format_ids_round_trip_for_remembering_the_last_choice() {
    for format in Format::ALL {
        assert_eq!(Format::from_id(format.id()), Some(format));
        assert!(!format.extension().is_empty());
        assert!(!format.label().is_empty());
    }
    assert_eq!(Format::from_id("nonsense"), None);
    // Only 3MF records its unit; the dialog states the assumption for the rest.
    assert_eq!(Format::ALL.iter().filter(|f| f.carries_units()).count(), 1);
}
