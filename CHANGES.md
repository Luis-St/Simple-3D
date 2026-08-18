# Change report — 2026-08-18 (third pass)

Closing the last two entries in `KNOWN_ISSUES.md`, and the bug that closing them
turned up: **a manipulator drag finished wherever the mouse button happened to
come up**. That one was found by driving the running application, not by a test.

`KNOWN_ISSUES.md` is this project's issue list. Its "Open" section is empty as of
this pass — checked, not assumed. The write-ups of what was actually wrong are
there; this file is the index.

## The one that mattered

**A drag chased its own tail.** `Drag::update` was handed the *live* gizmo each
frame, and the gizmo is rebuilt each frame from the very node the drag is moving.
`ray_axis` measures the cursor relative to `gizmo.origin`, so once the node had
moved 10 mm the cursor measured 10 mm nearer, the delta fell to zero, and
`write_position` — which works from `start_position` — put the node back at the
start. Next frame the origin was back too, the delta was 10 mm again, and it
moved. The position flipped between the two every frame; a completed drag landed
on whichever the last frame gave.

Every existing drag test called `update` **once**, against the gizmo built before
the move — and one update is always right. The oscillation needs a second frame
with a rebuilt gizmo. The criterion-23 test written earlier in this same pass had
the same blind spot, and would have gone on asserting a lie.

`Drag` now owns the `Gizmo` it began against and measures everything in that
frame, which is what a drag means; `update` no longer takes a gizmo at all.
`a_drag_lands_in_the_same_place_however_many_frames_it_took` drives 1, 2, 3, 4,
5, 20 and 21-frame gestures through all five handle kinds, rebuilding the gizmo
every frame as the viewport does, and requires the same landing from each.
Restoring the old behaviour fails it on the 2-frame case.

## Also fixed

- **A cancelled drag left a dead undo step.** Escape restores the pre-drag values
  itself, so the snapshot taken when the drag opened described the state the
  scene was already in — the next Ctrl+Z did nothing visible.
  `History::discard_last` drops it. (`scadstudio-core/src/undo.rs`)
- **Criterion 23's "a completed drag undoes in one step" was not asserted.** The
  record sat inside a function driven entirely by an `egui::Response`. The phase
  decision is now `gizmo::drag_phase(dragging, PointerState) -> DragPhase` over
  plain booleans and `App::manipulate_step` carries it out; `manipulate` only
  translates the `Response`.
- **`App::new` read the user's real config directory.** `App` holds a
  `config_dir`, with `App::with_config_dir` for tests and `App::new` still
  defaulting to `config::config_dir()`. `config` gained
  `load_settings_from`/`save_settings_to`; every keymap save goes through
  `App::persist_keymap`. This also closes criterion 28's last untested link.

## Verified in the running application

Driven over XWayland with synthetic input, reading the result out of the property
editor rather than out of the code:

| | |
|---|---|
| Criterion 26 | five right-arrow presses → Position X = 50 mm (5 × the 10 mm grid); one Ctrl+Z → 0 |
| Criterion 23 | 15-frame drag of the X handle → X = 10 mm and it **stays**; one Ctrl+Z → 0 |
| Cancelled drag | nudge → X = 10, drag, Escape → back to 10, one Ctrl+Z → 0, status "Undid Nudge" |
| Criterion 29 | orbit remapped to Middle: right-drag changes 0 pixels, middle-drag orbits — no restart |
| Criterion 28 | quit, relaunch: the dialog shows Orbit = Middle and middle-drag orbits |

The regression was caught by running the same scripted gesture against a build of
the previous commit: it ended at 10 mm, the new one at 0.

## Verification

    cargo test --workspace                 278 passed, 0 failed, 0 ignored
    cargo test --workspace --release       278 passed, 0 failed, 0 ignored
    cargo fmt --all --check                clean
    python3 tools/criteria_audit.py        all 29 criteria cited by a test

    performance    cold 117 ms, one-dimension edit 6.4 ms, 11,400 triangles
    boolean_cost   228 triangles, manifold yes

`cargo clippy --workspace --all-targets`: 24 warnings, all stylistic. Three are
"too many arguments" on the functions this pass extracted, matching the ones
already beside them.

---

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
