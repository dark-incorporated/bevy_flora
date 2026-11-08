# A tree from any parameters: bounded, finite, the same tree for the same input

Iteration 12, 2026-11-02. No issue is open on this repository (`gh issue list`, 0 open). The item is the first of
section 4's list ("a panic on caller parameters"), taken from a probe an earlier pass wrote and never ran: the driver
parked it (`wiggum/parked-pass-9-20261102-054127`, `examples/zz_probe.rs`, 160 lines), and it is adopted here.

## What the probe found

The probe sets every numeric field of eight presets, one at a time, to a hostile value (floats: `0`, `-1`, `1e-6`,
`1e6`, `1e30`, `-1e30`, `NaN`, `inf`; counts: `0`, `1`, `255`, `65535`, `u32::MAX`, `1e6`) and builds the tree and its
levels of detail, and every preset at hostile ages. It ran on release binaries of `main` (`8ad7e39`), three workers,
each case killed at 25 s without output or 3 GB resident; it was stopped after about 2 600 of its 6 816 field cases
(the rest repeat the same fields at other values) and the 140 age cases ran on their own. Findings, none of them a panic:

- **Memory and time, unbounded.** `branches.end`, `branches.density`, `branches.length` (negative, `1e6`, `1e30`,
  `NaN`, `inf`), `branches.start` (`-1e30`, `inf`), `trunk.height` (`1e6` and up), `trunk.leader_length` (`1e6`),
  `branches.sub_branch_count` (`255`), `strands.density` (`1e6` and up), `mesh.smooth_iterations` and
  `trunk.height_segments` and `trunk.radial_segments` (`u32::MAX`): the process hangs or passes 3 GB. A 65 535-deep
  `branches.recursion` aborts the process (`SIGABRT`, no unwinding to catch).
- **Meshes that are not finite.** 164 of the 2 600 cases return a main tree whose levels of detail hold `NaN` or
  infinite positions (138 more ran out of memory, 19 hung, 6 aborted): every float field set to `NaN` or `inf`, and finite ones that are only a little wrong
  (`trunk.taper_curve` `-1`, `branches.start` `-1`, `growth.child_duration_scale` `±1e30`). `age = NaN` does it to every
  preset; every other age (`-1`, `0`, `1e30`, `inf`, `1e-30`) is fine.
- Nothing in `TreeParams` is checked, and nothing is promised: `CLAUDE.md` says "parameters come from callers and from
  `.tree.ron` files" and forbids a panicking index, but a file with `height: 1e6` or a slider that passes through
  `-1` hangs the plugin's `BuildInBackground` task on the async compute pool, where it cannot be cancelled.

## Who sends hostile values

- The editor. bevy_grimoire's family issue F6 (`dark-incorporated/bevy_dark_factory#102`) is "tune a `.tree.ron` and watch it
  regrow": every keystroke in a number field is a candidate value (`1`, `1e`, `1e6`, `-`), and a regrow runs on each.
- A modded or hand-written `.tree.ron`: RON reads `NaN` and `inf`.
- A game that derives a field from another number (a height from a noise value, a density from a division) and gets
  `0/0`.

## Sources

- Model: the related-work passage of `10.1111_cgf.12744` (Computer Graphics Forum; not in the catalog, indexed by
  chunk) on Weber and Penn 1995's geometric method: "represents trees as collections of narrow near-conical tubes ... a
  user adjusts meaningful parameters, such as branching angles and segment lengths. Unfortunately, creating a new tree
  is cumbersome, requiring adjustment of up to 80 parameters" (`TreeParams` has about a hundred numeric fields, and
  an editor hands every one of them to a human); Aye, Brännström and Carlsson 2022, "Prediction of tree sapwood and
  heartwood profiles using pipe model and branch thinning theory" (`10.1093_treephys_tpac065`): "if the number of
  tips is primarily limited by access to light, we expect 1 <= d <= 3 depending on the geometry of the branch" (the
  tips of a real crown are limited by light and by space, so a model that makes more has left the species); Guo,
  Zhang, Li, Deussen and Yi 2017, "Tree Growth Modelling Constrained by Growth Equations" (`10.1111_cgf.13263`): "LoS
  is the loop of the model finishing simulation, which means that after LoS loops, the tree stops growing" (a growth
  model carries a bound on how long it iterates, as a parameter).
