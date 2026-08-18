# Change report — 2026-08-18 (fourth pass)

A visual pass over the whole interface, against the ScadStudio UI design
description. Nothing about geometry, evaluation, export or the file format
changed. `cargo test --workspace` is 292 tests, up from 288, none failing.

The previous pass's report is in git history; `KNOWN_ISSUES.md` is still the
issue list and this pass opened no entries in it.

## What the window looked like before

Default egui dark theme, one `style_mut` call setting two spacing values, and
every colour decision left to the toolkit. The tool modes were three words in a
row of the menu bar. Adding a shape meant Add ▸ category ▸ name. The viewport's
corner carried four lines of grey prose, in the same grey as the grid.

Screenshots of before and after were taken by running the real binary under
XWayland and grabbing the window with `xwd` — the manipulator bug found the last
pass is the standing argument for looking at the thing rather than at its tests.

## The visual language, in one place

`crates/scadstudio-app/src/theme.rs` is new and is now the only file that names a
colour, a row height or a type size:

- **`theme::token`** — the design's palette: `SURFACE_0..3`, `TEXT_HI/LO`, the
  amber `ACCENT`, the cyan `MEASURE`, the red `DANGER`, and the three axis
  colours. Amber for selection and cyan for measurement is the one deliberate
  departure from the blue every 3D tool defaults to, and it is what keeps the
  selection colour clear of the X/Y/Z handles — blue selection never manages
  that.
- **`theme::metric`** — 22 px list rows, 24 px input rows, 8 px panel padding,
  4 px gaps, a 40 px rail, a 32 px menu bar, a 24 px status bar.
- **`theme::apply`** — installs the whole `egui::Visuals` and `Spacing` at
  startup. Four distinct widget states, hover as a *surface* change rather than a
  text tint, and the active state as an accent **fill**, not a tint: which tool
  is in force must not be a shade of guesswork.

`render.rs`'s viewport palette now reads those same tokens rather than repeating
hex literals, so the ground plane and the docks cannot drift apart.

## Structure

- **A tool rail** (`panel_toolrail.rs`), 40 px on the far left, icons only.
  Move/rotate/resize, the handle frame, the three booleans, group and delete, and
  the view state at its foot. This is where the menu bar's word-toolbar went, and
  the row it occupied went back to the viewport.
  - The boolean buttons are momentary actions, never modes, and are **dimmed with
    a sentence** when the selection cannot be combined — `combine_blocked` states
    the two cases once and the tooltip shows it verbatim.
  - The tools are driven from `Mode::ALL`, so a new manipulator mode without a
    rail button does not compile.
- **A primitive palette** (`panel_primitives.rs`) under the outliner: every shape
  in the registry as a silhouette tile, in collapsible category blocks. One
  gesture and a glance instead of two gestures and a read. The Add menu is
  untouched and still lists all of them.
- **The right dock follows the selection.** Collapsible sections — Object,
  Dimensions, Transform, Measured, and Boolean for a group. With **nothing**
  selected it shows a **Document** panel (unit, grid, default segments, scene
  bounds) instead of a column of dead fields.
- **The status bar** reads left to right as one sentence of state: what is
  selected → its size → snap → grid → unit → message, with counts and timing on
  the right. Separated by dots; a vertical rule every few words turned it into a
  row of boxes.

## The outliner

Rewritten as drawn 22 px rows rather than stacked widgets:

- Selection is an accent tint plus a solid bar at the left edge, and the
  **primary** node of a multi-selection is a stronger tint than the rest — so the
  node the property editor is actually editing is identifiable without a second
  colour.
- Visibility is an eye glyph, always drawn, **dimmed rather than hidden** when
  off. The eye owns its own clicks: pressing it no longer also selects the row.
- **Operator badges** are inline. A group wears its own operator; a child that is
  being subtracted wears the cut mark in the danger colour; the base of a
  difference wears nothing, because it is what is being cut, not a cut. The root
  wears nothing either — everything is in the scene by definition.

The set-theory symbols `∪ ∖ ∩` would have been the obvious badge and were the
first attempt. The bundled UI face has no glyph for any of them, so the tree drew
three identical tofu boxes. The badges are now the same drawn shapes the rail's
boolean buttons carry, which is better anyway: one vocabulary, two places.

## Icons

`icon.rs` is new: every glyph is a handful of strokes in a unit square, scaled
into whatever rectangle it is given. No icon font and no SVG loader, because the
single self-contained binary is a hard constraint — and it stays crisp at
fractional DPI, which a bitmap sheet would not. A test asserts every primitive in
the registry has its own silhouette rather than falling back to the box.

## The viewport

- **The grid fades with distance** instead of stopping. A grid that simply ends
  leaves a hard square edge in mid-air and the eye reads that edge as part of the
  model. The axes fade the same way, for the same reason.
- **The grid and the axes no longer write depth.** They are drawn before the
  model, so they must lose every tie with it — including the exact tie a ground
  plane makes with a plate whose side walls it cuts. Fading made that old tie
  visible as dark dashes across the plate; not writing depth removes the tie
  rather than biasing around it.
- **An orientation cube** in the bottom-right corner, drawn with the live camera,
  so which way the model faces is answerable without orbiting to find out.
- The four lines of grey prose are one line. Everything they said — the display
  mode, the projection, the grid — is now a lit button on the rail.
- **Drag readouts are cyan and monospaced**: a measurement, not a message. The
  selection box is amber; its dimension labels are cyan.

## Numbers

Every value field is monospaced with tabular figures. A column of dimensions has
to line up, and no digit may change width while a value is being scrubbed — that
is the typographic tell that this application is about measurement. Units sit at
the right-hand end of a row, one size down and in the label colour, so they never
compete with the number they qualify. Axis fields carry a colour chip instead of
spelling out X, Y and Z.

## Tests worth knowing about

- `app::every_panel_draws_with_a_selection_and_with_none` and
  `an_empty_scene_still_draws_every_panel` render whole frames headlessly through
  every panel, in the three states that lay out differently (shape selected,
  nothing selected, group selected). A panel that panics or a layout that divides
  by a width it has not got now fails here.
- `theme::every_axis_has_its_own_colour` asserts the accent collides with none of
  them, which is the reason it is amber.
- `panel_outliner::a_group_wears_its_own_operator_and_a_cut_child_wears_the_cut`
  pins the badge rule, including that the base of a difference carries none.
- `render::the_grid_never_draws_over_geometry_it_is_coplanar_with` still holds.
  It had to stop skipping pixels *by axis colour*: a faded axis pixel is a blend
  of the axis with whatever is under it, which is the grid in one render and the
  background in the other. The axes are made invisible for that comparison
  instead, which leaves the question the test is actually asking.

## Known gaps against the design description

Stated rather than quietly skipped:

- No `Layout` mode tab. There is no second mode in this application to put behind
  one, and inventing an empty tab is worse than not having it.
- Numeric fields do not yet do drag-scrub on the label, expression evaluation,
  relative `+`/`-` entry or unit-suffixed input (`4 cm` in a millimetre
  document). The parsing lives in `scadstudio-core::unit::parse_number` and is a
  model change, not a visual one.
- Panels are not draggable between the docks, and `Tab` does not hide them.
- The view cube shows orientation but is not yet clickable for a face view; the
  View menu's camera presets are still how you get one.
