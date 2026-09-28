use super::*;
use simple3d_core::xform::Xform;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// A 10 mm cube with its low corner at `at`, placed directly in the world.
fn cube(id: NodeId, at: Vec3) -> Subject {
    Subject {
        id,
        bounds: (at, at + v(10.0, 10.0, 10.0)),
        parent: Xform::IDENTITY,
        position: at + v(5.0, 5.0, 5.0),
        rotation: Vec3::ZERO,
    }
}

fn tool(targets: usize, mode: Arrange) -> ArrangeTool {
    ArrangeTool { mode, ..ArrangeTool::on((1..=targets as NodeId).collect(), Some(targets as NodeId)) }
}

#[test]
fn aligning_by_the_minimum_moves_every_box_to_the_lowest_one_on_that_axis_only() {
    let subjects = [cube(1, v(0.0, 0.0, 0.0)), cube(2, v(30.0, 5.0, 0.0)), cube(3, v(60.0, -8.0, 4.0))];
    let mut tool = tool(3, Arrange::Align);
    tool.align[1] = Some(Side::Min);
    let placed = plan(&tool, &subjects).unwrap();
    let ys: Vec<f64> = placed.iter().map(|p| p.position.y).collect();
    assert_eq!(ys, vec![-3.0, -3.0, -3.0], "the boxes' low Y sides do not meet at -8");
    // X and Z are left alone, and nothing is copied.
    assert_eq!(placed[1].position.x, 35.0);
    assert_eq!(placed[2].position.z, 9.0);
    assert!(placed.iter().all(|p| !p.copy));
    // The lowest box is already there, so it needs no template.
    assert!(!placed[2].moves());
}

#[test]
fn aligning_to_the_last_selected_holds_that_one_still() {
    let subjects = [cube(1, v(0.0, 0.0, 0.0)), cube(2, v(30.0, 20.0, 0.0))];
    let mut tool = tool(2, Arrange::Align);
    tool.align[1] = Some(Side::Centre);
    tool.to_key = true;
    let placed = plan(&tool, &subjects).unwrap();
    assert_eq!(placed[0].position.y, 25.0, "the first box did not come to the key's centre");
    assert_eq!(placed[1].position.y, 25.0, "the key moved");
    tool.align[1] = None;
    assert!(plan(&tool, &subjects).is_err(), "aligning on no axis was not refused");
}

#[test]
fn distributing_equal_gaps_keeps_the_outer_two_and_evens_out_the_space_between() {
    // Sizes 10, 20 and 10 from 0 to 100: 60 mm of space, 30 mm per gap.
    let mut wide = cube(2, v(15.0, 0.0, 0.0));
    wide.bounds.1.x = 35.0;
    wide.position.x = 25.0;
    let subjects = [cube(1, v(0.0, 0.0, 0.0)), cube(3, v(90.0, 0.0, 0.0)), wide];
    let placed = plan(&tool(3, Arrange::Distribute), &subjects).unwrap();
    assert_eq!(placed[0].position.x, 5.0);
    assert_eq!(placed[1].position.x, 95.0);
    // The wide box's low side at 40, 30 mm after the first, and its centre ten further.
    assert_eq!(placed[2].position.x, 50.0);

    let mut centres = tool(3, Arrange::Distribute);
    centres.spacing = Spacing::Centres;
    assert_eq!(plan(&centres, &subjects).unwrap()[2].position.x, 50.0);
    // Two cannot be evened out, but they can be spaced by a set gap.
    assert!(plan(&centres, &subjects[..2]).is_err());
    let mut fixed = tool(2, Arrange::Distribute);
    fixed.spacing = Spacing::Fixed;
    fixed.gap = 2.0;
    assert_eq!(plan(&fixed, &subjects[..2]).unwrap()[1].position.x, 17.0);
}

#[test]
fn one_object_along_a_path_with_a_larger_count_makes_copies_evenly_spread() {
    let mut tool = tool(1, Arrange::Path);
    tool.source = PathSource::Drawn;
    tool.points = vec![v(0.0, 0.0, 0.0), v(40.0, 0.0, 0.0)];
    tool.count = 5;
    let placed = plan(&tool, &[cube(1, v(-50.0, 0.0, 0.0))]).unwrap();
    assert_eq!(placed.len(), 5);
    assert!(!placed[0].copy && placed[1..].iter().all(|p| p.copy), "not the original followed by copies");
    // The box's centre lands on the path every 10 mm, and every copy has a template.
    let xs: Vec<f64> = placed.iter().map(|p| p.position.x).collect();
    assert_eq!(xs, vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    assert!(placed.iter().all(Placement::moves));
    assert_eq!(placed[3].world.point(v(-45.0, 5.0, 5.0)), v(30.0, 0.0, 0.0), "the template is not carried there");
}

#[test]
fn following_a_path_round_a_corner_turns_the_object_about_its_centre() {
    let mut tool = tool(1, Arrange::Path);
    tool.source = PathSource::Drawn;
    tool.points = vec![v(0.0, 0.0, 0.0), v(20.0, 0.0, 0.0), v(20.0, 20.0, 0.0)];
    tool.count = 3;
    tool.follow = true;
    // A box offset from its own origin, so a turn about the wrong point would show.
    let mut subject = cube(1, v(0.0, 0.0, 0.0));
    subject.position = v(0.0, 0.0, 0.0);
    let placed = plan(&tool, &[subject]).unwrap();
    assert_eq!(placed[0].rotation.z, 0.0, "the first object turned");
    assert!((placed[2].rotation.z - 90.0).abs() < 1e-9, "the last did not turn with the path");
    // Its centre still lands on the path's end.
    let centre = placed[2].world.point(v(5.0, 5.0, 5.0));
    assert!((centre - v(20.0, 20.0, 0.0)).length() < 1e-9, "{centre:?}");
}

#[test]
fn a_path_with_no_length_is_refused_with_what_to_do() {
    let tool = tool(2, Arrange::Path);
    let why = plan(&tool, &[cube(1, Vec3::ZERO), cube(2, v(20.0, 0.0, 0.0))]).unwrap_err();
    assert!(why.contains("edges"), "{why}");
}
