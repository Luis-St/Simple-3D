//! A small software rasterizer with a depth buffer.
//!
//! The viewport is drawn on the CPU rather than through the GPU. That is a
//! deliberate response to two hard constraints: the preview must work on
//! integrated graphics and the application must "degrade gracefully rather than
//! refuse to start if accelerated rendering is unavailable" (spec section 2.7,
//! acceptance criterion 19). A software path has no shader compilation, no
//! driver feature detection and no fallback to write -- it simply always works,
//! and the window itself is the only thing that needs a graphics backend.
//!
//! Depth is stored as a *key* that is linear in screen space for the projection
//! in use, and larger always means nearer: `1/z` under perspective (where `1/z`
//! interpolates linearly) and `-z` under orthographic (where `z` does). That
//! keeps one comparison for both projections.

/// Straight-alpha RGBA.
pub type Rgba = [u8; 4];

pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// RGBA, row-major from the top left -- the layout `egui::ColorImage` wants.
    pub color: Vec<u8>,
    key: Vec<f32>,
}

/// One projected vertex: screen position and depth key.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: egui::Pos2,
    pub key: f32,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Frame {
        Frame {
            width,
            height,
            color: vec![0; width * height * 4],
            key: vec![f32::NEG_INFINITY; width * height],
        }
    }

    pub fn clear(&mut self, background: Rgba) {
        for pixel in self.color.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);
        }
        self.key.fill(f32::NEG_INFINITY);
    }

    /// Depth-tested, optionally alpha-blended write. `write_depth` is false for
    /// translucent passes, so ghosts do not hide each other.
    fn put(&mut self, x: usize, y: usize, key: f32, rgba: Rgba, write_depth: bool) {
        let i = y * self.width + x;
        if key <= self.key[i] {
            return;
        }
        let o = i * 4;
        if rgba[3] == 255 {
            self.color[o..o + 4].copy_from_slice(&rgba);
        } else {
            let a = rgba[3] as u32;
            for c in 0..3 {
                let src = rgba[c] as u32 * a;
                let dst = self.color[o + c] as u32 * (255 - a);
                self.color[o + c] = ((src + dst) / 255) as u8;
            }
            self.color[o + 3] = 255;
        }
        if write_depth {
            self.key[i] = key;
        }
    }

    /// Fill a screen-space triangle. Vertices may be in either winding order;
    /// back-face culling is the caller's decision, made in world space where it
    /// is meaningful.
    pub fn triangle(&mut self, v: [Vertex; 3], rgba: Rgba, write_depth: bool) {
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);
        let area = edge(p0, p1, p2);
        if area.abs() < 1e-9 {
            return;
        }
        // Work with a consistent winding so the inside test is a single sign.
        let (v, area) = if area < 0.0 { ([v[0], v[2], v[1]], -area) } else { (v, area) };
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);

        let min_x = p0.x.min(p1.x).min(p2.x).floor().max(0.0) as usize;
        let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as isize).clamp(0, self.width as isize - 1) as usize;
        let min_y = p0.y.min(p1.y).min(p2.y).floor().max(0.0) as usize;
        let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as isize).clamp(0, self.height as isize - 1) as usize;
        if min_x > max_x || min_y > max_y {
            return;
        }

        let inv_area = 1.0 / area;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = edge(p1, p2, p);
                let w1 = edge(p2, p0, p);
                let w2 = edge(p0, p1, p);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let key = (w0 * v[0].key + w1 * v[1].key + w2 * v[2].key) * inv_area;
                self.put(x, y, key, rgba, write_depth);
            }
        }
    }

    /// Draw a line, depth-tested. `bias` nudges it towards the eye so an edge
    /// drawn on the face it belongs to is not swallowed by it.
    ///
    /// The segment is clipped to the framebuffer *before* it is stepped along,
    /// rather than tested per pixel. That matters for the origin axes and the
    /// ground grid, whose lines run far outside the viewport: stepping them
    /// end to end would cost thousands of rejected samples each.
    pub fn line(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32) {
        let Some((a, b)) = self.clip_to_frame(a, b) else { return };
        let steps = ((b.pos.x - a.pos.x).abs().max((b.pos.y - a.pos.y).abs()).ceil() as usize).max(1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = a.pos.x + (b.pos.x - a.pos.x) * t;
            let y = a.pos.y + (b.pos.y - a.pos.y) * t;
            if x < 0.0 || y < 0.0 {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            if x >= self.width || y >= self.height {
                continue;
            }
            let key = a.key + (b.key - a.key) * t;
            self.put(x, y, key + bias, rgba, true);
        }
    }

    /// Liang-Barsky clip of a segment against the framebuffer rectangle,
    /// interpolating the depth key along with the position.
    fn clip_to_frame(&self, a: Vertex, b: Vertex) -> Option<(Vertex, Vertex)> {
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        let dx = b.pos.x - a.pos.x;
        let dy = b.pos.y - a.pos.y;
        // The framebuffer's last addressable pixel centre, with a little slack so
        // a line exactly on the edge still draws.
        let limits = [
            (-dx, a.pos.x - 0.0),
            (dx, (self.width as f32 - 0.001) - a.pos.x),
            (-dy, a.pos.y - 0.0),
            (dy, (self.height as f32 - 0.001) - a.pos.y),
        ];
        for (p, q) in limits {
            if p == 0.0 {
                if q < 0.0 {
                    return None; // parallel to this edge and outside it
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    if r > t1 {
                        return None;
                    }
                    t0 = t0.max(r);
                } else {
                    if r < t0 {
                        return None;
                    }
                    t1 = t1.min(r);
                }
            }
        }
        if t0 > t1 {
            return None;
        }
        let at = |t: f32| Vertex {
            pos: egui::pos2(a.pos.x + dx * t, a.pos.y + dy * t),
            key: a.key + (b.key - a.key) * t,
        };
        Some((at(t0), at(t1)))
    }

    /// Hand the framebuffer to egui as a texture image.
    pub fn to_color_image(&self) -> egui::ColorImage {
        egui::ColorImage::from_rgba_unmultiplied([self.width, self.height], &self.color)
    }

    #[cfg(test)]
    fn pixel(&self, x: usize, y: usize) -> Rgba {
        let o = (y * self.width + x) * 4;
        [self.color[o], self.color[o + 1], self.color[o + 2], self.color[o + 3]]
    }
}

