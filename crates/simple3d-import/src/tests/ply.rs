//! PLY: the header deciding where everything is, in either byte order, with
//! whatever else a file happens to carry per vertex.

use super::*;

/// A PLY from another program carries properties this one has no use for --
/// normals, colours, a scanner's confidence -- and the coordinates are still
/// where the header says. Every property is stepped over by its declared
/// width, so none of it shifts the geometry.
#[test]
pub(crate) fn properties_that_are_not_geometry_are_stepped_over() {
    let file = "ply\nformat ascii 1.0\n\
         element vertex 3\n\
         property float nx\nproperty float x\nproperty float y\nproperty float z\n\
         property uchar red\nproperty uchar green\nproperty uchar blue\nproperty float confidence\n\
         element face 1\nproperty list uchar int vertex_indices\nproperty uchar flags\n\
         end_header\n\
         1 0 0 0 255 0 0 0.5\n\
         1 10 0 0 255 0 0 0.5\n\
         1 0 10 0 255 0 0 0.5\n\
         3 0 1 2 7\n";
    let model = read_all(file.as_bytes(), None).unwrap();
    assert_eq!(model.triangle_count(), 1);
    let (lo, hi) = model.merged().bounds().unwrap();
    assert_eq!((lo.x, lo.y, hi.x, hi.y), (0.0, 0.0, 10.0, 10.0), "a property was read as a coordinate");
}

/// Big-endian binary, which nothing in this workspace writes and which a PLY
/// from a machine that does is entirely valid in.
#[test]
pub(crate) fn a_big_endian_binary_file_reads_the_same_as_a_little_endian_one() {
    let mut big = b"ply\nformat binary_big_endian 1.0\nelement vertex 3\n\
        property double x\nproperty double y\nproperty double z\n\
        element face 1\nproperty list uchar uint vertex_indices\nend_header\n"
        .to_vec();
    for point in [[0.0f64, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]] {
        for value in point {
            big.extend_from_slice(&value.to_be_bytes());
        }
    }
    big.push(3);
    for index in [0u32, 1, 2] {
        big.extend_from_slice(&index.to_be_bytes());
    }
    let model = read_all(&big, None).unwrap();
    assert_eq!(model.triangle_count(), 1);
    let (_, hi) = model.merged().bounds().unwrap();
    assert!((hi.x - 10.0).abs() < 1e-9 && (hi.y - 10.0).abs() < 1e-9, "the bytes were read the other way round");
}

/// A face may be a polygon, and is fanned like every other format's.
#[test]
pub(crate) fn a_polygon_face_is_fanned() {
    let file = "ply\nformat ascii 1.0\nelement vertex 4\n\
         property float x\nproperty float y\nproperty float z\n\
         element face 1\nproperty list uchar uint vertex_indices\nend_header\n\
         0 0 0\n10 0 0\n10 10 0\n0 10 0\n\
         4 0 1 2 3\n";
    let model = read_all(file.as_bytes(), None).unwrap();
    assert_eq!(model.triangle_count(), 2);
}

/// A face naming a vertex past the end of the list is refused. An index nobody
/// checked is the one way a mesh reader reaches past its own array.
#[test]
pub(crate) fn a_face_index_past_the_end_of_the_vertices_is_refused() {
    let file = "ply\nformat ascii 1.0\nelement vertex 3\n\
         property float x\nproperty float y\nproperty float z\n\
         element face 1\nproperty list uchar uint vertex_indices\nend_header\n\
         0 0 0\n10 0 0\n0 10 0\n\
         3 0 1 7\n";
    let error = read_all(file.as_bytes(), None).unwrap_err();
    assert!(matches!(error, ImportError::Malformed(_)), "{error:?}");
}

/// A header that does not say how the file is encoded, and a body that stops
/// half way through: both are refused with the reason.
#[test]
pub(crate) fn a_header_or_body_that_does_not_hold_up_is_refused_with_the_reason() {
    let no_format = b"ply\nelement vertex 1\nproperty float x\nend_header\n0\n";
    match read_all(no_format, None).unwrap_err() {
        ImportError::Malformed(why) => assert!(why.contains("encoded"), "{why}"),
        other => panic!("{other:?}"),
    }
    let cut_short = b"ply\nformat ascii 1.0\nelement vertex 3\n\
        property float x\nproperty float y\nproperty float z\n\
        element face 1\nproperty list uchar uint vertex_indices\nend_header\n0 0 0\n10 0\n";
    match read_all(cut_short, None).unwrap_err() {
        ImportError::Malformed(why) => assert!(why.contains("ends part-way"), "{why}"),
        other => panic!("{other:?}"),
    }
}
