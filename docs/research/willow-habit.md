# Willow habit: a dome as tall as it is wide, with no pole above it (D-052)

The system: the `Willow` preset's trunk and limbs (`src/presets.rs`), through the generator's primaries
(`generate_primaries` in `src/skeleton.rs`: `angle`, `up_attraction`, `flatness`, `crown_angle_variation`,
`apical_dominance`, `length`, `start`) and the gravity bend (`Node::axis`). `weeping-strands.md` hung the
strands and `strand-leaves.md` gave them leaves; D-052 stayed open, and iteration 16's critique listed what
is left: "the crown's top is still bare arcs on a slim flat-capped pole", and "from 30 m the tree reads as a
leafy dome with a pale fringe on one side". This note is the habit under the curtain: where the limbs leave
the trunk and how far they rise before they weep.

## Sources

- Botanical (species): Missouri Botanical Garden, Plant Finder, *Salix babylonica*
  (<https://www.missouribotanicalgarden.org/PlantFinder/PlantFinderDetails.aspx?kempercode=c148>, read this
  pass): "Height: 30.00 to 50.00 feet", "Spread: 30.00 to 50.00 feet" (the tree is as tall as it is wide), "a
  stout trunk topped by a graceful broad-rounded crown of branches that sweep downward to the ground". Orwa et
  al. 2009, Agroforestree Database, *Salix babylonica* (read in iteration 16): "Stem furrowed, usually dividing
  near the ground, crown open, branches pendulous."
- Botanical (model): Duchemin, Eloy, Badel and Moulia 2018, "Tree crowns grow into self-similar shapes controlled
  by gravity and light sensing" (`10.1098_rsif.2017.0976`, in the corpus, read this pass): the outline of a crown
  is a species mark and comes from how each growing shoot is turned by gravity and light.
- Shipping game (middleware): the SpeedTree Modeler manual, "Shape control"
  (<https://docs.unity.com/en-us/speedtree-modeler/10/shape-control.md>, read this pass) and the Branch
  generator's "Shape" and "Prune" tables (read in iterations 7 and 16).
- Technical: Luebke et al., *Level of Detail for 3D Graphics* (`book_level_of_detail_3d_graphics_luebke`, in the
  corpus): what the far levels and the impostor keep is the silhouette; this crate's own `lod.rs` and the
  coverage probe below (every triangle of the meshes rasterised on a 256 x 256 grid).

## Craft principle

A crown's outline is the species' signature and is set by how its shoots bend:

> "In many tree species the outer shape of the crown is a criterion for species identification, especially when
> trees have grown in isolation. The Mediterranean cypress has a distinctive elongated shape, while birch and
> oaks show more spherical shapes, and the Norway spruce has a conical profile."

(Duchemin et al. 2018.) What the willow's outline is, the Missouri Botanical Garden says in one line: "a stout
trunk topped by a graceful broad-rounded crown of branches that sweep downward to the ground", 30 to 50 feet
tall and 30 to 50 feet across. Two things follow for a model:

1. The crown is **broad-rounded and about as tall as wide**; the branches **sweep down from it**, they do not
   leave the trunk already hanging. A willow's limbs first rise (the species' stems divide "near the ground"
   and ascend) and weep over at their ends. The wood arches.
2. The trunk is **stout and short under the crown**; it does not stand up out of it as a bare pole. In
   Duchemin's model the growth front of a crown always has a top: the self-similar shapes "always feature a
   pointed top", that is, the shoots at the apex are part of the outline and carry the leaves.

