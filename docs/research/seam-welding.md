# A closed branch must not read as open because two copies of a point fell either side of a rounding step (#96)

Iteration 18, 2026-11-25. The item is the one open issue, **#96** (label `bug`): the test helper `open_boundary_vertices`
in `src/mesh/branch.rs`, which the protected tests `primary_bases_are_buried_in_the_trunk` and
`steep_primaries_end_in_a_closed_rounded_base` use to find a branch's open ends, reads a closed ring as open when a seam
vertex and the vertex it repeats fall either side of one of its rounding steps. Iteration 17 hit it: with the willow's
`branches.start` at 0.34 the first test failed with "willow: primary 151 has an open end -1.2456 m outside the trunk", the
wood was closed, and the preset was moved 0.005 until the coincidence went away. A change of look must not be able to fail
a test for no defect in the wood, and the test is worth keeping because a real open end 1.25 m from the trunk is a
floating branch.

## What is measured

`emit_ring` (`src/mesh/rings.rs`) writes `segments + 1` vertices per ring: vertex `segments` repeats vertex 0 so the bark's
`u` can run from 0 to `u_tiles` (the seam). Vertex `i` sits at `angle = i / segments * TAU + twist`, so the repeat is at
`TAU + twist`, and `sin_cos` of an angle that differs by `TAU` in `f32` differs by about 1.7e-7 in the unit circle: up to
4e-9 m on a 2.5 cm branch, and the sum `center + radial * r` adds the rounding of a coordinate near 4 m (an `f32` step of
4.8e-7). The two positions are the same point to `f32`; they are not the same bits. The helper identifies coincident
vertices by `(position * 1e4).round()`, a grid of 0.1 mm cells, so two points a rounding error apart land in different
cells with a probability of about (their distance) / (the cell) per axis, 0.2 percent per primary by iteration 17's
estimate, and every edge of the seam reads as used by one triangle.

Elsewhere the crate already welds by tolerance: `smoothing.rs` `weld` merges vertices within `1e-6` of the mesh's extent
on a grid of `4 epsilon` cells and looks in the neighbouring cells, "UV seams and cap rings repeat positions (to within
f32 rounding), and those repeats must stay together". The helper is the one place that quantises.

## Sources

- Model: Yang, Wang and Huo 2022, "Rule-based Procedural Tree Modeling Approach" (`10.48550_arxiv.2204.03237`), on how the
  branch surface of the tree models this crate follows is made: "use the spline function to interpolate a series of new
  nodes inside the branch; then move a disk along these nodes by a certain step to build the corresponding cylindrical
  surface" (the generalized cylinder). A disk swept along an axis is a ring of vertices whose last vertex is its first;
  where the surface closes on itself the model gives one point, and a mesh that cuts the seam for its texture gives two.
- Shipping game: Christopher Oat (AMD), "Real-Time Wrinkles", SIGGRAPH 2007 (`s2007-advances-pdf-slides-2-8-mb-2`, page 30; the
  course notes' chapter 4, "Animated Wrinkle Maps", is `s2007-advances-course-notes-0-15-mb-2`; the Ruby: Whiteout real-time
  short), on a face-wrinkle system that accumulates a weight per vertex: "the vertices marked with yellow arrows lie on a UV seam. The UV coordinates
  are different on either side of this seam and therefore these two triangles do not actually share any vertices as far as
  the vertex buffer is concerned. These vertices may have the same object-space positions but because there is a UV space
  discontinuity here, there must be two red vertices and two blue vertices in the vertex buffer", and its fix, "Perfect hash
  based on object space vertex position" / "Roll your own Vertex ID": a shipping renderer tells the copies of a seam apart
  from the mesh's other vertices by where they are, not by their index, so any tool that does the same must weld by
  position.
