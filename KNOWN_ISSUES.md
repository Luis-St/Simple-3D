# Known issues

**This file is the issue list.** Everything open lives here, newest first;
anything fixed moves down to "Resolved" with a write-up of what was actually
wrong. There is no separate tracker.

## Open

### Acceptance criteria 26 and 28 are implemented but not asserted end to end

Every other criterion is cited by name from the test that covers it
(`grep -rn "criterion" crates/`). These two are not, and the gap is real rather
than a missing comment:

- **26 — nudge with the arrow keys, hold to repeat.** The behaviour is there:
  `App::nudge` steps by `move_snap()` and passes a stable coalesce key
  (`nudge:{id}:{mode}`) so a repeat run collapses into one undo step. What is
  missing is a test tying the two together. The pieces are covered separately --
  `undo::rapid_edits_to_one_field_are_one_step` proves coalescing works given a
  stable key, and `gizmo::a_nudge_left_really_goes_left` proves the direction --
  but nothing asserts that a nudge *run* is one step, or that the step equals the
  snap increment.
- **28 — restart after rebinding.** `keymap::a_keymap_round_trips_through_its_file_form`
  covers the serialised form and `ui::menu_labels_show_the_current_binding`
  covers the menus, but nothing exercises `config::save_keymap` ->
  `config::load_keymap`, which is the actual on-disk path used at startup. The
  "persisted across a restart" half rests on untested glue.

Closing both wants a small refactor first, since the logic is currently
unreachable from a test: lift the axis/sign/step arithmetic out of `App::nudge`
(which needs a live `egui::Context` to construct) into `gizmo`, and give
`config` `load_keymap_from(dir)`/`save_keymap_to(dir)` variants so the disk path
can be driven against a temp directory instead of the user's real config.

## Worth knowing rather than fixing

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
