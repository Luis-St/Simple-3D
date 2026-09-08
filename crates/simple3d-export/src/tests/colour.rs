//! Colours through the 3MF extension.

use super::*;
use simple3d_geom::primitives;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn an_unpainted_model_is_written_without_the_colour_extension() {
    // Nothing that reads plain 3MF should have to cope with a namespace a
    // model does not use.
    let bytes = three_mf(&[Part::whole(&plate())], &Options::default(), &mut no_progress()).unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(!text.contains("colorgroup"));
    assert!(!text.contains("xmlns:m="));
}

#[test]
pub(crate) fn a_painted_model_carries_one_colour_group_and_a_colour_per_face() {
    let mut mesh = plate();
    mesh.set_tag(simple3d_geom::colour_tag([0x20, 0x40, 0x80]));
    // Two bodies, two colours, in one mesh -- what a boolean between two
    // painted shapes produces.
    let mut other = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(60.0, 0.0, 0.0));
    other.set_tag(simple3d_geom::colour_tag([0xFF, 0x00, 0x00]));
    mesh.append(&other);

    let bytes = three_mf(&[Part::whole(&mesh)], &Options::default(), &mut no_progress()).unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(text.contains("xmlns:m=\"http://schemas.microsoft.com/3dmanufacturing/material/2015/02\""));
    assert!(text.contains("<m:color color=\"#204080\"/>"));
    assert!(text.contains("<m:color color=\"#FF0000\"/>"));
    assert!(text.contains("pid=\"2\" pindex=\"0\""));
    // Index 0 is the unpainted default, so these two are 1 and 2 and every
    // triangle names one of them.
    assert_eq!(text.matches(" p1=\"1\"").count(), plate().indices.len());
    assert_eq!(text.matches(" p1=\"2\"").count(), other.indices.len());
    assert_eq!(text.matches(" p1=\"0\"").count(), 0);
}

#[test]
pub(crate) fn separated_objects_share_one_colour_group() {
    // Two objects painted the same colour name the same entry, and the
    // group's id sits clear of the objects' own ids.
    let mut left = plate();
    left.set_tag(simple3d_geom::colour_tag([0x20, 0x40, 0x80]));
    let mut right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    right.set_tag(simple3d_geom::colour_tag([0x20, 0x40, 0x80]));
    let parts = [Part { name: "A", mesh: &left }, Part { name: "B", mesh: &right }];
    let options = Options { bodies: BodyMode::TopLevel, ..Default::default() };
    let text = String::from_utf8_lossy(&three_mf(&parts, &options, &mut no_progress()).unwrap()).to_string();
    assert_eq!(text.matches("<m:colorgroup").count(), 1, "{text}");
    assert_eq!(text.matches("<m:color ").count(), 2, "one default and one painted colour: {text}");
    assert_eq!(text.matches("pid=\"3\" pindex=\"0\"").count(), 2, "{text}");
    assert_eq!(text.matches(" p1=\"1\"").count(), left.weld().indices.len() + right.weld().indices.len());
}

#[test]
pub(crate) fn the_colour_table_lists_each_colour_once_in_the_order_it_appears() {
    let mut mesh = plate();
    mesh.set_tag(simple3d_geom::colour_tag([1, 2, 3]));
    let mut second = plate().translated(Vec3::new(100.0, 0.0, 0.0));
    second.set_tag(simple3d_geom::colour_tag([1, 2, 3]));
    mesh.append(&second);
    let (colours, per_mesh) = colour_table(&[&mesh]);
    assert_eq!(colours, vec![[0x9A, 0xA4, 0xB2], [1, 2, 3]]);
    assert!(per_mesh[0].iter().all(|&i| i == 1));
}
