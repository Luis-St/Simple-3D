# Known issues

**This file is the issue list.** Everything open lives here, newest first;
anything fixed moves down to "Resolved" with a write-up of what was actually
wrong. There is no separate tracker.

## Open

Nothing. The two entries that were here are fixed and written up below, the
audit (`python3 tools/criteria_audit.py`) reports all 29 acceptance criteria
asserted by a test that names them, and the fixes were confirmed by driving the
running application, not only by their tests.

That is a statement about this moment, checked at this moment. It is not a claim
that the code is without fault -- the drag bug written up below had been shipping
for as long as drags have existed, and no test or reading of the code found it;
driving the app did.

## Worth knowing rather than fixing

- **`App` *is* reachable from a test.** Earlier notes here claimed the opposite,
  and used it to justify leaving criteria 26 and 28 unasserted. It is wrong:
  `egui::Context::default()` needs no window, no display and no GPU, so
  `App::new(&egui::Context::default(), None)` builds a working application in a
  test, and `App::run(command)` dispatches commands through the real path. This
  is what `app::tests` is built on. Give each such test its own config directory
  via `App::with_config_dir`; `App::new` reads the user's real one.

- **A one-frame drag test proves less than it looks.** `Drag::update` is called
  every frame against a gizmo the viewport rebuilds every frame, and a bug can
  hide entirely in the difference between the first call and the second -- one
  did, for as long as drags have existed (see below). A drag test that calls
  `update` once cannot see that class of fault at all. Drive several frames, and
  rebuild the gizmo between them, as `app::drag_gesture` does.

- **The boolean kernel classifies planes with a fixed epsilon** (`csg_bsp::EPSILON`,
  1e-5 mm) rather than exact or rational arithmetic. Pathologically thin or
  near-degenerate input can therefore still produce a non-manifold result. That
  is caught, not shipped: `Mesh::manifold_issue` gates every evaluation, the
  offending node is named in the outliner, and export refuses while the error
  stands (spec section 5.2).
- **Retriangulating a flat region is allowed to give up.** A boundary that
  pinches at a vertex, a loop that will not close, a polygon that runs out of
  ears -- any of these leave that region's original triangles in place. The
  result is a denser mesh, never a wrong one, and `repair::heal` additionally
  refuses the whole rebuild if it is less sound than what it replaced.

## Resolved

### A drag chased its own tail, and finished wherever the mouse button came up (fixed)

Found by driving the running application while checking the two fixes below --
no test caught it, and reading the code did not either.

`Drag::update` was handed the **live** gizmo every frame, and the gizmo is
rebuilt each frame from the very node the drag is moving. Its `origin` therefore
slid along with the result. `ray_axis(cursor, origin, axis)` measures the cursor
*relative to that origin*, so once the node had moved 10 mm the cursor measured
10 mm nearer, the computed delta fell to zero, and `write_position` -- which
works from `start_position` -- put the node back where it began. The next frame
the origin was back at the start, the delta was 10 mm again, and the node moved.
Frame by frame the position flipped between the two, and a completed drag
finished at whichever of them the last frame happened to land on.

Live symptom: drag the X handle 10 mm and release, and the plate returns to
where it started about half the time. Against the previous build the same
scripted gesture ended at 10 mm; a rebuild of the same commit ended at 0.

Why nothing caught it: every existing drag test called `Drag::update` **once**,
against the gizmo built before the move. One update is always correct -- the
oscillation needs a second frame with a rebuilt gizmo. The test written for
criterion 23 in this same pass had the same blind spot until the app disproved
it.

The fix is for `Drag` to own the `Gizmo` it began against and measure everything
in that frame, which is what a drag means: `update` no longer takes a gizmo at
all. `app::a_drag_lands_in_the_same_place_however_many_frames_it_took` drives 1,
2, 3, 4, 5, 20 and 21-frame gestures through all five handle kinds, rebuilding
the gizmo every frame as the viewport does, and requires the same landing from
each. Restoring the old behaviour fails it on the 2-frame case.

### A cancelled drag left a dead undo step (fixed)

