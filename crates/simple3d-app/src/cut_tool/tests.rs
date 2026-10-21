use super::*;
use crate::app::tests::two_boxes;
use crate::app::App;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Open the tool on a picked plane, as a click on a face would leave it.
fn pick(app: &mut App, point: Vec3, normal: Vec3) {
    // Evaluated first, as the app is by the time a face can be clicked.
    app.reevaluate_for_test();
    if app.cut_tool.is_none() {
        app.toggle_cut_tool();
    }
    app.cut_tool.as_mut().unwrap().face = Some(CutPlane { point, normal: normal.normalized() });
}

/// Press Cut and wait for the halves to land.
fn cut(app: &mut App) {
    app.start_cut();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while app.cut_job.is_some() {
        app.poll_cut();
        assert!(std::time::Instant::now() < deadline, "the cut never finished");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    app.reevaluate_for_test();
}

/// The world boxes of a split's pieces along `axis`, low end first.
fn spans(app: &App, split: NodeId, axis: usize) -> Vec<(f64, f64)> {
    let mut spans: Vec<(f64, f64)> = app
        .scene
        .node(split)
        .children
        .iter()
        .map(|piece| app.evaluated.node_world_bounds[piece])
        .map(|(lo, hi)| (lo.get(axis), hi.get(axis)))
        .collect();
    spans.sort_by(|a, b| a.partial_cmp(b).unwrap());
    spans
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn an_endless_plane_cuts_only_the_objects_it_runs_through_into_a_split_of_two() {
    let (mut app, a, b) = two_boxes("cut-endless", v(40.0, 0.0, 0.0));
    pick(&mut app, v(5.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
    let planned: Vec<NodeId> = app.cut_planned().iter().map(|p| p.node).collect();
    assert_eq!(planned, vec![a], "the plane at x = 5 runs through the first box only");
    cut(&mut app);
    assert!(app.cut_tool.is_none(), "Cut puts the tool away");
    assert!(!app.scene.contains(a), "the box was not replaced");
    assert!(app.scene.contains(b), "the box the plane missed was touched");
    let split = app.scene.node(app.scene.root()).children[0];
    assert!(app.scene.node(split).is_split());
    let spans = spans(&app, split, 0);
    assert_eq!(spans.len(), 2);
    assert!(
        close(spans[0].0, -10.0) && close(spans[0].1, 5.0) && close(spans[1].0, 5.0) && close(spans[1].1, 10.0),
        "{spans:?}"
    );
}

#[test]
fn joining_a_cut_object_puts_it_back_whole() {
    let (mut app, _, _) = two_boxes("cut-join", v(40.0, 0.0, 0.0));
    pick(&mut app, v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
    cut(&mut app);
    let split = app.scene.node(app.scene.root()).children[0];
    assert!(app.scene.node(split).is_split(), "nothing was cut");
    app.select_only(split);
    app.rejoin_selection();
    app.reevaluate_for_test();
    let joined = app.scene.node(app.scene.root()).children[0];
    assert!(!app.scene.node(joined).is_split());
    assert_eq!(app.scene.node(joined).spec().map(|spec| spec.type_id), Some("box"));
}

#[test]
fn a_limited_area_cuts_only_objects_whose_whole_cut_lies_inside_it() {
    let (mut app, a, b) = two_boxes("cut-area", v(40.0, 0.0, 0.0));
    // Through both boxes along Y; the area round the origin holds only the first one's cut.
    pick(&mut app, v(0.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
    let mut both: Vec<NodeId> = app.cut_planned().iter().map(|p| p.node).collect();
    both.sort_unstable();
    assert_eq!(both, vec![a.min(b), a.max(b)], "endless, the plane cuts both");
    let tool = app.cut_tool.as_mut().unwrap();
    tool.limited = true;
    tool.size = [30.0, 30.0];
    let planned: Vec<NodeId> = app.cut_planned().iter().map(|p| p.node).collect();
    assert_eq!(planned, vec![a]);
    // An area narrower than the box's cut leaves it whole rather than cutting part of it.
    app.cut_tool.as_mut().unwrap().size = [10.0, 30.0];
    assert!(app.cut_planned().is_empty());
}

#[test]
fn cutting_a_cut_object_again_halves_its_pieces_within_its_split() {
    let (mut app, _, _) = two_boxes("cut-again", v(40.0, 0.0, 0.0));
    pick(&mut app, v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
    cut(&mut app);
    let split = app.scene.node(app.scene.root()).children[0];
    pick(&mut app, v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
    assert_eq!(app.cut_planned().len(), 3, "the level plane crosses both halves and the other box");
    cut(&mut app);
    assert_eq!(app.scene.node(app.scene.root()).children[0], split, "the split was replaced");
    let pieces = &app.scene.node(split).children;
    assert_eq!(pieces.len(), 4);
    let name = app.scene.node(split).name.clone();
    let names: Vec<String> = pieces.iter().map(|&p| app.scene.node(p).name.clone()).collect();
    assert_eq!(names, (1..=4).map(|n| format!("{name} Piece {n}")).collect::<Vec<_>>());
    let spans = spans(&app, split, 2);
    assert!(
        spans.iter().all(|&(lo, hi)| close(lo, -10.0) && close(hi, 0.0) || close(lo, 0.0) && close(hi, 10.0)),
        "{spans:?}"
    );
}

#[test]
fn a_negative_part_is_not_cut() {
    let (mut app, a, b) = two_boxes("cut-negative", v(5.0, 0.0, 0.0));
    app.scene.get_mut(b).unwrap().negative = true;
    app.reevaluate_for_test();
    pick(&mut app, v(3.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
    let planned: Vec<NodeId> = app.cut_planned().iter().map(|p| p.node).collect();
    assert_eq!(planned, vec![a]);
}

#[test]
fn a_plane_lying_on_an_objects_face_leaves_it_whole() {
    let (mut app, _, _) = two_boxes("cut-on-face", v(40.0, 0.0, 0.0));
    pick(&mut app, v(0.0, 0.0, 10.0), v(0.0, 0.0, 1.0));
    assert!(app.cut_planned().is_empty(), "the boxes' tops only touch the plane");
}
