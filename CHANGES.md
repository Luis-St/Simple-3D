# Change report — 2026-08-18 (second pass)

Closing the open entry in `KNOWN_ISSUES.md` (acceptance criteria 26 and 28), plus
four more criteria that an audit showed were not asserted either — and were being
reported as covered by an audit method that over-counts.

`KNOWN_ISSUES.md` is this project's issue list: open items at the top, fixed ones
written up below with what was actually wrong. Two items are open, both recorded
there with what makes them hard. This file is the index.

## The headline

**All 29 acceptance criteria are now cited by name from a test that asserts them**,
verified by `tools/criteria_audit.py`, which exits non-zero and names the gaps.
Before this pass, 23 were. The previous count of 27 came from
`grep -rn "criterion" crates/`, which counts a doc comment as coverage.

| | Before | After |
|---|---|---|
| Criteria asserted by a test | 23 | 29 |
| Tests | 258 | 272 |

## Fixed

1. **Criteria 26 and 28 were implemented but not asserted end to end** — the
   entry that was open. Both were said to be unreachable from a test because
   `App` needs a live `egui::Context`. **That premise was wrong**:
   `egui::Context::default()` needs no window, display or GPU, so `App::new`
   works in a test and `App::run(command)` dispatches through the real path.
   The refactors were still worth doing, and both are now covered at two levels.
   - `App::nudge`'s arithmetic moved into `gizmo`: `nudge_step` → `Nudge` →
     `Nudge::apply`, with the undo record joined to them in `gizmo::apply_nudge`
     so the coalescing is part of what a test drives rather than call-site glue.
     `App::nudge` is now the status line and nothing else.
     (`crates/scadstudio-app/src/gizmo.rs`, `app.rs`)
   - `config` gained `load_keymap_from(dir)`/`save_keymap_to(dir)`, so the real
     save-then-load path runs against a temp directory instead of the user's
     config. (`crates/scadstudio-core/src/config.rs`)

2. **Four criteria were cited only from a doc comment.** 13, 19, 28 and 29 were
   counted as covered by the naive grep; none had a test asserting them.
   - **29 (remap the orbit button, no restart)** needed the same treatment as
     nudge. `panel_viewport::navigate` was one function reading pointer state and
     mutating the camera in place; the gesture decision is now
     `nav_gesture(&NavMap, held, ctrl, shift, alt) -> Option<Gesture>` and the
     camera update `apply_gesture`/`apply_zoom`. None of them hold state, which
     is exactly *why* a rebinding needs no restart — now asserted rather than
     asserted-about.
   - **19 (starts with no accelerated graphics)** is asserted by building the
     whole application headless and running a command through it.
   - **13** was only a misplaced comment; both `performance.rs` tests carry it now.

3. **Two criteria had tests covering part of what they ask.**
   - **27** asks for a preset switch *and then* a conflicting rebind, in that
     order; the two halves were covered by separate tests, neither doing the
     sequence.
   - **20**'s "the copy is left selected" clause belongs to `App`, not the
     clipboard, and was unasserted.

## Added

- `tools/criteria_audit.py` — the criterion-coverage audit done properly. It
  resolves each citation to the item it belongs to and counts it only when that
  item carries `#[test]`; the two comment positions need opposite searches, which
  is the other thing the naive version gets wrong. Run it before writing any
  claim about coverage into a document.
- 14 tests: 4 in `gizmo` (nudge step, snap increment, coalescing rules, the
  ungoverned-axis case), 4 in `app` (headless start, held-key run, mode switch,
  paste selection), 3 in `panel_viewport` (orbit remap, pan/orbit modifier rule,
  invert zoom), 2 in `config` (restart round trip, corrupt keymap file), 1 in
  `keymap` (criterion 27's sequence).

## Not changed

- **`cargo clippy --workspace --all-targets`: 22 warnings, all stylistic, all
  correct as written.** Each was read rather than counted. The ones that could
  plausibly have hidden a bug — `pick.rs`'s manual `!RangeInclusive::contains`
  (a deliberate epsilon on a Möller–Trumbore intersection) and the three
  `map_or` simplifications — are right. One of the 22 is new: `apply_nudge`
  takes 8 arguments, matching the existing 9-argument function beside it.

## Verification

    cargo test --workspace                 272 passed, 0 failed, 0 ignored
    cargo test --workspace --release       272 passed, 0 failed, 0 ignored
    cargo fmt --all --check                clean
    python3 tools/criteria_audit.py        all 29 criteria cited by a test

    cargo test --release -p scadstudio-core --test performance -- --nocapture
        cold 117 ms, one-dimension edit 6.4 ms, 11,400 triangles
    cargo run --release -p scadstudio-geom --example boolean_cost
        assembly 4.15 ms, 228 triangles, manifold yes

The two nudge tests were checked against a deliberate regression: pointing
`apply_nudge` at `record(.., None)` fails
`a_held_nudge_run_steps_by_the_snap_and_undoes_in_one`.

---

# Change report — 2026-08-18 (first pass)

Closing out the two open entries in `KNOWN_ISSUES.md`, one bug found by driving
the running application, and the packaging automation the spec asks for.

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
  `cargo fmt` agrees with the tree instead of reflowing all of it.
- `crates/scadstudio-geom/examples/boolean_cost.rs` — per-step time and triangle
  count for one assembly, for spotting a regression in either.

## Tests

258 passing, none ignored, in both debug and release.
