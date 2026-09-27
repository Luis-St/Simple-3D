//! 3MF: the package, its build, its assemblies and the ways one can be broken.

use super::*;

/// A build item's transform places the object; ignoring it stacks every part.
#[test]
pub(crate) fn a_build_items_transform_places_the_object() {
    // Identity rotation, moved 100 mm along X and 5 mm up.
    let placed = " transform=\"1 0 0 0 1 0 0 0 1 100 0 5\"";
    let model = read_all(&package(&tetrahedron_model("millimeter", "", placed)), None).unwrap();
    let (lo, hi) = model.merged().bounds().unwrap();
    assert_eq!((lo.x, lo.y, lo.z), (100.0, 0.0, 5.0));
    assert_eq!((hi.x, hi.y, hi.z), (110.0, 10.0, 15.0));
}

/// An assembly's component transforms and the build's transform both apply, in that order.
#[test]
pub(crate) fn an_assembly_of_components_is_flattened_with_both_transforms_applied() {
    let document = "<?xml version=\"1.0\"?>\n\
        <model unit=\"millimeter\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\
        <resources>\
        <object id=\"1\" type=\"model\"><mesh><vertices>\
        <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"2\" y=\"0\" z=\"0\"/>\
        <vertex x=\"0\" y=\"2\" z=\"0\"/><vertex x=\"0\" y=\"0\" z=\"2\"/>\
        </vertices><triangles>\
        <triangle v1=\"0\" v2=\"2\" v3=\"1\"/><triangle v1=\"0\" v2=\"1\" v3=\"3\"/>\
        <triangle v1=\"1\" v2=\"2\" v3=\"3\"/><triangle v1=\"0\" v2=\"3\" v3=\"2\"/>\
        </triangles></mesh></object>\
        <object id=\"2\" name=\"Pair\" type=\"model\"><components>\
        <component objectid=\"1\"/>\
        <component objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 10 0 0\"/>\
        </components></object>\
        </resources>\
        <build><item objectid=\"2\" transform=\"1 0 0 0 1 0 0 0 1 0 0 100\"/></build></model>";
    let model = read_all(&package(document), None).unwrap();
    assert_eq!(model.parts.len(), 1, "the assembly should be one part");
    assert_eq!(model.parts[0].name, "Pair");
    assert_eq!(model.parts[0].mesh.triangle_count(), 8, "one of the two copies is missing");
    let (lo, hi) = model.parts[0].mesh.bounds().unwrap();
    assert_eq!((lo.x, lo.z), (0.0, 100.0), "the item's own transform was not applied");
    assert_eq!((hi.x, hi.z), (12.0, 102.0), "the component's transform was not applied");
}

/// A mirroring transform flips the winding back, so the part does not arrive inside out.
#[test]
pub(crate) fn a_mirrored_placement_has_its_winding_corrected() {
    let mirrored = " transform=\"-1 0 0 0 1 0 0 0 1 0 0 0\"";
    let model = read_all(&package(&tetrahedron_model("millimeter", "", mirrored)), None).unwrap();
    let mesh = model.merged();
    let plain = read_all(&package(&tetrahedron_model("millimeter", "", "")), None).unwrap().merged();
    // Both enclose volume of the same sign: the mirrored one is not inside out.
    let volume = simple3d_export::signed_volume(&mesh.weld());
    let reference = simple3d_export::signed_volume(&plain.weld());
    assert!(volume * reference > 0.0, "the mirrored part came in inside out: {volume} against {reference}");
}

/// An object assembled out of itself would recurse forever; it is reported as a broken file.
#[test]
pub(crate) fn an_object_that_contains_itself_is_refused_rather_than_recursed_into() {
    let document = "<model unit=\"millimeter\"><resources>\
        <object id=\"1\" type=\"model\"><components><component objectid=\"2\"/></components></object>\
        <object id=\"2\" type=\"model\"><components><component objectid=\"1\"/></components></object>\
        </resources><build><item objectid=\"1\"/></build></model>";
    let error = read_all(&package(document), None).unwrap_err();
    match error {
        ImportError::Malformed(why) => assert!(why.contains("out of itself"), "{why}"),
        other => panic!("{other:?}"),
    }
}