This crate's willow (0.47.1) was the opposite on both counts. Its limbs left the trunk at 46 degrees from
vertical with `up_attraction` -0.5, so they set out nearly level and a gravity bend of 2.25 laid them over at
once: an umbrella. They came from 40 to 100 percent of a 5 m trunk, and the longest were the topmost
(`CrownShape::Hemispherical`) with the least apical dominance, so the crown was a flat mound round a trunk
whose top was also the tree's top. Measured on eight seeds (below): crown height was **0.46 to 0.61 of its
width** (median 0.53, where the species' is about 1), and the trunk's top was 88 to 100 percent of the tree's
height: the pole is the top of the tree.

## How shipping games do it

SpeedTree does not leave a crown's outline to the forces; the tool has a step that confines it:

> "Shape control is a feature of the Branch Generator that lets you quickly confine your tree within a shape
> that is anchored somewhere in the tree's hierarchy."

and it says why the order matters for a weeping tree:

> "Deformations done to the branch spine, such as noise, gravity, and forces, all happen after shape control.
> Because of this, they are capable of pulling your branches outside the desired shape ... The easiest modeling
> method while working with shape control is to keep your generators free of these deformations until you've
> gotten your shape control working correctly. Then, you can proceed to the other deformations."

and the usual start is a sphere: "Shape control is enabled on the trunk. This will create a round 'lollipop'
tree". An artist therefore models the willow as a rounded envelope first, with branches that start inside it
and rise to it, and lets gravity weep the ends afterwards; they do not start from limbs set out level. This
crate has no envelope step (`CrownShape` is a length profile by height and `crown_influence` blends it), so the
equivalent is in the primaries: a steeper start angle, a shorter reach at the top (`apical_dominance`), and
gravity left to do the weeping.

Luebke et al. say what has to survive to the far levels:

> "The object will render a realistic silhouette regardless of viewing angle without storing multiple versions."

(of a geometric impostor). A willow whose silhouette is a low mound round a pole is that mound at 100 m and in
the pool's impostor; the dome is what has to be in the picture.

## Model adopted

Preset values only: the generator already builds an arching limb (a direction with a vertical part, bent over by
`t^2 * bend * lever`, so a limb that leaves at 50 degrees above level reaches its top at about three quarters
of its length) and the strands already hang from the outer wood. What the willow lacked was the start:

- **Limbs leave steeper** (`angle` 46 -> 26 degrees from vertical; with `up_attraction` -0.4 the direction
  leaves about 50 degrees above level, where 0.47.1's left at 8) and **from lower down** (`start` 0.4 -> 0.345),
  so the crown is a fountain that rises from the upper trunk and arches, not an umbrella at its top.
- **The reach falls toward the top** (`apical_dominance` 0.2 -> 0.55: the topmost limb's apical factor is 0.72
  of a base limb's, was 0.9), **and the topmost limbs lean out more** (`crown_angle_variation` -0.3 -> +0.3: the
  top limb leaves 9 degrees flatter than the base one, where it was 9 degrees steeper), so the top rounds off
  in a dome and does not end in a vertical thread of leaves.
- **Longer limbs on a shorter trunk** (`length` 0.7 -> 0.85, `trunk.height` 5.0 -> 4.6, `flatness` 0.2 -> 0.1):
  the arch carries the crown to about 7.0 m (mean over eight seeds, 5.4 before), so the tree is 1.29 times the
  height it was and 0.81 times the width.

`up_attraction` stays under -0.3 (`willow_values` pins it) and the crown is still `Hemispherical` and still
reaches the top of the trunk (`willow_crown_reaches_the_top_of_the_trunk`), so the curtain tests keep their
meaning.

## Parameters

Design defaults (no source gives an angle or a reach: the species' habit is a ratio and a form), found as
follows: grid and random searches over eight preset fields (several hundred variants, three to eight seeds
each), scored on the crown's height over its width, the curtain's cover (below), the highest 1 percent of leaf
height against the highest point and the triangle count, then a visual check of the survivors at 14 and 20 m
from three bearings (the scores alone picked narrow upright trees that read as elms, not willows).

| Field | 0.47.1 | Now |
|---|---|---|
| `trunk.height` | 5.0 | 4.6 |
| `branches.start` | 0.4 | 0.345 |
| `branches.length` | 0.7 | 0.85 |
| `branches.angle` | 46 | 26 |
| `branches.up_attraction` | -0.5 | -0.4 |
| `branches.flatness` | 0.2 | 0.1 |
| `branches.apical_dominance` | 0.2 | 0.55 |
| `branches.crown_angle_variation` | -0.3 | 0.3 |

## Validation targets

1. **Habit, measured on the meshes** (the highest vertex over the wider horizontal extent of wood and leaves,
   seeds 42, 1, 2, 3 and 4): at least 0.6 on every seed (0.47.1: 0.48, 0.57, 0.58, 0.50, 0.47), and the tree's
   highest vertex at least 1.2 times the trunk's height (0.47.1: 1.00 to 1.14; the pole is the top).
2. **Curtain kept**: the lowest leaf under 0.3 of the trunk's height and the bottom third of the trunk's
   height as wide as the middle third (`willow_curtains_hang_to_the_ground`, protected, unchanged); the
   probe's cover of the lower and middle thirds of the crown from the front and the side at least 0.4 (0.47.1:
   0.44 to 0.62 lower, 0.40 to 0.67 middle).
3. **Cost**: foliage triangles no more than 0.47.1's mean (38 900 over the eight seeds), wood no more than 5
   percent over its mean (39 800), the build probe's median no slower than 0.47.1's.
4. **GPU growth** parity on `age_willow_6` and the willow scenarios no worse than iteration 16's 1.043 and 1.350;
   `grown_leaves_and_flowers_match_the_cpu_meshers` passes.
5. Every other preset's meshes are bit-identical (their `stats.tsv` columns 1-10).
6. Same seed, same tree.

## Measured (iteration 17)

The probe (`examples/zz_probe.rs`, deleted before the commit) on seeds 42, 1, 2, 3, 4, 5, 6 and 7, mature, level 0:

| | highest vertex over width | trunk top over the tree's top | foliage triangles (mean) | wood triangles (mean) | lowest leaf | lower third covered (front / side) | middle third (front / side) |
|---|---|---|---|---|---|---|---|
| 0.47.1 | 0.46 to 0.61 (median 0.53) | 88 to 100 % | 36 844 to 41 156 (38 895) | 37 818 to 40 800 (39 828) | -0.26 to 0 m | 0.48 to 0.57 / 0.44 to 0.62 | 0.40 to 0.67 / 0.47 to 0.63 |
| now | 0.67 to 0.99 (median 0.88) | 58 to 76 % | 36 544 to 38 772 (37 889) | 40 416 to 40 890 (40 661) | -0.03 to 0.17 m | 0.41 to 0.52 / 0.42 to 0.56 | 0.47 to 0.66 / 0.50 to 0.69 |

The crown is 1.29 times as tall and 0.81 times as wide, with 2.6 percent fewer leaf triangles and 2.1 percent
more wood; the curtain's lower-third cover is within 0.07 of what it was (the dome's box is taller, so the same
ropes cover a smaller share of a larger area). The tree's top over its 99th-percentile leaf height (a thread of
leaves above the dome) was 0.52 to 1.04 m in 0.47.1 (the bare pole) and is 0.45 to 1.05 now: this probe does not
separate the two, and it is not a target; the pictures are what show the pole gone. Alternatives tried and
dropped: `up_attraction` +0.2 and -0.2 gave an 8.7 m tree 10 m wide with the best silhouette and failed the
protected `willow_values`; `apical_dominance` 1.0 made a ball 6 m across (taller than wide, no longer
"broad-rounded"); a higher `crown_angle_variation` with a steep angle left a vertical thread of leaves above
the dome (the spike probe: 1.4 m). Over eight seeds the crown's height varies by 1.6 times (a limb that arches
higher stands out), which is the species' own variation; the preset does not try to remove it.