- Shipping game: SpeedTree Modeler's branch generator (Unity manual, web), which bounds what a designer can ask of it:
  a **Clamp** group with "**Min** Set the minimum length of branches. **Max** Set the maximum length of branches.",
  and for the spine's accuracy "Higher values result in slower computes" with the advice "Set this value to the
  maximum desired fidelity early".
- Technical: Rust `f32::clamp` (std documentation): "Restrict a value to a certain interval unless it is NaN ...
  Note that this function returns NaN if the initial value was NaN as well", and "Panics if `min > max`, `min` is NaN,
  or `max` is NaN". The generator's own `.clamp` calls (`hang_strands`: `s.start.clamp(0.0, 1.0)`) pass a `NaN`
  through, which is why a clamp in the middle of the code does not make a tree safe; the guard has to be a function
  of the field, written once, that maps `NaN` to a value.

## Craft principle

A parametric generator is a function from a point in a high-dimensional box to a tree, and a *game* only ever wants
the box. The related-work passage's count (up to 80 parameters) says the box is large; Guo et al.'s `LoS` says a growth model
bounds its own iteration; the pipe-model paper says a crown's tip count is limited by physical space and light. The
honest contract is therefore: every field has a range inside which the generator is defined, a value outside it
means the nearest value inside it, and the tree has a size no species exceeds. SpeedTree's Clamp Min and Max are the
same contract on one field, written into the tool.

## How shipping games do it

SpeedTree's Modeler bounds the designer, not the game: every generator property has a slider range, branch length
has a Clamp group, and "higher values result in slower computes" is documented beside the property that costs. A
game ships the baked mesh, never the generator's inputs, so the runtime never meets a hostile value. This crate is
the generator *and* the runtime (a `.tree.ron` is an input at play time, an editor is a client), so the tool's
slider ranges have to live in the crate.

## Model adopted

1. `TreeParams::sanitized(&self) -> Cow<'_, TreeParams>`: borrowed when every numeric field is inside its range
   (every preset, every gallery scenario), else a copy with each field mapped by one rule: `NaN` to the field's
   default, anything else to the nearest end of its range (`inf` to the top, `-inf` to the bottom). Enums, booleans,
   colours and the seed are untouched.
2. A size no tree exceeds: `MAX_NODES` branches in the skeleton and `MAX_LEAVES` leaves, both public constants.
   Generation stops adding nodes at `MAX_NODES` (the order is the generator's own: leader, primaries, their splits and
   sub-branches, strands, so a capped tree keeps its lower and earlier wood), and a skeleton whose leaves would pass
   `MAX_LEAVES` loses its last nodes until they fit. The leaves a node carries are a function of its length and
   height, so the count is known without building a mesh.
3. An age of `NaN` is 0 (a tree that has had no time), at the one place an age is read (`Skeleton::aged`) and where
   it is written for the GPU.
4. Where it is applied: the three entry points that take parameters from a caller and build (`build_tree`,
   `build_lods`, `generate_skeleton`), the two per-frame stages the plugin and the felling code call with a tree's
   own component (`aged_skeleton`, `build_aged`), `skeleton_mesh`, and the plugin, which clamps a `ProceduralTree`
   in place when it is added or changed, so every system reads the tree that is drawn (the felling hinge, the shape
   capsules, the fire fuel), and an editor reading the component back shows the value in force.

## Parameters

