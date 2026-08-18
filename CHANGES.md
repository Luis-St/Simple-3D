# Change report — 2026-08-18 (sixth pass)

*The engineering report for one pass of work: what changed, why, and what was
deliberately not built. `CHANGELOG.md` is the user-facing record of releases;
earlier reports are in git history.*

One thing: **the four pointer gestures the fifth pass shipped are now performed
by tests.** The dock header drag, the field-label scrub, the view-cube click and
Shift+right-click to place the 3D cursor were all wired up last pass and none of
them had ever been carried out — that was the single open entry in
`KNOWN_ISSUES.md`, and it is closed.

`cargo test --workspace` is **328 tests**, up from 320, none failing.
`python3 tools/criteria_audit.py` is clean: all 29 acceptance criteria are still
cited by at least one test. Nothing about geometry, evaluation, export or the
file format changed.

## How a gesture is executed

`egui_kittest` 0.32.3 exists and is built against the pinned egui 0.32.3, so no
version bump was needed. It is a **dev-dependency**, and so is the one feature it
forces on the window integration:

```toml
[dev-dependencies]
egui_kittest = { version = "=0.32.3", default-features = false }
eframe = { version = "0.32", default-features = false,
           features = ["glow", "default_fonts", "wayland", "x11", "accesskit"] }
```

`egui_kittest` drives egui through AccessKit, which is a *feature* of egui and of
`egui-winit` underneath it — without that second line the test build fails to
compile `egui-winit`, whose `PlatformOutput` pattern then misses a field. Naming
`eframe` again under `[dev-dependencies]` turns the feature on for the test build
only: with resolver 2 a dev-dependency's features do not reach `cargo build`. The
shipped binary is unchanged.

Three things had to change in the application itself, all of them small and all
of them improvements on their own terms:

- **`App::ui(ctx)`** is the panels, the docks, the viewport and the modals, in
  the order they stack, lifted out of `eframe::App::update`. A test drives *that*
  — the same frame the window draws, not a re-creation of it. `update` now calls
  it, and so does the whole-frame drawing test that already existed.
- **The grips have names.** `dock::header_id(panel)`,
  `panel_properties::grip_id(name)` and `panel_viewport::cube_id()` replace ids
  that egui derived from where the widget happened to sit. A test asks the
  context where a named widget was drawn and puts the pointer *there*, so a
  layout change moves the test with it rather than breaking it. This also fixes
  something that was latent: `App::scrub` remembers a gesture in flight by the
  grip's id, and that id used to change if the panel relaid itself out mid-drag.
- Nothing else. No test-only branch in any drawing code.

The tests are `crates/simple3d-app/src/gestures.rs`, eight of them:

| Gesture | What is asserted |
| --- | --- |
| Header drag | The header claims the drag (nothing underneath it does), the drop target reads as the right dock's first slot while the pointer is there, the panel ends up in that dock in that order — and is *drawn* on the right on the next frame. |
| Header click | The same widget's other gesture: it rolls the panel up, does not move it, and a second click unrolls it. |
| Label scrub | 60 px across the "Width (X)" label is 10 mm, in one undo step, and one Undo takes the whole drag back. |
| Scrub that leaves its label | The pointer crosses the Depth and Thickness labels on its way; the field the drag *began* on is the one that changes and the other two do not. |
| Cube face click | Clicking where the cube draws its front face asks the camera for the front view, from where the camera was. |
| Cube click vs. the viewport | The click does not orbit the camera and does not reach the selection behind the cube (a viewport click on empty space clears it). |
| Shift+right-click | The 3D cursor lands on the plate under the pointer, snapped to the move snap, without orbiting. |
| Shift+right-click on nothing | With a point the view proves is neither geometry nor ground, the cursor goes back to the origin. |

Two details worth keeping:

- **The face to click is computed from `cube_project`**, the same function the
  cube draws itself with, rather than from a hardcoded corner of the cube. A
  test that assumed where "front" is drawn would have agreed with last pass's
  quarter-turn bug instead of catching it.
- **The empty spot for the second cursor test is searched for, not guessed.**
  From the starting view every pixel of the viewport meets the ground plane, so
  the camera is tipped under it first and the test then finds a pixel where the
  view itself reports no ground and no mesh. It asserts one exists.

## What executing them turned up

- **The `taken` guard in `panel_viewport::show` is belt and braces.** It exists so
  a click on the cube does not also orbit or select behind it. Removing it and
  re-running the gestures changes nothing: egui already hands a click, and in
  fact a drag begun on the cube, to the cube. The guard is right about intent and
  is kept — but it is not what makes the behaviour true, and the two cube tests
  assert the behaviour rather than the guard.
- **A long status message ran straight over the readout at the right end of the
  status bar** — "Opened /very/long/path…" printed on top of "4 ms · 5 nodes ·
  156 triangles". Found by opening a project from a deep directory and looking
  at the window, not by any test. The message now gets what is left of the bar
  after `theme::metric::STATUS_READOUT`, is elided into it, and carries the full
  text on hover.

## What was checked by looking

Three models were built through the real core, written out as projects, and each
one opened in the release binary under XWayland and photographed with `xwd`:

| Model | Built from | Result |
| --- | --- | --- |
| Bracket | 60 × 25 × 4 plate less two ⌀5 holes | 60 × 25 × 4 mm, 5843.9 mm³ (6000 − 2 × 78.5), manifold |
| Spacer | ⌀12 tube, ⌀8 bore, 10 tall | 12 × 12 × 10 mm, 624.3 mm³ (π × 20 × 10 at 32 segments), manifold |
| Tray | 40 × 40 × 12 box less a 34 × 34 pocket, boss unioned back in | 40 × 40 × 12 mm, 11307.8 mm³, manifold |

All three evaluate with no errors, export to 3MF and binary STL, and round trip
through the project format byte for byte. All three render correctly in the
window, with the outliner, the bounds readout and the status bar agreeing with
the numbers above.

## Still not built, and why

- **The gestures still cannot be driven in the *running* window.** There is no
  `xdotool` or `ydotool` on this machine, so what is executed is a real frame of
  the real interface against a headless context, with real pointer events. That
  covers which widget claims a gesture and what it does with it; it does not
  cover the window system underneath egui.
- **Panel *sections* do not move.** Unchanged: Object, Dimensions, Transform and
  Measured collapse but stay in the Properties panel. The design's docking
  language is about panels, and four more headers in the drag model answers no
  question anyone is asking.
- **No `Layout` mode tab.** Unchanged: there is no second mode to put behind one.
- **The expression reader has no functions and no variables.** `sqrt(2)*10` and a
  named dimension reused across three fields are both reasonable and neither
  exists. A variable is a model change rather than a parser change — it would
  have to live in the project file — and that decision was not taken here.
- **The 3D cursor is still session state.** Not saved with the project, because
  saving it would change the file format. That remains a decision to take
  explicitly rather than by accident.