## What great looks like

A weeping willow in leaf, seen from 14 to 40 m: a broad dome about as tall as it is wide, closed at the top
(no pole, no thread), on a stout short trunk that is hidden by the curtain from a few metres out; from the
dome's rim and its arches a screen of long pale fringe that hangs to a hand's breadth above the grass, the
fringe on the outside and the arched limbs showing through on the inside; the whole silhouette still a dome at
the impostor's distance.

## Deviations

- Eight preset fields move (table above): a design decision, an assumed owner decision recorded in D-082
  (`qa/visual_findings.md`). They are values and not a generator change because the generator already builds an
  arching limb and the same fields set every other preset's habit; no parameter was added.
- `branches.start` is 0.345 and not the 0.3 the search scored best: the topmost limb sits
  `spacing * (1 + frac(zone * density))` under the trunk's top (zone is `trunk.height * (end - start)`, spacing
  `1 / density`), and the protected `willow_crown_reaches_the_top_of_the_trunk` asks for 0.93 of it; at 0.3 it was 0.906,
  at 0.345 it is 0.94. 0.34 passed that test and failed `primary_bases_are_buried_in_the_trunk` (see below); 0.345 passes
  both.
- `primary_bases_are_buried_in_the_trunk` read a false open end on the first candidate (`start` 0.34, willow primary 151,
  seed 42): its helper `open_boundary_vertices` welds vertices on a 0.1 mm rounding grid, and a seam pair of one ring
  that lay either side of a rounding step (y 4.3598 and 4.3599, the same vertex) read as an open edge 1.25 m out. The
  protected helper was not changed (a check changes only additively); the preset moved 0.005 off it and the finding is
  filed for the owner (backlog 5 of iteration 17's log).
- `up_attraction` is left at -0.4 although the best silhouette seen in the search had it at -0.2: `willow_values`
  pins it under -0.3 and a protected check is not loosened.
- The ratio is 0.88 at the median and 0.67 at the least, not the species' 1: a taller tree costs triangles in
  proportion, the strands keep a hand's breadth off the ground, and the height varies by 1.6 times over seeds
  (below). The target is 0.6, which the pole never met on four of five seeds.

## Pending index

Weber and Penn 1995, Hollender et al. (as in `weeping-strands.md`); the Missouri Botanical Garden and URI pages
and Orwa et al. are web sources, not in the corpus; the SpeedTree manual pages likewise.
