//! Drawing a frame: preparing it once, then filling the rows in bands.

use super::*;
use crate::raster::{Frame, Image};
use simple3d_core::config::DisplayMode;

/// Prepare and draw in one step, the tests' entry point; the application prepares once and hands
/// the result to whichever engine draws.
#[cfg(test)]
pub fn render(request: &Request<'_>) -> Image {
    render_prepared(request, &prepare_frame(request))
}

/// The software renderer, from a prepared frame.
pub fn render_prepared(request: &Request<'_>, prepared: &Prepared) -> Image {
    let [_, height] = request.size;
    render_in_bands(request, prepared, band_count(height.max(1)))
}

/// The scene prepared for the software renderer: screen-space primitives in drawing order, and
/// the axis pieces with their rule, decided once for every band. The GPU never uses this.
pub struct Prepared {
    pub(crate) steps: Vec<Step>,
    pub(crate) axes: Vec<AxisStep>,
}

pub fn prepare_frame(request: &Request<'_>) -> Prepared {
    let material = axis_material_live(&request.items, &request.grid, &request.section, &request.live);
    let axes = prepare_axes(&request.view, &request.palette, &request.grid, &material);
    Prepared { steps: prepare_with(request), axes }
}

/// Draw the frame in `bands` bands. The count must not change the picture (tested at the bottom
/// of this file); it is a parameter only for that test.
pub(crate) fn render_in_bands(request: &Request<'_>, prepared: &Prepared, bands: usize) -> Image {
    let [width, height] = request.size;
    let (width, height) = (width.max(1), height.max(1));
    let Prepared { steps, axes } = prepared;

    // One buffer for the whole frame; each band writes its own rows directly, avoiding a final copy
    // that cost more than drawing on large viewports.
    let mut color = vec![0u8; width * height * 4];

    // Rows are shared out by work (`balanced_ranges`).
    let ranges = balanced_ranges(steps, height, bands);
    if ranges.len() == 1 {
        let mut frame = Frame::band(&mut color, width, height, 0, height);
        draw(&mut frame, request, steps, &[&(0..steps.len() as u32).collect::<Vec<_>>()], axes);
        return Image { width, height, color };
    }

    // `bins[chunk][band]`: which of the chunk's steps reach the band.
    let bins = bin_steps(steps, &ranges);
    // One disjoint slice per band, in row order.
    let mut rest = &mut color[..];
    let mut slices: Vec<&mut [u8]> = Vec::with_capacity(ranges.len());
    for &(lo, hi) in &ranges {
        let (mine, tail) = rest.split_at_mut((hi - lo) * width * 4);
        slices.push(mine);
        rest = tail;
    }

    std::thread::scope(|scope| {
        for (band, (&(lo, hi), slice)) in ranges.iter().zip(slices).enumerate() {
            let (steps, axes, bins) = (steps, axes, &bins);
            scope.spawn(move || {
                // The band's steps in chunk order, which is preparation order.
                let mine: Vec<&[u32]> = bins.iter().map(|chunk| &chunk[band][..]).collect();
                let mut frame = Frame::band(slice, width, height, lo, hi);
                draw(&mut frame, request, steps, &mine, axes);
            });
        }
    });
    Image { width, height, color }
}

/// All of it as steps, for tests.
#[cfg(test)]
pub(crate) fn prepare(request: &Request<'_>) -> Vec<Step> {
    prepare_with(request)
}

/// Everything row-independent: projected, culled, shaded and ordered as drawing requires.
pub(crate) fn prepare_with(request: &Request<'_>) -> Vec<Step> {
    let view = request.view;
    let mut steps = Vec::new();
    if request.grid.visible {
        push_grid(&mut steps, &view, &request.grid, &request.palette);
    }
    let cut = &request.section[..];
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        // Each vertex projected once for everything this item draws.
        let needs_screen = item.style != Style::Selected && cut.is_empty();
        let screen = if needs_screen { project_all(&view, &item.renderable.mesh.positions) } else { Vec::new() };
        let screen = &screen[..];
        match item.style {
            Style::Solid => {
                let palette = &request.palette;
                match request.mode {
                    DisplayMode::Wireframe => {
                        push_wireframe(&mut steps, &view, item.renderable, screen, palette.wire, cut)
                    }
                    DisplayMode::Shaded => {
                        push_shaded(&mut steps, &view, item.renderable, screen, palette.solid, 255, tag_base, cut)
                    }
                    DisplayMode::ShadedWithEdges => {
                        push_shaded(&mut steps, &view, item.renderable, screen, palette.solid, 255, tag_base, cut);
                        push_edges(&mut steps, &view, item.renderable, screen, palette.edge, tag_base, cut);
                    }
                }
                push_cap(&mut steps, &view, item.renderable, palette, cut, request.mode);
            }
            // The selection is always outlined, with more bias than the solid's own edges so it wins ties.
            // Glowing bodies are outlined too; the glow itself comes last.
            Style::Selected | Style::Glow => push_selection(
                &mut steps,
                &view,
                item.renderable,
                request.palette.selected,
                tag_base,
                request.mode,
                cut,
            ),
            Style::Ghost => push_ghost(&mut steps, &view, item.renderable, screen, request.palette.ghost, cut),
        }
    }
    // Templates of what a tool would place (issue 70), shaded like ghosts, with their seen edges solid.
    // The CPU carries each body there itself; only the GPU draws a renderable moved.
    for (renderable, xform) in &request.templates {
        let placed = renderable.placed(xform);
        let screen = if cut.is_empty() { project_all(&view, &placed.mesh.positions) } else { Vec::new() };
        let palette = &request.palette;
        push_template(&mut steps, &view, &placed, &screen, palette.template, cut);
    }
    // Last, over the finished model: glows for buried bodies, and tool preview cells.
    for item in request.items.iter().filter(|item| item.style == Style::Glow) {
        let screen = if cut.is_empty() { project_all(&view, &item.renderable.mesh.positions) } else { Vec::new() };
        push_glow(&mut steps, &view, item.renderable, &screen, request.palette.glow, cut);
    }
    push_preview(&mut steps, &view, &request.preview, request.palette.selected, cut);
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        // After the solids, since the mark sits on the surface; wireframe has none.
        push_plane_marks(&mut steps, &view, &request.items, &request.palette, &request.grid, cut);
    }
    steps
}

/// Draw the scene into one frame: a single band or the whole.
pub(crate) fn draw(frame: &mut Frame, request: &Request<'_>, steps: &[Step], mine: &[&[u32]], axes: &[AxisStep]) {
    fill_background(frame, &request.palette);
    draw_steps(frame, steps, mine);
    frame.set_tag(0);
    // Axes last: hidden by material they run through and by what is in front, except on the approach
    // to the surface they enter, which depth alone would hide (issue 47). The grid is drawn first.
    for step in axes {
        frame.line_through(step.a, step.b, step.colour, AXIS_BIAS, &step.seen);
    }
}
