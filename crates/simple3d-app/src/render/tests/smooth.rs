//! Smooth shading of mesh bodies: their corner normals, the lines left on them, and the pixels.

use super::*;
use simple3d_geom::Mesh;
use std::collections::BTreeSet;

/// `mesh` as if it came from node source 7.
fn from_seven(mut mesh: Mesh) -> Mesh {
    mesh.set_source(7);
    mesh
}

fn seven() -> BTreeSet<u32> {
    [7].into()
}

#[test]
pub(crate) fn only_triangles_from_a_smooth_source_get_corner_normals() {
    let ball = from_seven(primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 16));
    assert!(Renderable::prepare_node(&ball, false, &BTreeSet::new()).corner_normals.is_empty());
    assert!(Renderable::prepare_node(&ball, false, &[8].into()).corner_normals.is_empty());
    let smooth = Renderable::prepare_node(&ball, false, &seven());
    assert_eq!(smooth.corner_normals.len(), smooth.mesh.indices.len());
    assert!(smooth.corner_normals.iter().all(|corners| corners[0] != simple3d_geom::Vec3::ZERO));
}

#[test]
pub(crate) fn a_smooth_cylinder_loses_its_facet_lines_but_keeps_its_rims() {
    // 22.5 degree facets: lined at the flat crease of 20 degrees, not at the smooth one of 30.
    let cylinder = from_seven(primitives::cylinder_mesh(20.0, 20.0, 20.0, 16));
    let flat = Renderable::prepare_node(&cylinder, false, &BTreeSet::new());
    let smooth = Renderable::prepare_node(&cylinder, false, &seven());
    assert_eq!(flat.edges.len(), 16 * 3, "two rims and a line per facet");
    assert_eq!(smooth.edges.len(), 16 * 2, "only the two rims");
    assert_eq!(smooth.edge_faces.len(), smooth.edges.len());
}

#[test]
pub(crate) fn a_smooth_box_stays_sharp() {
    let cube = from_seven(primitives::box_mesh(20.0, 20.0, 20.0));
    let smooth = Renderable::prepare_node(&cube, false, &seven());
    assert_eq!(smooth.edges.len(), 12);
    for (index, corners) in smooth.corner_normals.iter().enumerate() {
        for n in corners {
            assert!((*n - smooth.normals[index]).length() < 1e-9, "a box corner leaned off its face");
        }
    }
}

/// The distinct colours of the model's pixels.
fn shades(prepared: &Renderable) -> usize {
    let req = request(vec![Item { renderable: prepared, style: Style::Solid }], DisplayMode::Shaded);
    let frame = render(&req);
    let mut colours: Vec<&[u8]> = (0..frame.width * frame.height)
        .filter(|&i| !is_background(&frame, i, &req.palette))
        .map(|i| &frame.color[i * 4..i * 4 + 3])
        .collect();
    colours.sort_unstable();
    colours.dedup();
    colours.len()
}

#[test]
pub(crate) fn a_smooth_ball_is_drawn_in_blended_shades_rather_than_one_per_facet() {
    let ball = from_seven(primitives::ellipsoid_mesh(60.0, 60.0, 60.0, 12));
    let flat = Renderable::prepare_node(&ball, false, &BTreeSet::new());
    let smooth = Renderable::prepare_node(&ball, false, &seven());
    let req = request(vec![Item { renderable: &smooth, style: Style::Solid }], DisplayMode::Shaded);
    let steps = prepare(&req);
    assert!(steps.iter().any(|step| matches!(step, Step::Blended { .. })));
    assert!(!steps.iter().any(|step| matches!(step, Step::Triangle { .. })), "a smooth face was drawn flat");
    let (flat, smooth) = (shades(&flat), shades(&smooth));
    assert!(smooth > flat * 3, "{smooth} shades smooth against {flat} flat");
}