- Technical: Christer Ericson, *Real-Time Collision Detection* (`book_real_time_collision_detection_ericson`), 12.5
  "Vertex welding": "User input is often required to set the welding distance, in that automatically determining it is a
  difficult problem"; "Figure 12.2 The welding tolerance must be set appropriately. (a) Too small, and some vertices that
  should be included in the welding operation could be missed. (b) Too large, and vertices that should not be part of the
  welding operation could be erroneously included"; "A practical approach to the problem is to use the hashed uniform grid
  ... CELL_SIZE should at least be larger than 2 * WELD_EPSILON. Then the square welding neighborhood around V can at most
  overlap four neighboring grid cells (in 2D, eight cells for the cubic welding neighborhood in 3D)"; and on how the
  neighbours are chosen, "Only the grid cells intersected by the tolerance neighborhood of a vertex ... must be tested
  against". On the order a welder works in: "Sequential processing implies an order dependency ... for all three approaches
  there are degenerate situations in which, although highly unlikely, two vertices arbitrarily close to each other may weld
  to two different positions. The last issue ... can be avoided by using a simultaneous welding model", which "identifies all
  sets of vertices such that each vertex of the set is within the tolerance distance of one or more other vertices of the
  set". And on the tolerance itself (Chapter 11, "Robustness"): an absolute test `Abs(x - y) <= epsilon` "is typically
  preferred whenever applicable".

## Craft principle

A mesh is closed when every edge is shared by two triangles. Whether two vertices are *the same point* is a question about
distance, and a grid of cells answers it by distance only up to the cell: two points a hair apart on either side of a cell's
edge are different cells (Ericson's "Too small, and some vertices that should be included ... could be missed"). The fix the
source gives is not a finer cell but a cell at least as wide as the tolerance and a look at the neighbouring cells, so that
every pair within the tolerance meets. A simultaneous weld (the sets of vertices each within the tolerance of another) has
no order, so a mesh reads the same whichever way its triangles were written.

## How shipping games do it

They identify a seam's copies by position (Oat) and weld by a tolerance with a hashed grid (Ericson, whose book is the
reference for the collision code of shipped engines), after a tolerance has been chosen for the mesh. This crate's own
smoothing pass does it that way already; the test helper is the one holdout.

## Model adopted

