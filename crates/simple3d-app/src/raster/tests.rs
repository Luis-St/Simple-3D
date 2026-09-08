use super::*;

const RED: Rgba = [255, 0, 0, 255];
const BLUE: Rgba = [0, 0, 255, 255];
const BG: Rgba = [0, 0, 0, 255];

fn vertex(x: f32, y: f32, key: f32) -> Vertex {
    Vertex { pos: egui::pos2(x, y), key }
}

#[test]
fn a_triangle_covers_its_interior_and_nothing_outside_it() {
    let mut store = vec![0u8; 32 * 32 * 4];
    let mut frame = Frame::new(&mut store, 32, 32);
    frame.clear(BG);
    frame.triangle([vertex(2.0, 2.0, 1.0), vertex(28.0, 2.0, 1.0), vertex(2.0, 28.0, 1.0)], RED, true);
    assert_eq!(frame.pixel(5, 5), RED, "inside the triangle");
    assert_eq!(frame.pixel(26, 26), BG, "outside the hypotenuse");
    assert_eq!(frame.pixel(31, 31), BG, "outside the bounding box");
}

#[test]
fn winding_order_does_not_change_coverage() {
    let make = |flip: bool| {
        let mut store = vec![0u8; 16 * 16 * 4];
        let mut frame = Frame::new(&mut store, 16, 16);
        frame.clear(BG);
        let v = [vertex(1.0, 1.0, 1.0), vertex(14.0, 1.0, 1.0), vertex(1.0, 14.0, 1.0)];
        let v = if flip { [v[0], v[2], v[1]] } else { v };
        frame.triangle(v, RED, true);
        frame.color.to_vec()
    };
    assert_eq!(make(false), make(true));
}

#[test]
fn the_nearer_triangle_wins_regardless_of_draw_order() {
    for far_first in [true, false] {
        let mut store = vec![0u8; 16 * 16 * 4];
        let mut frame = Frame::new(&mut store, 16, 16);
        frame.clear(BG);
        let far = [vertex(0.0, 0.0, 0.5), vertex(16.0, 0.0, 0.5), vertex(0.0, 16.0, 0.5)];
        let near = [vertex(0.0, 0.0, 2.0), vertex(16.0, 0.0, 2.0), vertex(0.0, 16.0, 2.0)];
        if far_first {
            frame.triangle(far, RED, true);
            frame.triangle(near, BLUE, true);
        } else {
            frame.triangle(near, BLUE, true);
            frame.triangle(far, RED, true);
        }
        assert_eq!(frame.pixel(3, 3), BLUE, "far_first={far_first}: the near triangle should win");
    }
}

#[test]
fn a_translucent_pass_blends_without_writing_depth() {
    let mut store = vec![0u8; 8 * 8 * 4];
    let mut frame = Frame::new(&mut store, 8, 8);
    frame.clear(BG);
    let quad = [vertex(0.0, 0.0, 1.0), vertex(8.0, 0.0, 1.0), vertex(0.0, 8.0, 1.0)];
    frame.triangle(quad, [255, 255, 255, 128], false);
    let blended = frame.pixel(1, 1);
    assert!(blended[0] > 100 && blended[0] < 160, "expected a half blend, got {blended:?}");
    // Depth was not written, so a second translucent pass blends again
    // rather than being rejected at equal depth.
    frame.triangle(quad, [255, 255, 255, 128], false);
    assert!(frame.pixel(1, 1)[0] > blended[0]);
}

#[test]
fn geometry_outside_the_frame_is_clipped_not_wrapped() {
    let mut store = vec![0u8; 16 * 16 * 4];
    let mut frame = Frame::new(&mut store, 16, 16);
    frame.clear(BG);
    frame.triangle([vertex(-100.0, -100.0, 1.0), vertex(200.0, -50.0, 1.0), vertex(-50.0, 200.0, 1.0)], RED, true);
    // No panic, and the covered part is filled.
    assert_eq!(frame.pixel(1, 1), RED);
}

