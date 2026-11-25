# Strand leaves: a willow curtain that reads as leaves, not wires (D-052)

The system: the leaves a `NodeKind::Strand` bears (`add_distributed_leaves` in `src/foliage.rs`, the leaf card
cut in `build_foliage`, the leaf budget in `skeleton.rs`, `GrowthState` in `src/gpu/state.rs`) and the
`Willow` preset that uses them. `weeping-strands.md` made the strands (iteration 7 of this loop, 392 of them
on the mature willow, 619 m of rope). D-052 stayed open on what they carry: at 8 to 18 m the willow reads as a
fence of straight pale wires with a leaf now and then, and from 30 m, where level 1 drops the wood, as a low
bush on a pole (measured below). The wood is not the defect; the leaves on it are too few, too big for their
number, and each costs 16 triangles.

## Sources

- Botanical (species): Orwa et al. 2009, Agroforestree Database, *Salix babylonica* (World Agroforestry,
  <https://apps.worldagroforestry.org/treedb/AFTPDFS/Salix_babylonica.PDF>, read this pass). Not in the
  corpus; the Flora of China page (efloras) answered 403 behind a bot wall, and an Oregon State extension sheet
  seen in a search excerpt gives the same leaf, 7.5 to 15 cm.
- Botanical (model): Mirabet, Besnard, Vernoux and Boudaoud 2012, "Noise and robustness in phyllotaxis"
  (`10.1371_journal.pcbi.1002389`, in the corpus): leaves are laid down one at a time along a shoot at a
  regular divergence, so the number of leaves on a twig is the twig's length over the spacing between nodes,
  not a share of anything else.
- Shipping game (middleware): the SpeedTree Modeler manual, "Leaf generator"
  (<https://docs.unity.com/en-us/speedtree-modeler/10/leaf-generator-properties.md>, read this pass), with the
  hanging-branch material of `weeping-strands.md`.
- Technical: `book_real_time_rendering_4th_akenine`, Akenine-Moller, Haines and Hoffman, section 13.6
  (vegetation as cards and what they cost, in the corpus); this crate's own `lod.rs` (`coarse_leaves`) and the
  scans' tables (`textures/leaves.rs`: 8 triangles a side for a willow leaf, 6 for the coarse outline).

## Craft principle

What a willow carries on a twig is a long row of narrow leaves. The species description:

> "Leaves lanceolate to linear lanceolate, 9-16 cm long, 0.5-1.5 cm wide, margins finely serrate, slightly
> shiny, dark green above, grey-green with distinctly reticulate venation beneath."
> "Stem furrowed, usually dividing near the ground, crown open, branches pendulous."

and its photograph is captioned "Branchlets hanging vertically, almost to the ground". A leaf is 6 to 30 times
as long as it is wide (this crate's scan is 4 times: aspect 0.25, so a leaf of the preset is already three
times fatter than the species' for the card to read at a distance). How many leaves a twig has is the
phyllotaxis of a shoot:

> "Lateral organs (leaves and flowers) are successively initiated as primordia at the shoot apex yielding a
> regular arrangement on the stem known as phyllotaxis."

so the count along a twig is the twig's length over the spacing of its nodes, and a long thin twig has many
more leaves than a short thick branch of the same crown: the rate belongs to the kind of wood. This crate
gives a strand the leaf rate of every other branch (`foliage.density`, 6.5 a metre on the willow, so a
strand 1.6 m long, the mean, has 8) and the leaf is 0.34 m long, so a strand reads as wire with a leaf every
20 cm. No paper in the corpus gives an internode length for *Salix babylonica* (the web gave none either), so
the rate is a design default, set against the validation targets below, not against a measurement.

## How shipping games do it

SpeedTree draws a hanging crown as a branch generator whose children are twigs, and puts its leaves on them in
a generator of their own, with a size, a count and, for each leaf type, a mesh or a plane:

> "Multiple leaf types can be defined and applied to nodes at random." / "Geometry: A named mesh from the mesh
> bank will be used for this leaf type. A value of "none" results in a plane being used instead."

and says what the choice between a plane and tight geometry is for:

> "Camera facing: Leaf meshes are oriented to face the camera at all times, exactly like a leaf card. However,
> camera-facing leaf meshes can take advantage of optimized geometry to reduce overdraw and costly transparent
> pixels."

Real-Time Rendering says why a crown is built of many small cut-out leaves and not a few big cards:

> "To avoid overdraw, professional packages such as SpeedTree represent and simplify a model with large meshes
> of alpha-textured sets of leaves and limbs. While geometry processing then takes more time, this is more than
> offset by lower overdraw costs."

The lesson for a curtain is the one the middleware draws: leaves are plentiful and each is cheap geometry,
cut to its shape, with the leaf's own few corners. This crate cuts a card to the scan's outline (10 points,
16 triangles for both sides of a willow leaf), which is right for a leaf the eye reaches and wasteful for
the fortieth leaf of a rope.

## Model adopted

A strand's leaves differ from a branch's in four ways, all of them in `FoliageParams` (the leaf code reads
no other struct, so `leaf_points_for_node` keeps its signature):

1. **Their own rate**: `strand_density`, a share of `density` (1 = as every branch, the default), so a strand
   of length `l` bears `density * strand_density * l * height_modifier` leaves under `AllBranches` (the budget
   in `fit_leaves`, `leaf_bound`, and the GPU's `leaf_count` take the same number: one function, `leaf_rate`).
2. **Their own size**: `strand_size`, a share of `leaf_size` (1 default). A row of many leaves is a row of
   small ones; the willow's leaf is 0.34 m on a 5 m tree because one leaf stood for a tuft.
3. **Their own direction**: `strand_angle`, degrees out of line with the rope (0, the default, is the old
   behaviour: every leaf hangs along it), to the side that the slot's turn round the rope says, the golden
   angle from the leaf before (spiral phyllotaxis, Mirabet et al.). This is what makes a rope a ribbon: a
   leaf hanging along its rope covers a leaf's width of a curtain whose ropes are half a metre apart (the
   probe below: 2.6 times the leaves on the strands moved the lower third's coverage from 0.335 to 0.336),
   one leaving it at 45 degrees covers a leaf's length times the sine of that on each side.
4. **A cheaper card**: a strand leaf is cut as a four-point lance, the base, the tip and the two widest points
   of the scan's own outline, with the scan's atlas coordinates, so it wears the same texture; without a scan,
   the procedural leaf's four corners. Both sides, as every card: 8 vertices and 4 triangles, a quarter of the
   scanned card's. The flag is `LeafPoint::strand`, set where the point is made, and it survives the coarse
   levels (`coarse_leaves` copies the point).

Leaf `i` of a strand keeps its place as the strand grows (`leaf-places.md`: slot, rank, flush), its direction is
a function of its slot, and the shaders and `GpuGrowth` already take whatever number of vertices a leaf has
(`firsts`), so neither changes.

## Parameters

| Field | Default | Willow | Range | Meaning |
|---|---|---|---|---|
| `foliage.strand_density` | 1.0 | 1.4 | 0 to 16 | share of `density` a strand's leaves have |
| `foliage.strand_size` | 1.0 | 0.9 | 0 to 8 | share of `leaf_size` a strand's leaves have |
| `foliage.strand_angle` | 0.0 | 45.0 | 0 to 90 | degrees a strand's leaves point out of line with it |

## Validation targets

1. A tree with no strands is bit-identical: every scenario of the gallery that does not name the willow keeps
   `stats.tsv` columns 1-10 (a strand leaf flag is the only new state, and it is false there).
2. **Coverage, measured independently of the generator** (a throwaway probe rasterises every triangle of the
   willow's meshes on a 256 x 256 grid over the lower third of the crown's box, from the front and the side,
   and counts the cells a triangle covers): the lower third of the mature willow is covered over at least 0.45
   of its area from both views at level 0 (1.4 times the baseline below), and the same, within 10 points, at
   levels 1 and 2 (the coarse levels keep every third and every eighth leaf but scale them up). Baseline in 0.47.1 (mature willow, seed 42,
   the crown's box 8.9 x 5.2 x 10.8 m, `examples/zz_probe.rs`, deleted before the commit): lower third covered
   0.335 from the front and 0.271 from the side at level 0, 0.285 and 0.234 at level 1, 0.302 and 0.228 at
   level 2; the middle third 0.406 and 0.365 at level 0. A fence: two thirds of the curtain is open sky.
3. The mature willow has more leaves and no more triangles than 0.47.1's (74 080 foliage triangles), or at most
   10 percent more; its leaf budget (`leaf_bound`) stays under half of `MAX_LEAVES` (the test that holds every
   preset to it).
4. CPU and GPU growth agree on `age_willow_6` (`compare_gallery`: mean and worst no worse than 0.218 and 0.922 on
   the seven-scenario set), no mesh flag, leaves of a strand are born where they were.
5. Same seed, same tree; a strand leaf never changes shape or place with age (the existing tests for leaf
   places and shapes, extended to strand leaves).
6. Cost: `build_tree` of the willow (11.7 ms mature in 0.47.1, one core) is quoted; every other preset's build
   probe within 5 percent; the forest bench's three modes within 5 percent (no willow in them).

## Measured (iteration 16)

The coverage probe (`examples/zz_probe.rs`, deleted; mature willow, seed 42, lower third of the crown, front / side,
level 0) over the design space, before the rate was capped:

| density x, size x, angle | leaves on strands | covered |
|---|---|---|
| 1, 1, 0 (0.47.1) | 3 212 | 0.335 / 0.271 |
| 2.5, 0.7, 0 | 8 304 | 0.336 / 0.269 |
| 5, 0.55, 0 | 16 799 | 0.355 / 0.280 |
| 1, 1.0, 40 | 3 212 | 0.551 / 0.468 |
| 1.5, 0.9, 40 | 4 915 | 0.582 / 0.505 |
| 3, 0.65, 35 | 10 008 | 0.576 / 0.497 |
| 3, 0.65, 50 | 10 008 | 0.626 / 0.543 |
| 1.4, 0.9, 45 (adopted) | 4 559 | 0.585 / 0.509 |

More strands (2.4 a metre for 592 of them, leaves per strand cut to match) gave the same 0.59 / 0.53 with 4 400 more
vertices of wood, so the strand count stays 1.6 a metre.

The adopted willow against the targets: (1) 502 of the 526 gallery scenarios have the same `stats.tsv` columns 1-10, the
changed ones are the 27 willow scenarios and the oak's three with leaves on strands; (2) 0.585 / 0.509 against the
0.45 asked, 0.562 / 0.485 at level 1 and 0.552 / 0.475 at level 2 (within 4 points of level 0); (3) the mature willow
has 5 977 leaves where it had 4 630, in 40 924 triangles where it had 74 080, and `leaf_bound` is under half of
`MAX_LEAVES`; (4) **not met**: `compare_gallery` on `age_willow_6` is 1.043 where it was 0.922 (the seven's mean of
means 0.235 where it was 0.218, the other six images identical to the digit), and over the 36 willow scenarios the worst
is 1.350 (`age_willow_8`) where it was 1.162 (`grow_willow_7`): the GPU's leaves take their normals and crown shading
from the mature tree (`src/gpu/mod.rs`, "What differs from the CPU path") and the lower crown's leaves now cover 1.75
times the area that difference shows on, with no difference in where a leaf is (`grown_leaves_and_flowers_match_the_cpu_meshers`
passes); (5) the existing leaf-place and shape tests pass and a strand leaf's direction is a function of its slot;
(6) `build_tree` of the willow 5.0 ms where it was 11.7 (best of 5), every other preset unmoved (their meshes are
bit-identical).

## What great looks like

The reference is a weeping willow in leaf seen from 10 to 30 m: a dome, and from its rim and its arches a
screen of long pale-green fringe that hangs to a hand's breadth above the grass, dense enough that you see
the curtain's lit face and its shadow side, not through it to the field behind, with a few gaps where the
strands part and the wind opens them. Up close, each strand is a thin rope of narrow leaves that overlap, so
the rope itself is nearly hidden; the leaf is a clean lance, not a blob.

## Deviations

- `strand_density`, `strand_size`, `strand_angle` and the willow's values (1.4, 0.9, 45) are design defaults
  (`qa/visual_findings.md`, D-081): no source gives a leaf spacing or a leaf angle for the species (the angle is
  what a willow's alternate, outward-pointing leaves look like, not a measurement), and the leaf is larger than
  the species' because the tree is smaller (a 5 m willow stands for one of 12 to 20 m: Orwa et al. "to 20 m tall").
- The rate is **1.4, not the 3 the first design had**: at 3 the protected growth test
  `a_leaf_that_appears_grows_from_nothing` failed (a leaf from 0.28 to 0.59 of its size in one 0.01-year step),
  because a leaf eases in over the growth of branch the next one takes and a denser strand flushes quicker; the
  assertion stands. The rate was not what made the curtain: 2.6 times the leaves in line with the ropes moved the
  cover from 0.335 to 0.336, and the same count of leaves turned 40 degrees reached 0.55 (below).
- The scan's leaf is wider than the species' (aspect 0.25 against 0.03 to 0.17), kept so a card still reads
  at 30 m; the lance is as wide as the scan at its widest.
- A strand leaf has no scanned serration or taper; at 4 triangles it is a diamond, and at the distances where a
  strand's leaf is a few pixels wide (over 5 m) the outline of a 16-triangle leaf is below what shows. At under
  2 m, inside the crown, the diamond shows (the junction close-up of `mature_willow`).

## Pending index

- Weber and Penn 1995 (willow row of the parameter table), Hollender et al. (WEEP): paywalled, as in
  `weeping-strands.md`.
- An internode length and a leaves-per-metre figure for *Salix babylonica* (a shoot morphometry paper): not
  found by `distill_search` or the web this pass; the rate stays a design default.
- The Flora of China treatment (`efloras.org`, taxon 200005760): bot-walled (403); its numbers (9 to 16 cm by
  0.5 to 1.5 cm) are the Agroforestree Database's, seen in a search excerpt.
