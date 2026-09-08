use super::axis::*;
use super::body::*;
use super::feature::*;
use super::nearest::*;
use super::plane::*;
use simple3d_geom::Vec3;

use simple3d_geom::primitives;

fn has_point(features: &[Feature], kind: FeatureKind, p: Vec3) -> bool {
    features.iter().any(|f| f.kind == kind && (f.point - p).length() < 1e-6)
}

#[test]
fn a_box_offers_its_eight_corners_twelve_edge_midpoints_and_six_face_centres() {
    // A cube from -5 to 5 on every axis: the counts are exact once the
    // triangulated, unwelded mesh has been welded and its faces merged.
    let mesh = primitives::box_mesh(10.0, 10.0, 10.0);
    let features = features_of(&mesh);

    let count = |kind: FeatureKind| features.iter().filter(|f| f.kind == kind).count();
    assert_eq!(count(FeatureKind::Vertex), 8, "a box has eight corners, not three per triangle");
    assert_eq!(count(FeatureKind::EdgeMidpoint), 12, "a box has twelve edges");
    assert_eq!(count(FeatureKind::FaceCentre), 6, "a box has six faces, not two triangles per side");

    // The named corner, edge midpoint and face centre are all really there.
    assert!(has_point(&features, FeatureKind::Vertex, Vec3::new(5.0, 5.0, 5.0)));
    assert!(has_point(&features, FeatureKind::EdgeMidpoint, Vec3::new(0.0, 5.0, 5.0)));
    assert!(has_point(&features, FeatureKind::FaceCentre, Vec3::new(0.0, 0.0, 5.0)), "the top face centre is missing");
}

#[test]
fn a_face_centre_sits_in_the_middle_of_the_face_not_at_a_triangles_centroid() {
    // The whole reason to merge coplanar triangles: a triangle centroid of
    // the top face would be at (+/-, +/-, 5) off toward a corner, never at
    // its middle.
    let mesh = primitives::box_mesh(20.0, 8.0, 4.0);
    let features = features_of(&mesh);
    assert!(
        has_point(&features, FeatureKind::FaceCentre, Vec3::new(0.0, 0.0, 2.0)),
        "the top face centre was not at the middle of the face"
    );
    assert!(has_point(&features, FeatureKind::FaceCentre, Vec3::new(10.0, 0.0, 0.0)), "the +X face centre is missing");
}

#[test]
fn the_features_on_screen_are_the_ones_under_the_cursor_within_reach() {
    let features = vec![
        Feature::point(Vec3::new(0.0, 0.0, 0.0), FeatureKind::Vertex),
        Feature::point(Vec3::new(100.0, 0.0, 0.0), FeatureKind::Vertex),
        Feature::point(Vec3::new(6.0, 0.0, 0.0), FeatureKind::FaceCentre),
    ];
    // A trivial orthographic-ish projection: X and Y straight to screen.
    let project = |p: Vec3| Some(egui::pos2(p.x as f32, p.y as f32));

    // Both of the two in reach come back, each with how far off it is, so a
    // caller that will not take the nearest can work outward from it.
    let cursor = egui::pos2(3.0, 2.0);
    let found = near_on_screen(&features, project, cursor, 10.0);
    assert_eq!(found.len(), 2, "the far corner was within reach: {found:?}");
    let (nearest, distance) = found.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    assert_eq!(nearest.point, Vec3::ZERO);
    assert!((distance - (3.0f32 * 3.0 + 2.0 * 2.0).sqrt()).abs() < 1e-4);

    // Nothing within reach returns nothing, which is how a drag knows to fall
    // back to the grid.
    assert!(near_on_screen(&features, project, egui::pos2(50.0, 50.0), 10.0).is_empty());
}

#[test]
fn an_edge_feature_carries_the_edge_it_is_the_middle_of() {
    // Issue 78: catching an edge anywhere along it needs its two ends, not
    // just the midpoint the feature reports.
    let mesh = primitives::box_mesh(10.0, 10.0, 10.0);
    let features = features_of(&mesh);
    let edges: Vec<&Feature> = features.iter().filter(|f| f.kind == FeatureKind::EdgeMidpoint).collect();
    assert_eq!(edges.len(), 12);
    for edge in edges {
        let (a, b) = edge.span.expect("an edge midpoint without its edge");
        assert!(((a + b) * 0.5 - edge.point).length() < 1e-9, "the midpoint is not the middle of its span");
        assert!((a - b).length() > 1.0, "a degenerate edge");
    }
    // The other kinds are points and nothing more.
    assert!(features.iter().filter(|f| f.kind != FeatureKind::EdgeMidpoint).all(|f| f.span.is_none()));
}

#[test]
fn a_point_part_way_along_an_edge_is_caught_at_the_place_pointed_at() {
    let project = |p: Vec3| Some(egui::pos2(p.x as f32, p.y as f32));
    let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0));

    // A quarter of the way along, three pixels off the line: the caught point
    // is the one under the pointer, not either end and not the midpoint.
    let (at, distance) = nearest_on_edge(a, b, project, egui::pos2(25.0, 3.0), 10.0).unwrap();
    assert!((at - Vec3::new(25.0, 0.0, 0.0)).length() < 1e-6, "caught {at:?}");
    assert!((distance - 3.0).abs() < 1e-4);

    // Past an end it clamps to that end rather than running off the edge.
    let (at, _) = nearest_on_edge(a, b, project, egui::pos2(-40.0, 0.0), 100.0).unwrap();
    assert!((at - a).length() < 1e-6, "the catch ran off the end of the edge");

    // Out of reach across the line catches nothing.
    assert!(nearest_on_edge(a, b, project, egui::pos2(25.0, 40.0), 10.0).is_none());
}

