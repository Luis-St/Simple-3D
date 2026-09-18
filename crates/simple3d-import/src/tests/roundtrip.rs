//! What issue 105 asks for, measured directly: every format the exporter
//! writes is read back, and what comes back is the shape that went out.

use super::*;

/// The issue in one test: for each of the six things the export dialog offers,
/// write a plate and read the file back. Every one has to come back as the same
/// surface -- the same triangles, the same corners, still watertight.
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

/// The format is read out of the bytes, so a file whose extension is wrong --
/// or missing, as a downloaded file often is -- still reads as what it is.
/// Only OBJ needs its name, having no header to be recognised by.
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
        // And the wrong name is not believed over the content.
        let lied_to = read_all(&bytes, Some(Format::Stl)).unwrap_or_else(|e| panic!("{}: {e}", format.label()));
        assert_eq!(lied_to.triangle_count(), model.triangle_count());
    }
}

/// A 3MF written as several named bodies (issue 58) comes back as several named
/// parts, which is what makes the import the other half of that export: the
/// rows that went into the file are the rows that come out of it.
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
    // And they came back where they were, not stacked on one another.
    let (lo, hi) = model.merged().bounds().unwrap();
    assert!((hi.x - lo.x - 140.0).abs() < 1e-6, "the two plates span {} rather than 140", hi.x - lo.x);
}

/// The unit a 3MF records is applied on the way in. Everything above this crate
/// is millimetres, so a file in inches has to arrive twenty-five times larger
/// than its numbers -- and the import has to be able to say what it converted
/// from.
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

/// A painted export comes back painted (issue 84's colours through issue 105's
/// import), and an unpainted one does not come back painted in the exporter's
/// stand-in grey.
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

/// Nothing is brought in from a file that holds no triangles, whichever way it
/// manages to hold none.
#[test]
pub(crate) fn a_file_with_no_geometry_in_it_is_refused_as_empty() {
    assert_eq!(read_all(b"", None).unwrap_err(), ImportError::Empty);
    assert_eq!(read_all(b"# only a comment\n", Some(Format::Obj)).unwrap_err(), ImportError::Empty);
    // Vertices with no faces are not a surface.
    assert_eq!(read_all(b"v 0 0 0\nv 1 0 0\nv 0 1 0\n", Some(Format::Obj)).unwrap_err(), ImportError::Empty);
    let empty_ply = b"ply\nformat ascii 1.0\nelement vertex 0\nproperty float x\nproperty float y\n\
        property float z\nelement face 0\nproperty list uchar uint vertex_indices\nend_header\n";
    assert_eq!(read_all(empty_ply, None).unwrap_err(), ImportError::Empty);
}

/// A reader that is asked to stop, stops -- the same contract the exporter's
/// progress callback has, and what lets the application's import run on a
/// thread with a Cancel button beside it.
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

/// Progress runs from nothing to done. It is what the footer's bar reads, so a
/// reader that only ever reports 0.0 would leave the bar sitting still through
/// a minute of work.
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
