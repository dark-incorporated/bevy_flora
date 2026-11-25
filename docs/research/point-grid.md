# Keeping a layer of plants clear of what already stands: `PointGrid`

Iteration 15, 2026-11-25. No issue is open on this repository (`gh issue list`, 0 open). The item comes from the
section 4 fallback, "what the crate's consumers hand-roll": four copies of the same structure, and the crate's own guide
tells the reader to write the slow one.

## Why not iteration 14's first backlog item

Iteration 14 ranked `FloraSeason::of_date` first (the editor's `forest_season::season_for` turns a calendar date into
`FloraSeason::of_year`). Read again this pass, it fails the ownership gate ("the clock and the calendar are the
caller's"): `bevy_elements` already owns the date-to-season mapping and drives `FloraSeason` from it
(`bevy_elements/src/flora.rs::drive_season`, `SeasonState::at`, a real-calendar curve: autumn 1 September to 15 October,
leaf fall 15 October to 30 November, leaf-out 1 April to 10 May, blossom 1 to 20 April), and `of_year(t)` is already the
hook that takes the year's progress. The editor's four lines are the caller doing its half. A third calendar in this crate
would also disagree with the second: mapped onto the calendar from 1 March, `of_year` hangs the leaves until 23 January,
where Dox et al. 2020 (`10.1093_treephys_tpaa058`) put the onset of foliar senescence of beech "in mid-late October" and of
birch "in late September to mid-October" at 51 degrees N. Recorded as a falsified backlog item, not deleted.

## What the consumers write today

Every layer after the first has to stay clear of the layers already planted, and a distance test against a list is what
each of them writes:

- Sanctuary, `src/prototype/layout.rs`: a private `PointGrid` (`insert`, `near(p, radius)`, a 16 ft hash grid with the
  reach computed from the radius so any radius stays exact) that deadfall (`deadfall_from_trees_ft`) and undergrowth
  (`undergrowth_from_trunks_ft`) ask with two different radii.
- This crate's `examples/forest.rs` (`--undergrowth`): a `HashMap<(i32, i32), Vec<Vec2>>` of 2 m cells and a nine-cell walk,
  for shrubs 1.5 m from every trunk.
- bevy_grimoire's editor (`bevy_dark_factory`, `ops_forest.rs::plant`, the forest brush of F6): for every candidate of a
  stroke, `forest.trees.iter().any(|t| distance^2 < spacing^2)`: one pass over the whole forest per candidate.
- This crate's guide (`docs/guide/forests.md`, "Undergrowth"): `trees.iter().any(|t| t.position.distance(at) < 1.5)`, with the
  comment "For a big forest, look the trunks up in a grid instead of all of them (the `forest` example does)".

`Scatter::points_where` already takes a density closure, so the missing piece is only the lookup the closure calls.

## Sources

- Model: Canham, Papaik, Uriarte, McWilliams, Jenkins and Twery 2006, "Neighborhood analyses of canopy tree competition
  along environmental gradients in New England forests" (Ecological Applications 16, doi
  `10.1890/1051-0761(2006)016[0540:NAOCTC]2.0.CO;2`, stem `10.1890_1051-0761(2006)016[0540:naoctc]2.0.co;2`): a plant's
  neighbourhood is the plants within a radius, and the radius belongs to the question asked, not to the population. Benes,
  Albrecht, Deussen, Pirk and Niese 2022, "Procedural Urban Forestry" (ACM TOG, doi `10.1145/3502220`, stem
  `10.1145_3502220`): planting rules are distances to what already stands.
- Shipping practice: Ericson, "Real-Time Collision Detection", chapter 7 (stem `book_real_time_collision_detection_ericson`):
  the hashed uniform grid, the structure game engines use for exactly this question. A survey of procedural
  modelling for virtual worlds (stem `10.1111_cgf.12276`; the catalog title is empty): the layers of a generated world must
  see each other.
  The consumers above (Sanctuary, the editor) are the shipping evidence that the structure is written again each time.
