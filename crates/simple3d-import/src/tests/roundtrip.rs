//! Issue 105 measured directly: every exported format reads back as the shape that went out.

use super::*;

/// For each export dialog option, a plate written and read back is the same watertight surface.
#[test]
pub(crate) fn every_format_the_exporter_writes_can_be_read_back() {
    let mesh = plate();
    let welded = mesh.weld();
    for format in simple3d_export::Format::ALL {
        let bytes = exported(&mesh, format);
        let model = read_all(&bytes, named_as(format)).unwrap_or_else(|e| panic!("{}: {e}", format.label()));
        let read = model.merged().weld();
        assert_eq!(
            read.triangle_count(),
            welded.triangle_count(),
            "{} came back with a different number of triangles",
            format.label()
        );
        assert_eq!(
            read.positions.len(),
            welded.positions.len(),
            "{} came back with a different number of vertices",
            format.label()
        );
        assert!(
            bounds_differ(&read, &welded) < 1e-3,
            "{} came back a different size: {:?} against {:?}",
            format.label(),
            read.bounds(),
            welded.bounds()
        );
        assert!(read.manifold_issue().is_none(), "{} came back as a surface with a hole in it", format.label());
    }
}

/// The format is detected from content, so wrong or missing extensions still read; only OBJ needs
/// its name.
#[test]
pub(crate) fn a_file_is_read_by_its_content_rather_than_by_its_name() {
    let mesh = plate();
    for format in simple3d_export::Format::ALL {
        if format == simple3d_export::Format::Obj {
            continue;
        }
        let bytes = exported(&mesh, format);
        let model = read_all(&bytes, None).unwrap_or_else(|e| panic!("{}: {e}", format.label()));
        assert!(model.triangle_count() > 0, "{} was not recognised from its content", format.label());
        // A wrong name is not believed over the content.
        let lied_to = read_all(&bytes, Some(Format::Stl)).unwrap_or_else(|e| panic!("{}: {e}", format.label()));
        assert_eq!(lied_to.triangle_count(), model.triangle_count());
    }
}

/// A 3MF of several named bodies (issue 58) comes back as those named parts.
#[test]
pub(crate) fn a_three_mf_of_several_bodies_comes_back_as_those_bodies() {
    let left = plate();
    let right = plate().translated(Vec3::new(100.0, 0.0, 0.0));
    let bytes =
        exported_parts(&[("Left plate", left.clone()), ("Right plate", right)], simple3d_export::Format::ThreeMf);
    let model = read_all(&bytes, None).unwrap();
    let names: Vec<&str> = model.parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, vec!["Left plate", "Right plate"], "the objects came back under different names");
    for part in &model.parts {
        assert_eq!(part.mesh.weld().triangle_count(), left.weld().triangle_count());
    }
    // They came back where they were, not stacked.
    let (lo, hi) = model.merged().bounds().unwrap();
    assert!((hi.x - lo.x - 140.0).abs() < 1e-6, "the two plates span {} rather than 140", hi.x - lo.x);
}

/// A 3MF's unit is applied on import, so inches arrive 25.4 times larger, and the import reports it.
#[test]
pub(crate) fn a_three_mf_in_another_unit_is_converted_to_millimetres() {
    let model = read_all(&package(&tetrahedron_model("inch", "", "")), None).unwrap();
    assert_eq!(model.unit, Some(Unit::Inch));
    let (lo, hi) = model.merged().bounds().unwrap();
    assert!((lo - Vec3::ZERO).length() < 1e-9);
    assert!((hi.x - 254.0).abs() < 1e-6, "ten inches came in as {}mm", hi.x);

    // Millimetres are left exactly as written.
    let plain = read_all(&package(&tetrahedron_model("millimeter", "", "")), None).unwrap();
    assert_eq!(plain.unit, Some(Unit::Millimetre));
    assert!((plain.merged().bounds().unwrap().1.x - 10.0).abs() < 1e-9);
}

/// A painted export comes back painted, and an unpainted one not in the exporter's stand-in grey.
#[test]
pub(crate) fn the_colours_a_three_mf_carries_come_back_on_the_faces_that_had_them() {
    let mut painted = plate();
    let red = simple3d_geom::colour_tag([0xC8, 0x32, 0x28]);
    painted.set_tag(red);
    let bytes = exported(&painted, simple3d_export::Format::ThreeMf);
    let model = read_all(&bytes, None).unwrap();
    let mesh = model.merged();
    assert!(mesh.triangle_count() > 0);
    assert!(
        (0..mesh.triangle_count()).all(|i| simple3d_geom::tag_colour(mesh.tag(i)) == Some([0xC8, 0x32, 0x28])),
        "the faces came back some other colour"
    );

    let plain = read_all(&exported(&plate(), simple3d_export::Format::ThreeMf), None).unwrap().merged();
    assert!(
        (0..plain.triangle_count()).all(|i| plain.tag(i) == 0),
        "an unpainted model came back painted in the neutral the exporter writes"
    );
}

/// Nothing is imported from a file with no triangles, however it manages that.
#[test]
pub(crate) fn a_file_with_no_geometry_in_it_is_refused_as_empty() {
    assert_eq!(read_all(b"", None).unwrap_err(), ImportError::Empty);
    assert_eq!(read_all(b"# only a comment\n", Some(Format::Obj)).unwrap_err(), ImportError::Empty);
    // Vertices without faces are not a surface.
    assert_eq!(read_all(b"v 0 0 0\nv 1 0 0\nv 0 1 0\n", Some(Format::Obj)).unwrap_err(), ImportError::Empty);
    let empty_ply = b"ply\nformat ascii 1.0\nelement vertex 0\nproperty float x\nproperty float y\n\
        property float z\nelement face 0\nproperty list uchar uint vertex_indices\nend_header\n";
    assert_eq!(read_all(empty_ply, None).unwrap_err(), ImportError::Empty);
}

/// A reader asked to stop stops, as the exporter's callback contract says, allowing Cancel.
#[test]
pub(crate) fn an_import_stops_when_the_caller_says_so() {
    let mesh = plate();
    for format in simple3d_export::Format::ALL {
        let bytes = exported(&mesh, format);
        let mut refuse = |_: f32| false;
        assert_eq!(
            read_bytes(&bytes, named_as(format), &mut refuse).unwrap_err(),
            ImportError::Cancelled,
            "{} read on regardless",
            format.label()
        );
    }
}

/// Progress runs from nothing to done, so the footer's bar moves.
#[test]
pub(crate) fn progress_is_reported_up_to_one() {
    let mesh = plate();
    for format in simple3d_export::Format::ALL {
        let bytes = exported(&mesh, format);
        let mut seen: Vec<f32> = Vec::new();
        let mut watch = |fraction: f32| {
            seen.push(fraction);
            true
        };
        read_bytes(&bytes, named_as(format), &mut watch).unwrap();
        assert!(seen.first() == Some(&0.0), "{} did not start at nothing: {seen:?}", format.label());
        assert!(seen.last() == Some(&1.0), "{} did not finish: {seen:?}", format.label());
    }
}
