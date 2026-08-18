# Change report — 2026-08-18 (fifth pass)

The gaps the fourth pass listed against the ScadStudio UI design description,
closed. The numeric input model, movable docks, a clickable view cube,
multi-selection editing in the property panels, a 3D cursor, an inline
confirmation for deleting a group, and messages that fade.

Nothing about geometry, evaluation, export or the file format changed.
`cargo test --workspace` is 320 tests, up from 292, none failing.
`python3 tools/criteria_audit.py` is clean: all 29 acceptance criteria are still
cited by at least one test.

The previous pass's report is in git history; `KNOWN_ISSUES.md` is still the
issue list, and this pass **opened one entry** in it: every pointer gesture added
below -- the header drag, the label scrub, the cube click, the cursor placement --
is unexecuted, because there is no way to press a button at a coordinate on this
machine. The arithmetic under each of them is tested; the wiring between a real
pointer and that arithmetic is not.

## The numeric input model

The design calls this the signature of the application, and none of it existed.
It does now, and the whole of it is in
`scadstudio-core::unit` — a field is a field, so a rule that holds in one holds
in all of them.

A field accepts:

- **An expression.** `40/3`, `12+8`, `(2+3)*4`, `100 - 2*15`. Four operators,
  parentheses, one leading sign per factor. Deliberately no functions and no
  variables: this is arithmetic done in the field instead of in a calculator,
  not a language.
- **A value in another unit.** `4 cm` is 40 in a millimetre document and 0.04 in
  a metre one, and the suffix binds to its own term, so `4cm + 5` is 45 mm. The
  document's unit is what the field *shows*; it was never what the user had to
  *think* in.
- **A delta.** `+2` is two more than whatever is there. With several nodes
  selected it resolves against each of them separately — "two millimetres wider"
  means a different number for every shape it lands on, and that is the point of
  having it.
- **A drag on its label.** The label is the grip, not the field: a click in a
  text field has to put a caret where it was clicked, and a drag in it has to
  select text. The label beside it has no such job. Shift is fine, Ctrl is
  coarse — the same two modifiers the manipulator already uses for the same two
  meanings. On an axis row the colour chip is the grip, because the chip *is*
  that field's label.

**A leading `-` is only a delta when a space or an `=` follows it** — `- 5` and
`-= 5` adjust, `-5` is still minus five. This is a departure from the design,
which asks for a leading `+` or `-`. A position field has to be able to hold a
negative number, and no field can read the same six keystrokes two ways; `+` has
no such conflict, so a bare `+5` is a delta. The alternative was to make `-5`
mean "five less" and leave no way to type minus five at all.

**A scrub is one undo step.** The snapshot is taken on the frame the drag starts
and every frame after it only touches the scene — the same shape the manipulator
drag already had. `a_whole_scrub_is_one_undo_step` drags forty frames and checks
the stack grew by one, and that one undo puts every node back.

**A refused value keeps the old one and marks the field.** It never clears it:
the text that was typed is the thing that has to be corrected, and throwing it
away to show the previous value is the least useful thing a field could do. The
mark is the field's own frame in the danger colour, so it is impossible to read
the number without also reading that it was refused. Nothing partial ever
happens across a multi-selection: if one node cannot take the value, none of
them do.

## Docking

`dock.rs` is new, and the layout model it drives is in
`scadstudio_core::config::Layout`, beside the dock widths that were already
persisted there.

- Three panels — Outliner, Primitives, Properties — move between the two docks
  and reorder within one by dragging their header. The tool rail, menu bar,
  status bar and viewport are the window's frame and do not move: a rail that
  can end up somewhere else is a rail you have to go looking for.
- A header click rolls its panel up to that bar. The dock's leftover height goes
  to the last panel that is not rolled up, so a dock of nothing but headers is a
  reachable state and draws.
- `Tab` hides both docks. Hiding is one flag rather than a saved copy of the
  arrangement, so restoring is exact by construction rather than by care — the
  test says so by comparing the layout across a hide and a show.
- View ▸ Panels moves a panel without dragging, View ▸ Reset layout puts
  everything back (`Ctrl+Shift+L`).
- A layout file that has lost or duplicated a panel is **repaired** on load
  rather than left as it is: a panel that appears in neither dock would have no
  way back, and nothing in the interface could bring it there.

**The layout is per user, not per document.** The prompt asked for per document
and also asked that `AppSettings` be extended rather than a second store
invented; those pull opposite ways, and this is the direction I took. Panel
positions are how one person likes their window, not a property of the model,
and putting them in the project file would mean opening a colleague's model
rearranged your workspace — and would change the file format, which no pass has
done.

