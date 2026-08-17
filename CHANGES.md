# Change report — 2026-08-18

Closing out the two open entries in `KNOWN_ISSUES.md`, one bug found by driving
the running application, and the packaging automation the spec asks for.
`KNOWN_ISSUES.md` now has no outstanding entries.

Full write-ups of what was actually wrong in each case are in that file's
"Resolved" section; this is the index.

## Fixed

| | Before | After |
|---|---|---|
| 200-primitive scene, cold evaluation | ~10 s | ~0.12 s |
| 200-primitive scene, one-dimension edit | ~9.6 s | ~6 ms |
| 200-primitive scene, triangles | 73,900 | 11,400 |
| Plate + hole + slot + boss, triangles | ~1,500 | 228 |

1. **The union fold defeated its own disjointness test.** `evaluate_boolean`
   skipped the BSP kernel for non-overlapping operands, but folded over them, so
   the accumulator's bounding box grew to span everything unioned so far. By the
   eleventh of fifty assemblies every remaining one looked like it overlapped and
   went through a BSP tree of the whole pile. `union_all` now keeps the result as
   a set of mutually disjoint parts, each with its own box.
   (`crates/scadstudio-geom/src/lib.rs`)

2. **Boolean output was far denser than the solid it described.** New
   `crates/scadstudio-geom/src/planar.rs` rebuilds each flat region of a boolean
   result from its own boundary loops — bridging holes into the outer loop and
   ear-clipping — instead of leaving the fan of slivers an infinite-plane clip
   produces. Wired into `repair::heal`, which keeps the rebuild only if it is
   both smaller and at least as sound as what it replaced.

3. **Groups never reported a measured size.** `Evaluator::walk` recorded world
   bounds for primitives only, so selecting any group — including the scene
   root — showed "No geometry yet." permanently. Found by driving the running
   app. `Evaluated::node_world_bounds` now covers every node.
   (`crates/scadstudio-core/src/eval.rs`, `scadstudio-app/src/panel_properties.rs`)

## Added

- `.github/workflows/release.yml` — the automated build the spec's section 11
  requires and `rust-toolchain.toml` already referenced: portable single-file
  executables for Linux and Windows from one commit, version taken from the tag.
  `rust-toolchain.toml`'s Windows target changed from `-gnu` to `-msvc`, since
  the workflow builds natively on a Windows runner rather than cross-compiling.
- `rustfmt.toml` recording the width the codebase was already written to, so
  `cargo fmt` agrees with the tree instead of reflowing all of it. The tree is
  formatted and `cargo fmt --all --check` is clean.
- `crates/scadstudio-geom/examples/boolean_cost.rs` — per-step time and triangle
  count for one assembly, for spotting a regression in either.

## Tests

258 passing, none ignored, in both debug and release.

- `a_single_value_edit_reuses_the_cache` is no longer `#[ignore]`d, and the cold
  ceiling dropped from 40 s to 5 s.
- New in `scadstudio-geom`: `a_union_of_scattered_solids_never_reaches_the_kernel`,
  `merging_two_islands_still_catches_a_third_that_now_touches`,
  `a_boolean_result_is_no_denser_than_the_solid_it_describes`,
  `rebuilding_a_flat_region_keeps_its_boundary`,
  `a_region_that_cannot_be_rebuilt_keeps_its_original_triangles`.
- New in `scadstudio-core`: `a_group_measures_the_assembly_it_evaluates_to`,
  `a_rotated_group_is_measured_over_its_geometry_not_its_box`.
- `a_drilled_plate_exports_as_a_watertight_solid` now reads the geometry back out
  of the written file — bore open, in the right place, surface closed — rather
  than only checking the file is non-trivially sized.

## Verified in the running application

Driven with simulated input against the release binary. Acceptance criteria
confirmed end to end: 4 (hole round, in place, watertight), 6 (metres reads
0.04 × 0.02 × 0.004 and back), 10 (group in one action), 12 (viewport picking),
14 (garbage in a numeric field reverts silently and is named in the status bar),
plus the About dialog's version and settings path (section 11).

A 3MF written by the binary was checked by hand: valid container,
`unit="millimeter"`, exact 40 × 20 mm, every edge shared by exactly two
triangles.

## Also

- `README.md` claimed three of the four crates were empty stubs, which stopped
  being true several commits ago. Rewritten to describe what is actually there,
  with a table of where to start for each kind of change.
- The file-save dialog is an XDG desktop portal, outside the reach of the X11
  automation used here; that one step was completed by hand.
- The pre-existing clippy warnings (28, all stylistic — `needless_range_loop`
  where indexing two arrays together is clearer, and similar) were left alone
  rather than churned. `planar.rs` is clippy-clean.