#[test]
fn a_degenerate_triangle_draws_nothing() {
    let mut store = vec![0u8; 8 * 8 * 4];
    let mut frame = Frame::new(&mut store, 8, 8);
    frame.clear(BG);
    frame.triangle([vertex(1.0, 1.0, 1.0), vertex(5.0, 1.0, 1.0), vertex(3.0, 1.0, 1.0)], RED, true);
    assert!(frame.color.chunks_exact(4).all(|p| p == BG), "a zero-area triangle painted something");
}

#[test]
fn lines_reach_both_endpoints() {
    let mut store = vec![0u8; 16 * 16 * 4];
    let mut frame = Frame::new(&mut store, 16, 16);
    frame.clear(BG);
    frame.line(vertex(2.0, 8.0, 1.0), vertex(13.0, 8.0, 1.0), RED, 0.0);
    assert_eq!(frame.pixel(2, 8), RED);
    assert_eq!(frame.pixel(13, 8), RED);
    assert_eq!(frame.pixel(7, 8), RED);
    assert_eq!(frame.pixel(7, 9), BG);
}

#[test]
fn an_edge_biased_towards_the_eye_draws_over_its_own_face() {
    let mut store = vec![0u8; 16 * 16 * 4];
    let mut frame = Frame::new(&mut store, 16, 16);
    frame.clear(BG);
    frame.triangle([vertex(0.0, 0.0, 1.0), vertex(16.0, 0.0, 1.0), vertex(0.0, 16.0, 1.0)], RED, true);
    frame.line(vertex(0.0, 4.0, 1.0), vertex(8.0, 4.0, 1.0), BLUE, 0.01);
    assert_eq!(frame.pixel(4, 4), BLUE, "the edge was swallowed by its face");
}

#[test]
fn a_line_biased_away_from_the_eye_loses_a_depth_tie() {
    // How the ground grid gets out of the way of geometry it is coplanar with.
    let mut store = vec![0u8; 16 * 16 * 4];
    let mut frame = Frame::new(&mut store, 16, 16);
    frame.clear(BG);
    frame.line(vertex(0.0, 4.0, 1.0), vertex(15.0, 4.0, 1.0), BLUE, -0.01);
    frame.triangle([vertex(0.0, 0.0, 1.0), vertex(16.0, 0.0, 1.0), vertex(0.0, 16.0, 1.0)], RED, true);
    assert_eq!(frame.pixel(4, 4), RED, "the line survived under the face it is coplanar with");
}

#[test]
fn a_long_line_running_far_outside_the_frame_still_draws() {
    // The origin axes are thousands of units long; clipping has to happen
    // before stepping, or they cost thousands of rejected samples -- and an
    // earlier length cap made them vanish altogether.
    let mut store = vec![0u8; 32 * 32 * 4];
    let mut frame = Frame::new(&mut store, 32, 32);
    frame.clear(BG);
    frame.line(vertex(-40000.0, 16.0, 1.0), vertex(40000.0, 16.0, 1.0), RED, 0.0);
    assert_eq!(frame.pixel(0, 16), RED);
    assert_eq!(frame.pixel(31, 16), RED);
    assert_eq!(frame.pixel(16, 16), RED);
}

#[test]
fn clearing_resets_both_colour_and_depth() {
    let mut store = vec![0u8; 8 * 8 * 4];
    let mut frame = Frame::new(&mut store, 8, 8);
    frame.clear(BG);
    frame.triangle([vertex(0.0, 0.0, 5.0), vertex(8.0, 0.0, 5.0), vertex(0.0, 8.0, 5.0)], BLUE, true);
    frame.clear(BG);
    assert_eq!(frame.pixel(1, 1), BG);
    // A far triangle now draws, which it would not if depth had survived.
    frame.triangle([vertex(0.0, 0.0, 0.1), vertex(8.0, 0.0, 0.1), vertex(0.0, 8.0, 0.1)], RED, true);
    assert_eq!(frame.pixel(1, 1), RED);
}