### One egui trap worth recording

egui remembers a side panel by the rectangle its *contents* filled, not the one
it was given. The property panel's contents happened to be narrower than the
dock, so the dock shrank to fit them — and, being remembered, kept shrinking
until it hit its minimum. The right dock came up 200 px wide instead of 320 with
its own fields clipped off the edge of the window. The dock now claims its full
width before drawing anything into it. This was found by looking at the window,
not by a test, which is the second pass running where that has been true.

## The view cube

It was drawn but inert, and it was also **wrong**: its projection rotated its own
way and disagreed with the viewport by a quarter turn — an isometric view
showing a plate to the right of the origin labelled the visible faces TOP, LFT
and FRT. An orientation cube that lies about orientation is worse than no cube.

It is now a cube with faces, derived from the viewport's own basis — the same
yaw and pitch, the same screen right, up and forward — so the two cannot come
apart again. `the_cube_and_the_viewport_agree_about_which_way_is_which` checks
across five cameras that every axis points the same way across, the same way up,
and lands on the same side of the depth on both.

- A face turns the camera to that view, over the design's 200 ms, easing in and
  out, and taking the short way round: turning from 170° to -170° is twenty
  degrees, not three hundred and forty.
- The dot at the centre switches perspective and orthographic; a ring around it
  says which it is now.
- A face turned away from the eye is never what a click lands on, so the cube
  can never ask for the side of itself you cannot see.
- The presets are the existing `Command`s, so the cube and the View menu are the
  same seven answers reached two ways.

**Reduced motion is an application preference** (View ▸ Reduce motion), not a
desktop one: egui 0.32 surfaces no such signal, and guessing at one from
environment variables would be a worse lie than a checkbox. With it on the
camera simply arrives.

## Multi-selection in the property panels

- A field shows the value when every selected node agrees and an **em dash**
  when they do not. Typing over the dash applies to all of them; leaving it
  alone edits nothing, so tabbing through a panel of mixed values cannot flatten
  them.
- Dimensions appear when every selected node is the *same* kind of shape. A
  mixed selection says so in a sentence rather than showing an empty panel or a
  set of fields that would quietly edit one of them without saying which.
- Visible, Anchor, Position and Rotation apply to the whole selection. Name does
  not: renaming four nodes to one name makes the outliner unreadable, so the
  field says what is selected instead.
- Measured still reports the primary node, and now says which one that is.

## The smaller things

- **A 3D cursor.** Shift+right-click puts it on whatever is under the pointer,
  or on the ground plane when that is nothing, snapped to the same grid a move
  snaps to. New shapes land there. The palette's hint line and every tile's
  tooltip are generated from the same sentence, so they cannot come to disagree
  about where a shape will go — which they would have, since the old hint said
  "at the origin" in two places.
- **Deleting a group asks.** "Delete" is two different actions wearing one word,
  so the outliner asks in place — a strip in the tree, not a dialog over the
  window, because the question is about the tree. The two answers are named for
  what they do; neither of them is "OK". Keeping the children promotes them into
  the group's own slot, in order.
- **Messages fade** six seconds after they arrive, over a second. "Ready" never
  fades: it is the state of the application, not news about it. The clock is
  driven by watching the message rather than by stamping it, so no `status = …`
  anywhere in the application can forget to.

## Still not built, and why

- **No `Layout` mode tab.** Unchanged from the last pass: there is no second
  mode in this application to put behind one, and inventing an empty tab is
  worse than not having it.
- **Panel *sections* do not move.** Object, Dimensions, Transform and Measured
  collapse but stay in the Properties panel. The design's docking language is
  about panels, and making every section independently dockable would put four
  more headers in the drag model for no question anyone is asking.
- **No pointer gesture added this pass has ever been run.** The header drag, the
  label scrub, the cube click and the cursor placement are covered as far as
  their arithmetic and no further; there is no `xdotool` on this machine and
  `egui_kittest` is not in the dependency tree, so a button cannot be pressed at
  a coordinate either live or headlessly. This is the pass's one open issue and
  is written up as such in `KNOWN_ISSUES.md`.
- **The expression reader has no functions and no variables.** `sqrt`, `sin` and
  a named dimension reused in three fields are all reasonable and none of them
  are here.
- **The 3D cursor is session state.** It is not saved with the project, because
  saving it would change the file format.

## What was checked by looking

The binary was built and run under XWayland and the window grabbed with `xwd`,
as the last two passes did. That is how the dock-width collapse and the cube's
quarter-turn error were found; both are the kind of fault every test in the
suite would have gone on passing through.