`manipulate` recorded an undo snapshot when a drag began and never took it back
when Escape cancelled. Escape already restores the pre-drag values itself, so the
snapshot described the state the scene was already in: the next Ctrl+Z appeared
to do nothing, and the user had to press it twice to reach the edit before the
drag. `History::discard_last` drops a recorded-then-abandoned snapshot, and the
`Cancel` phase calls it. Covered by `app::a_cancelled_drag_leaves_no_undo_step_behind`
and confirmed in the running app: nudge, drag, Escape, one Ctrl+Z, and the status
bar reads "Undid Nudge".

### Criterion 23's "a completed drag undoes in one step" was not asserted (fixed)

The record sat inside `panel_viewport::manipulate`, which was driven entirely by
an `egui::Response` -- and a `Response` cannot be built outside a running frame.
The phase decision is now `gizmo::drag_phase(dragging, PointerState) -> DragPhase`
over plain booleans, and carrying it out is `App::manipulate_step`; `manipulate`
is reduced to translating the `Response` into that `PointerState`. The undo
record happens on `Begin` and on no other phase, which is the whole mechanism, so
`a_completed_drag_undoes_in_one_step_however_many_frames_it_took` can drive a
twenty-frame gesture and require exactly one step. Verified live as well.

### `App::new` read the user's real config directory (fixed)

`App` now holds a `config_dir`, `App::new` defaults it to `config::config_dir()`
and `App::with_config_dir` overrides it; `config` gained
`load_settings_from`/`save_settings_to` to match the keymap pair. Every save site
goes through `App::persist_keymap`, so a running application cannot read one
directory and write another. `app::tests` each get their own temp directory
instead of overwriting `settings` and `keymap` after construction, and
`the_default_config_directory_is_the_users_own` keeps a shipped binary pointed at
the right place.

This also closes criterion 28's last untested link:
`a_restarted_app_starts_on_the_keymap_the_last_one_saved` builds an `App`, rebinds,
persists, drops it, and builds a second `App` over the same directory. Confirmed
in the running app too -- orbit remapped to the middle button, quit, relaunch, and
the new binding is both in the dialog and in effect.


### Four criteria were cited only from a doc comment, not from a test (fixed)

The audit that is supposed to find these — "which of the 29 criteria does a test
cite by name?" — was being run as `grep -rn "criterion" crates/` and the hits
counted. That over-reports, because a module-level or function-level doc comment
citing a criterion is a hit too. Four criteria were resting on one.

`tools/criteria_audit.py` is that audit done properly: it resolves each citation
to the item it belongs to and only counts it when that item carries `#[test]`.
Run it before writing any claim about criterion coverage into a document; it
exits non-zero and names the gaps. The criteria it found:

- **19 — starts with no accelerated graphics.** Cited from `raster.rs`'s module
  doc. The rasterizer's own tests do prove the drawing happens on the CPU, but
  nothing asserted the *starting*. `app::the_application_starts_with_no_graphics_at_all`
  now builds the whole application on a headless `egui::Context` and runs a
  command through it.
- **29 — remap the orbit button, navigation follows without a restart.** Cited
  from the doc comment on `panel_viewport::navigate`, which was one long
  function reading pointer state and mutating the camera in place. The gesture
  decision is now `nav_gesture(&NavMap, held, ctrl, shift, alt) -> Option<Gesture>`
  and the camera update `apply_gesture`/`apply_zoom`, none of which hold state —
  which is precisely *why* there is nothing to restart, and is now the thing
  `remapping_the_orbit_button_takes_effect_without_a_restart` asserts.
- **13 — the 200-primitive scene.** A whole test file is devoted to it, so this
  one was only a misplaced comment: the citation lived in `performance.rs`'s
  module doc rather than on either test. Both tests now carry it.
- **26 and 28**, which were the acknowledged open entry. See below.

Two criteria were cited from real tests that covered only part of what they ask:

- **27** asks for a preset switch *and then* a conflicting rebind, in that order.
  `switching_preset_changes_navigation_and_mode_keys` covered the first half and
  `rebinding_onto_a_used_combination_names_the_holder` the second, starting from
  a default map. `a_conflict_is_named_after_switching_preset_too` does the
  sequence the criterion describes.
- **20**'s "is left selected" clause is `App`'s, not the clipboard's, and was
  unasserted. `app::a_pasted_copy_is_left_selected` covers it, including that a
  nudge immediately afterwards moves the copy and not the original.

### Acceptance criteria 26 and 28 were implemented but not asserted end to end (fixed)