The ranges are one table in `src/limits.rs`, each documented there, chosen from the presets' own spread and the
gallery's extreme scenarios (`extreme_*`: `twist` 720, `root_flare_spread` 1.5, `flare` 3, `height` 40, `density` 8,
`collar_length` 6, `up_attraction` -1, `height_segments` 32) with at least a factor of ten of room, and from what each
field does at its end: a fraction of a height is 0 to 1 (`branches.start` of `-1` is wood under the ground), a count
has a top (`recursion` 8, `sub_branch_count` 32, `radial_segments` 64, `smooth_iterations` 16), an exponent has no
zero or negative end (`taper_curve` 0 to 16: `t^-1` is infinite at the base), a length has a ceiling (`height`
500 m, a redwood is 115), and a count of branches *per metre* has no small top (the fern's 6 cm stem carries 300, so
`branches.density` goes to 1000 and the nodes are what is capped). `MAX_NODES = 5 000` (the largest gallery tree has
893 nodes, the `extreme_giant` oak; 5.6 times under) and `MAX_LEAVES = 60 000` (the heaviest preset, the willow with
its strands, has 4 630 leaves, measured with `collect_leaf_points` on all 20 presets at maturity; the 40 m giant oak
about 21 000, which a first budget of 20 000 cut, so the budget is nearly three times it). One branch carries at most
1 024 leaves and 256 strands.

Found while measuring the worst tree the ranges allow (every size at its top): one build used 3 GB and was still
growing. The cause is not in the generator: `PinSpheres` in `mesh/branch.rs`, the spheres smoothing pins wood
inside, bucketed each sphere into every 0.25 m cell of its bounding cube, so a trunk 800 m across (a 50 m radius,
flared 16 times) took 6e7 cells a sphere. The grid now doubles its cell until all the spheres together cover two
million cells (a preset never does: the finest cell is kept), so wide wood costs a few more spheres a lookup. With it
the worst trees build in 1.0 s and 310 MB (5 000 branches, smoothing, 32 sides) and 0.4 s and 180 MB (leaves at the
budget), release, Apple M5; before it the same trees took 2.0 to 6.3 s and 480 to 830 MB, and a trunk 800 m across
under smoothing was still allocating after 12 s, at 2.9 GB.

## Validation targets

- Every preset: `sanitized()` is borrowed (no field moves), and the 526 gallery scenarios' `stats.tsv` columns 1 to 10
  and PNGs are identical before and after (no tree changes).
- A generator test over every numeric field of `TreeParams` found by reflection (so a field added later is covered
  without anyone remembering it), at `NaN`, `inf`, `-inf`, `0`, `-1`, the type's largest value and the range's own two
  ends: `build_tree` and `build_lods` return in bounded time and give finite positions and normals and valid indices.
- The worst case, every size field at its top at once, stays inside `MAX_NODES` and `MAX_LEAVES`.
- `Skeleton::aged` and `build_tree` at `age = NaN` equal age 0.
- The plugin's tree with a hostile component is built, drawn, and the component reads back in range.
- Cost: no regression over 5% on the forest bench (the checks run once per build, not per vertex) and none on the
  build probe.

## What great looks like

Typing `1e9` into the height field of the editor draws a 500 m tree in the time an ordinary build takes, and typing
`-` or deleting the field draws the default tree for that frame, never a frozen window and never a hole of `NaN`
vertices; a `.tree.ron` with a typo loads as the nearest tree it can be. Nothing about a preset or a gallery image
changes.

## Deviations

- The ranges, `MAX_NODES` and `MAX_LEAVES` are design defaults chosen above, not measurements of a species. They are
  public constants and one table, so an owner can move them in one place.
- A tree over the budget is cut, not refused: the alternative (an error from `build_tree`) would change a signature
  every caller uses and leave the plugin with a tree it cannot draw. The budget is far above any tree the crate
  draws, so a cut tree is a hostile one. The cut is the end of the generator's order, which for leaves is the top of
  the crown: a fairer cut (thinning every branch's leaves) would have to reach the placement code the GPU mirrors.
- `NaN` age is 0, not the default `TreeAge` (infinity): a game whose growth timer divides 0 by 0 shows a seedling for
  a frame, not a full-grown tree for a frame.

## Pending index

- The two web sources (the SpeedTree manual page, already listed in `corpus.md`, and the Rust `f32` documentation,
  added to it) are not in home-still; `10.1111_cgf.12744` answers `distill_search` but has no catalog entry.
- No source measures how many branches or leaves a real crown has per species; the caps are set from this crate's own
  largest trees.
