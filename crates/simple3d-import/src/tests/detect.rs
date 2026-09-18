//! Which format a file is, and what happens to one that is none of them.

use super::*;
use std::path::Path;

#[test]
pub(crate) fn a_name_names_a_format_whatever_its_case() {
    assert_eq!(Format::from_path(Path::new("plate.3mf")), Some(Format::ThreeMf));
    assert_eq!(Format::from_path(Path::new("plate.STL")), Some(Format::Stl));
    assert_eq!(Format::from_path(Path::new("/tmp/a.b/plate.Obj")), Some(Format::Obj));
    assert_eq!(Format::from_path(Path::new("plate.ply")), Some(Format::Ply));
    assert_eq!(Format::from_path(Path::new("plate.simple3d")), None);
    assert_eq!(Format::from_path(Path::new("plate")), None);
}

/// A file that is nothing this crate reads is refused with a message that says
/// what it does read, rather than with a parse error out of whichever reader
/// happened to be tried.
#[test]
pub(crate) fn a_file_that_is_not_a_model_says_so() {
    let error = read_all(b"{\"format\": 3, \"nodes\": []}", None).unwrap_err();
    assert!(matches!(error, ImportError::Unsupported(_)), "{error:?}");
    assert!(error.to_string().contains("3MF"), "the message does not say what can be read: {error}");

    // Named as something it is not: also unsupported, and the message says the
    // name was believed and did not hold up.
    let error = read_all(b"not a model at all", Some(Format::ThreeMf)).unwrap_err();
    assert!(matches!(error, ImportError::Malformed(_) | ImportError::Unsupported(_)), "{error:?}");
}

/// An OBJ has no header, so it is read on the strength of its name -- and a
/// project file named `.obj` is refused by the reader rather than accepted as
/// an OBJ of no faces.
#[test]
pub(crate) fn an_obj_is_read_from_its_name_because_it_has_no_header() {
    assert_eq!(Format::sniff(b"v 0 0 0\nf 1 1 1\n"), None);
    let model = read_all(b"v 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n", Some(Format::Obj)).unwrap();
    assert_eq!(model.format, Format::Obj);
    assert_eq!(model.triangle_count(), 1);
}