Both behaved correctly; neither was tested. The note here said the arithmetic was
"unreachable from a test" because `App` needs a live `egui::Context`. **That
premise was wrong** — see "Worth knowing" above — and it is worth recording,
because it is what kept these two open. The refactors below are still the right
shape, but they were an improvement rather than a precondition.

- **26.** The axis/sign/step arithmetic moved out of `App::nudge` into
  `gizmo::nudge_step` -> `Nudge` -> `Nudge::apply`, with the undo record joined
  to it in `gizmo::apply_nudge` so the coalescing is part of what a test drives
  rather than call-site glue that a test can skip past. `App::nudge` is now the
  status line and nothing else. Covered at both levels:
  `gizmo::a_held_nudge_run_steps_by_the_snap_and_undoes_in_one` for the
  arithmetic and the single undo step, and
  `app::holding_an_arrow_key_nudges_by_the_snap_and_undoes_in_one_step` from
  `App::run(Command::NudgeRight)`, which is what a keypress actually reaches.
  Verified by regression: pointing `apply_nudge` at `record(.., None)` fails the
  first of those.
- **28.** `config` gained `load_keymap_from(dir)`/`save_keymap_to(dir)`, with
  `load_keymap`/`save_keymap` as one-line wrappers over `config_dir()`, so
  `a_rebinding_survives_a_restart_and_the_menus_follow` can drive the real
  save-then-load path against a temp directory. It asserts the reloaded map, its
  preset, and that `shortcut_text` — what the menus render — differs from the
  default. The one link still untested is `App::new`'s call to `load_keymap`;
  that is in "Open" above, with what it would take.

### The 200-primitive performance target was missed by two orders of magnitude (fixed)

The spec's acceptance criterion 13 scene -- fifty assemblies, each a plate with a
hole and a slot cut from it plus a boss unioned on, so 200 primitives inside 100
nested boolean groups -- took **~10 s** to evaluate cold and **~9.6 s** to update
after a one-dimension edit, against a target of "well under a second". It now
takes ~0.12 s and ~6 ms. `crates/scadstudio-core/tests/performance.rs` asserts
both, and `a_single_value_edit_reuses_the_cache` is no longer `#[ignore]`d.

The old note here blamed the fifty inner *difference* groups and said the root
union had been ruled out. That was backwards, and the reasoning that ruled it out
is worth recording because it is an easy trap.

`evaluate_boolean` did skip the BSP kernel for operands whose bounding boxes do
not overlap -- but it folded `union` over the operands, so the accumulator's box
was the box of *everything unioned so far*. The first few assemblies really are
disjoint from that box and skip the kernel; by the eleventh, the accumulated box
spans two rows of the build plate and every remaining assembly's box falls inside
it. From there on each one is run through a BSP tree of the whole pile. Measuring
the skip's effect on the *cold* run (10.4 s to 10.0 s) hid this completely,
because the skip does fire -- for the first ten operands out of fifty.

`union_all` keeps the accumulated result as a set of **mutually disjoint parts**,
each with its own box, and only invokes the kernel against the parts an operand
can actually touch. Merging two parts grows the merged box, which can bring it
into contact with a part that was previously clear, so the search restarts until
nothing overlaps. `a_union_of_scattered_solids_never_reaches_the_kernel` and
`merging_two_islands_still_catches_a_third_that_now_touches` cover both halves.

The mesh-density fix below compounded it: 73,900 triangles became 11,400.

### Boolean output was denser than the solid it described (fixed)

A BSP boolean clips against *infinite* planes, so subtracting a 16-segment
cylinder from a 40 x 20 mm plate sliced the plate's entire top and bottom face
along sixteen lines running right across it. A plate with a hole, a slot and a
boss arrived at ~1500 triangles for a solid ~230 describe, and every later
boolean in the chain paid for those triangles again.

`csg_bsp::try_merge_group` already recombined each *input* face's triangulation
before the BSP saw it, which stopped the growth compounding for simple faces, but
it bails on a face with a hole in it or a concave one -- exactly the faces an
earlier boolean produces.

The fix is `planar::retriangulate_flat_regions`, run from `repair::heal`: group
the output triangles by plane, recover each region's boundary loops from the
edges used once, bridge the holes into the outer loop, and ear-clip. Two things
that were not obvious while building it:

- **Ear clipping stalls on a healed boundary.** `split_t_junctions` deliberately
  fills the boundary with collinear vertices so this face's edges match the
  neighbouring face's. A collinear vertex is never a valid ear apex, so a run of
  them can be left as the final three vertices with no ear to take. Dropping them
  as zero-area ears reopens the T-junction the healer just closed. What works is
  to strip them before triangulating and run `split_t_junctions` *again*
  afterwards -- the neighbouring faces still have their corners there, so exactly
  the same splits come back, into far fewer and larger triangles.
- **Compact between the two passes.** Retriangulating orphans every vertex that
  was interior to a flat region, and those orphans sit *on* the large new
  triangles that replaced them. Left in `positions`, the second T-junction pass
  finds them all as on-edge vertices and splits the mesh straight back to where
  it started (measured: 100 triangles back up to 398).

`repair::heal` keeps the rebuild only if it is smaller *and* at least as sound as
what it replaced, so a region the pass cannot interpret costs nothing.

### A group never reported its measured size (fixed)

`Evaluator::walk` recorded a world-space mesh for primitives only, and the
property editor derived the "Measured" panel from that -- so selecting any group,
including the scene root, showed "No geometry yet." permanently rather than
transiently. `Evaluated::node_world_bounds` now covers every node, measured over
the subtree's transformed points rather than by transporting its local box (which
would report a rotated 40 mm plate as 42 mm across, the box's diagonal).

### BSP CSG kernel left unpaired edges on `subtract`/`intersect` (fixed)

The two failing tests were **not** a transcription error in the `csg.js` port,
which is why diffing it method-by-method against the reference found nothing.
Two separate defects were at work, both in the mesh that comes *out* of the
BSP rather than in the tree algorithm:

1. **T-junctions.** The BSP clips whole polygons against the other solid's
   tree, so two polygons sharing a physical edge are not split at the same
   points along it. Subtracting a cylinder from a plate splits the plate's top
   face along the cylinder's plane at `y = 4`, which crosses the top face's
   `x = ±20` boundary edges — but the plate's side face at `x = 20` lies
   entirely on one side of that plane and is never split. The shared edge ends
   up with three vertices on one side and two on the other. No gap in the
   surface, but not edge-manifold, and slicers reject it. This is inherent to
   the algorithm.

   Fixed by `repair::split_t_junctions`: repeatedly split any triangle edge
   that has another mesh vertex lying on its interior. `repair::weld_tolerant`
   feeds it, replacing the old rounding-bucket weld that could miss a
   coincident pair straddling a bucket boundary.

2. **Zero-area triangles.** `try_merge_group` reconstitutes a flat face's
   triangulation into one boundary loop, and that loop legitimately contains
   collinear vertices (a previous boolean's T-junction points). Fan-triangulating
   from a vertex that sits inside — or next to — such a collinear run emits
   degenerate triangles, whose three edges are still counted by the manifold
   check. Fixed by choosing a fan apex whose neighbours are also genuine
   corners, with `repair::drop_slivers` as a backstop.

Two smaller correctness fixes came out of the same work:

- `try_merge_group` now rejects a **concave** merged loop. Both
  `split_polygon` and the fan triangulation assume convexity, so an L-shaped
  face left behind by an earlier boolean was being mis-split and
  mis-triangulated. Such a group falls back to per-triangle polygons.
- `try_merge_group`'s internal maps were `HashMap`, and it picks the loop's
  starting vertex by iteration order. `HashMap`'s order is randomised *per
  instance*, so identical input could produce different meshes within a single
  process — a direct violation of the spec's determinism requirement (section
  5.2), and the reason the old symptom looked flaky. They are `BTreeMap`s now,
  and `boolean_evaluation_is_deterministic` covers it.

### Coplanar-triangle merge for BSP input

`primitives.rs` triangulates every flat face with an arbitrary internal
diagonal. Feeding those in as separate 3-vertex polygons let a neighbouring
face, clipped at a different point along the same physical boundary edge,
produce a T-vertex. `csg_bsp::try_merge_group` undoes each flat face's internal
triangulation back into a single convex polygon before the BSP sees it.
Exercised by `rounded_box_is_manifold`, `wedge_is_manifold` and the boolean
tests.
