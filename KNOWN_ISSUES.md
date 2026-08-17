# Known issues

## BSP CSG kernel: `subtract`/`intersect` can leave unpaired edges on real geometry

**Status:** open, actively being debugged. Reproduce with:

```
cargo test -p scadstudio-geom boolean_diff_simple_boxes boolean_difference_hole_in_plate_is_manifold
cargo run -p scadstudio-geom --example csg_bug_repro
```

Both fail deterministically on current `main`. 22/24 geometry tests pass;
these two (plus, transitively, anything that unions/intersects/subtracts
non-trivially) are the current blocker before `scadstudio-core`'s boolean
group evaluation can be trusted.

### What's confirmed

- `crates/scadstudio-geom/src/csg_bsp.rs` is a line-by-line port of Evan
  Wallace's public-domain `csg.js` BSP algorithm (`Plane.splitPolygon`,
  `Node.build/invert/clipPolygons/clipTo/allPolygons`, and the
  `union`/`subtract`/`intersect` call sequences). I re-fetched the reference
  source and diffed it against the Rust port method-by-method; I could not
  find a transcription discrepancy.
- The bug is **not** caused by the coplanar-triangle-merging step
  (`try_merge_group` / `mesh_to_polygons`) added to fix T-vertices between
  independently-triangulated faces (see below) — I verified this by
  temporarily disabling the merge and reproducing the same class of failure
  on plain per-triangle input.
- The bug **is** order-dependent: with the original `HashMap`-based grouping
  (nondeterministic iteration order per process), a trivially disjoint union
  of two boxes 1000mm apart — which should be a pure pass-through with zero
  actual plane splitting — came back with anywhere from 24 (correct) to 76
  triangles across repeated runs of the identical input. Forcing a specific
  processing order (`.rev()` on the now-deterministic `BTreeMap` grouping)
  reproduces a *tripling* of triangle count (6 polygons -> 18) from a single
  `BspNode::clip_to` call between two spatially disjoint boxes, where no
  vertex of either box should classify as anything but cleanly FRONT or BACK
  at every plane of the other's tree (verified by hand: the boxes are ~980mm
  apart on the clipping axis, nowhere near the 1e-5 epsilon).
- `mesh_to_polygons` grouping was switched from `HashMap` to `BTreeMap`
  specifically so evaluation is deterministic (also a spec requirement,
  section 5.2) — this incidentally makes the two box-pair test cases above
  *reliably* fail every run instead of flakily, which is strictly easier to
  debug against, but it also means the disjoint-union symptom described
  above no longer reproduces with today's code without manually forcing a
  bad ordering again (see `git log` / this file's history for how, or just
  work from the two failing unit tests instead — same underlying bug,
  triggers reliably as-is).

### Working theory / where to look next

Since a spatially trivial case can still trigger spurious splitting once the
BSP tree has a particular shape, suspect the recursion in `BspNode::build`
or `BspNode::clip_to`/`clip_polygons` — most likely something that causes a
polygon to be visited/split more than once as it's threaded through nested
front/back children, or a case where `self.plane` classification disagrees
between two calls on numerically-identical input (shouldn't happen with
plain `f64` and no mutation of already-built planes, but worth instrumenting
first). Suggested next step: add temporary `eprintln!` tracing back into
`op()` (removed before this commit to keep normal runs quiet) and step
through the *smallest* failing case — `boolean_diff_simple_boxes` — rather
than the disjoint-union case, since it fails deterministically now.

Also worth trying as a time-boxed fallback if the recursive-tree bug proves
hard to pin down: reimplement `evaluate_boolean` using an *exact* or
higher-level approach less sensitive to tree shape (e.g. a non-recursive/
flattened BSP clip, or classifying+re-triangulating per convex cell instead
of the classic recursive node structure) rather than continuing to
patch the current port.

## Coplanar-triangle merge for BSP input (fixed, documented for context)

`crates/scadstudio-geom/src/primitives.rs` generators triangulate every flat
face (a box wall, a fan-triangulated cap) with an arbitrary internal
diagonal. Feeding those triangles into the BSP kernel as separate 3-vertex
polygons meant a neighbouring face, clipped at a *different* point along the
same physical boundary edge, could produce a T-vertex — a real edge on one
side of the cut with no exact-matching partner on the other, because the
diagonal's own clipped fragment introduced an extra vertex the neighbour
never generated. `csg_bsp::try_merge_group` undoes each flat face's internal
triangulation back into a single convex polygon (by dropping edges shared by
two triangles of the same coplanar group and chain-walking what's left into
one boundary loop) before the BSP algorithm ever sees it. This is confirmed
working via `rounded_box_is_manifold`, `wedge_is_manifold`, and the
`boolean_union_touching_at_edge_is_manifold` / `boolean_intersect_simple_boxes`
tests, all of which exercise it. It intentionally bails out (falls back to
per-triangle polygons for that face) if a plane group's boundary isn't a
single simple loop -- e.g. a face that's already the result of an earlier
boolean and genuinely has a hole in it -- so it should be safe to leave in
place while the subtract/intersect bug above is chased down.
