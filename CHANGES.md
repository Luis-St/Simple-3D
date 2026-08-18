# Change report — 2026-08-19 (seventh pass)

*The engineering report for one pass of work: what changed, why, and what was
deliberately not built. `CHANGELOG.md` is the user-facing record of releases;
earlier reports are in git history.*

This pass is **the thirteen items on the reported issue list**, all of them, plus
the tidy-up that came with it. Four were bugs, six were missing features, two
were defaults that were wrong, and one was a build feature that was never turned
on.

`cargo test --workspace` is **354 tests**, up from 328, none failing.
`python3 tools/criteria_audit.py` is clean: all 29 acceptance criteria are still
cited by at least one test.

## The four bugs

### A face the orthographic camera could see was culled

Reported with a screenshot of a plate seen almost edge-on, missing a side wall.

`render::draw_shaded` culled a triangle with `normal.dot(eye - centroid)`. Under
perspective that is exactly right, and it has to be measured per triangle. Under
**orthographic** projection there is no eye: every ray runs along the view
direction, and `eye - centroid` answers correctly only near the middle of the
frame. The further a triangle sits from the camera's target the more the answer
tilts — 100 mm off target at a distance of 120 mm is nearly forty degrees — and
every wall within that of edge-on was culled although it faced the viewer.

The fix is four lines. The test was the hard part, and it is worth writing down
why: **a dropped wall is not a hole.** It is a slice missing from the *edge* of
the silhouette, so scanning each row for background between two painted pixels
finds nothing. What catches it is that a box is convex, so what it covers is
exactly the convex hull of its projected corners; the test builds that hull and
counts pixels a margin inside it that were never painted. The first two versions
of this test passed against the unfixed code.

### A move handle had to be grabbed several times

Reported with a screen capture of a plate refusing to move.

egui does not call a press a drag until the pointer has moved past its
threshold, which is most of the 9 px a handle is grabbable within.
`panel_viewport::manipulate` asked "what is under the pointer" on the frame the
drag started — by which time the pointer had usually left the handle it pressed.
`hover_handle` was `None`, `drag_phase` returned `Idle`, and the gesture was
reported as a plain click that re-selected the shape.

`App::grabbed` now records what was under the pointer when the button went
**down**, and `Drag::begin` measures from `press_origin` rather than from
wherever the pointer had slipped to. `hover_handle` still exists and still does
what it always did: draw the highlight.

### Clicking a shape did nothing while nothing was selected

Picking was the last few lines of `manipulate`, which returns early when
`app.primary()` is `None`. With an empty selection — which, after this pass, is
what the application opens on — the first click into the viewport was thrown
away. It is its own step in `panel_viewport::show` now, guarded by a flag the
manipulator returns saying whether it claimed the pointer.

Both of these are executed by `egui_kittest`, not reasoned about:
`clicking_a_shape_selects_it_when_nothing_is_selected_yet` and
`a_press_on_a_move_handle_grabs_it_even_though_the_pointer_leaves_it`. Both were
run against the old code first and both failed.

### Undo moved the camera

A snapshot is a whole `Scene`, and the camera is part of a `Scene` because it is
saved with the project (spec section 6.1). Restoring one therefore also restored
where the user was looking from. `undo::restore` keeps the live camera across
both undo and redo.

## Scale, and why it is not resize

The design's position was that a resize rewrites the dimension a shape is
defined by and writes no scale factor anywhere, and that scaling a *group* was
out of scope because doing it truthfully means rewriting every descendant's
dimensions and relative positions.

That position is kept. Scale is added **beside** resize rather than in place of
it, and the difference is the point:

| | writes | works on | asks |
| --- | --- | --- | --- |
| Resize | the dimension | an axis some parameter governs | "how big" |
| Scale | `Node::scale` | anything, groups included | "how many times" |

`Node::scale` is a `Vec3` applied in the node's own axes, after the anchor and
before the rotation, and it composes down the tree like any other transform.
Three things had to change under it:

- **`Xform::inverse` is a general 3×3 inverse now.** It was the transpose, which
  is only the inverse while the linear part is orthonormal — and a scale is
  precisely what makes it not. A transposed scaled matrix divides where it should
  multiply. A degenerate matrix (only a zero scale produces one, and
  `Node::sane_scale` does not let one through) inverts to the identity rather
  than to a field of NaN.
