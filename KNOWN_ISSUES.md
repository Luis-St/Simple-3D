# Known issues

## Boolean output is denser than it needs to be

A BSP boolean clips against *infinite* planes. Subtracting an 8 mm 24-segment
cylinder from a 40 × 20 mm plate therefore slices the plate's entire top and
bottom face along 24 lines that run right across it, not just around the hole.
The result is correct, watertight and manifold, but a plate with one hole and
one boss comes out at ~1600 triangles where ~300 would describe the same solid.

`csg_bsp::try_merge_group` recombines each *input* face's triangulation before
the BSP sees it, which stops the growth compounding for simple faces, but it
deliberately bails on a face that has a hole in it or is concave — exactly the
faces an earlier boolean produces. So a long chain of booleans on the same face
still accumulates triangles.

The real fix is to retriangulate each coplanar region of the *output* from its
boundary loops (which needs a polygon-with-holes triangulator: ear clipping
plus hole bridging). A cheaper greedy "merge coplanar neighbours while the
union stays convex" pass was tried and rejected: on the representative
plate-with-hole-and-boss case it removed only 5% of the triangles, because the
regions left behind by the infinite-plane cuts are mostly concave.

Practical impact today: exports are larger than ideal and deep boolean chains
get slower than the spec's 200-primitive target would like. Correctness is not
affected — `Mesh::manifold_issue` gates every evaluation result.

## Resolved

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
