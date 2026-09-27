//! OBJ: its groups, the ways it names a vertex, and its polygons.

use super::*;

/// File-wide vertex numbers are remapped per part, and groups come out in file order.
#[test]
pub(crate) fn the_objects_in_a_file_come_in_as_parts_of_their_own() {
    let file = "# two triangles, one each\n\
         v 0 0 0\nv 10 0 0\nv 0 10 0\n\
         v 100 0 0\nv 110 0 0\nv 100 10 0\n\
         o left\nf 1 2 3\n\
         o right\nf 4 5 6\n";
    let model = read_all(file.as_bytes(), Some(Format::Obj)).unwrap();
    let parts: Vec<(&str, usize, usize)> = model
        .parts
        .iter()
        .map(|part| (part.name.as_str(), part.mesh.triangle_count(), part.mesh.positions.len()))
        .collect();
    assert_eq!(parts, vec![("left", 1, 3), ("right", 1, 3)], "a part carried the whole file's vertices");
    let (lo, hi) = model.parts[1].mesh.bounds().unwrap();
    assert_eq!((lo.x, hi.x), (100.0, 110.0), "the second object's faces landed on the wrong vertices");
}

/// The three face vertex forms and negative indices all name the same triangle.
#[test]
pub(crate) fn a_face_may_name_its_vertices_in_any_of_the_forms_obj_allows() {
    let file = "v 0 0 0\nv 10 0 0\nv 0 10 0\n\
         vt 0 0\nvn 0 0 1\n\
         f 1 2 3\nf 1/1 2/1 3/1\nf 1/1/1 2/1/1 3/1/1\nf 1//1 2//1 3//1\nf -3 -2 -1\n";
    let model = read_all(file.as_bytes(), Some(Format::Obj)).unwrap();
    assert_eq!(model.triangle_count(), 5);
    assert_eq!(model.parts[0].mesh.positions.len(), 3, "the same vertex was brought in more than once");
}

/// A quad is two triangles and an n-gon a fan.
#[test]
pub(crate) fn polygons_are_fanned_into_triangles() {
    let file = "v 0 0 0\nv 10 0 0\nv 10 10 0\nv 0 10 0\nv -5 5 0\n\
         g quad\nf 1 2 3 4\n\
         g pentagon\nf 1 2 3 4 5\n";
    let model = read_all(file.as_bytes(), Some(Format::Obj)).unwrap();
    let counts: Vec<usize> = model.parts.iter().map(|part| part.mesh.triangle_count()).collect();
    assert_eq!(counts, vec![2, 3]);
}

/// A file naming no object is one part, named after the file by the caller.
#[test]
pub(crate) fn a_file_with_no_groups_in_it_is_one_unnamed_part() {
    let model = read_all(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", Some(Format::Obj)).unwrap();
    assert_eq!(model.parts.len(), 1);
    assert_eq!(model.parts[0].name, "");
}

/// A face naming an undefined vertex is refused where it breaks.
#[test]
pub(crate) fn a_face_naming_a_vertex_that_is_not_there_stops_the_import() {
    let error = read_all(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 9\n", Some(Format::Obj)).unwrap_err();
    match error {
        ImportError::Malformed(why) => {
            assert!(why.contains("line 4") && why.contains("9"), "the message does not say where: {why}")
        }
        other => panic!("{other:?}"),
    }
}

/// Comments and material statements are skipped.
#[test]
pub(crate) fn what_an_obj_carries_besides_geometry_is_read_past() {
    let file = "mtllib plate.mtl\n# a comment\nusemtl red\ns off\n\
         v 0 0 0 1.0\nv 10 0 0\nv 0 10 0\n\
         vt 0.5 0.5\nvn 0 0 1\nvp 0 0\n\
         f 1 2 3 # the only face\n";
    let model = read_all(file.as_bytes(), Some(Format::Obj)).unwrap();
    assert_eq!(model.triangle_count(), 1);
}