`open_boundary_vertices(mesh)` welds every pair of vertices whose coordinates differ by at most `OPEN_BOUNDARY_WELD_M` on
every axis (an absolute per-axis tolerance, Ericson's `Abs(x - y) <= epsilon`), simultaneously: a hashed grid of cells
`OPEN_BOUNDARY_WELD_M` wide, the 27 cells around a vertex looked at, a union-find over the pairs found. It then counts the
edges between the welded vertices used by exactly one triangle and returns the welded vertices on them. Degenerate edges
(both ends welded together) are skipped, as before. The tolerance is the one the helper already had: its rounding cell is
0.1 mm, and every pair it welded (same cell: each axis differs by less than 1e-4) is within `1e-4` per axis, so the welds of
the old helper are a subset of the new one's; the new one adds the pairs across a cell edge. Nothing a hole is made of
changes: a hole is a missing triangle, and a missing triangle leaves edges that no 0.1 mm weld removes.

The assertions of both tests are unchanged.

## Parameters

`OPEN_BOUNDARY_WELD_M = 1.0e-4` (metres; the old cell). The mesh's coordinates are tens of metres at most, where an `f32`
step is below 2e-6, so Ericson's warning that "for large coordinates the addition of a small epsilon leaves the coordinate
unchanged" does not apply.

## Validation targets

1. The issue's repro: the willow with `branches.start = 0.34`, `primary_bases_are_buried_in_the_trunk`'s own check, fails
   with the rounding helper (primary 151) and passes with the tolerance helper.
2. A closed mesh with one seam pair straddling a rounding step reads open with the old helper and closed with the new; the
   same mesh with one triangle removed reads open with both and names the same boundary; the pair moved apart by three
   tolerances reads open with both (a real gap is not welded).
3. Over every preset, many seeds and 4 and 6 sides (the two tests' counts), the new helper reports no open vertex the old one
   did not, and every pair the old grid cut in two is a pair of vertices within the tolerance (the gap measured).
4. The two protected tests pass on the unchanged presets; `cargo nextest run --all-features` passes as before.
5. Nothing the crate builds changes: the diff is under `#[cfg(test)]`.

## Measured

Iteration 18, `cargo nextest run --all-features` on the branch (Apple M5, `test` profile), with the rounding helper kept beside
the new one in a throwaway test (deleted before the commit):

- **The repro.** The willow with `branches.start` 0.34 (the other seven habit values as shipped): the rounding helper fails
  `primary_bases_are_buried_in_the_trunk`'s check on **primary 151**, the id the issue names; the tolerance helper on none.
- **The synthetic mesh.** The closed tetrahedron whose first vertex is written twice, 2e-6 m apart across the 0.5e-4 step: the
  rounding helper (emulated in Python with `f32` rounding, keys `(0,0,0)` and `(1,0,0)`) reads **four open edges**; the new
  test passes (closed; the same mesh with a face removed names exactly the hole's three vertices; the pair three tolerances
  apart reads open).
- **A sweep** over the 20 presets, seeds 0 to 99 and 4 and 6 sides (**45 010** builds of a primary's tube plus its base, the
  two tests' construction): the rounding helper read an open end on 32 243 (the root end, as designed), the tolerance helper on
  32 238. **20 builds** (0.044 percent) held a coincident pair the rounding grid had cut in two. **The tolerance helper
  reported no open vertex the rounding helper had not (0 of 32 238).** The largest gap between the two members of a cut pair was
  **4.8e-7 m**; of the 104 pair observations in the healed builds (each pair seen from both members), **64** were a ring's seam
  (the two vertices `sides` indices apart) and **40** were a different pair of rings that meet: a base dome's rim against the
  tube's first ring (vertex 3 against vertex 44 of a 46-vertex wood with 4 sides). **Making the generator's seam copy
  bit-identical, option (2) of the deviation below, would have fixed the first kind only.** Half of the healed vertices are
  the far ends of the edges those twins closed.
- The two protected tests and the new one pass; the rest of the suite is in the log.

## What great looks like

A check that fires on a hole and on nothing else: the same answer for a tree and for the tree moved a ring-vertex's
rounding error to one side, no preset literal changed to dodge a step, and the helper's behaviour on a seam, a cap ring and
a base dome stated by a test beside it.

## Deviations

A protected test file's helper is changed (the loop's rule is that a pass does not edit a protected check; the issue
asked for this fix and iteration 17 left it for the owner). Assumed decision, recorded on #96: the options were (1) weld by
tolerance in the helper, the issue's own fix, the assertions untouched; (2) make the generator's seam copy bit-identical to
vertex 0 (`i % segments` in `emit_ring` and `emit_cap`), which removes the observed false open without touching a test and
also gives a consumer that welds by exact position (Oat's perfect hash) a seam it can find, but leaves the helper a
quantiser that any other coincident pair can trip, moves every ring vertex by up to 1e-7 m (a gallery re-baseline for no
visible change) and fixes nothing the issue names; (3) keep moving preset values off a step. Picked (1): the smallest, the
reversible one, the issue's stated intent. It adds checks (a test of the helper itself) and removes none: the welds of the
old helper are a subset of the new one's, and both assertions stand. (2) is a separate, defensible change, listed in the
log's backlog.

## Pending index

- Weber and Penn 1995 and the web sources of earlier notes stay pending as in `docs/research/corpus.md`.
- Oat's slides (`s2007-advances-pdf-slides-2-8-mb-2`) are indexed; the catalog's title field reads only "Christopher Oat" (the
  title is on page 1 of the document: "Real-Time Wrinkles", SIGGRAPH 2007).
