//! Projecting resident meshes: rows, planes, depth extents and the shared uniforms.

use super::*;

impl Resident {
    /// The projection rows (screen x, screen y, depth key) for this mesh, folded with the
    /// storage offset in double precision. When moved, they apply after `u_model`.
    fn rows(&self, view: &View, placing: &Placing) -> [[f32; 4]; 3] {
        rows_at(view, placing.xform.point(self.origin))
    }

    /// A plane as the shaders test it, positive where `Plane::depth` is.
    pub(in crate::gpu) fn plane(&self, plane: &Plane, placing: &Placing) -> [f32; 4] {
        let n = plane.normal;
        let origin = placing.xform.point(self.origin);
        [n.x as f32, n.y as f32, n.z as f32, (n.dot(origin) - plane.offset) as f32]
    }

    /// The section plane as a clip distance: positive where the model is kept.
    pub(super) fn clip(&self, section: Option<Plane>, placing: &Placing) -> [f32; 4] {
        match section {
            Some(plane) => self.plane(&plane, placing).map(|value| -value),
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Every cut's walls for `BOX_COMMON`, and how many belong to each; cuts past the shader limit are dropped.
    fn cuts(&self, cuts: &[Plane], placing: &Placing) -> (Vec<i32>, Vec<[f32; 4]>) {
        let (mut counts, mut walls) = (Vec::new(), Vec::new());
        for cut in cuts.iter().take(simple3d_geom::section::MAX_CUTS) {
            let own = cut.walls();
            counts.push(own.len() as i32);
            walls.extend(own.iter().map(|wall| self.plane(wall, placing)));
        }
        (counts, walls)
    }

    /// The largest stored coordinate, which single-precision error is relative to.
    pub(super) fn extent(&self) -> f64 {
        let lo = self.lo.x.abs().max(self.lo.y.abs()).max(self.lo.z.abs());
        let hi = self.hi.x.abs().max(self.hi.y.abs()).max(self.hi.z.abs());
        lo.max(hi)
    }

    /// The mesh's world-space box, moved by `placing`.
    pub(in crate::gpu) fn world_box(&self, placing: &Placing) -> (Vec3, Vec3) {
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for corner in 0..8 {
            let world = placing.xform.point(self.origin + box_corner(self.lo, self.hi, corner));
            lo = lo.min(world);
            hi = hi.max(world);
        }
        (lo, hi)
    }

    /// The depth keys of this mesh's box corners under `view`.
    pub(in crate::gpu) fn keys(&self, view: &View, placing: &Placing) -> impl Iterator<Item = f32> {
        let [_, _, key] = self.rows(view, placing);
        let (lo, hi, xform) = (self.lo, self.hi, placing.xform);
        (0..8).map(move |corner| {
            let p = xform.vector(box_corner(lo, hi, corner));
            key[0] * p.x as f32 + key[1] * p.y as f32 + key[2] * p.z as f32 + key[3]
        })
    }

    /// The polygon `plane` leaves in this mesh's box, in order. The box is slightly enlarged so a
    /// cut lying exactly on a box face still meets it.
    pub(in crate::gpu) fn cap_polygon(&self, plane: &Plane, placing: &Placing) -> Vec<Vec3> {
        let margin = Vec3::new(1.0, 1.0, 1.0) * (self.extent() * 1e-3 + 1e-6);
        let (lo, hi) = (self.origin + self.lo - margin, self.origin + self.hi + margin);
        let corner = |index: usize| placing.xform.point(box_corner(lo, hi, index));
        let mut points: Vec<Vec3> = Vec::new();
        for a in 0..8 {
            for bit in [1, 2, 4] {
                let b = a | bit;
                if b == a {
                    continue;
                }
                let (pa, pb) = (corner(a), corner(b));
                let (da, db) = (plane.depth(pa), plane.depth(pb));
                if (da <= 0.0) != (db <= 0.0) {
                    points.push(pa + (pb - pa) * (da / (da - db)));
                }
            }
        }
        if points.len() < 3 {
            return Vec::new();
        }
        let centre = points.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / points.len() as f64;
        let u = (points[0] - centre).normalized();
        let v = plane.normal.cross(u);
        points.sort_by(|a, b| {
            let angle = |p: &Vec3| (*p - centre).dot(v).atan2((*p - centre).dot(u));
            angle(a).partial_cmp(&angle(b)).unwrap_or(std::cmp::Ordering::Equal)
        });
        points
    }
}

impl Gpu {
    /// Widen the depth range to take in a boolean preview's shapes.
    pub(in crate::gpu) fn see_csg(&self, request: &Request<'_>, passes: &mut Passes) {
        let Some(csg) = &request.live.csg else { return };
        let half = self.csg_half.iter().flat_map(|(_, shapes)| shapes.iter().map(|(half, _)| (half, None)));
        for (shape, moved) in csg.leaves.iter().map(|(leaf, moved)| (*leaf, *moved)).chain(half) {
            let Some(resident) = self.resident.get(&shape.id) else { continue };
            for key in resident.keys(&request.view, &Placing::moved(moved)) {
                passes.saw(key);
            }
        }
    }

    /// Compute each cap's fill polygon and widen the depth range to take it in.
    pub(in crate::gpu) fn place_caps(&self, view: &View, plan: &mut Plan, passes: &mut Passes) {
        for cap in &mut plan.caps {
            let Some(resident) = self.resident.get(&cap.id) else { continue };
            let polygon = simple3d_geom::section::within(&resident.cap_polygon(&cap.plane, &cap.placing), &cap.bounds);
            // Cut by the other sections here rather than per pixel: the fill is in screen space and
            // has no world position to test.
            for index in 1..polygon.len().saturating_sub(1) {
                let corners = [polygon[0], polygon[index], polygon[index + 1]];
                for piece in simple3d_geom::section::clip_by_all(&cap.others, corners).triangles() {
                    for &at in piece {
                        let vertex = to_vertex(view, view.to_view(at));
                        passes.saw(vertex.key);
                        cap.fill.push(GpuVertex::new(vertex, cap.colour, 0, 0));
                    }
                }
            }
        }
    }

    /// Widen the depth range to take in every resident mesh drawn, plus the largest line bias.
    pub(in crate::gpu) fn see_resident(&self, request: &Request<'_>, plan: &Plan, passes: &mut Passes) {
        let items =
            request.items.iter().map(|item| (item.renderable.id, Placing::of(&request.live, item.renderable.id)));
        let overlays = plan.overlays.iter().map(|draw| (draw.id, draw.placing));
        // Templates are drawn moved, so the depth range must reach where they are drawn (issue 70).
        let templates =
            request.templates.iter().map(|(renderable, xform, _)| (renderable.id, Placing::moved(Some(*xform))));
        for (id, placing) in items.chain(overlays).chain(templates) {
            let Some(resident) = self.resident.get(&id) else { continue };
            for key in resident.keys(&request.view, &placing) {
                passes.saw(key);
                passes.saw(key + MARK_BIAS * key.abs());
            }
        }
    }
}

pub(in crate::gpu) unsafe fn set_forward(gl: &glow::Context, program: &Program, view: &View) {
    let forward = view.forward();
    if let Some(at) = program.at("u_forward") {
        gl.uniform_3_f32(Some(at), forward.x as f32, forward.y as f32, forward.z as f32);
    }
}

pub(in crate::gpu) unsafe fn set_projection(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    view: &View,
    section: &[Plane],
    placing: &Placing,
) {
    let rows = resident.rows(view, placing);
    for (name, row) in ["u_row_x", "u_row_y", "u_row_key"].into_iter().zip(rows) {
        set4(gl, program, name, &row);
    }
    // Sections are tested per pixel (`BOX_COMMON`); the clip distance is only for the cap count pass.
    set4(gl, program, "u_clip", &resident.clip(None, placing));
    if let Some(at) = program.at("u_cut_count") {
        let (counts, walls) = resident.cuts(section, placing);
        gl.uniform_1_i32(Some(at), counts.len() as i32);
        if let Some(at) = program.at("u_cut_walls[0]") {
            if !counts.is_empty() {
                gl.uniform_1_i32_slice(Some(at), &counts);
            }
        }
        if let Some(at) = program.at("u_walls[0]") {
            if !walls.is_empty() {
                gl.uniform_4_f32_slice(Some(at), walls.as_flattened());
            }
        }
        set_within(gl, program, resident, placing, &[]);
    }
    if let Some(at) = program.at("u_model") {
        // Row-major, as `Xform` stores its matrix.
        let m: Vec<f32> = placing.xform.m.as_flattened().iter().map(|&value| value as f32).collect();
        gl.uniform_matrix_3_f32_slice(Some(at), true, &m);
    }
    if let Some(at) = program.at("u_hide") {
        gl.uniform_2_u32(Some(at), placing.hide[0], placing.hide[1]);
    }
}

/// The walls a line must stay inside, for `BOX_COMMON`'s `u_within`.
pub(in crate::gpu) unsafe fn set_within(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    placing: &Placing,
    within: &[Plane],
) {
    let Some(count_at) = program.at("u_within_count") else { return };
    let walls: Vec<[f32; 4]> = within.iter().take(4).map(|wall| resident.plane(wall, placing)).collect();
    gl.uniform_1_i32(Some(count_at), walls.len() as i32);
    if walls.is_empty() {
        return;
    }
    set4(gl, program, "u_within[0]", walls.as_flattened());
    if let Some(at) = program.at("u_cut_slack") {
        // A few times the single-precision error of the numbers in play, as for `u_on_plane`.
        let offset = walls.iter().fold(0.0_f64, |most, wall| most.max(wall[3].abs() as f64));
        gl.uniform_1_f32(Some(at), ((resident.extent() + offset) * 4e-6 + 1e-6) as f32);
    }
}

/// Which winding faces the eye on the card, for culling and the cap's winding count.
/// Counter-clockwise on screen becomes clockwise after the row flip; the basis is checked
/// rather than assumed so a mirrored camera still culls correctly.
pub(in crate::gpu) fn front_face(view: &View) -> u32 {
    let (right, up) = view.basis();
    if right.cross(up).dot(-view.forward()) > 0.0 {
        glow::CW
    } else {
        glow::CCW
    }
}

/// The projection rows for positions stored relative to `origin`, in double precision.
pub(in crate::gpu) fn rows_at(view: &View, origin: Vec3) -> [[f32; 4]; 3] {
    let (right, up) = view.basis();
    let forward = view.forward();
    let s = view.pixels_per_mm();
    let d = origin - view.eye();
    let row = |v: Vec3, c: f64| [v.x as f32, v.y as f32, v.z as f32, c as f32];
    [
        row(right * s, view.centre.x as f64 + d.dot(right) * s),
        row(-(up * s), view.centre.y as f64 - d.dot(up) * s),
        row(-forward, -d.dot(forward)),
    ]
}
