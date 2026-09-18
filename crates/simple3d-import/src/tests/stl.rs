//! STL: which encoding a file is, the solids it holds, and where it is refused.

use super::*;

/// The trap every STL reader falls into: a *binary* file whose 80-byte header
/// begins with the word `solid`, which plenty of programs write. Read as text
/// it holds no facets at all, so the arithmetic decides and not the word.
#[test]
pub(crate) fn a_binary_file_whose_header_says_solid_is_still_read_as_binary() {
    let mesh = plate();
    let mut bytes = exported(&mesh, simple3d_export::Format::StlBinary);
    let banner = b"solid plate written by a program that fills the header in";
    bytes[..banner.len()].copy_from_slice(banner);
    let model = read_all(&bytes, Some(Format::Stl)).unwrap();
    assert_eq!(model.triangle_count(), mesh.weld().triangle_count(), "it was read as text and came back short");
}

/// A text STL may hold several solids, which is how some programs write an
/// assembly. Each one comes in as its own part under the name it was given,
/// rather than as one bag of triangles.
#[test]
pub(crate) fn several_solids_in_one_text_file_come_in_as_several_parts() {
    let file = "solid left\n\
         facet normal 0 0 1\n outer loop\n  vertex 0 0 0\n  vertex 1 0 0\n  vertex 0 1 0\n endloop\nendfacet\n\
         endsolid left\n\
         solid right\n\
         facet normal 0 0 1\n outer loop\n  vertex 5 0 0\n  vertex 6 0 0\n  vertex 5 1 0\n endloop\nendfacet\n\
         facet normal 0 0 1\n outer loop\n  vertex 6 0 0\n  vertex 6 1 0\n  vertex 5 1 0\n endloop\nendfacet\n\
         endsolid right\n";
    let model = read_all(file.as_bytes(), Some(Format::Stl)).unwrap();
    let parts: Vec<(&str, usize)> =
        model.parts.iter().map(|part| (part.name.as_str(), part.mesh.triangle_count())).collect();
    assert_eq!(parts, vec![("left", 1), ("right", 2)]);
}

/// The winding is what is believed, not the stored normal. The two disagree in
/// files from other programs, and only one of them can be the surface: the
/// vertex order is what every check in this workspace reads.
#[test]
pub(crate) fn the_vertex_order_is_kept_and_the_stored_normal_ignored() {
    let file = "solid one\n\
         facet normal 0 0 -1\n outer loop\n  vertex 0 0 0\n  vertex 10 0 0\n  vertex 0 10 0\n endloop\nendfacet\n\
         endsolid one\n";
    let model = read_all(file.as_bytes(), Some(Format::Stl)).unwrap();
    let mesh = &model.parts[0].mesh;
    let normal = mesh.triangle_normal(mesh.indices[0]);
    assert!(normal.z > 0.9, "the winding was rewritten to match the stored normal: {normal:?}");
}

/// A facet loop of more than three vertices is a flat polygon, and is fanned
/// rather than having everything past the third vertex thrown away.
#[test]
pub(crate) fn a_facet_of_four_vertices_comes_in_as_two_triangles() {
    let file = "solid quad\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 10 0 0\n\
         vertex 10 10 0\n   vertex 0 10 0\n  endloop\n endfacet\nendsolid quad\n";
    let model = read_all(file.as_bytes(), Some(Format::Stl)).unwrap();
    assert_eq!(model.triangle_count(), 2);
    let (lo, hi) = model.merged().bounds().unwrap();
    assert_eq!((lo.x, lo.y, hi.x, hi.y), (0.0, 0.0, 10.0, 10.0), "the fourth corner was dropped");
}

/// A file that is the right shape for neither encoding is refused with the
/// reason, rather than read as an empty model.
#[test]
pub(crate) fn a_file_that_is_neither_encoding_is_refused() {
    // Long enough to be a binary STL, and its triangle count does not match its
    // size.
    let mut bytes = vec![0u8; 200];
    bytes[80..84].copy_from_slice(&9999u32.to_le_bytes());
    let error = read_all(&bytes, Some(Format::Stl)).unwrap_err();
    assert!(matches!(error, ImportError::Malformed(_)), "{error:?}");
}

/// A coordinate that is not a number stops the import. Half a model is not a
/// model: a body missing the triangles after the damage is a surface with a
/// hole in it, and the user is better told than handed one.
#[test]
pub(crate) fn a_coordinate_that_is_not_a_number_stops_the_import() {
    let file = "solid one\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex nan 0 0\n\
         vertex 0 10 0\n  endloop\n endfacet\nendsolid one\n";
    let error = read_all(file.as_bytes(), Some(Format::Stl)).unwrap_err();
    match error {
        ImportError::Malformed(why) => assert!(why.contains("line 5"), "the message does not say where: {why}"),
        other => panic!("{other:?}"),
    }
}
