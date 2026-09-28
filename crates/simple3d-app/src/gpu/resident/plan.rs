//! Which resident draws a frame asks for.

use super::*;

/// Which resident draws a frame needs, mirroring `render::prepare_with`.
pub(in crate::gpu) fn plan(request: &Request<'_>) -> Plan {
    let palette: &Palette = &request.palette;
    let mut plan = Plan::default();
    if request.mode == DisplayMode::ShadedWithEdges {
        plan.csg_edges = Some(palette.edge);
    }
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        let id = item.renderable.id;
        let placing = Placing::of(&request.live, id);
        match item.style {
            Style::Solid => {
                let solid = FaceDraw { id, placing, mode: SOLID, base: opaque(palette.solid), tag_base };
                match request.mode {
                    DisplayMode::Wireframe => {
                        plan.lines.push(LineDraw { id, placing, colour: palette.wire, bias: 0.0, tag_base: None })
                    }
                    DisplayMode::Shaded => plan.solids.push(solid),
                    DisplayMode::ShadedWithEdges => {
                        plan.solids.push(solid);
                        plan.lines.push(LineDraw {
                            id,
                            placing,
                            colour: palette.edge,
                            bias: EDGE_BIAS,
                            tag_base: Some(tag_base),
                        });
                    }
                }
                // One cap per face the cut opens. A box side facing away from the camera is not filled,
                // which the winding count also needs; its line is still drawn as the opening's near rim.
                let forward = request.view.forward();
                for (cut, section) in request.section.iter().enumerate() {
                    let others: Vec<Plane> =
                        request.section.iter().enumerate().filter(|&(o, _)| o != cut).map(|(_, p)| *p).collect();
                    for (index, face) in simple3d_geom::section::faces(section).into_iter().enumerate() {
                        let plane = face.plane;
                        let facing = index == 0 || plane.normal.dot(forward) < 0.0;
                        if request.mode != DisplayMode::Wireframe && facing {
                            let colour = shade(palette.cut, plane.normal, forward, 255);
                            plan.caps.push(CapDraw {
                                id,
                                placing,
                                plane,
                                bounds: face.bounds.clone(),
                                others: others.clone(),
                                colour,
                                base: palette.cut,
                                fill: Vec::new(),
                            });
                        }
                        if request.mode != DisplayMode::Shaded {
                            let colour = match request.mode {
                                DisplayMode::Wireframe => palette.wire,
                                _ => palette.edge,
                            };
                            plan.crossings.push(CrossingDraw {
                                id,
                                placing,
                                planes: vec![(plane, colour)],
                                cuts: others.clone(),
                                within: face.bounds,
                            });
                        }
                    }
                }
            }
            Style::Ghost => plan.ghosts.push(FaceDraw { id, placing, mode: GHOST, base: palette.ghost, tag_base: 0 }),
            Style::Glow | Style::Selected => {
                if item.style == Style::Glow {
                    plan.glows.push(FaceDraw { id, placing, mode: GLOW, base: palette.glow, tag_base: 0 });
                }
                // Without adjacency only creases can outline a body (`push_selection`'s fallback).
                if item.renderable.outline.is_empty() {
                    plan.lines.push(LineDraw {
                        id,
                        placing,
                        colour: palette.selected,
                        bias: SELECTION_BIAS,
                        tag_base: Some(tag_base),
                    });
                } else {
                    plan.outlines.push(OutlineDraw {
                        id,
                        placing,
                        colour: palette.selected,
                        tag_base,
                        all_creases: request.mode == DisplayMode::Wireframe,
                    });
                }
            }
        }
    }
    let planes = mark_planes(request);
    if !planes.is_empty() {
        for item in request.items.iter().filter(|item| item.style == Style::Solid) {
            let id = item.renderable.id;
            let placing = Placing::of(&request.live, id);
            plan.crossings.push(CrossingDraw {
                id,
                placing,
                planes: planes.clone(),
                cuts: request.section.clone(),
                within: Vec::new(),
            });
        }
    }
    plan
}

/// The principal planes whose marks this frame draws, with colours.
pub(in crate::gpu) fn mark_planes(request: &Request<'_>) -> Vec<(Plane, Rgba)> {
    if !request.grid.plane_marks || request.mode == DisplayMode::Wireframe {
        return Vec::new();
    }
    let colours = mark_colours(&request.palette);
    (0..3)
        .filter(|&axis| request.grid.axes[MARK_AXIS[axis]])
        .map(|axis| (Plane::on_axis(axis, 0.0, false), colours[axis]))
        .collect()
}

impl Plan {
    pub(super) fn needs(&self) -> std::collections::HashMap<u64, Needs> {
        let mut needs: std::collections::HashMap<u64, Needs> = std::collections::HashMap::new();
        let faces = self.solids.iter().chain(&self.ghosts).chain(&self.glows).map(|draw| draw.id);
        let faces = faces.chain(self.crossings.iter().map(|draw| draw.id)).chain(self.caps.iter().map(|draw| draw.id));
        let faces = faces.chain(self.csg.iter().copied());
        for id in faces {
            needs.entry(id).or_default().faces = true;
        }
        if self.csg_edges.is_some() {
            for &id in &self.csg {
                needs.entry(id).or_default().edges = true;
            }
        }
        for draw in self.lines.iter().chain(&self.overlays) {
            needs.entry(draw.id).or_default().edges = true;
        }
        for draw in &self.outlines {
            needs.entry(draw.id).or_default().outline = true;
        }
        needs
    }
}

/// A solid is drawn opaque regardless of palette alpha, as in `push_shaded`.
fn opaque(colour: Rgba) -> Rgba {
    [colour[0], colour[1], colour[2], 255]
}