- Technical: Bridson 2007, "Fast Poisson Disk Sampling in Arbitrary Dimensions" (SIGGRAPH Sketches; the background grid
  `scatter.rs` already uses; not indexed: `Pending index`). `FloraIndex` (`src/index.rs`) is the same hashed grid for
  entities; its note is `docs/research/flora-index.md` (the unbounded-radius bug, #92, that this type must not repeat).

## Craft principle

Canham et al. on what a neighbourhood is: "Our previous studies with adult trees in both temperate and tropical forests
(Canham et al. 2004, Uriarte et al. 2004b) used larger plots and estimated the effective neighborhood radius for each
species of target tree (i.e., empirically determined the radius beyond which inclusion of neighbors did not improve the
likelihood of the model). Effective neighborhood radii in those two studies ranged from 3.2 to 19.8 m, but the effect of
neighbors declined significantly with distance". The radius is the species' and the question's: a juniper under a pine and
a fallen log among the same pines ask the same set of trunks with different radii (Sanctuary does: two tunables). So the
set carries no distance; the query does.

Benes et al. on where a placement rule's distances come from: "Typically, the city only defines certain planting rules
such as the distance between individual trees should depend on the tree height... Most of these rules are combined into a
so-called building activity area (or building envelope) that is an extension of the 2D projection of the building by about
600cm perpendicularly from each wall of the building and 150cm from each driveway." And: "Implementing the above-described
spacing rules results in vegetation that is regularly placed along roads and sidewalks and that follows a so-called
Poisson-disc distribution (minimal distance requirement) among plants and from the building envelope." A second layer of a
forest, a brush stroke over a painted forest and a replant after felling are all "Poisson-disc among plants and from what
is there": a distance to a set of points.

## How shipping games do it

Ericson on the structure: "One way to reduce the memory requirements of grids is to use hashed storage, as described in
Section 7.1.3, which has the benefit of allowing the grid to be of infinite extent." and, of a hierarchical grid but true of any
uniform one, on choosing the cell: "The grid cells can assume any relative size and should ideally be selected with respect to
existing object sizes." A hashed grid
holds only the cells that have something in them and is exact for any query that looks at every cell the query's box
touches, so the cell size changes the price of a query and never its answer. The survey on why a layer must ask:
"dissimilar features often influence each other; think, for example, of the interactions between river and hill, road and
forest, road and river, etc. As a result, the execution of each pm method might have to take into account the location,
type and properties of other features." Sanctuary's four scatters (trees, deadfall, undergrowth, rocks) are that: each
takes `plantable(...)` and the trunk grid into its density.

## Model adopted

`PointGrid`: points on the ground (world x and z, as `ScatterPoint::position`), a hashed grid of square cells.

- `PointGrid::default()` / `with_cell(cell)` / `FromIterator<Vec2>` / `Extend<Vec2>` / `insert(at) -> bool`: a point that is
  not finite has no cell and is refused (`false`, nothing stored). `with_cell` only tunes speed (a cell that is not a positive
  finite number is [`PointGrid::CELL`]; no answer depends on it).
- `near(at, radius) -> bool`: is some stored point strictly less than `radius` from `at` (`Vec2::distance`, as the
  hand-rolled tests compute it). A radius that is not a positive number (zero, negative, `NaN`) and a query point that is not
  finite find nothing. An infinite or huge `radius` is answered at once: the cells of the query's box are clamped to the box of
  cells ever filled, and when that is still more cells than are occupied the occupied cells are walked instead
  (the bug class of #92: a query priced by how far it might reach).
- The box a query scans is padded by a few `f32::EPSILON` of the larger of `|at|` and `radius`, so a point one ulp inside the
  radius is never lost to the rounding of `at - radius`.
- Pure CPU, `bevy::math` only, in `scatter.rs` (the module that places trees); `Clone`, `Debug`, `Default`. No `Reflect`
  (a `HashMap` of cells is not data a caller edits).
- The call site is `Scatter::points_where` with a density that asks the grid:
  `layer.points_where(|at| if trunks.near(at, 1.5) { 0.0 } else { 1.0 })`. It composes with `points_with`, with a clearing
  and with a `FloraGround` slope test, which a `Scatter::points_clear_of(&grid, radius)` method would not.

## Parameters

| Name | Value | Why |
|---|---|---|
| `PointGrid::CELL` | 8.0 world units | `FloraIndex::CELL`'s; a speed default, not a limit. Sanctuary's 16 ft and the example's 2 m are tunings for a radius. |
| scan padding | `4 * f32::EPSILON * max(abs(at), radius)` | covers the rounding of `at - radius` and `at + radius`. |

## What great looks like

A layer is planted around what stands and nothing else changes: the shrubs of `forest --undergrowth shrub:2.5` are the same
shrubs at the same places as before (the placement count and the gallery do not move), a brush stroke over 50 000 painted
trees costs a grid build and a lookup per candidate instead of 50 000 distance tests per candidate, and an unbounded radius
returns at once.

## Validation targets

1. `near` equals the brute force `points.iter().any(|p| p.distance(at) < radius)` over thousands of random points and
   queries, negative coordinates and cell borders included, for cells much smaller and much larger than the radius.
2. A radius of 0, negative, `NaN` finds nothing; `f32::INFINITY` and `f32::MAX` return at once (in time proportional to the
   occupied cells, not to the reach); an empty grid finds nothing; a point that is not finite is refused.
3. A second `Scatter` thinned with `near` keeps exactly the trees the brute-force test keeps.
4. `examples/forest.rs`'s undergrowth is the same set of shrubs after the cut-over (the same placement count on the same
   seed), and the hand-rolled grid is gone from it.
5. At 40 000 points and 200 000 queries a lookup costs microseconds against the brute force's milliseconds (measured in a
   throwaway probe; the figure goes in the log).

## Deviations

None to a default, preset or range. The name `PointGrid` and the methods `near` and `insert` are Sanctuary's own, so its
private type is a drop-in swap when it moves its pin. Assumed decisions (owner may override): a bare `bool` for the refusal of
`insert` (the one reason is "not finite"); a stored point keeps no payload (a caller that needs the tree behind a hit has
`FloraIndex`, which does); no `remove` (a forest being edited rebuilds the grid, an `O(n)` pass).

## Pending index

Bridson 2007 (not in the corpus; `paper_search` finds the abstract only). Epic's PCG framework documentation and SpeedTree's
placement tooling were not read this pass.