#[test]
fn the_axes_cross_a_body_at_points_that_can_be_caught() {
    // Issue 78: a box straddling the origin is crossed by all three axes, and
    // each crossing is a corner with the run between a pair as an edge.
    let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
    let features = axis_features(&mesh, [true, true, true]);

    let crossings: Vec<&Feature> = features.iter().filter(|f| f.kind == FeatureKind::AxisCrossing).collect();
    assert_eq!(crossings.len(), 6, "three axes in and out of the box: {crossings:?}");
    for expected in [
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(-10.0, 0.0, 0.0),
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(0.0, -5.0, 0.0),
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::new(0.0, 0.0, -3.0),
    ] {
        assert!(
            crossings.iter().any(|f| (f.point - expected).length() < 1e-6),
            "no crossing at {expected:?} among {crossings:?}"
        );
    }

    // The crossings and nothing else: the run between them is inside the
    // material, where the line is cut out of the drawing, so there is
    // nothing there to catch.
    assert_eq!(features.len(), crossings.len(), "the stretch inside the body was offered as a snap target");
}

#[test]
fn a_plane_cuts_a_triangle_in_at_most_one_segment() {
    let above = [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 2.0), Vec3::new(0.0, 1.0, 3.0)];
    assert!(plane_crossing(above, 2).is_none());
    let crossing = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(2.0, 0.0, 1.0), Vec3::new(0.0, 2.0, 1.0)];
    let (a, b) = plane_crossing(crossing, 2).expect("this triangle straddles z = 0");
    assert!(a.z.abs() < 1e-9 && b.z.abs() < 1e-9, "the cut has to lie in the plane: {a:?} {b:?}");
    // A triangle lying in the plane is left to its neighbours: its own
    // edges are the mark, and it has no interior crossing.
    let flat = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    assert!(plane_crossing(flat, 2).is_none());
}

#[test]
fn a_body_the_principal_planes_cut_carries_their_marks_as_lines() {
    // The mark is what the renderer draws on the solid, so what can be
    // caught is what is on screen: a segment per triangle the plane cuts,
    // every one of them lying in that plane.
    let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
    for switch in 0..3 {
        let mut axes = [false; 3];
        axes[switch] = true;
        // Which plane that switch governs -- X's and Y's are exchanged
        // (issue 75), so this is not the switch's own axis for two of three.
        let axis = MARK_AXIS[switch];
        let lines = plane_mark_lines(&mesh, axes);
        assert!(!lines.is_empty(), "the plane perpendicular to axis {axis} cuts this box and marked nothing");
        for (a, b) in &lines {
            assert!(
                component(*a, axis).abs() < 1e-9 && component(*b, axis).abs() < 1e-9,
                "a mark off its own plane: {a:?} {b:?}"
            );
            assert!((*a - *b).length() > 1e-9, "a mark of no length");
        }
    }
    // All three at once is all three marks, and none of them without a plane
    // switched on.
    assert_eq!(
        plane_mark_lines(&mesh, [true; 3]).len(),
        (0..3).map(|axis| plane_mark_lines(&mesh, [axis == 0, axis == 1, axis == 2]).len()).sum::<usize>()
    );
    assert!(plane_mark_lines(&mesh, [false; 3]).is_empty());

    // A body no plane reaches has no mark on it, rather than a line hanging
    // in the air beside it.
    let away = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 100.0, 100.0));
    assert!(plane_mark_lines(&away, [true; 3]).is_empty());
}

#[test]
fn the_shown_axes_are_offered_as_lines_to_catch() {
    let lines = axis_lines([true, true, true], 500.0);
    assert_eq!(lines.len(), 3);
    for (i, (a, b)) in lines.iter().enumerate() {
        // Each runs through the origin, both ways, along one axis only.
        assert!(((*a + *b) * 0.5).length() < 1e-9, "axis {i} is not centred on the origin");
        assert!((a.length() - 500.0).abs() < 1e-9 && (b.length() - 500.0).abs() < 1e-9);
    }
    assert_eq!(axis_lines([false, true, false], 10.0), vec![(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, 10.0, 0.0))]);
    assert!(axis_lines([false; 3], 10.0).is_empty());
}

#[test]
fn an_axis_that_is_turned_off_or_misses_the_body_offers_nothing() {
    let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
    // Only Z is shown: only Z's two crossings and its one run.
    let features = axis_features(&mesh, [false, false, true]);
    assert_eq!(features.iter().filter(|f| f.kind == FeatureKind::AxisCrossing).count(), 2);
    assert!(features.iter().all(|f| f.point.x.abs() < 1e-9 && f.point.y.abs() < 1e-9));
    assert!(axis_features(&mesh, [false; 3]).is_empty());

    // A body the axes miss entirely has no crossings, rather than points
    // conjured somewhere near it.
    let away = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 100.0, 100.0));
    assert!(axis_features(&away, [true, true, true]).is_empty());
}
