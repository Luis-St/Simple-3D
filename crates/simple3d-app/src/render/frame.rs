//! Drawing a frame: preparing it once, then filling the rows in bands.

use super::*;
use crate::raster::{Frame, Image};
use simple3d_core::config::DisplayMode;

/// Draw the scene. The frame is cut into horizontal bands and each is drawn on
/// its own thread, from the same list of primitives in the same order -- see
/// `Frame`'s own note on why the split is by rows rather than by work.
///
/// Everything that has to be decided before drawing starts is decided once,
/// here, and shared: `axis_material` walks every triangle of every item to find
/// where the axes run through material, and doing that once per band would put
/// most of the work back.
/// Prepare and draw in one step. The application prepares once and hands the
/// result to whichever engine is drawing, so this is the tests' way in.
#[cfg(test)]
pub fn render(request: &Request<'_>) -> Image {
    render_prepared(request, &prepare_frame(request))
}

/// The software renderer, from a frame that has already been prepared.
pub fn render_prepared(request: &Request<'_>, prepared: &Prepared) -> Image {
    let [_, height] = request.size;
    render_in_bands(request, prepared, band_count(height.max(1)))
}

/// The scene worked out but not yet drawn: screen-space primitives in drawing
/// order, and the pieces of the origin axes with the rule that governs them.
///
/// This is where the two renderers meet. Projection, culling, shading, the
/// grid's falloff and the whole of the axis-through-material question are
/// decided here, once, on the CPU; what the GPU renderer does differently is
/// only how the resulting primitives are turned into pixels. Anything decided
/// here cannot drift between the engines, which is the point of it.
pub struct Prepared {
    pub(crate) steps: Vec<Step>,
    pub(crate) axes: Vec<AxisStep>,
}

pub fn prepare_frame(request: &Request<'_>) -> Prepared {
    let material = axis_material(&request.items, &request.grid, request.section);
    let axes = prepare_axes(&request.view, &request.palette, &request.grid, &material);
    Prepared { steps: prepare(request), axes }
}

/// Draw the frame in a given number of bands. The band count must not change
/// the picture -- see the test at the bottom of this file, which holds the two
/// against each other -- so it is a parameter only so that the test can ask.
pub(crate) fn render_in_bands(request: &Request<'_>, prepared: &Prepared, bands: usize) -> Image {
    let [width, height] = request.size;
    let (width, height) = (width.max(1), height.max(1));
    let Prepared { steps, axes } = prepared;

    // The one buffer the whole frame is drawn into. Each band is handed the
    // stretch of it holding its own rows, so what the threads write is already
    // the finished image -- no copy at the end, which on a large viewport was
    // costing more than the drawing.
    let mut color = vec![0u8; width * height * 4];

    // Rows are handed out so the remainder is spread over the first few bands
    // rather than piled onto the last one.
    let ranges: Vec<(usize, usize)> =
        (0..bands).map(|i| (height * i / bands, height * (i + 1) / bands)).filter(|(lo, hi)| lo < hi).collect();
    if ranges.len() == 1 {
        let mine: Vec<u32> = (0..steps.len() as u32).collect();
        let mut frame = Frame::band(&mut color, width, height, 0, height);
        draw(&mut frame, request, steps, &mine, axes);
        return Image { width, height, color };
    }

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
        for ((&(lo, hi), mine), slice) in ranges.iter().zip(&bins).zip(slices) {
            let (steps, axes) = (steps, axes);
            scope.spawn(move || {
                let mut frame = Frame::band(slice, width, height, lo, hi);
                draw(&mut frame, request, steps, mine, axes);
            });
        }
    });
    Image { width, height, color }
}

/// Everything of the model that is the same whichever rows are being drawn:
/// projected, culled, shaded and ordered exactly as the drawing order requires.
pub(crate) fn prepare(request: &Request<'_>) -> Vec<Step> {
    let view = request.view;
    let mut steps = Vec::new();
    if request.grid.visible {
        push_grid(&mut steps, &view, &request.grid, &request.palette);
    }
    let cut = request.section;
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        match item.style {
            Style::Solid => match request.mode {
                DisplayMode::Wireframe => {
                    push_wireframe(&mut steps, &view, item.renderable, request.palette.wire, cut);
                    push_cap(&mut steps, &view, item.renderable, &request.palette, cut, request.mode);
                }
                DisplayMode::Shaded => {
                    push_shaded(&mut steps, &view, item.renderable, request.palette.solid, 255, tag_base, cut);
                    push_cap(&mut steps, &view, item.renderable, &request.palette, cut, request.mode);
                }
                DisplayMode::ShadedWithEdges => {
                    push_shaded(&mut steps, &view, item.renderable, request.palette.solid, 255, tag_base, cut);
                    push_edges(&mut steps, &view, item.renderable, request.palette.edge, tag_base, cut);
                    push_cap(&mut steps, &view, item.renderable, &request.palette, cut, request.mode);
                }
            },
            Style::Selected => {
                // Always outlined, in every display mode: the selection has to
                // be visible, and an outline reads clearly over a shaded body.
                // A larger bias than the solid's own edges, or the two would tie
                // at equal depth and the outline would lose.
                push_selection(
                    &mut steps,
                    &view,
                    item.renderable,
                    request.palette.selected,
                    tag_base,
                    request.mode,
                    cut,
                );
            }
            Style::Ghost => push_ghost(&mut steps, &view, item.renderable, request.palette.ghost, cut),
            // The outline here; the glow itself comes last, after everything
            // that could be standing in front of it.
            Style::Glow => push_selection(
                &mut steps,
                &view,
                item.renderable,
                request.palette.selected,
                tag_base,
                request.mode,
                cut,
            ),
        }
    }
    // Last of all, over the finished model: what a buried body is pointed out
    // with, and the cells of a tool's preview.
    for item in request.items.iter().filter(|item| item.style == Style::Glow) {
        push_glow(&mut steps, &view, item.renderable, request.palette.glow, cut);
    }
    push_preview(&mut steps, &view, &request.preview, request.palette.selected, cut);
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        // After the solids: the mark belongs on the surface, and in wireframe
        // there is no surface for it to sit on.
        push_plane_marks(&mut steps, &view, &request.items, &request.palette, &request.grid, cut);
    }
    steps
}

/// Draw the whole scene into one frame -- a band of one, or the lot.
pub(crate) fn draw(frame: &mut Frame, request: &Request<'_>, steps: &[Step], mine: &[u32], axes: &[AxisStep]) {
    fill_background(frame, &request.palette);
    draw_steps(frame, steps, mine);
    frame.set_tag(0);
    // Last: an axis is hidden by the material it runs through, which is cut out
    // of the line, and by anything in front of it -- except on the approach to a
    // surface it is about to go into, which is drawn over the shape it is
    // arriving at. Depth alone eats that approach, because the line is behind
    // the shape's own front faces for the whole stretch between the silhouette
    // and the point it enters, and losing it is what made the origin read as
    // being somewhere behind the model (issue 47). The grid is untouched, drawn
    // first and covered by everything.
    for step in axes {
        frame.line_through(step.a, step.b, step.colour, AXIS_BIAS, &step.seen);
    }
}