- **`Drag::start_extent` measures in world millimetres.** A drag is measured on
  screen, and the screen shows the node after its scale; the conversion back into
  a dimension, or into a factor, happens in one place, in `size_axis`. Before
  this the corner-drag proportions code was already comparing a world distance
  against a local extent, which was harmless only because nothing could scale.
- **`Gizmo::axis_scale`** is the length of `own`'s unnormalised columns: how many
  world millimetres one local millimetre covers along each axis, this node's
  scale and every ancestor's. `own_scale` is the node's own, and the difference
  between them is what a shift of `Node::position` — which lives in the parent's
  frame — has to be divided by.

The anchor is applied *before* the scale, which is what keeps a base-anchored
shape standing on z = 0 whatever it is scaled by. There is a test for it.

`scale` is left out of the project file when it is `1, 1, 1`, so a project
without one is byte-identical to what an earlier version wrote.

## The library of saved primitives

`simple3d_core::library` is one file per entry in `library/` under the config
directory, holding the same `Clip` the clipboard uses — which is the same
`NodeData` schema as the project file, for the third time. Nothing new had to be
serialised.

Two things worth knowing:

- **`clipboard::paste` gained a sibling, `insert`, that says whether the arriving
  node is a copy.** A saved primitive is not a copy of anything in the project it
  lands in, and calling it "Bracket copy" is a lie the user then has to correct.
- **A name is a file name.** `library::sanitise` replaces the characters neither
  platform accepts rather than refusing the name, because a saved primitive
  silently failing to save is worse than one with a tidied name. A name with
  nothing left in it is refused.

The library is per user and is not undoable: it is not part of any document.

## Everything else

- **Placement.** `config::Placement` — origin, 3D cursor, view centre, beside the
  selection. `App::insertion_point_world` computes the world point and carries it
  back through the parent's frame before writing it into `Node::position`, which
  is in the parent's coordinates: without that, adding into a rotated group put
  the shape somewhere else entirely. There is a test that adds into a group
  turned 90° and moved 50 mm.
- **The step is not the grid.** `SceneSettings::snap_step`, defaulting to 1 mm.
  They were one number, which meant a 1 mm step forced a 1 mm ground grid — a
  solid block of lines. The status bar shows both.
- **`SceneSettings::axes_visible: [bool; 3]`**, three commands, three menu
  entries, three checkboxes, `Alt+X/Y/Z`.
- **`Scene::is_shown`** answers "is this actually drawn", walking up the
  ancestors — the question `node.visible` does not answer for a child of a hidden
  group. It replaces a private copy in `pick.rs` and is what the selection
  outline and the ghost pass now ask.
- **The outliner's context menu collects the chosen command and runs it after the
  menu has laid out.** Labelling a button borrows the keymap; running a command
  wants the whole application. Collecting first is the only arrangement that
  borrows once.

## The window's decorations

`simple3d-app` now depends on `winit` directly, on Unix that is not macOS, for
one feature: `wayland-csd-adwaita`. GNOME and the wlroots-style compositors draw
no decorations for a client, and winit only draws its own when that feature is
on. eframe does not re-export it, so naming winit directly and letting feature
unification apply it to the winit eframe already builds is the whole mechanism —
nothing in this crate calls into winit. `sctk-adwaita` is now in the build,
which is the observable half of the change.

**It has not been looked at.** Nothing here can screenshot a Wayland surface, and
the application's own frame tests run headless. The reasoning is sound and the
dependency is present; the title bar, its buttons and the resize edges want a
pair of eyes. That is in `KNOWN_ISSUES.md` under "Unverified rather than
known-good".

## Still not built, and why

- **The four later items** the issue list marks for a later pass are in
  `KNOWN_ISSUES.md` under Open, untouched: the origin axes behaving as other 3D
  software's do, a file-manager icon for `.simple3d`, a mark on an object where a
  level hint meets it, and a colour per object or group.
- **Scale is not offered in the export dialog's terms.** The export scale factor
  and a node's scale are two different numbers with the same name, and nothing
  reconciles them; they do not interact, and the export scale still multiplies
  the finished mesh.
- **A group's scale is not bakeable.** There is no "apply scale", which would
  rewrite every descendant's dimensions and clear the factor. That is the
  operation the original design called out of scope, and it still is — the
  factor makes it unnecessary rather than easy.
- **The 3D cursor is still session state.** Unchanged: not saved with the
  project, because saving it would change the file format. Now that Placement
  names it, that decision is at least visible.
- **The expression reader has no functions and no variables.** Unchanged.