/// An undeclared build object and an out-of-range vertex index are both refused with the reason.
#[test]
pub(crate) fn a_model_that_does_not_hold_together_is_refused_with_the_reason() {
    let missing = "<model unit=\"millimeter\"><resources></resources>\
        <build><item objectid=\"7\"/></build></model>";
    match read_all(&package(missing), None).unwrap_err() {
        ImportError::Malformed(why) => assert!(why.contains("object 7"), "{why}"),
        other => panic!("{other:?}"),
    }

    let bad_triangle = "<model unit=\"millimeter\"><resources>\
        <object id=\"1\" type=\"model\"><mesh><vertices>\
        <vertex x=\"0\" y=\"0\" z=\"0\"/></vertices><triangles>\
        <triangle v1=\"0\" v2=\"1\" v3=\"2\"/></triangles></mesh></object>\
        </resources><build><item objectid=\"1\"/></build></model>";
    match read_all(&package(bad_triangle), None).unwrap_err() {
        ImportError::Malformed(why) => assert!(why.contains("names vertex"), "{why}"),
        other => panic!("{other:?}"),
    }
}

/// A package with no model part, and a file that is not a package at all.
#[test]
pub(crate) fn a_package_without_a_model_part_says_what_it_holds_instead() {
    let bytes = deflated_zip(&[("Metadata/thumbnail.png", b"not a model")]);
    match read_all(&bytes, Some(Format::ThreeMf)).unwrap_err() {
        ImportError::Malformed(why) => {
            assert!(why.contains("no model part") && why.contains("thumbnail"), "{why}")
        }
        other => panic!("{other:?}"),
    }

    // A zip of something else: a package, but not a model.
    let bytes = deflated_zip(&[("notes.txt", b"nothing to do with 3D")]);
    assert!(matches!(read_all(&bytes, None).unwrap_err(), ImportError::Malformed(_)));
}

/// A deflated model part, as in every 3MF from other programs, exercises the DEFLATE path.
#[test]
pub(crate) fn a_deflated_model_part_is_decompressed() {
    let model = read_all(&package(&tetrahedron_model("millimeter", "", "")), None).unwrap();
    assert_eq!(model.triangle_count(), 4);
    assert_eq!(model.format, Format::ThreeMf);
}

/// Base materials, as slicers write colours: the same painted triangles from a different element.
#[test]
pub(crate) fn a_colour_from_base_materials_is_read_as_well_as_from_a_colour_group() {
    let document = "<model unit=\"millimeter\"><resources>\
        <basematerials id=\"5\">\
        <base name=\"PLA\" displaycolor=\"#1188CCFF\"/>\
        </basematerials>\
        <object id=\"1\" type=\"model\" pid=\"5\" pindex=\"0\"><mesh><vertices>\
        <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"10\" y=\"0\" z=\"0\"/>\
        <vertex x=\"0\" y=\"10\" z=\"0\"/><vertex x=\"0\" y=\"0\" z=\"10\"/>\
        </vertices><triangles>\
        <triangle v1=\"0\" v2=\"2\" v3=\"1\" p1=\"0\"/><triangle v1=\"0\" v2=\"1\" v3=\"3\" p1=\"0\"/>\
        <triangle v1=\"1\" v2=\"2\" v3=\"3\" p1=\"0\"/><triangle v1=\"0\" v2=\"3\" v3=\"2\" p1=\"0\"/>\
        </triangles></mesh></object>\
        </resources><build><item objectid=\"1\"/></build></model>";
    let model = read_all(&package(document), None).unwrap();
    let mesh = model.merged();
    assert!(
        (0..mesh.triangle_count()).all(|i| simple3d_geom::tag_colour(mesh.tag(i)) == Some([0x11, 0x88, 0xCC])),
        "the material's colour did not reach the faces"
    );
}