fn edge(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Clip a view-space triangle against the near plane, so geometry the camera is
/// inside does not wrap around and smear across the screen. Returns 0, 1 or 2
/// triangles.
pub fn clip_near(tri: [scadstudio_geom::Vec3; 3], near: f64) -> Vec<[scadstudio_geom::Vec3; 3]> {
    let inside: Vec<bool> = tri.iter().map(|v| v.z >= near).collect();
    let count = inside.iter().filter(|&&i| i).count();
    match count {
        0 => Vec::new(),
        3 => vec![tri],
        _ => {
            let lerp = |a: scadstudio_geom::Vec3, b: scadstudio_geom::Vec3| {
                let t = (near - a.z) / (b.z - a.z);
                a.lerp(b, t)
            };
            if count == 1 {
                let i = inside.iter().position(|&x| x).unwrap();
                let (a, b, c) = (tri[i], tri[(i + 1) % 3], tri[(i + 2) % 3]);
                vec![[a, lerp(a, b), lerp(a, c)]]
            } else {
                // Two vertices in front: the quad that remains needs two triangles.
                let i = inside.iter().position(|&x| !x).unwrap();
                let (out, a, b) = (tri[i], tri[(i + 1) % 3], tri[(i + 2) % 3]);
                let na = lerp(a, out);
                let nb = lerp(b, out);
                vec![[a, b, nb], [a, nb, na]]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_geom::Vec3;

    const RED: Rgba = [255, 0, 0, 255];
    const BLUE: Rgba = [0, 0, 255, 255];
    const BG: Rgba = [0, 0, 0, 255];

    fn vertex(x: f32, y: f32, key: f32) -> Vertex {
        Vertex { pos: egui::pos2(x, y), key }
    }

    #[test]
    fn a_triangle_covers_its_interior_and_nothing_outside_it() {
        let mut frame = Frame::new(32, 32);
        frame.clear(BG);
        frame.triangle([vertex(2.0, 2.0, 1.0), vertex(28.0, 2.0, 1.0), vertex(2.0, 28.0, 1.0)], RED, true);
        assert_eq!(frame.pixel(5, 5), RED, "inside the triangle");
        assert_eq!(frame.pixel(26, 26), BG, "outside the hypotenuse");
        assert_eq!(frame.pixel(31, 31), BG, "outside the bounding box");
    }

    #[test]
    fn winding_order_does_not_change_coverage() {
        let make = |flip: bool| {
            let mut frame = Frame::new(16, 16);
            frame.clear(BG);
            let v = [vertex(1.0, 1.0, 1.0), vertex(14.0, 1.0, 1.0), vertex(1.0, 14.0, 1.0)];
            let v = if flip { [v[0], v[2], v[1]] } else { v };
            frame.triangle(v, RED, true);
            frame.color.clone()
        };
        assert_eq!(make(false), make(true));
    }

    #[test]
    fn the_nearer_triangle_wins_regardless_of_draw_order() {
        for far_first in [true, false] {
            let mut frame = Frame::new(16, 16);
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
        let mut frame = Frame::new(8, 8);
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
        let mut frame = Frame::new(16, 16);
        frame.clear(BG);
        frame.triangle([vertex(-100.0, -100.0, 1.0), vertex(200.0, -50.0, 1.0), vertex(-50.0, 200.0, 1.0)], RED, true);
        // No panic, and the covered part is filled.
        assert_eq!(frame.pixel(1, 1), RED);
    }

    #[test]
    fn a_degenerate_triangle_draws_nothing() {
        let mut frame = Frame::new(8, 8);
        frame.clear(BG);
        frame.triangle([vertex(1.0, 1.0, 1.0), vertex(5.0, 1.0, 1.0), vertex(3.0, 1.0, 1.0)], RED, true);
        assert!(frame.color.chunks_exact(4).all(|p| p == BG), "a zero-area triangle painted something");
    }

    #[test]
    fn lines_reach_both_endpoints() {
        let mut frame = Frame::new(16, 16);
        frame.clear(BG);
        frame.line(vertex(2.0, 8.0, 1.0), vertex(13.0, 8.0, 1.0), RED, 0.0);
        assert_eq!(frame.pixel(2, 8), RED);
        assert_eq!(frame.pixel(13, 8), RED);
        assert_eq!(frame.pixel(7, 8), RED);
        assert_eq!(frame.pixel(7, 9), BG);
    }

    #[test]
    fn an_edge_biased_towards_the_eye_draws_over_its_own_face() {
        let mut frame = Frame::new(16, 16);
        frame.clear(BG);
        frame.triangle([vertex(0.0, 0.0, 1.0), vertex(16.0, 0.0, 1.0), vertex(0.0, 16.0, 1.0)], RED, true);
        frame.line(vertex(0.0, 4.0, 1.0), vertex(8.0, 4.0, 1.0), BLUE, 0.01);
        assert_eq!(frame.pixel(4, 4), BLUE, "the edge was swallowed by its face");
    }

    #[test]
    fn a_line_biased_away_from_the_eye_loses_a_depth_tie() {
        // How the ground grid gets out of the way of geometry it is coplanar with.
        let mut frame = Frame::new(16, 16);
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
        let mut frame = Frame::new(32, 32);
        frame.clear(BG);
        frame.line(vertex(-40000.0, 16.0, 1.0), vertex(40000.0, 16.0, 1.0), RED, 0.0);
        assert_eq!(frame.pixel(0, 16), RED);
        assert_eq!(frame.pixel(31, 16), RED);
        assert_eq!(frame.pixel(16, 16), RED);
    }

    #[test]
    fn a_triangle_entirely_in_front_of_the_near_plane_is_untouched() {
        let tri = [Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 5.0), Vec3::new(0.0, 1.0, 6.0)];
        let out = clip_near(tri, 0.05);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], tri);
    }

    #[test]
    fn a_triangle_entirely_behind_the_near_plane_is_dropped() {
        let tri = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -2.0), Vec3::new(0.0, 1.0, -0.5)];
        assert!(clip_near(tri, 0.05).is_empty());
    }

    #[test]
    fn clipping_keeps_every_result_in_front_of_the_near_plane() {
        let near = 0.05;
        // One vertex in front, then two.
        for tri in [
            [Vec3::new(0.0, 0.0, 4.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 1.0, -2.0)],
            [Vec3::new(0.0, 0.0, 4.0), Vec3::new(1.0, 0.0, 3.0), Vec3::new(0.0, 1.0, -2.0)],
        ] {
            let out = clip_near(tri, near);
            assert!(!out.is_empty());
            for piece in &out {
                for v in piece {
                    assert!(v.z >= near - 1e-9, "clipped vertex still behind: {v:?}");
                }
                // And each piece has real area, so it will actually rasterize.
                let n = (piece[1] - piece[0]).cross(piece[2] - piece[0]);
                assert!(n.length() > 1e-9, "clipping produced a degenerate triangle");
            }
        }
        assert_eq!(clip_near([Vec3::new(0.0, 0.0, 4.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 1.0, -2.0)], near).len(), 1);
        assert_eq!(clip_near([Vec3::new(0.0, 0.0, 4.0), Vec3::new(1.0, 0.0, 3.0), Vec3::new(0.0, 1.0, -2.0)], near).len(), 2);
    }

    #[test]
    fn clipping_two_in_front_preserves_the_winding() {
        let near = 0.05;
        let tri = [Vec3::new(0.0, 0.0, 4.0), Vec3::new(2.0, 0.0, 4.0), Vec3::new(0.0, 2.0, -2.0)];
        let original = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalized();
        for piece in clip_near(tri, near) {
            let n = (piece[1] - piece[0]).cross(piece[2] - piece[0]).normalized();
            assert!(n.dot(original) > 0.9, "winding flipped: {n:?} vs {original:?}");
        }
    }

    #[test]
    fn clearing_resets_both_colour_and_depth() {
        let mut frame = Frame::new(8, 8);
        frame.clear(BG);
        frame.triangle([vertex(0.0, 0.0, 5.0), vertex(8.0, 0.0, 5.0), vertex(0.0, 8.0, 5.0)], BLUE, true);
        frame.clear(BG);
        assert_eq!(frame.pixel(1, 1), BG);
        // A far triangle now draws, which it would not if depth had survived.
        frame.triangle([vertex(0.0, 0.0, 0.1), vertex(8.0, 0.0, 0.1), vertex(0.0, 8.0, 0.1)], RED, true);
        assert_eq!(frame.pixel(1, 1), RED);
    }
}
