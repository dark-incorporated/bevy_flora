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
whose top was also the tree's top. Measured on eight seeds (below): crown height was **0.46 to 0.62 of its
width** (median 0.52, where the species' is about 1), and the trunk's top was 88 to 98 percent of the tree's
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
`t^2 * bend * lever`, so a limb that leaves at 60 degrees up arches and reaches its top at two thirds of its
length) and the strands already hang from the outer wood. What the willow lacked was the start:

- **Limbs leave steeper** (`angle` 46 -> 26 degrees from vertical; with `up_attraction` -0.4 the direction
  leaves about 55 degrees above level) and **from lower down** (`start` 0.4 -> 0.3), so the crown is a fountain
  that rises from the upper trunk and arches, not an umbrella at its top.
- **The reach falls toward the top** (`apical_dominance` 0.2 -> 0.55: the topmost limb is 0.72 of a base limb's
  length, was 0.9), **and the topmost limbs lean out more** (`crown_angle_variation` -0.3 -> +0.3: the top limb
  leaves 9 degrees flatter than the base one, was 4.5 steeper), so the top rounds off in a dome and does not
  end in a vertical thread of leaves.
- **Longer limbs on a shorter trunk** (`length` 0.7 -> 0.85, `trunk.height` 5.0 -> 4.6, `flatness` 0.2 -> 0.1):
  the arch carries the crown to about 6.8 m, so the tree is 1.3 times the height it was and about as wide.

`up_attraction` stays under -0.3 (`willow_values` pins it) and the crown is still `Hemispherical` and still
reaches the top of the trunk (`willow_crown_reaches_the_top_of_the_trunk`), so the curtain tests keep their
meaning.

## Parameters

Design defaults (no source gives an angle or a reach: the species' habit is a ratio and a form), found as
follows: a random search over eight preset fields, 1 500 builds on eight seeds, scored on the crown's height
over its width, the curtain's cover (below), the highest 1 percent of leaf height against the highest point (a
thread above the dome) and the triangle count, then a visual check of the leaders at 14, 20 and 30 m.

| Field | 0.47.1 | Now |
|---|---|---|
| `trunk.height` | 5.0 | 4.6 |
| `branches.start` | 0.4 | 0.3 |
| `branches.length` | 0.7 | 0.85 |
| `branches.angle` | 46 | 26 |
| `branches.up_attraction` | -0.5 | -0.4 |
| `branches.flatness` | 0.2 | 0.1 |
| `branches.apical_dominance` | 0.2 | 0.55 |
| `branches.crown_angle_variation` | -0.3 | 0.3 |

## Validation targets

1. **Habit, measured on the meshes** (bounding box of wood and leaves, seeds 42, 1, 2, 3 and 4): the crown's
   height is at least 0.6 of its width on every seed (0.47.1: 0.48, 0.58, 0.61, 0.51, 0.48), and the tree is at
   least 1.2 times the trunk's height (0.47.1: 1.04 to 1.18; the pole is the top).
2. **Curtain kept**: the lowest leaf under 0.3 of the trunk's height and the bottom third of the trunk's
   height as wide as the middle third (`willow_curtains_hang_to_the_ground`, protected, unchanged); the
   probe's cover of the lower and middle thirds of the crown from the front and the side at least 0.4 (0.47.1:
   0.44 to 0.57 lower, 0.42 to 0.59 middle), and the highest point of the tree no more than 1.2 m above the
   highest 1 percent of its leaf vertices (no thread of leaves standing out of the dome).
3. **Cost**: foliage triangles no more than 0.47.1's mean (36 800 to 41 200 over the eight seeds), wood within
   2 percent of it (39 500 to 40 800), the build probe's median no slower than 0.47.1's.
4. **GPU growth** parity on `age_willow_6` and the willow scenarios no worse than iteration 16's 1.043 and 1.350;
   `grown_leaves_and_flowers_match_the_cpu_meshers` passes.
5. Every other preset's meshes are bit-identical (their `stats.tsv` columns 1-10).
6. Same seed, same tree.

## Measured (iteration 17)

The probe (`examples/zz_probe.rs`, deleted before the commit) on seeds 42, 1, 2, 3, 4, 5, 6 and 7, mature, level 0:

| | crown height over width | trunk top over the tree's top | foliage triangles | wood triangles | lowest leaf | lower third covered (front / side) |
|---|---|---|---|---|---|---|
| 0.47.1 | 0.46 to 0.62 (median 0.52) | 88 to 98 % | 36 844 to 41 156 | 37 818 to 40 800 | -0.26 to 0 m | 0.48 to 0.57 / 0.44 to 0.62 |
| now | 0.64 to 1.03 (median 0.83) | 59 to 81 % | 35 396 to 37 864 | 40 404 to 41 064 | -0.13 to 0.08 m | 0.41 to 0.53 / 0.44 to 0.55 |

The crown is 1.3 times as tall and 0.9 times as wide; the curtain's lower-third cover is within 0.07 of what it
was (the dome's box is taller, so the same ropes cover a smaller share of a larger area). Alternatives tried and
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
- `up_attraction` is left at -0.4 although the best silhouette seen in the search had it at -0.2: `willow_values`
  pins it under -0.3 and a protected check is not loosened.
- The ratio is 0.83 at the median, not the species' 1: the strands' ropes hang from the arches and floor
  avoidance keeps a hand's breadth under them, and the tree is drawn 2 m shorter than the 4.6 m trunk plus limbs
  would make it at ratio 1; a taller tree costs triangles in proportion.

## Pending index

Weber and Penn 1995, Hollender et al. (as in `weeping-strands.md`); the Missouri Botanical Garden and URI pages
and Orwa et al. are web sources, not in the corpus; the SpeedTree manual pages likewise.