/// Namespace prefixes do not matter: `<m:color>`, `<ns2:color>` and `<color>` are the same element.
#[test]
pub(crate) fn a_namespace_prefix_does_not_change_which_element_something_is() {
    let document = "<model unit=\"millimeter\"><resources>\
        <ns2:colorgroup id=\"9\"><ns2:color color=\"#204080\"/></ns2:colorgroup>\
        <object id=\"1\" type=\"model\"><mesh><vertices>\
        <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"10\" y=\"0\" z=\"0\"/>\
        <vertex x=\"0\" y=\"10\" z=\"0\"/>\
        </vertices><triangles>\
        <triangle v1=\"0\" v2=\"1\" v3=\"2\" pid=\"9\" p1=\"0\"/>\
        </triangles></mesh></object>\
        </resources><build><item objectid=\"1\"/></build></model>";
    let model = read_all(&package(document), None).unwrap();
    let mesh = model.merged();
    assert_eq!(simple3d_geom::tag_colour(mesh.tag(0)), Some([0x20, 0x40, 0x80]));
}

/// An XML-escaped name comes back as typed.
#[test]
pub(crate) fn an_escaped_object_name_is_read_back_unescaped() {
    let named = " name=\"Bracket &amp; base &lt;2&gt;\"";
    let model = read_all(&package(&tetrahedron_model("millimeter", named, "")), None).unwrap();
    assert_eq!(model.parts[0].name, "Bracket & base <2>");
}

/// Bambu Studio and OrcaSlicer put meshes in separate model parts reached via `p:path`, whose ids
/// may repeat the main part's.
#[test]
pub(crate) fn a_component_in_another_model_part_is_read_from_that_part() {
    let root = "<?xml version=\"1.0\"?>\n\
        <model unit=\"millimeter\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\" \
        xmlns:p=\"http://schemas.microsoft.com/3dmanufacturing/production/2015/06\" requiredextensions=\"p\">\
        <resources>\
        <object id=\"1\" type=\"model\"><mesh><vertices>\
        <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"1\" y=\"0\" z=\"0\"/>\
        <vertex x=\"0\" y=\"1\" z=\"0\"/><vertex x=\"0\" y=\"0\" z=\"1\"/>\
        </vertices><triangles>\
        <triangle v1=\"0\" v2=\"2\" v3=\"1\"/><triangle v1=\"0\" v2=\"1\" v3=\"3\"/>\
        <triangle v1=\"1\" v2=\"2\" v3=\"3\"/><triangle v1=\"0\" v2=\"3\" v3=\"2\"/>\
        </triangles></mesh></object>\
        <object id=\"2\" name=\"Placed\" type=\"model\"><components>\
        <component p:path=\"/3D/Objects/object_1.model\" objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 0 0 0\"/>\
        </components></object>\
        </resources>\
        <build><item objectid=\"2\" transform=\"2 0 0 0 2 0 0 0 2 100 0 0\"/></build></model>";
    let bytes = deflated_zip(&[
        ("[Content_Types].xml", b"<?xml version=\"1.0\"?><Types/>"),
        ("_rels/.rels", b"<?xml version=\"1.0\"?><Relationships/>"),
        ("3D/3dmodel.model", root.as_bytes()),
        ("3D/Objects/object_1.model", tetrahedron_model("millimeter", "", "").as_bytes()),
    ]);
    let model = read_all(&bytes, None).unwrap();
    assert_eq!(model.parts.len(), 1);
    assert_eq!(model.parts[0].name, "Placed");
    assert_eq!(model.parts[0].mesh.triangle_count(), 4, "the object of the same id in the main part was read instead");
    let (lo, hi) = model.parts[0].mesh.bounds().unwrap();
    assert_eq!((lo.x, hi.x), (100.0, 120.0), "the build's transform was not applied to the other part's mesh");
}
