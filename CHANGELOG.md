# Changelog

All notable changes to `bevy_flora`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version bump
(`0.x`) may break the API.

## [Unreleased]

A change of `TreeCondition` can be drawn as a decline over seconds (`docs/research/condition-glide.md`), a leaf that appears on a growing branch grows from nothing instead of popping in (`docs/research/leaf-ease-in.md`), and a preset is found by its name and a tree says when it is grown, what an editor and a game each wrote for themselves (`docs/research/preset-names.md`), and a tree is built from any parameters at all, in bounded time and memory, whatever a `.tree.ron` file or an editor's number field held (`docs/research/hostile-parameters.md`).

### Added

- **`ConditionGlide { seconds }`: a change of `TreeCondition` can be drawn as a decline** over seconds, not
  in the frame after it is set (`docs/research/condition-glide.md`, backlog 2 of iterations 6 to 9). A
  resource, 0 by default: a change is drawn at once, as every caller has had it. Above 0 the share of the
  crown's leaves that is lost and the dryness of the leaves that are left rise continuously from what the
  tree shows now to what the new class shows (a smoothstep through the classes' own values: lost 0, 0.35,
  0.8 and withered 0, 0.5, 1 at healthy, sick and dying, so a settled tree is drawn exactly as before). A
  change half way through a glide starts where the tree is, so the picture never jumps whatever the game
  does; `Healthy`, or removing the component, recovers a tree the same way, the leaves coming back; a tree
  that appears with its condition (a save loaded, a pool whose assets arrive after its trees) shows it at
  once, as does any tree while the resource is 0. `FloraIndex` and `condition_of` answer the class the game
  set at once, only the picture glides. The time is the app's `Time` (a paused game does not decline) and
  the value that counts is the one the resource has when the change is made. Exported from the crate root
  and the prelude.
- `inspect --decline <seconds>[,<after>[,<every>]]` (with `--condition`, `dying` by default) sets the
  condition once the tree is ready and photographs the glide: `--condition dying --decline 8,0,1 --frames 9`
  is eight seconds, a frame a second (deterministic: the clock steps 1/60 s a frame). Meshes, the impostor and
  the falling leaves show the same level; the last frame of that run is byte for byte the image of `--condition
  dying`, and the first of a healthy tree.
- **`TreePreset::from_name`, `FromStr`, `Display` and `UnknownPreset`: a preset from its name**
  (`docs/research/preset-names.md`). `TreePreset::from_name("dragon_tree")` and `"dragon_tree".parse::<TreePreset>()`
  are the inverse of `TreePreset::name` (exact, snake_case: `"Oak"`, `"oak "` and the `serde` spelling
  `"DragonTree"` are not presets) and `Display` prints it, so the two round-trip. `parse`'s error,
  `UnknownPreset` (exported from the crate root; `input()` is the string asked for), is a `std::error::Error`
  whose message names what was asked for and lists every preset, a sentence a command line or an editor can
  show as it is. bevy_emerge's tree editor wrote
  `TreePreset::ALL.iter().find(|t| t.name() == name)` twice and this crate's examples six times; a downstream
  crate cannot add `FromStr` to a type it does not own.
- **`TreeParams::mature_at()`: the age at which a tree is fully grown**, in the tree's own growth units (an oak
  and a palm take different numbers). It is the age `Growing` runs a tree to and `TreeMature` reports, and
  above it a `TreeAge` changes nothing, so `TreeAge(0.5 * params.mature_at())` is a tree half way through its
  growing time of any species. It was reachable only by building the whole tree
  (`build_tree(&params, f32::INFINITY).mature_at`, as Sanctuary's forest did once per kind) or through
  `generate_skeleton`; this reads the skeleton and returns the same number bit for bit. `TreeAge` and
  `GrowthParams::maturity_age` (which is not that age) point at it.

- **`TreeParams::sanitized`, `MAX_NODES` and `MAX_LEAVES`: a tree from any parameters**
  (`docs/research/hostile-parameters.md`). Setting each numeric field of a preset to a hostile value, one at
  a time (`NaN`, infinities, `-1`, `1e6`, `1e30`, `u32::MAX`), hung the build, ran it past 3 GB or gave
  levels of detail with `NaN` positions: in the first 2 600 of 6 816 cases 164 returned non-finite meshes,
  138 ran out of memory, 19 hung and 6 aborted the process (a 65 535-deep `branches.recursion`); an age of
  `NaN` did it to every preset. Nothing in `TreeParams` was checked, and a `.tree.ron` with `height: 1e6` or
  a slider passing through `-1` hangs a `BuildInBackground` task on the compute pool, where it cannot be
  cancelled. Every numeric field now has a range (`src/limits.rs`; wide: at least ten times what the
  presets and the gallery's extreme scenarios use), and `TreeParams::sanitized()` returns the parameters a
  tree is built from, borrowed when they are in range (every preset) and otherwise a copy where a `NaN`
  reads as the field's default and anything else outside its range is the nearest end (`height` `1e9` is
  500 m, `branches.start` `-1` is 0, `radial_segments` `u32::MAX` is 64); a colour with a `NaN` channel
  reads as its default. `build_tree`, `build_lods`, `generate_skeleton` and `skeleton_mesh` do it, so they
  take any parameters, and `Skeleton::aged` reads an age of `NaN` as 0. A tree has at most `MAX_NODES`
  (5 000) branches (generation stops adding wood there) and `MAX_LEAVES` (60 000) leaves (the last nodes,
  the top of the crown, are dropped to fit); the presets have at most 588 nodes and 4 630 leaves and the
  gallery's 40 m giant oak 893 and about 21 000, so no tree the crate draws is cut. One branch bears at most
  1 024 leaves under `AllBranches` (the GPU's `leaf_count` reads the same function) and 256 strands.
  `FloraPlugin` writes the sanitised parameters into a `ProceduralTree` that is added or changed and an
  age of `NaN` as 0 (`sanitize_trees`, before the rebuild; a tree in range is not marked changed, a clamped
  one is rebuilt once), and a tree from a `ProceduralTreeAsset` takes its asset's parameters in range while
  the asset keeps the file's own words. Exported from the crate root: `MAX_NODES`, `MAX_LEAVES`.

### Changed

- **Breaking: `bevy_flora` meets the family's machine plate.** `FloraSystems` is the crate's one root set and holds every system `FloraPlugin` adds (the plugin's orderings are unchanged). `bevy_flora::prelude` is gone: everything it exported was already at the crate root, so `use bevy_flora::prelude::*;` becomes `use bevy_flora::{..};` with the names a file uses; the docs, guides, README and examples are migrated. `TreeShape`, `WoodCapsule`, `FromPool`, `PooledLook`, `PooledTree`, `LeafBurst`, `PooledDeadfall`, `IntoDeadfall`, `DeadfallKind`, `TreeRender`, `TreeTrunkMesh`, `TreeFoliageMesh`, `TreeFlowerMesh` and `TreeLodMesh` derive `Reflect` with `#[reflect(Component)]` (`TreeFruit` and `FallenFruit` gain `reflect(Component)`), and `FloraPlugin` registers them. `TreeRender` ignores its handles, skeleton and level data and has no reflected constructor. The crate has no settings resources: `LodSettings` and `DeadfallSettings` are values a pool is built from, not resources. `tests/machine_plate.rs` checks all of it.

- The draw of a condition is a level (0 healthy, 0.5 sick, 1 dying) in the `MeshTag`, not a class: bit 10 says
  a glide runs and bits 11 to 18 hold the level in 254 steps above the class bits, which stay what the game
  set. The shaders read it through `condition_level`, `level_defoliation` and `level_withering` (`common.wesl`)
  in place of the class functions, in every leaf stage (the main pass, the prepass, the GPU-growth twins and
  the impostor). A tree with no glide carries the bits it always did, and `forest`, `gallery` and every
  settled tree draw as before: of 526 gallery scenarios 522 are the same images and 4 (`painterly_oak`,
  `painterly_maple`, `toon_oak`, `toon_maple`) differ in one pixel of 1 228 800 each, the same pixel in the
  pair of a species (shader code changed, so instruction order may have); `stats.tsv`
  columns 1 to 10 are identical and no flag appears, on the CPU and under `--gpu`; CPU against `--gpu` over
  all 526: mean of means 0.029 of 255 and worst 1.162, the seven CI scenarios 0.218 and 0.922, all as before.
- What a glide costs, measured on an Apple M5 (10 000 pooled oaks, `forest --mode pool`, all declining at
  once, interleaved with a run without): a 6-second glide takes the frame from 5.0 to 6.5 ms for its length
  (p95 5.4 to 10.5), a 30-second one from 5.0 to 5.6 (p95 5.4 to 6.1): the cost is the number of mesh tags
  that change, and each tree changes its tag at its own moments (a hash of the entity), so a forest that began
  together does not rewrite every tag in one frame. The systems alone, headless: 0.40 ms a frame for 10 000
  settled trees, 0.50 ms while all glide over 30 seconds. A forest with no glide in flight is unchanged: the
  forest bench, interleaved base against new, medians of four, ms a frame (cpu): 10 000 pooled oaks 4.97
  (13.84) to 5.00 (13.84), 1 000 naive 34.88 (18.71) to 34.89 (18.77), 1 000 growing on the GPU 178.5 (243.4)
  to 180.1 (245.6): within 1% (the last is the bench's own spread).
- A tree that has a condition carries a crate-internal `ConditionShown` (what its meshes show of it) and, for
  the length of a glide, `ConditionGliding` in sparse storage, so only trees that glide are visited each frame;
  the falling-leaf systems run after the system that advances glides.
- **A leaf eases in** (`FoliagePlacement::AllBranches`: oak, willow, maple, spruce, pine, japanese maple,
  shrub, cherry, apple, juniper, fern). Leaves keep their places as a branch lengthens (0.47.0), but the
  one that appeared still appeared at its node's whole leaf size: measured over every `AllBranches` preset,
  a tree swept from 0 to maturity in 0.05-year steps (the README clip's pace), a leaf was on average a
  third of its mature size at first sight, one in four more than half of it, the largest at full size (a
  33 cm oak card out of nothing: an oak popped 1.5 leaves a frame, a willow 16). A branch now carries a
  real number of leaves (`density * length * height_mod`, at least one), and the leaf whose turn comes
  next is born at nothing and grows to its size, smoothstep, over the growth of branch the following leaf
  takes to be born; the leaves before it are whole, so a leaf is never larger than when grown and a
  mature tree is what it was, byte for byte. At 0.01-year steps no leaf is above 0.03 of its size at
  first sight (the willow's strands; 0.01 for the rest) and none changes size by more than 0.18 between
  two steps (both were 1.00 at 0.05-year steps).
- A young tree carries one more, part-grown, leaf per branch than it did, so a CPU build of one has more
  foliage: a median 13% more leaf vertices over the young gallery scenarios (up to 60% on a Japanese maple
  with five leaves), and takes up to 7% longer to build at age 4 (the shrub 0.69 to 0.73 ms, the spruce 0.40
  to 0.42 ms, the juniper 0.33 to 0.34 ms, the maple 0.32 to 0.33 ms); a mature tree builds as fast as
  before (within 1%), and a pool, a level of detail or an impostor (built from the mature tree) costs what
  it did (the forest bench: 10 000 pooled oaks 4.99 to 4.97 ms a frame, 1 000 naive 34.98 to 34.98,
  1 000 growing on the GPU 176.9 to 176.3, interleaved, medians of four).
- `GpuGrowth` shows the same: the node buffer's `leaf_count` is the real number (it was truncated), and
  the vertex stage scales a leaf by `smoothstep(clamp(leaf_count - rank, 0, 1))` times the node's leaf
  scale (`growth_common.wesl`, `piece_scale`; mirrored in `gpu/frame.rs`), so the CPU tree and the
  GPU-grown tree agree on a leaf's size as well as its place. CPU against GPU over the gallery: mean of
  means 0.029 of 255 (0.030 before), worst 1.16 (1.19), and the seven CI scenarios 0.218 and 0.922
  (0.221 and 0.920).
- Of the 526 gallery scenarios, 462 are the same images; the 64 that changed are young trees of the
  eleven `AllBranches` presets (`grow_*`, `age_*`, `*young_*`, `hull_*` at ages 2 and 5, `condition_*`
  at 4 and 6, `extreme_fastgrow_2`, `extreme_lagthick_6`, `strands_oak_4`), by 0.1 of 255 on average and
  1.6 at most (`hull_spruce_2`: the stylised hull crown is built from where the leaves are, whatever their
  size, so a part-grown leaf moves it a little earlier); no scenario at maturity moved (`stats.tsv`
  columns 1-10 are identical for all of them, on the CPU and under `--gpu`).
- The examples find a preset with `TreePreset::from_name` (`capture_growth`, `forest` for `--preset`,
  `--species` and `--undergrowth`, `inspect`, `tree_file`: six hand-written `find`s) and `capture_growth`
  reads a tree's age from `TreeParams::mature_at`; the clips are unchanged by it (the same number, built the
  same way). `tree_file --export` still takes a name in any case.

- Smoothing's pin grid no longer costs memory as the cube of the wood's width: `PinSpheres` bucketed every
  pinned sphere into each 0.25 m cell of its bounding cube, so wood 800 m across (a 50 m trunk radius flared
  16 times, now a legal value) took 6e7 cells a sphere and a smoothed tree 3 GB and growing. The grid doubles
  its cell until all the spheres together cover two million cells; a preset never does (it keeps the finest
  cell), so every smoothed tree is the same mesh. The worst trees the ranges allow build in 1.0 s and 310 MB
  (5 000 branches, smoothing, 32 sides) and 0.4 s and 180 MB (leaves at the budget), release, Apple M5; before
  the grid they took 2.0 to 6.3 s and 480 to 830 MB.
- Nothing a preset or the gallery draws changed: of 526 gallery scenarios all 526 images are byte for byte the
  main binary's and `stats.tsv` columns 1 to 10 are identical, no flag appears (first attempt, with a leaf
  budget of 20 000, cut the 40 m `extreme_giant` oak, which has about 21 000 leaves: the budget is 60 000).
  CPU against `--gpu`, the seven CI scenarios: mean of means 0.218 of 255, worst 0.922, as before. Cost,
  forest bench interleaved base against new, medians of three, ms a frame (cpu): 10 000 pooled oaks 9.26
  (29.98) to 9.23 (29.78), 1 000 naive 37.99 (39.03) to 38.05 (39.12), 1 000 growing on the GPU 419.7 (539.8)
  to 403.8 (529.1) (the machine ran about twice as slowly as in the last entries, other loops sharing it; the
  pairs are what count). `sanitized()` costs 94 to 110 ns a call (best of 20 x 50 000, in range), so a build
  of the smallest preset (the dead tree, 19 to 57 us) pays under 1%.

### Tests

- `limits.rs`: `every_preset_and_the_default_are_inside_every_range` (and says which fields if one is not),
  `every_numeric_field_is_limited` (all 107 numeric fields found by reflection, so a field added later is
  covered, each at `NaN`, both infinities and `±1e30`: finite and inside any range afterwards),
  `a_hostile_value_in_any_field_still_builds_a_sound_tree` (the same fields at six hostile values, a birch with
  fruit, blossom and autumn, mature and young with its levels of detail: finite positions and normals, valid
  indices, at most `MAX_NODES` nodes; 640 builds in 2.5 s; before, it hangs on the first),
  `a_hull_and_a_prism_with_hostile_numbers_build_too`, `the_leaves_the_ranges_allow_are_cut_at_the_leaf_budget`
  and `the_wood_the_ranges_allow_is_cut_at_the_node_budget` (every size at its top), `an_age_of_nan_is_an_age_of_zero`,
  `no_tree_the_crate_draws_comes_near_the_budget` (the presets and the 40 m oak: at least 4 times under the
  node budget, 2 times under the leaf budget), `a_tube_never_has_more_sides_than_the_range_says`
  (`skeleton_mesh` with `u32::MAX` sides, rings and smoothing); `plugin.rs`:
  `a_tree_with_hostile_parameters_is_held_in_range_and_built_once`; `tree_asset.rs`:
  `a_tree_follows_its_asset_as_the_generator_can_draw_it`. No existing test was changed. 369 lib and 37 doc
  tests.
- `a_preset_is_found_by_its_name_and_by_nothing_else` (all 20 round-trip through `from_name`, `FromStr` and
  `Display`; a near miss, a different case, a padded name and the `serde` spelling are refused),
  `the_error_says_what_was_asked_for_and_what_there_is` (the input and all 20 names, usable as a
  `Box<dyn Error>`) and `mature_at_is_the_age_a_tree_stops_changing` (every preset, three seeds: the number
  `build_tree` reports; the wood and leaves at that age are the full-grown ones to under 1e-6 m, the wood at
  half of it is not).
- `a_leaf_that_appears_grows_from_nothing` (every `AllBranches` preset, 0.01-year steps: no leaf above a
  tenth of its size at first sight, none moving by more than 0.3 of it between steps; it fails on the old
  pop, an oak leaf appearing at 0.365 of its size), `a_leaf_is_born_at_nothing_and_reaches_its_size_over_the_next_leafs_growth`
  (the curve: 0 before its turn, 1 a leaf-spacing later, slow in and out, a branch's first leaf always
  whole), and `all_branches_leaves_appear_as_the_branch_lengthens` now also requires leaves easing in on
  the GPU side (`grown_leaves_and_flowers_match_the_cpu_meshers` holds every part-grown leaf's vertices
  to the CPU mesher's).
- Condition glide (`condition.rs`): `a_change_of_condition_glides_to_its_class_and_arrives_as_the_class_tag` (a
  dying change over two seconds: the class bits say the class and the index answers it on the first frame, the
  level starts at healthy, rises monotonically without a frame step over a few percent, is half way half way
  through and ends as the class's own tag, bit for bit), `a_change_in_the_middle_of_a_glide_starts_where_the_tree_is_and_a_recovery_glides_back`
  (a reversal from 199 of 254 is continuous, `Healthy` and a removed component recover to no bits at all),
  `a_tree_that_appears_with_its_condition_shows_it_at_once_and_so_does_a_glide_of_nothing` (procedural and
  pooled), `a_pooled_trees_glide_survives_a_look_change_and_a_rebuild_keeps_it`,
  `a_pause_stops_a_glide_and_a_settled_tree_has_none_running`, `trees_that_began_together_do_not_step_together`
  (it fails with the per-tree fractions removed: all 120 trees step in one frame), the level functions (the
  class values at the anchors, monotone between, clamped outside) and `a_declined_crown_at_a_level_has_lost_the_leaves_a_worse_level_loses_and_more`;
  `falling.rs`: `a_tree_part_way_through_its_decline_drops_the_leaves_it_shows_and_they_fall_as_dry`. The
  existing condition tests pass unchanged (no protected check was touched). 354 lib and 69 doc tests.

## [0.47.0] - 2026-10-31

A tree can be sick or dying and is drawn so (issue #87); the willow weeps to the ground (D-052); and a growing
branch keeps every leaf where it was, so the CPU tree and `GpuGrowth` put them in the same places (D-078).

### Added

- `TreeCondition { Healthy, Sick, Dying }` (ordered from well to ill; reflected and registered by the
  plugin): a component a game inserts on a tree's entity, a `ProceduralTree` or a `PooledTree`. A tree
  without one is healthy, so a pooled forest pays nothing for the trees that are well; inserting
  `Healthy` or removing the component clears it. It is the tree's own, not its pool's: it survives a
  change of variant, a reload of the pool, and a `PooledTree` taken off its entity and put back. A felled
  tree keeps the component as it was cut and leaves the index.
- The tree is **drawn in its condition**. A `Sick` crown loses about a third of its leaves
  (`TreeCondition::defoliation()`, 0.35) and the leaves that stay wither half way to a dry ochre-brown
  (`TreeCondition::withering()`, 0.5), each its own way, so it is thin and mottled; a `Dying` one loses
  four fifths (0.8) and what is left is dry (1.0). Wood, fruit and flowers are as they were; the lost
  leaves are lost in the shadow maps too (a thin crown lets the light through). Every way a tree is
  drawn shows it: the main pass, the prepass and shadows, every level of detail of a `TreeLod` or a
  pool, the impostors (patches of the baked crown wither and thin), and trees that grow on the GPU
  (`GpuGrowth`; CPU against GPU over the condition scenarios: mean of means 0.023 of 255). It composes
  with `FloraSeason` (a sick tree's leaves wither from the colour the season gives them to the dry
  colour, a mix and not a product; its lost leaves are lost whatever the season does) and with
  `RecolorTree` and `PooledLook`. The class travels in bits 8 and 9 of the `MeshTag` of every mesh the
  tree is drawn with, above the glide tag of a pooled look, so trees of one mesh and material still draw
  in one batch whatever their conditions, and a tree with no condition draws exactly as before (all 492
  earlier gallery scenarios, byte for byte). The shaders read the tag only while some tree has a
  condition or a look glides (the glide header of the shared buffer, `tags_read`), so a forest of healthy
  trees pays no per-instance fetch. A device with no flora materials (WebGL2) shows the state and not
  the look.
- Leaves that fall from a sick or dying tree (a shake, a landing, the season's leaf fall, `DropLeaves`)
  are leaves it still has, and fall as dry as those left on it.
- `TreeCondition::defoliation()` and `TreeCondition::withering()`: the shares, public.
- `FloraUniform::withered`: the ratio of the dry leaf colour to the species' leaf colour (as `autumn` is of
  the autumn colour), zero for wood.
- `gallery`: 30 `condition_*` scenarios (sick and dying on eleven presets, a young oak, a young pine,
  autumn and leaf fall, and the stylised hull conifer); `inspect --condition sick|dying`;
  `forest --sick <share> --dying <share>`.
- `IndexedTree::condition` and `FloraIndex::condition_of(entity) -> Option<TreeCondition>` (`None` for a
  tree the index does not hold). The plugin keeps them up to date from `Changed<TreeCondition>` and
  `RemovedComponents<TreeCondition>`, and reads the component whenever it indexes a tree, so a condition
  set before a pooled tree's pool has loaded shows when it does.
- `TreeCondition` in the crate root and the prelude; a crate-root doctest and the README's "Finding trees"
  section show it.
- **`BranchParams::strands` (`StrandParams`) and `NodeKind::Strand`: a weeping habit** (D-052, the willow's
  curtain). Slender twigs hang straight down from the outer part of every branch and keep hanging past its
  arch, for `length` times the trunk's height or down to `clearance` above the floor, as the long rope-like
  twigs of a willow do: `density` strands per metre of branch along the part past `start`, each leaning off
  plumb by up to `spread` degrees. A strand is a node of the skeleton one level below its branch, so it
  grows with it (born when its branch reaches the point it hangs from), carries leaves along its whole
  length (every other branch bunches them toward its base), sways, burns, falls and is drawn at every level
  of detail like any twig, and trees that grow on the GPU grow it. It is a straight rope of two rings of
  wood (`min_twig_radius` thick), and its leaf `i` keeps its place on the grown strand at every age, so the
  CPU and `GpuGrowth` agree on its leaves. Strands draw from streams of their own and come after every
  other node, so a tree without them (`density` 0, the default, and every preset but the willow) is the tree
  it was, bit for bit, and one with them keeps every other branch where it was (tested). A `.tree.ron`
  without the field loads with none. Gallery: `strands_oak`, `strands_oak_plumb`, `strands_oak_floor2` and
  `strands_oak_4` (526 scenarios).
- **The willow weeps** (a design decision, `qa/visual_findings.md` D-077): `strands` density 1.6 per metre from
  a tenth of each branch, up to 0.7 of the trunk's height; its limbs start lower (`branches.start` 0.5 to
  0.4) and rise a little more (`angle` 50 to 46, `angle_curve` -0.4 to -0.1), its trunk is slimmer at the top
  (`trunk.taper` 0.4 to 0.22), and its leaves are fewer and larger (`foliage.density` 10 to 6.5,
  `leaf_size` 0.26 to 0.34). The crown is a dome with a curtain to a hand's breadth above the grass: the
  lowest leaf is under 30% of the tree's height, where it was a low mound on a bare post. It costs: a mature
  willow is 119k vertices (wood 30k, leaves 93k) where it was 55k, builds in 4.4 ms where it took 2.2, and its
  coarse levels hold 24k and 9k leaf vertices where they held 9.5k and 3.6k.

### Changed

- **Breaking:** `IndexedTree` has a new public field, `condition`, so a struct literal of it needs one
  (a game builds none: the index hands them out); `FloraUniform` has a new public field, `withered`
  (a struct literal of it needs one; the plugin builds every one). The next release is a minor one.
- **Breaking:** `BranchParams` has a new public field, `strands` (a struct literal of it needs one, or
  `..Default::default()`), and `NodeKind` a new variant, `Strand` (an exhaustive `match` needs an arm). A
  `.tree.ron` without the field loads with no strands. The next release is a minor one.
- Of the 522 earlier gallery scenarios, 489 have the meshes they had (`stats.tsv` columns 1-10 identical);
  the 33 that moved are 29 willow scenarios and the pipe-model shrub and fern (Fixed, below). Every one of
  the 488 non-willow scenarios whose mesh did not move is the same image below the HUD.
- **Breaking:** `Node` has a new public field, `full_length` (a struct literal of it needs one; the skeleton
  generator sets it). The next release is a minor one.
- Of the 526 gallery scenarios, 462 are the same images as with the weeping strands above; the 64 that changed are young trees of
  the 11 `AllBranches` presets (`grow_*`, `age_*`, `*young_*`, `hull_*` at ages 2 and 5, `condition_*` at 4 and
  6, `extreme_fastgrow_2`, `extreme_lagthick_6`, `strands_oak_4`): their leaves are the same leaves in slightly
  different places. Mature trees do not move (`stats.tsv` identical; the only columns that changed are the
  bounding boxes of 37 young scenarios and the leaf count of two apple ones, `age_apple_8` and
  `pipeyoung_apple_5`, whose fruit rides on the leaf slot it was drawn for).

### Fixed

- **Leaves keep their places as a branch grows** (`AllBranches`: oak, willow, maple, spruce, pine, japanese
  maple, shrub, cherry, apple, juniper, fern; `docs/research/leaf-places.md`). A branch spread the leaves it
  had over its length, so each time it gained one, every leaf of it moved to a new place: measured over
  every preset at ages 0 to 20 in steps of 0.1, an oak leaf hopped 15 cm (p95, up to 28) per step, a
  maple's 18 cm (up to 36), and a leaf of the tree grown on the CPU stood a mean 8 to 14 cm, p95 35 to 58
  cm and up to 1.3 m from the place `GpuGrowth` gave it. A branch now has the leaves of its grown self as
  slots, each at a fixed fraction of its length with draws of its own (size, direction, turn, tint, hue),
  and shows the ones that appear first: the tip's, then the gaps' (`foliage::leaf_ranks`), so a young limb
  carries leaves along its whole length as it did, none moves when another appears, and the CPU tree and
  `GpuGrowth` agree on every leaf (0 against up to 1.3 m; no change to the shaders or the node buffer: the
  mature mesh is tagged with each leaf's rank). CPU against GPU over the gallery: mean of means 0.030 of
  255 (it was 0.080; 83 of 526 PNGs byte-identical, it was 79 of 492), worst 1.19 (a young willow; it was
  2.11), and the seven CI scenarios 0.221 and 0.920 (they were 0.694 and 1.452). `GpuGrowth`'s list of
  differences loses `AllBranches` leaves.
- `TreeShape`'s crown box did not hold every leaf of a tree whose branches arch: it spanned a branch's start
  and end, and a bent branch's axis rises past both before it falls, so leaves on the arch of a drooping
  limb could stand outside it (the willow at age 5, with its new limbs; `the_crown_holds_every_leaf_and_
  the_box_holds_the_crown_at_every_age` found it). The box now follows the samples of the axis.
- With `MeshParams::pipe_radius_enabled`, the floor check left no room for the pipe model's thicker wood,
  so a branch that drooped to the floor could dip up to `pipe_radius_min` below it (`BELOW_FLOOR` on the
  willow's pipe scenario). Floor avoidance now keeps wood `pipe_radius_min` above the floor when the pipe
  model is on (`pipe_shrub`, `pipe_fern` and their young scenarios move by a centimetre or two; no other
  preset changes, and nothing changes with the pipe model off). A strand keeps its own radius under the
  pipe model (the floor on twigs would make each one a rod as thick as the thinnest branch).

### Tests

- `a_condition_set_on_a_tree_reads_back_through_the_index_as_set_changed_and_cleared`,
  `a_pooled_trees_condition_is_its_own_and_survives_what_the_pool_does`,
  `a_condition_set_before_the_tree_is_indexed_applies_when_it_is`,
  `a_felled_trees_condition_does_not_panic_and_stays_on_its_entity` and
  `a_recolour_and_a_condition_in_one_frame_both_apply_and_equal_looks_still_share_a_material`: the five
  acceptance criteria of #87 (the last now also proves that a condition makes no material of its own).
  `index.rs`'s `tree()` helper gains `condition: Healthy` (an assumed owner exception: a new field's
  default in a literal; no assertion changed).
- `the_meshes_of_a_tree_carry_its_condition_as_set_changed_rebuilt_and_cleared`,
  `a_pooled_trees_meshes_keep_their_looks_tag_beside_the_condition`,
  `a_declined_leaf_is_lost_by_its_own_draw_and_withers_toward_the_dry_colour`,
  `a_dying_tree_drops_only_the_leaves_it_has_left_and_they_fall_dry` and
  `a_withered_leaf_takes_the_dry_colour_whatever_the_species_and_wood_never_withers`.
- `strands_hang_from_the_wood_and_stop_above_the_floor`, `strands_leave_every_other_branch_where_it_was`,
  `a_tree_that_asks_for_no_strands_has_none`, `strands_on_any_tree_keep_off_the_trunk_and_the_floor`,
  `a_strand_is_born_when_its_branch_reaches_the_point_it_hangs_from`,
  `every_branchs_leaves_keep_their_places_as_it_grows` (its strand half fails when the leaves re-spread
  over the strand's length at each age) and `willow_curtains_hang_to_the_ground` (D-052's own acceptance: the lowest leaf vertex under
  30% of the tree's height, and the bottom third as wide as the middle within 10%: strands hang plumb, so
  a curtain is as wide at the ground as at its widest where a mound narrows to it; three seeds). A
  `StrandParams` doctest. No existing assertion changed in that work.
- `every_branchs_leaves_keep_their_places_as_it_grows` (every `AllBranches` preset and the willow's strands,
  every age from 30% to 95% of maturity: a young branch's leaves are slots of the grown branch, at the same
  fractions of it, with the same draws; fails with the re-spread),
  `a_young_branch_shows_leaves_over_its_whole_length` (the order leaves appear in, and its gaps),
  `a_count_that_lands_on_a_whole_number_keeps_its_leaves_in_place` (fails with `length / progress`), and
  `grown_leaves_and_flowers_match_the_cpu_meshers` now runs over the `AllBranches` presets too (it skipped
  them; it compares the leaves of the CPU tree and of `GpuGrowth` vertex by vertex at seven ages, with the
  leaves that face out from their branch's tip made to face along it: that facing is the mature tip's on
  the GPU, a documented difference). Assumed owner exception: `a_strands_leaves_keep_their_places_as_it_grows`
  asserted that a young strand's leaves are the first of the grown strand's; the rule it pins (a leaf keeps
  its place as its branch grows) is unchanged and now holds for every `AllBranches` branch, but the leaves
  that show first are the tip's and then the gaps', so the test became the general one above, which
  checks the same places, draws and counts, on more.

## [0.46.1] - 2026-10-30

The plant lean is a fixed share of the plant's own height (the owner's 1792016453, "This bush seems to
be swaying with the wind a lot, but the big trees aren't even a little. This should be more uniform.").

### Changed

- `wind_bend`'s lean measured every species against its own trunk: the height term
  (`h = local_y * flora.tree.x`, square) is 1.0 at *any* plant's top, so a 0.5 m shrub reached the
  same `h` an 8 m pine did, and the `scale_y * trunk_height` factor made the lean proportional to the
  plant's height in units. What decided who moved was the species' `sway` (the shrub presets carried
  1.2-1.5, the pines' 0.6-0.9), so a bush leaned 4.8% of its own height and the pine 2.8% of its far
  greater one. The lean now measures the vertex's own distance from the root
  (`MAIN_BEND * response.x * length(rel)`): every species leans a fixed share of its own
  height, `MAIN_BEND` is unchanged (the top-of-plant lean is reproduced exactly), and the profile down
  the trunk is linear in that distance rather than a square of the trunk-normalised height.
- The presets' `wind.sway` is retuned from a 0.2-1.5 spread to a 0.9-1.6 multiplier band (bonsai
  0.2 -> 0.9, baobab and dragon_tree 0.3 -> 0.9, redwood 0.5 -> 0.9, cypress and the dead tree
  0.6 -> 0.9, pine, spruce, oak, cypress and the rest 0.7-1.2 -> 0.9-1.1, palm 1.5 -> 1.4, willow
  1.4 -> 1.6; flutter and the frequencies untouched), so a species' `sway` says what the params doc
  says it does - a willow over an oak - without a bush out-leaning a pine.

### Tests

- `the_lean_comes_from_the_vertexs_own_distance_from_the_root`: the shader's lean must come from
  `length(rel)` and must not be trunk-normalised (holds 1792016453 fixed).
- `wind_bend_takes_no_scale_and_the_lean_has_no_scale_factor`: `wind_bend` takes no scale and none
  of its six callers hands it one (issue #90).
- `a_shrub_and_a_pine_sway_by_near_the_same_share_of_their_own_height`: mirrors the formula over the
  shipped presets; a pine and a shrub within 0.02 of each other's share, the pine's absolute lean
  still greater, and the species' sway spread at most 2:1.
- `the_uniform_holds_the_tree_and_nothing_of_the_weather`: willow's pinned response follows the retune.

### Fixed

- `forest --offscreen` (and `--screenshot`) never got past its first frames on Linux:
  `WinitSettings::continuous` waits for a window's redraw there (X11 and Wayland), and an offscreen
  run has no window, so 10 000 pooled trees sat in loading for 30 minutes. Offscreen, the benchmark
  now runs a reactive update mode with no wait, which polls. `inspect`, which never opens a window,
  still hangs on Linux the same way.
- `build_tree` panicked (`assertion failed: min <= max` in `u32::clamp`) when
  `MeshParams::min_radial_segments` was above `max_radial_segments` with `adaptive_resolution` on,
  from code, a `.tree.ron` (`max_radial_segments: 3` under the default floor of 4, saved while
  `file_watcher` regrows the tree) or a reflection inspector (issue #91). The upper bound now wins
  when they cross: every branch gets `max_radial_segments` sides, never fewer than 3. Bounds in
  order build the same trees as before. New gallery scenario `extreme_crossed_sides` (492
  scenarios).
- `FloraIndex::nearest`, `within`, `obstacles_within` and `cast_ray` never returned for an unbounded
  bound (`f32::INFINITY` or `f32::MAX`): each walked every grid cell the bound covers (2^64 of them),
  and a ray that missed every trunk walked on forever (issue #92). A query's walk is now cut to the
  box of cells the index fills and, where that box is mostly empty, only the occupied cells are
  walked, in the same order as before; a ray stops where its track leaves the box. Queries with a
  finite bound return the same trees in the same order. An infinite bound means "wherever it is".
- The forest benchmark's `cpu ... ms/frame` had a resolution of 1 s on Linux (`ps -o cputime` prints
  whole seconds there), up to 1.7 ms/frame of rounding over 600 frames (issue #93). It reads
  `utime + stime` from `/proc/self/stat` there (10 ms); macOS keeps `ps`.
- A tree drawn at a `Transform` scale other than 1 leaned that many times too far (D-073, issue
  #90): `rel = world - origin` is a distance between two world-space positions and already carries
  the tree's scale, and the first 0.46.1 lean multiplied `scale_y` in a second time, so a tree at
  scale `s` leaned `s` times the share of its own height it leans at scale 1. Measured with two
  mature pines under a steady wind (crown top's shift over the tree's height): 3.43% at scale 1 and
  10.34% at scale 3 with the double count, 3.14% and 3.10% before 0.46.1. It showed in a `Scatter`
  forest (scale 0.85-1.15 by default) as a spread of leans, and in a world in feet or centimetres,
  whose trees are scaled by 3.28 or 100, as trees bent too far, lain nearly flat by the bend's
  rescale. `wind_bend` no longer takes `scale_y`; at scale 1 nothing moves.

### Docs

- `AGENTS.md`'s release checklist re-records all eleven README clips, not five, with the capture and
  `ffmpeg` lines for `fire`, `push`, `chop`, `harvest`, `playground` and `isometric` (issue #94).

## [0.46.0] - 2026-10-13

The fire's hottest spots and a save of only what burnt (issue #77): a game bucketed the burning
cells into squares itself to place its lights, and hand-rolled a sparse form of a 262 144-cell grid
to save it. On crates.io this is the release after 0.36.0: it carries 0.37.0 to 0.45.0 too,
entered below, each also released on GitHub.

### Added

- `FloraFire::hottest(square, n)`: the burning cells gathered into world squares `square` wide,
  the `n` hottest as `FireSpot { at, heat, share }` (the heat-weighted middle of a square's
  burning cells, their heat summed, and that heat over the cells a square holds), hottest first,
  ties in the squares' order. Only the touched rectangle is read. The `fire` clip's lights use it.
- `FloraFire::snapshot()` and `FloraFire::restore_snapshot`: `FireSnapshot { side, cells }` lists
  `(index, char, heat)` of the cells holding any (serializable with the `serde` feature), and
  restores a fire of as many cells as `restore` does from every cell, a cell still burning
  burning on; a snapshot of another grid size is refused.

### Fixed

- `ARCHITECTURE.md` gave the shared buffer's weather 60 vectors and the fire's grid slot 60; since
  0.45.0 they are 189.

### Measured

- **Look.** The `fire` clip, its six lights now placed by `hottest`, is 0.45.0's frame for frame;
  so are the 491 gallery scenarios (CPU and `--gpu`), the other clips (but a chop frame at a mean
  of 0.000 of 255) and a pooled forest's screenshot, no `ERROR`.
- **Cost.** No system was added: `hottest` and `snapshot` read only the touched rectangle when the
  game calls them. 100 000 pooled trees (`forest --offscreen --deterministic`, Apple M4 Pro, two
  alternating pairs on a loaded machine): 10.7 to 11.5 ms a frame against 10.9 to 14.4.

## [0.45.0] - 2026-10-13

Gliding recolours and a shadow range that follows the camera (issue #76): a game's palette
switched its trees at the middle of a glide because a look's colours could only snap, and it
quieted the shadows of every tree beyond a fixed distance of the origin, once, as it planted them.
`RecolorPool` gained a field, so its struct literals need `glide` (a minor release). On crates.io
this is the release after 0.36.0: it carries 0.37.0 to 0.44.0 too, entered below, each also
released on GitHub.

### Added

- `RecolorPool::glide`: seconds over which the look glides from the colours it shows to the new
  ones, blended in linear RGB, on the GPU. Every mesh of a pooled tree, impostor and pooled
  deadfall carries its look's `MeshTag` (a tag per pool and look, up to
  `RecolorPool::MAX_GLIDE_LOOKS`, 64), the shared buffer holds what each gliding look's wood and
  leaves are multiplied by, and the shaders multiply: a frame of a glide writes those few vectors
  and no material, which would have Bevy re-prepare every mesh drawn with it (a first version that
  wrote the blend into the materials every frame cost 20 000 trees +60 ms of CPU a frame, 5 000
  +9 ms). The materials take the new colours once, when the glide arrives; an unlit look turns lit
  or unlit then; turned leaves keep the autumn colour and flowers the blossom; the look's felled
  wood (`FromPool`) follows frame by frame. A later recolour takes over from wherever the look is;
  0 changes it at once, as before. Past 64 looks, or without flora materials, a look glides by
  being recoloured at once every frame. `RecolorTree` still changes a tree at once.
- `FloraShadows { radius }`: the standing trees (pooled or `ProceduralTree`) farther than `radius`
  metres (times `FloraUnits`) from the main camera's place on the ground (where an orthographic
  camera looks) cast no shadow, kept as the camera moves and as trees are planted or felled,
  touching only the trees that cross the edge. Trees the game quiets itself are left alone;
  removing the resource gives the others their shadows back. `forest --shadow-radius` uses it.
- `forest --looks --recolor --glide <s>`.

### Measured

- **Look.** `forest --trees 3000 --looks --recolor --glide <s> --warmup-secs 2` shot 2 s into
  glides of 8, 4 and 2.67 s (a quarter, half and three quarters of the way) and at once: the
  autumn band goes from red through pink and magenta to violet and the unlit band from cyan
  through olive to yellow, impostors with them, the oak's own band unmoved. With nothing gliding
  the shaders draw as before: a 20 000-tree pooled forest and the three-look forest (`--wind 0`)
  are 0.44.0's byte for byte, and so are the gallery (but the painterly and toon maple and oak, CPU
  and `--gpu`, at a mean of 0.000 of 255), the eleven clips (but six push frames and a chop frame
  at 0.000) and the pooled forest's screenshot, no `ERROR`. `forest --shadow-radius 40` from two
  camera places: only the trees near each camera cast; 100 000 trees with a 150 m range end as
  0.44.0's, which marked them as they were placed, at a mean of 0.007 (trees at the edge are
  measured to the bark now, not the trunk's centre).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, alternating pairs, a machine
  shared with other builds: load 4 to 15). A glide running through the whole measurement of a
  20 000-tree three-look forest against a recolour at once: the CPU 20.5 to 21.2 ms a frame
  against 20.6 to 22.2 (the first version, which wrote the materials, 86 to 90 against 22 to 27).
  100 000 pooled trees with a 150 m shadow range against 0.44.0's spawn-time marking: four pairs
  9.4 to 11.6 ms against 7.6 to 13.1, the pair differences -1.9 to +1.8 ms (mean -0.1); the
  range's pass ran 26 times while the forest was planted and not once after. Without either,
  13.1 to 13.7 ms against 13.3 to 18.0.

## [0.44.0] - 2026-10-13

Felled wood follows its pool's look (issue #75): a pooled tree that was felled became a
`ProceduralTree` that `RecolorPool` no longer reached, so a game kept a marker of its own on every
pooled tree and sent a `RecolorTree` per felled tree with each palette change. A later
`RecolorPool` now overrides colours a game gave such wood with `RecolorTree` (a minor release). On
crates.io this is the release after 0.36.0: it carries 0.37.0 to 0.43.0 too, entered below, each
also released on GitHub.

### Added

- `FromPool { pool, look }`: put on a pooled tree that is felled or notched as it leaves its pool,
  and on its stump and logs. Every `RecolorPool` of that pool and look recolours them with the
  standing trees (a felled tree's leaves too, in their still materials once it lies); remove it to
  keep one as it is.

### Measured

- **Look.** An oak of a pool, felled to lie (and another bucked into 2 m logs) under a pool
  recoloured every frame to violet wood and orange leaves (a throwaway copy of `inspect`): its trunk,
  leaves, stump and logs take the colours, the cut faces stay end grain; without the forwarding
  they keep the bark and green they were cut in. Nothing else that draws changed: the 491 gallery
  scenarios (but the CPU toon maple and oak at a mean of 0.000 of 255), the eleven clips (but three
  push frames and a chop frame at 0.000) and a pooled forest's screenshot are 0.43.0's byte for
  byte, no `ERROR`.
- **Cost** (`forest --trees 100000 --mode pool --offscreen --deterministic`, Apple M4 Pro, three
  pairs in alternating order with 0.43.0): 7.69 to 7.79 ms a frame against 7.69 to 8.08, the CPU
  27.50 to 27.63 against 27.70 to 27.87.

## [0.43.0] - 2026-10-13

A pooled tree felled before a save comes back as one insert, found by a name rather than by its
place (issue #74): a game kept the felled trees' positions, matched each to the pooled tree nearest
it within a tolerance, and spawned a `PooledDeadfall` copying that tree's pool, variant, look,
transform and shadow opt-out. `ScatterPoint` gained a field, so struct literals of it need it (a
minor release). On crates.io this is the release after 0.36.0: it carries 0.37.0 to 0.42.0 too,
entered below, each also released on GitHub.

### Added

- `IntoDeadfall(kind)`: inserted on a `PooledTree`, it turns the tree into deadfall of its own
  pool, variant and look where it stands, a `Stump`, a tree `Lying` the way it fell or one
  `Bucked` into logs: the same entity, now a `PooledDeadfall`, its transform, `PooledLook` and
  `NotShadowCaster` kept, out of the index's trees and in its obstacles. A tree spawned with it is
  deadfall from its first frame. A pool built without deadfall leaves the tree standing, with one
  warning.
- `ScatterPoint::id`: a scattered tree's number, the same for a seed, area and spacing whatever a
  density or a stand keeps, so a save can name the trees that were felled (serde: missing in old
  data, 0).

### Measured

- **Look.** `inspect --mode deadfall` (stumps, lying and bucked trees of two variants, oak and
  spruce, flat and on a slope) spawned as `PooledTree` with `IntoDeadfall` renders byte for byte
  as spawned as `PooledDeadfall`. Nothing else that draws changed: the 491 gallery scenarios, the
  eleven clips (but one chop frame at a mean of 0.000 of 255) and a pooled forest's screenshot are
  0.42.0's byte for byte, no `ERROR`.
- **Cost** (`forest --trees 100000 --mode pool --offscreen --deterministic`, Apple M4 Pro, three
  pairs in alternating order with 0.42.0): 7.63 to 7.71 ms a frame against 7.64 to 7.72, the CPU
  27.40 to 27.73 against 27.47 to 27.83: the conversion's system costs nothing while no tree asks.

## [0.42.0] - 2026-10-13

`FloraIndex` tells undergrowth from trees and answers in an order set by the forest (issue #73): a
game filtered out its junipers and ferns by height and ordered ties by `Entity`, whose numbers follow
the order of spawning. `TreeParams` gained a field, so struct literals of it that list every field
need it (a minor release). On crates.io this is the release after 0.36.0 (crates.io takes only a
few versions of a crate a day): it carries 0.37.0 to 0.41.0 too, entered below, each also released
on GitHub.

### Added

- `TreeParams::undergrowth`: a plant of the forest floor rather than a tree, true for the `Shrub`,
  `Juniper` and `Fern` presets (nothing about how they are drawn changes; `.tree.ron` files without
  it load as before, false).
- `IndexedTree::undergrowth`: the indexed plant's flag, to filter on where only trees count.

### Changed

- `FloraIndex::within` answers in an order that depends only on where the trees stand: each cell
  keeps its trees by position (x, then z), and the cells are walked in order. `nearest` breaks a
  tie the same way. The lying wood (`obstacles_within`) is kept the same way.

### Measured

- **Look.** Nothing that draws changed: the 491 gallery scenarios (but the CPU toon maple and oak at
  a mean of 0.000 of 255), the eleven clips (the chop clip's woodcutter asks `nearest` for each tree)
  and a pooled forest's screenshot are 0.41.0's byte for byte, no `ERROR`.
- **Cost** (`forest --trees 100000 --mode pool --offscreen --deterministic`, Apple M4 Pro, three
  pairs in alternating order with 0.41.0): ready after 2.1 to 2.2 s both, the longest loading frame
  189 to 195 ms against 191 to 195, then 7.88 to 7.95 ms a frame against 7.90 to 8.08: keeping each
  cell's few trees in order costs the load nothing to see.

## [0.41.0] - 2026-10-13

Stumps, logs and fallen trunks for a game's collision (issue #72): a stump told only which tree it
came from, and `FloraIndex` held standing trees only, so a game made its own shapes for the wood on
the forest floor (Sanctuary: about 125 lines, a stump radius guessed from the felled tree's
parameters and a hard-coded cut height). `TreeStump` gained a field, so struct literals of it need
it (a minor release).

On crates.io this is the release after 0.36.0 (crates.io takes only a few versions of a crate a
day): it carries 0.37.0 to 0.40.0 too, entered below, each also released on GitHub.

### Added

- `TreeStump::capsule`: the stump in its own space, from its foot to the middle of its cut, with
  the trunk's radius at the cut (felled and pooled stumps alike), as `TreeLog::capsule` is a log's.
- `FloraIndex::obstacles_within(at, radius)` and `obstacles()`: the lying wood as
  `Obstacle { entity, kind, capsule }` in world space: every stump, bucked log, and the trunk and
  limbs above the cut of a tree lying on the ground (a felled tree at rest that has a `TreeShape`,
  and a pooled deadfall's lying tree). `Obstacle::distance_to` measures across the ground. They
  are kept in a grid of their own, so the standing trees' queries cost what they did.
- `WoodCapsule::placed` (a capsule where a `GlobalTransform` puts it) and `WoodCapsule::above` (the
  part of it above a height).

### Changed

- `TreeStump` is no longer `Eq` (it holds a capsule of floats); `PartialEq` stays.

### Measured

- **Look.** Nothing that draws changed: the 491 gallery scenarios on the CPU and with `--gpu`, the
  eleven clips (but one fire frame at a mean of 0.000 of 255) and a pooled forest's screenshot are
  0.40.0's byte for byte, no `ERROR`.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.40.0): 100 000
  pooled oaks 7.73 ms a frame in the median of four pairs against 7.95, 1 000 individual oaks 24.52
  and 24.92 ms against 24.77 and 25.85: the obstacles cost a frame with no lying wood nothing to
  see.

## [0.40.0] - 2026-10-13

Pools that look alike draw with the same materials (issue #71): a game that builds a species at
several ages, in several looks, drew every pool and look with materials of its own, each a batch of
its own in the main pass and every shadow cascade (Sanctuary: 72 materials for 12 pools, about 2 ms
of its frame, and 60 lines and a 120-frame wait to share them by hand).

On crates.io this is the release after 0.36.0: 0.37.0, 0.38.0, 0.38.1, 0.39.0 and 0.39.1 were merged
but not published there (crates.io takes only a few versions of a crate a day), so 0.40.0 carries
all of their changes too, entered below: impostors for cameras that look down, pools built in the
background, pooled forests that load three times faster, hull crowns and prism trunks, world
units, colour looks and recolouring, pooled deadfall, the juniper and fern presets, and falling
leaves that do not depend on spawn order. Upgrading from 0.36 meets the breaking changes of 0.39.0.

### Changed

- A pool's flora materials are shared by every pool whose surfaces are equal: the ages of one
  species, and two looks of one wood colour, draw with the same ones. A forest of four species at
  three ages in three looks draws with at most 24 materials instead of 72.
- `RecolorPool` recolours that pool's look and no other, materials shared or not: when every look
  that draws with a material asks for the same colours in one frame (a palette gliding over a
  species' pools) the material is written in place, as before; a look recoloured on its own takes
  materials of its own (or the materials already drawn in those colours) and its trees are pointed
  at them, so `TreePool::materials` may hold other handles after a recolour. The pool asset is
  still never marked modified.

### Added

- `forest --ages <n>` builds every species' pool at n ages and spreads the trees among them.

### Measured

- **Cost** (`forest --trees 20000 --mode pool --scatter spruce,pine,maple,oak --ages 3 --looks
  --offscreen --deterministic`, Apple M4 Pro, six pairs in alternating order against the same code
  with the sharing taken out): 4.55 ms a frame in the median against 5.49 (4.50 to 4.99 against
  5.35 to 5.53, every pair faster), CPU time 14.63 against 15.85 ms a frame.
- **Look.** The 491 gallery scenarios render byte for byte as in 0.39.1 on the CPU and with
  `--gpu` (but the painterly maple and oak at a mean of 0.000 of 255; no `ERROR`), and so do the
  nine clips (but one fire frame and three push frames at a mean of 0.000) and the playground's
  demo: their falling leaves no longer re-shuffle when a release adds a system (0.39.1). The
  isometric demo differs in 151 frames at a mean of 0.000. A forest of four species at three ages
  in three looks, shared against the same code with the sharing taken out, before and after a
  recolour, differs by a mean of 0.59 of 255, the same needles in the same colours a hair apart
  (zoomed 4x): no pool draws with another's colours.

## [0.39.1] - 2026-10-13

The same frame every time (issue #68): which leaves a tree drops no longer depends on the order
entities were spawned in, and every pair of the plugin's systems that share data now has an order.

### Fixed

- Which leaves fall was drawn from the tree's `Entity` (and the drop's place among the frame's
  messages): the same scene dropped other leaves when its trees were spawned in another order, a
  save restored or one more system added anywhere, and each release that added a system re-shuffled
  the leaves of the clips. Now a drop draws from where the tree stands and the time (a second drop
  of the same tree in the frame from a stream of its own), and the season sheds the trees within
  reach nearest a camera first (ties by position), each from its own draws, so when `max_leaves`
  runs out it is the farther trees that keep their leaves, not the last spawned.
- `index_trees` ran before or after Bevy's asset events as the executor chose, so a pool added in a
  frame was indexed that frame or the next; it now runs after them. The pool systems now run after
  the procedural trees' (both make textures and write `TreeShape`s) and `drop_fallen_fruit` after
  the felling systems and before the pooled trees' (they all move `Transform`s, deadfall's
  included): the plugin's `PreUpdate`, `Update` and `PostUpdate` build with ambiguity detection as
  an error, which a new test holds them to.

### Measured

- **Look.** The 491 gallery scenarios render byte for byte as in 0.39.0 on the CPU and with
  `--gpu` (but the CPU painterly maple and oak at a mean of 0.000 of 255; no `ERROR`), and so do
  the growth, grove, seasons, fire and push clips and the isometric demo. The shake, fell, chop and
  harvest clips and the playground's demo drop other leaves of the same trees (a mean difference up
  to 4.4 of 255 in chop and 6.5 in the playground, all of it falling leaves and their shadows: old,
  new and their difference side by side at chop 183 and playground 185, every tree, stump, log and
  apple where it was), and their GIFs are re-encoded, the last time a new system will re-shuffle
  them. A pooled forest's screenshot differs by leaves a hair elsewhere in their sway (a mean of
  0.300 of 255; zoomed 4x, the same trees and leaves), most likely because its trees get their
  children a frame apart from before.
- **Cost.** Nothing is added to a frame in which no leaves fall (the leaf systems run only then;
  the rest is order between systems). Measured for the same change on top of 0.38.1, with the
  Apple M4 Pro's GPU shared with a document-conversion model and a language model throughout
  (100 000 pooled oaks took 11.9 to 29 ms a frame in either version, about 8 on a quiet machine):
  in six alternating pairs each it took a median 3.3 ms less with 100 000 pooled oaks and 2.8 ms
  less with 1 000 individual ones, and in four earlier pairs the CPU time per frame was 53.1
  against 50.8 ms (pooled) and 25.2 against 25.7 ms (individual): no regression to be seen in the
  noise.

## [0.39.0] - 2026-10-13

A forest in the colours of the game and a world in its units: low-poly conifers, undergrowth,
recolouring, pooled deadfall and any world unit (issues #49, #50, #54, #56 and #57). The crown
and trunk styles are new fields of `TreeParams`, `TreeStump::tree` is an `Option` and a pool's
materials are a `Vec`, so a minor release. The impostors keep 0.37.0's elevation, crown reach and
shadows, now baked grey and tinted per look, and a pool built in the background (0.38.0) is a pool
of one look, without deadfall.

### Added

- **`FloraUnits { units_per_metre }`** (#50), a resource (default 1.0: a world in metres, nothing
  changes) for a game whose world unit is not the metre: its trees are built in metres and placed
  with a `Transform` scale of about 3.28 in a world in feet, and everything the crate measures in
  the world itself now follows the unit. The rule, written on every field it touches: a field
  that sizes or times the crate's own simulation is in **metres**, multiplied by `units_per_metre`
  where it meets the world (`FellTree::cut_height`, `TreeNotch::height`, `BuckTree`,
  `FireSpread::speed` and `reach`, `FoliagePusher::radius` and `height`, `SeeThroughTrees::radius` and `in_front` (its `floor` is a world height and
  stays), `TreeShake::radius`, `GustFront::speed` and `width`, `LodDistances` (under `LodScale`
  and under an orthographic camera's zoom, whose view height is read in metres),
  `FallingLeaves::autumn_distance`, and the crate's own constants: gravity for felled trees and
  fallen fruit, the saw's and the notch's 5 cm margins, the bucked logs' gap, how fast falling
  leaves fall and drift, the pushers' rooting height and the 2 m cross-fade between levels of
  detail); a position, or a size that addresses the world, is in **world units** (every `at`,
  `target`, `centre`, `origin` and `feet`; `FloraFire::new`'s size and the radius of `ignite`,
  `extinguish` and `scorch`; `FloraIndex`'s radii and distances; `Scatter`'s area, spacing and
  grove size; `FloraGround`'s heights). The shaders get the unit in the shared buffer (slot 2,
  lane w): the pushers' reach, the thin stems' radii and the falling leaves' drift follow it.
  `FloraWind::speed` multiplies time, not a length, and stays as it was. Set it before planting:
  a pooled tree's bands of distance are made when it is spawned. Tests: a felled oak lands on the
  same update in a world in feet as in metres, a notched one is cut at the same metres, bucked
  logs are the same logs the same gap apart, a fire spreads the same number of cells a second, the
  default pusher covers the same share of a shrub, falling leaves fall for as long, and the bands
  scale. `FloraUnits` is in the crate root and the prelude.
- **Deadfall takes looks and the world's units** (#56). A `PooledLook` on a `PooledDeadfall` picks that
  look's materials (the look's calm wood and leaves, and its end grain, unlit with the look), made
  once for each pool and look; `RecolorPool` and a change of `PooledLook` re-dress the deadfall
  of that look, stump, logs and lying leaves included; `RecolorTree` on a pooled stump or log
  works (a stump and its logs carry what it needs). A lying tree rests on `FloraGround` and bucked logs
  lie a `LOG_GAP` of `FloraUnits` apart, so a world in feet lays the same deadfall as a world in
  metres, scaled (a test holds the turn and the logs' places). A deadfall is cut at the scale
  its tree is placed at: a cut height or log length is in metres where the instance's scale
  is `units_per_metre`. `inspect --mode deadfall` photographs it.
- **Pooled deadfall** (#56). `build_pool_with_deadfall(params, variants, age, lod, &DeadfallSettings,
  meshes)` (`build_pool` is unchanged) also cuts every variant as `FellTree` and `BuckTree` would:
  `PoolVariant::deadfall` holds the stump and its face (level 0), the wood above the cut at each
  level and its face, up to 600 contact points and the cut's outline (for the rest angle of each
  instance), and the logs. `DeadfallSettings { cut_height: 0.5, log_length: Some(2.0), min_radius:
  0.08 }` is in metres. A `PooledDeadfall { pool, variant, kind }` (`DeadfallKind::Stump`,
  `Lying { direction }`, `Bucked { direction }`) is placed from it: a `TreeStump` with shared wood and
  the cut face; the lying part turned about the cut's hinge until its limbs reach the ground there
  (`FloraGround` included, as `FellTree` does) with the variant's leaves, its levels of detail and
  its `TreeShape`; or the stump and `TreeLog`s in a row from it, as `buck_trees` lays them. Its
  wood and leaves are of the calm look (no sway), made once per pool. Deadfall is never in the
  `FloraIndex`, shakes and sheds nothing, is fire fuel, and takes `NotShadowCaster` like a pooled
  tree. `BuckTree` on a lying one makes it bucked in place. `felling` helpers it shares with
  `buck_trees` (`lay_log`, `trunk_alone`, `hinge_point`, `rest_angle`) are `pub(crate)`.

### Changed

- **Hull crowns and prism trunks** (#49). `TreeParams::crown` (`CrownStyle`, default `Leaves`) can be
  `Hull { tiers, sides, lift, jitter }`: the leaf cards are not drawn and a faceted stack of cones
  is wrapped round the leaf cloud instead (`tiers * sides * 2` flat triangles), and
  `TreeParams::trunk_style` (`TrunkStyle`, default `Round`) can be `Prism { sides }`: the trunk has
  that many faces, branch tubes and collars no more, nothing is smoothed, and every triangle of the
  wood has its own vertices and face normal (`MeshData::flat_shaded`). Every preset keeps leaf cards
  and a round trunk and renders as before; both fields are in `.tree.ron` files and under the
  `serde` feature (a file that leaves them out gets the defaults). `TreeParams::gpu_growable()` is
  true only for leaf cards on a round trunk.
  - `CrownStyle::Hull::trunk_only`: the wood of a hull tree is its trunk chain only (trunk and
    leader, no side branches; the hull spruce's wood goes from 9 425 triangles to 120), and the
    hull's apex covers the leader's tip. The skeleton keeps its branches. `false` draws them all.
  - Felling a prism tree: `hinge_point` took the outline with the most points for the trunk's
    cut, and a flat-shaded trunk has few, so a limb the cut crossed lengthwise won; the tree
    rested at 12 degrees instead of falling. It takes the outline that encloses the most now.
  - New pure module `crown_hull` (`HullSpec`, `Hull`, `Facet`, `crown_triangles`, `crown_extent`,
    `percentile`): the cloud is each leaf's position and its tip (`position + up * size`), cut into
    `tiers` bands from its 5th-percentile height (lifted to `lift` of the top) to its top; a band's
    ring is the 90th percentile of its points' distance from the trunk (never wider than the band
    below, at least 0.06 of the crown's height), times 1.08, knocked about by `jitter` from
    `SeededRng::from_parts(seed ^ HULL_SALT, tier * 64 + side)`, so a hull depends on age only
    through the cloud; no hull under 8 points or half a metre. The facets' colours (the leaf
    formula, from draws of their own) and phases (one for a side face and the underside face on the
    same ring points) are functions of the seed, the tier and the side alone.
  - `LodSettings::hull_detail` (default `[1.0, 0.7, 0.5]`): a coarse level's hull has `tiers` and
    `sides` times it (at least 2 and 3), is built from every leaf, not only the ones the level
    keeps (those carry fruit and flowers), and is clamped into level 0's bounds.
  - Shading: `FloraUniform::tree.y` is 1 for a hull; `fragment.wesl` lights its facets matte
    (`hull_surface`, roughness 0.85, no veins, no scan, no light through them) and the impostor
    skips the leaf gloss gate; snow lies on the facets that face up as on leaves. Plain-material
    devices paint a hull without diffuse transmission.
  - `GpuGrowth` is ignored for a tree that is not `gpu_growable`, with a warning logged once: it is
    built on the CPU (`gpu::grows_on_gpu` is the one place that asks), and a change of parameters
    moves a tree between the paths (`sync_growth_mode` reacts to `Changed<ProceduralTree>`).
  - Falling leaves from a hull are chips, not facets: `CHIPS` (96) isosceles triangles 0.12 m long
    in the plane of facets picked by area, in their colours, each its facet's moment in the season.
  - `buck_trees` meshes a prism trunk as the tree has it, and a log's end takes the corners of the
    cut only (a flat-shaded trunk's cut has a point in the middle of each side too).
  - `examples/gallery.rs`: `hull_spruce`, `hull_fir` and `hull_cypress` (mature, ages 2 and 5 and
    under snow), flagging `HULL_TRIS` when a mature crown's triangles are not `tiers * sides * 2`;
    `examples/inspect.rs`: `--hull <tiers>,<sides>[,<lift>[,<jitter>]]` and `--prism <sides>`.
- **`TreePreset::Juniper` and `TreePreset::Fern`** (#57), evergreen undergrowth for a winter
  forest (`TreePreset::ALL` has 20 presets; names `juniper` and `fern`). The juniper is the shrub's
  branching (a 0.4 m stub of a trunk, many leaning, arching stems with side shoots) under spruce-style
  needle clusters in dark blue-green, cedar bark, no autumn colour; the fern is a dozen or so arching stalks (branch density 300)
  from a 6 cm trunk, each with single leaf cards along it, green all year. Neither has an autumn
  colour (`autumn: None`), so a winter `FloraSeason` leaves them green and snows only their
  up-facing parts; they are meant to be pooled with half the default `LodDistances` like `Shrub`,
  take a `FoliagePusher`, and take `CrownStyle::Hull`. The gallery renders both in summer and
  under snow (`undergrowth_juniper_*`, `undergrowth_fern_*`) besides every per-preset group.
- **Looks of a pool** (`PoolColors { wood, leaves, unlit }`, `TreePool::looks`,
  `TreePool::add_look`, `PooledLook`): a pool is drawn in look 0, the colours of its parameters, or
  in any look added to it before it goes into the assets; a `PooledTree` picks one with
  `PooledLook(n)` (absent is 0, taken modulo the looks; changing it re-dresses the tree's children
  and nothing else). Every look shares the pool's meshes: three looks of one forest draw the same
  meshes with three pairs of materials. A look is made as the pool's own is: scans keep their
  pattern with their average colour divided out, autumn and blossom colours stay the same ratio of
  the leaf colour away.
- **`RecolorPool { pool, look, colors }`** (a message): rewrites the pool's materials and the
  look's impostor materials in place, writing only what changed, and never takes the pool asset
  mutably, so no tree is re-indexed or rebuilt (a palette that glides through the day costs a few
  material writes a frame). A pool that was not drawn yet takes the colours when it first is.
- **`RecolorTree { tree, colors }`** (a message): the same for a tree that is not pooled, or no
  longer is: a `ProceduralTree` (every level of a `TreeLod` tree), its cut faces and, when it is
  felled, its stump; or a `TreeStump` or `TreeLog` on its own (a bucked tree's stump and logs are
  separate entities). Materials are swapped for the shared ones of the new colours, no mesh is
  rebuilt, and the tree keeps the colours through later rebuilds. A felled tree lying on the ground
  keeps its materials that do not sway. A `GpuGrowth` tree takes its colours from its parameters.
- **`PoolColors::unlit`**: the surface is drawn as its colour alone (after the rain, the snow and
  the fire have had their say with it): no lighting, no shadows received, no light through the
  leaves, no toon rim. The wood, leaves, impostors and cut faces of the look all do it
  (`StandardMaterial::unlit` on the base, `STANDARD_MATERIAL_FLAGS_UNLIT_BIT` in `fragment.wesl`
  and `impostor.wesl`); an unlit look is a different `SharedFlora` material from a lit one of the
  same colours.
- **`TreePool::tree_params_for(variant, look)`**: the variant's parameters painted in a look's
  colours. A pooled tree that is felled or notched becomes the `ProceduralTree` of its variant *in
  its look* (and in the colours a `RecolorPool` has since given it), unlit included.
- **`ImpostorTint`**, and `forest --looks [--recolor]`: three looks of one forest in three bands,
  and a recolour of two of them mid-run.

### Changed (breaking)

- **`TreeStump::tree` is an `Option<Entity>`** (#56): `None` for the stump of a pooled
  deadfall, which no tree was cut from.
- **`TreeParams` has two new fields, `crown` and `trunk_style`** (#49), and `CrownStyle::Hull` four
  (`tiers`, `sides`, `lift`, `jitter`) and a fifth, `trunk_only`: a struct literal of `TreeParams`
  needs `..` (every preset has them, as `CrownStyle::Leaves` and `TrunkStyle::Round`).
- **`TreePool::materials` is a `Vec<PoolMaterials>`, one pair per look** (was an `Option`: empty
  where it was `None`). `TreePool` has a new public field, `looks`, which `build_pool` fills with
  look 0.
- **`ImpostorAtlas::material` is `ImpostorAtlas::materials`**, one `Handle<ImpostorMaterial>` per
  look, sharing the albedo and surface images. `ImpostorMaterial::new` takes the look's
  `ImpostorTint` and whether it is unlit after `wind`, and `ImpostorMaterial` has a `tint` field at
  binding 4.
- **Impostors are baked neutral grey** (linear 0.5, wood and leaves alike) and the shader
  multiplies in twice the look's linear colour where the picture holds wood or leaves, before the
  season recolours the leaves: a look needs no new bake, and look 0 draws the pool's colours.
  The tint is applied at any `impostor_elevation`, and the impostor shadow prepass takes the same
  material.

### Measured

- **Look.** The 441 gallery scenarios the release has had since 0.32.0 render pixel for pixel as
  0.38.1's (a build of `10fb800` on the same machine; `compare_gallery`: mean difference 0.000 of
  255 for all 441, worst 0.000, no `ERROR`, the mesh columns of `stats.tsv` identical) on the CPU.
  The 50 new scenarios (hull spruce, fir and cypress at ages 2 and 5, mature and under snow; the
  juniper and the fern through every per-preset group) carry no flag. Over all 491 the CPU and
  `--gpu` renders differ by a mean 0.08 of 255 (79 PNGs byte for byte, the worst a young shrub at
  2.11). No gallery scenario shows an impostor, so the impostors are measured by the isometric
  demo: its re-recorded clip differs from 0.38.1's by a mean 0.20 of 255 a frame (0.33 at most),
  the level of the palette. The growth, grove, seasons, fire and push clips are frame for frame
  0.38.1's; shake, fell, chop, harvest and the playground's demo differ in which leaves fall (mean
  0.13 to 1.6 of 255 a frame).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, bmb, interleaved with 0.38.1 in
  ten rounds on a machine other jobs shared, so only the best and the median of the ten are worth
  quoting): 1 000 individual oaks 30.44 ms best and 45.46 median (0.38.1: 30.31 and 44.88), 100 000
  pooled oaks 9.50 and 16.23 ms (10.80 and 17.67): no difference beyond the spread of the runs. A
  first set of five rounds, with the machine quieter and against 0.37.0, had 24.60 and 26.91 ms
  (24.15 and 26.51) and 7.84 and 7.98 ms (7.66 and 8.06).
- **Clips.** All eleven README clips were re-recorded (growth, grove, seasons, shake, fell, fire,
  push, chop, harvest, the playground and the isometric demo) and their first, middle and last
  frames looked at.

## [0.38.1] - 2026-10-13

Pooled forests load three times faster (issue #66). A trace of 100 000 pooled oaks loading put
most of the time in Bevy's `calculate_bounds`: it recomputes the box of every entity whose
`Mesh3d` is new from all of its mesh's vertices, so every pooled tree's seven children walked
meshes of thousands of vertices, and it overwrote the boxes the pool gives them.

### Fixed

- A pooled tree's children keep the pool's boxes (`NoAutoAabb`): its levels of detail their
  level's, its impostor the box that reaches a crown's reach past the quad and five reaches up
  (0.37.0) instead of the quad's own, which a camera looking down or a light could cull too soon.
  A `ProceduralTree`'s own meshes still get Bevy's boxes, which follow their rebuilds.

### Measured

- **Loading** (`forest --trees 100000 --mode pool --offscreen --deterministic`, Apple M4 Pro, three
  pairs with 0.38.0 run in alternating order on a machine loaded by other builds): the forest is
  ready after 2.1 to 2.3 s instead of 6.7 to 6.8 s, its longest frame while loading 158 to 170 ms
  instead of 302 to 321 ms; once in, it draws at 7.66 to 8.24 ms a frame against 7.91 to 8.29 ms.
- **Look.** The 441 gallery scenarios render byte for byte as in 0.38.0 on the CPU and with
  `--gpu` (no `ERROR`), and so do the nine clips and the playground's demo but four push and chop
  frames at a mean of 0.000 of 255. Where impostors and coarse levels are drawn the boxes change
  what is culled: twelve frames of the isometric demo differ at their left and right borders, by a
  few pixels of leaves that swayed out of a tight box and are no longer cut away (a mean of 0.008),
  and a pooled forest seen from above or from the side by isolated single pixels (a mean of 0.035
  and 0.002), where a few more entities drawn reorder overlapping crowns; the trees are the same.

## [0.38.0] - 2026-10-13

Pools built in the background (issue #63): `build_pool` builds a pool on all cores, and the frame
that calls it waits, which a game building its species at load, or a new biome's as the player
walks into it, feels. `BackgroundPools` builds them off the main thread. Nothing changes for code
that does not use it.

### Added

- `BackgroundPools`, a system parameter (in the prelude): `build(params, variants, age, &lod)`
  hands back a `Handle<TreePool>` at once and builds the pool on Bevy's async compute pool; the
  plugin adds its meshes and inserts the pool when the build is done, a few frames later.
  `PooledTree`s placed from the handle meanwhile appear then (and once its impostors are baked), as
  they do for any pool not yet added. The pool is the one `build_pool` makes from the same
  arguments, mesh for mesh; `pending()` counts the pools still building, and dropping every handle
  to a pool still building cancels it. It borrows no `Assets<TreePool>`, so a system can take it
  next to a `ResMut<Assets<TreePool>>`.
- `forest --background-pools` builds the pools with it; the loading report gives the longest frame
  while the forest came in.

### Changed

- The `isometric` example builds its pools with `BackgroundPools` (its clip is the same frame for
  frame).
- `build_pool` is now the background build's two halves run in the frame (the work that needs no
  world, then the meshes added): the same pool as before.

### Measured

- **What a pool costs the frame that builds it** (`build_pool`, best of three, Apple M4 Pro, 14
  cores): an eight-variant pool 0.5 ms (the dead tree) to 14.4 ms (the apple tree), 9.0 ms for the
  oak and 116 ms for all eighteen presets; one variant alone 0.1 to 8.7 ms (38.5 ms for all), so a
  machine with fewer cores waits longer and wasm, which builds the variants one after the other,
  about 0.3 s for every preset's eight variants. With `BackgroundPools` the frame pays for adding
  the meshes only.
- **Look.** The 441 gallery scenarios render as in 0.37.0 on the CPU and with `--gpu`, at a mean
  difference of 0.000 of 255 (byte for byte but the painterly and toon maple and oak; no `ERROR`).
  The growth, grove, seasons and fire clips are 0.37.0's frame for frame and push at a mean of
  0.000; the shake, fell, chop and harvest clips and the playground's demo drop other leaves of the
  same trees (the new system in `Update` reorders the draws that pick them), every tree, stump, log
  and apple where it was, and their GIFs are re-encoded. The isometric demo, its pools built in
  the background, is 0.37.0's frame for frame.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, four pairs with 0.37.0 run in
  alternating order on a machine loaded by other builds): 1 000 individual oaks 32.59 ms in the
  median against 32.59 (the runs from 27 to 47 ms), 100 000 pooled oaks 8.24 ms against 8.29: no
  difference. A pool forest built with `--background-pools` draws at the frame time of one built at
  Startup once it is in (7.88 and 15.56 ms against 7.81 and 15.83 in two pairs, the machine
  switching between two speeds between the pairs).

## [0.37.0] - 2026-10-13

Impostors for cameras that look down (issue #62, D-070 in `qa/visual_findings.md`): under the
`isometric` example's camera the impostor half of a forest read flatter and lighter than the
level 2 half beside it. `LodSettings` has two new fields, so a literal that names all of its
fields needs them: a minor release.

### Added

- **`LodSettings::impostor_elevation`** (degrees, 10 by default, at most 60): a pool's impostor
  pictures are taken from that height above the horizon. An isometric or top-down game sets its
  camera's pitch, and its far trees show their crowns from above as the near ones do.
- **`LodSettings::impostor_shadows`** (off by default): a pool's impostors cast shadows, each its
  tree's silhouette with the quad turned toward the light, where the shadow cascades reach
  (`impostor_prepass.wesl`). `NotShadowCaster` on a pooled tree still keeps its impostor out.
- `forest --impostor-elevation <deg>` and `--impostor-shadows`. The `isometric` example builds its
  pools from its camera's pitch, with impostor shadows (`docs/isometric.gif`).

### Changed

- An impostor is drawn at the front of its crown: each corner of its quad moves toward the camera
  along its own view ray by the crown's reach, so the picture on screen is the same and the
  picture's lowest part, the near edge of a crown seen from above, is no longer hidden under the
  ground. Under an orthographic camera every impostor faces along the camera's view.
- `ImpostorMaterial::params.z` carries the crown's reach (0 draws the quad where it stands).

### Measured

- **Look.** The 441 gallery scenarios render byte for byte as in 0.36.0 on the CPU and with
  `--gpu` but the CPU toon maple and oak (a mean difference of 0.000 of 255; no `ERROR`), and so do
  the nine clips and the playground's demo frame for frame but three push frames by a level: none
  of them draws an impostor. A perspective pooled forest at the default 10° differs from 0.36.0's
  by a mean of 0.000 of 255 (the quad drawn at the front of the crown leaves the picture where it
  was), and with `--impostor-shadows` the far band, with the sun in front or behind, darkens no
  impostor under its own shadow. In the isometric demo the impostor half of the forest shows its
  crowns from above and stands on its shadows (D-070, moved to Fixed).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.36.0 on a
  machine loaded by other builds, about 20% slower than quiet): 1 000 individual oaks, run after
  0.36.0 in each pair, 31.37 ms against 30.26 (medians of five pairs) and, run before it, 25.35,
  25.39, 26.25 and 26.11 ms against 25.46, 28.45, 26.25 and 27.78: the binary run second was the
  slower, so no difference; 100 000 pooled oaks 9.45, 8.46 and 8.32 ms (0.36.0: 10.00, 8.77 and
  8.27). Impostor shadows cost 100 000 pooled oaks 1.0 to 1.6 ms (9.93, 9.22 and 9.31 ms against
  8.33, 8.48 and 8.14 without, the default camera's cascades): they are off by default.

## [0.36.0] - 2026-10-13

Orthographic cameras, for a game with an isometric camera (issues #51 and #52): the levels of
detail and impostors follow an orthographic camera's zoom, and the see-through fade takes the
shape such a game's own fade has. `SeeThroughTrees` has four new fields, so a literal that names
only `target` and `radius` needs `..default()`: a minor release.

### Added

- **Levels of detail by zoom.** While the main camera (the active `Camera3d` with the lowest
  `order`) is orthographic, the plugin scales every level-of-detail band of pooled trees, their
  impostors included, and of `TreeLod` trees by its zoom: a tree where the view meets the ground
  (`FloraGround`, or y = 0) gets the level a 45° perspective camera gives it from where that camera
  sees as tall a view. The bands are rewritten whenever the zoom moves by more than 2%, and trees
  at different distances cross over at slightly different zooms (#51).
- **`LodScale(f32)`** (a resource, in the prelude, 1 by default): multiplies every
  level-of-detail distance, on top of the zoom; a quality setting under any camera.
- **`SeeThroughTrees` under an orthographic camera** cuts a cylinder along the view, as every ray
  is parallel, instead of a cone, and four new fields shape the cut to match a fade the game draws
  on its own meshes: `keep`, the share of a tree kept at the centre (0 by default); `inner`, where
  the edge starts to feather (0.7 of the radius); `in_front`, cutting only what stands more than
  that many metres before the target (`None`: over the last tenth of the way, as before); `floor`,
  a world height below which nothing is cut (`None`) (#52). With the defaults it cuts exactly as
  before.
- **`examples/isometric.rs`**: an orthographic camera at 35° following a walker through about
  1 300 pooled trees, the see-through shaped like an isometric game's fade, zooming with the mouse
  wheel while the levels of detail follow; `--demo` records `docs/isometric.gif`. README:
  "Isometric and top-down games: orthographic cameras".
- `forest --ortho <view height>`.

### Changed

- `SeeThroughTrees` has the four fields above.
- The see-through fade takes four vectors of the shared buffer instead of two, so the pushers
  start at slot 27 and the fire at 60.

### Measured

- **Look.** The 441 gallery scenarios render at a mean difference of 0.000 of 255 from 0.35.0's on
  the CPU and with `--gpu` (byte for byte but the painterly and toon maple and oak; no `ERROR`), the
  growth, grove, seasons and fire clips frame for frame and push but one frame by a level; the
  shake, fell, chop and harvest clips and the playground's demo drop other leaves of the same
  trees (the order the plugin's systems run in moved), every tree, stump, log and apple where it
  was. A perspective `forest --see-through` screenshot is byte for byte 0.35.0's.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.35.0): 1 000
  individual oaks 24.69, 24.62 and 24.67 ms (0.35.0: 24.11, 24.62 and 24.71), 100 000 pooled oaks
  7.72 and 7.62 ms (7.63 and 7.81), 1 000 oaks seen through (`--see-through`) 27.33 and 27.29 ms
  (27.21 and 27.23).
- **The isometric demo** writes the same frames twice. A test holds the bands of a pooled and a
  `TreeLod` tree to `LodScale` (a rebuilt tree's new levels included) and to an orthographic
  camera's zoom at two view heights, back to their own distances under a perspective camera.
- **Found and fixed before release.** The demo's first camera stood 90 m back: at the widest zoom
  the ground at the screen's bottom was behind it, and the trees there, nearest the camera, stayed
  finer than the rest. It now stands 400 m back, and `LodScale`'s docs say to stand an orthographic
  camera well back. A forest scrolling and swaying under the camera made a 16 MB GIF; the demo's
  camera holds still on a still day (7 MB).

## [0.35.0] - 2026-10-13

Three requests from a game moving its forest onto the crate (issues #53, #55 and #58): trees kept
out of the shadow maps, stands the game places, and where the fire burns for its lights and AI.
New API only, hence a minor release.

### Added

- **`NotShadowCaster` on a tree reaches every mesh it is drawn with**: on a `PooledTree` its three
  levels of detail (the impostor never cast), on a `ProceduralTree` its wood, leaves, flowers,
  `TreeLod` levels and cut faces, rebuilt or new. Taking it off the tree gives the shadows back,
  so a game can let only the trees near the player cast. `forest --shadow-radius <m>` does (#53).
- **`Scatter::points_with(density, weight)`**: `weight(at, species)` multiplies a species' share
  where a tree stands, for the stands a game places (snags in a dead patch, saplings along an
  edge). A weight of 0 shuts a species out there; where every weight is 1 the forest is
  `points_where`'s, tree for tree (`points_where` is now `points_with` with weights of 1). With
  groves a tree joins the nearest grove of a weighted species, distances divided by the weights,
  so groves still meet along bent borders inside a stand and a weight fading over a rim bends the
  stand's edge. `forest --stand <x,z,radius>` grows the last `--scatter` species there alone;
  README: `docs/stand.jpg` (#55).
- **`FloraFire::burning()`**: every cell burning now (heat above 0, cooling embers included) with
  its centre in world x, z, for the game's lights and AI, and **`FloraFire::touched_bounds()`**,
  the world rectangle outside which every cell is untouched (no char, no heat). `burning` reads
  only inside that rectangle, and the fire is marked changed only in a frame it stepped or the
  game changed it, so a system run on `resource_changed::<FloraFire>` reads it ten times a second
  while anything burns (#58). The issue asked for both to be empty once the embers cooled:
  `burning` is, and the bounds keep covering the char, as a rectangle outside which every cell is
  untouched must.
- The fire clip (`docs/fire.gif`) lights the ground and the trunks around the fire with six point
  lights over its hottest 4 m squares (`burning`).

### Changed

- A fire step walks only the touched rectangle instead of the whole grid (a small fire on a
  512-cell grid looked at 262 144 cells ten times a second), and the shader header's rectangle
  comes from it instead of a scan of every cell each time the grid is written. The cells are
  visited in the same order, so a fire burns exactly as before.

### Measured

- **Look.** The 441 gallery scenarios render byte for byte as in 0.34.0 on the CPU and with
  `--gpu` (no `ERROR`), and so do the growth, grove, seasons, shake, fell, chop and harvest clips
  frame for frame and the playground's demo; three push frames differ by a level in a few pixels
  (a mean of 0.000 of 255). The fire clip is 0.34.0's until its fire is lit and its lights come
  on (frame 12).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.34.0): 1 000
  individual oaks 24.62, 24.63 and 24.63 ms (0.34.0: 24.68, 24.63 and 24.66), 100 000 pooled oaks
  7.60 and 7.52 ms (7.48 and 7.77).
- **Shadows kept near.** With `--shadow-radius 150` (only the trees within 150 m of the camera
  cast) 1 000 individual oaks take 20.26 and 20.29 ms instead of 24.61 and 24.60 (18% less),
  100 000 pooled oaks 7.21 and 7.30 ms instead of 7.51 and 7.55 (4%: pooled levels are cheap
  casters).
- **The fire's readers.** A test lights a 15 m stand in the corner of a 512-cell grid and checks,
  every few steps from the spark to cold embers, `burning` against a scan of every cell and the
  bounds against the touched cells edge to edge, and the shaders' rectangle with them.

## [0.34.0] - 2026-10-13

The playground: a small first-person game built on the crate, to read as an example of wiring its
gameplay APIs together, and `TreeBurnt`, which it wanted. New API only, hence a minor release.

### Added

- **`TreeBurnt { tree, at }`**, a message the plugin sends once for each tree (`ProceduralTree` or
  `PooledTree`) the fire burns through: the char where its trunk stands passes **`BURNT`** (0.95).
  It marks the tree **`Burnt`** then; a tree that has `Burnt` (restored from a save, with the
  fire's grid) is not reported again. Checked when the fire changed (it stepped, or the game set
  its cells). `TreeBurnt`, `Burnt` and `BURNT` in the crate root, the first two in the prelude.
- **`examples/playground.rs`**: walk among an apple tree, an oak and a ring of mixed trees, strike
  a trunk with an axe (`FloraIndex::cast_ray`, `TreeNotch`; the fourth blow fells it with
  `FellTree`), buck it (`BuckTree`), shake a tree (`TreeShakes`) and gather the apples that fall
  (`FallenFruit`), pick apples off it (`TreeFruit::nearest`, `pick`), light a fire (`FloraFire`,
  counting `TreeBurnt`), call the rain (`FloraWetness`), move the year on (`FloraSeason::of_year`),
  save and load (`FellTree::lying`, `TreeFruit::with_picked`, `Burnt`, `FloraFire::restore`).
  Every action reads a `Controls` resource, so `--demo` plays a script through the same systems
  and records eight seconds of it (`docs/playground.gif`).
- README: "Everything at once: the playground", with `docs/playground.gif`; `TreeBurnt` in Fire.

### Measured

- **Look.** The 441 gallery scenarios render as in 0.33.0 on the CPU and with `--gpu` (byte for
  byte but the CPU toon maple and oak, at a mean difference of 0.000 of 255; no `ERROR`), and so
  do the growth, grove, seasons, fire and push clips frame for frame; the shake, fell, chop and
  harvest clips drop other leaves of the same trees.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.33.0): 1 000
  individual oaks 24.69, 24.62 and 24.65 ms (0.33.0: 24.10, 24.64 and 24.59), 100 000 pooled oaks
  7.61 and 7.61 ms (7.61 and 7.58): nothing runs while no fire changes.
- **The playground.** Its demo run twice writes the same frames; a variant of it that lights a
  fire, saves, moves the year on and loads came back with the felled oak lying bucked into its
  two logs, the fallen apples gone (the game's own) and the rain and the year as saved.
- **Found and fixed before release.** The demo's first take never walked: its controls played on
  the update before the clock's step, when no time had passed; they wait for the step now. Its
  next takes ended on a stump with the oak fallen straight away from the camera, its length
  hidden, and on the apple tree; the player now steps back and aside to see the oak lie and be
  bucked. The GIF at the clips' 20 fps and 96 colours was 9.9 MB (a first-person camera changes
  every pixel of every frame): 16 fps and 64 colours make it 6.9 MB.

## [0.33.0] - 2026-10-13

Saving and loading. What a game did to its trees comes back from a save as it was left: a felled
tree lying where it fell, picked fruit off the tree, a fire's grid still burning. `FellTree`
gains a field, hence a minor release (a `FellTree { .. }` written out in full needs
`lying: false` or `..default()`).

### Added

- **`FellTree::lying`**: lay the tree down at once where it would come to rest, with its stump and
  cut faces, on its still materials, and no landing (no `TreeLanded`, no shake, no leaves shed):
  a tree felled before a save, restored. A fall always ends exactly at the rest angle the cut
  finds, so it lies as the fallen tree did; `BuckTree` cuts it into logs as soon as it lies.
- **`TreeFruit::picked`** (the keys of the picked fruit, to save) and **`TreeFruit::with_picked`**
  (a tree's fruit with those keys picked: insert it with the tree, before or after it is built).
- **`FloraFire::restore`**: set every cell as `FloraFire::cells` gave them: the char as it was,
  and the cells still burning burn on from now. `false`, changing nothing, for a grid of
  another size.
- Examples: `inspect --fell <deg> --lying`.
- README: "Saving and loading the forest": what to save for a tree, a notch, a felled tree,
  picked fruit, a fire, and the weather.

### Measured

- **Look.** The 441 gallery scenarios render byte for byte as in 0.32.0 on the CPU and with
  `--gpu` (no `ERROR`), and so do the nine clips frame for frame, but one of the chop clip's
  (another leaf falling).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.32.0, on a
  machine another job was loading, a quarter slower than usual for both): 1 000 individual oaks
  30.65, 30.54 and 31.32 ms median (0.32.0: 31.39, 30.64 and 29.49), 100 000 pooled oaks 9.09 and
  9.10 ms (8.73 and 9.84): alike; nothing new runs a frame.
- **The restored tree.** An oak felled toward 45° and settled, against the same oak restored
  lying: the same pose (the test asserts a millimetre and a milliradian), stump and cut faces;
  the rendered frames differ only where the felled oak's own landing shake still rustles its
  leaves (PSNR 34 dB).

## [0.32.0] - 2026-10-13

Harvest. A fruiting tree's fruit can be picked, and a shake knocks fruit to the ground for the
game to gather. New API only, hence a minor release.

### Added

- **`TreeFruit`**, a component the plugin keeps on every fruiting `ProceduralTree` drawn on the
  CPU (`FoliageParams::fruit`, the apple): the fruit it grew (`Fruit { key, position, radius,
  moment }`, in the tree's own space; `Fruit::world`, `Fruit::in_season`), `on_tree(&season)` for
  the fruit hanging now, `nearest(&season, tree, at, max)` for the one nearest a hand, and
  `pick(key)`: the shaders stop drawing the fruit on every level of detail (its vertices in the
  tree's leaf meshes are marked), it stays picked when the tree is rebuilt, and the next blossom
  brings it back (`regrow`). A fruit's key is its phase, which holds as the tree grows.
- **Shakes knock fruit loose**: each new `TreeShake` picks a share of the hanging fruit of the
  trees it reaches (`FallingFruit::per_shake`, 0.3 of it at strength 1, the leaves' falloff), each
  by its own draw, and drops it as a **`FallenFruit`** entity (a sphere of the fruit's colour, its
  scale its radius) that falls, bounces and lies on the ground (`FloraGround`, or the height the
  tree stands at), despawned `FallingFruit::rest` (60) seconds after it comes to rest. A
  **`FruitFell`** message tells of each. `FallingFruit` is a resource the plugin adds.
- `harvest` module; `TreeFruit`, `Fruit`, `FallingFruit`, `FallenFruit` and `FruitFell` in the
  crate root, all but `Fruit` in the prelude.
- Examples: `capture_growth --harvest`, an apple tree shaken and another picked into a basket
  (`docs/harvest.gif`).
- README: "Picking fruit: `TreeFruit`", with `docs/harvest.gif`.

### Measured

- **Look.** The 424 gallery scenarios that are not the apple's render as in 0.31.0 on the CPU and
  with `--gpu` (a mean difference of 0.000 of 255, no `ERROR`); the apple's 17 have no
  `stats.tsv` flag and differ between the CPU and `--gpu` by a mean of means of 0.154. The
  growth, grove, seasons, fire and push clips render as in 0.31.0 frame for frame; the shake, fell
  and chop clips drop other leaves of the same trees.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.31.0): 1 000
  individual oaks 24.63, 24.60 and 24.60 ms (0.31.0: 24.03, 24.63 and 24.63), 100 000 pooled oaks
  7.62 and 7.58 ms (7.61 and 7.56): nothing runs for trees without fruit.
- **Found and fixed before release.** The clip's first take framed a row of three trees and a
  picker standing in front of the crown it picked: the apples were a few pixels across and a
  picked one simply went; now two trees, the picker beyond the crown, and each picked apple flies
  into a basket that fills.

## [0.31.0] - 2026-10-13

Fruit. A tree can bear fruit (`FoliageParams::fruit`): round, hanging below some of its leaves and
swaying with them, brought and taken by the season. A new `Apple` preset carries it: white blossom
flushed pink in spring, red apples through summer, yellow-brown leaves in autumn. `FoliageParams`
gains a field and `TreePreset` a variant (`TreePreset::ALL` is 18 long), hence a minor release.

### Added

- **`FoliageParams::fruit: Option<FruitParams>`** and **`FruitParams { color, radius, share }`**
  (default: red apples 4.5 cm in radius on one leaf in eight; in the crate root and the prelude).
  Each leaf whose own fixed draw falls under `share` hangs a fruit below its stem: a slightly squat
  sphere (63 vertices; 35 on the coarser levels of detail) in the leaf mesh, right after its leaf,
  so it sways with the leaf and grows with it (a young tree's fruit is small, `GpuGrowth` grows it),
  and a pool's coarser levels carry fewer, larger fruit. Its normals stay round in every style (the
  crown's bent normals and the painterly clump normals pass it by), its underside is baked darker
  so it reads as round in the crown's shade, its skin is glossy (roughness 0.35) and lets no light
  through. An impostor leaves it out. Vertex alpha 0.25 marks a fruit (a flower's is 0, a leaf's 1).
- **The season's fruit**: none while `FloraSeason::blossom` is above 0, then fruit all summer,
  dropping one by one in the first two thirds of `leaf_fall`, on any tree. A fire burns the fruit
  away with the leaves, rain and snow treat it as a leaf, and the falling leaves never take one.
- **`TreePreset::Apple`**: a round-headed apple tree on a 1.6 m trunk with grey, flaking bark
  (`BarkScan::Zelkova`) and thick scaffold limbs, birch-scan leaves, white blossom flushed pink, red
  apples and yellow-brown autumn leaves: 212 nodes, 10 900 wood vertices and 86 600 foliage
  vertices with its blossom (the flowers are drawn only while it is out).
- `gallery` renders the apple: 441 scenarios.
- README: fruit under Seasons, with `docs/fruit.jpg` (an orchard and an apple tree's crown up
  close); 18 presets.

### Measured

- **Look.** The 424 scenarios of 0.30.0 render as they did on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and so do all eight clips frame for frame. The 17 apple
  scenarios have no `stats.tsv` flag; the CPU against `--gpu` differ by a mean of means of 0.154
  (the worst, `age_apple_6`, 1.36: `AllBranches` leaves the CPU path respaces, as on the cherry).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.30.0): 1 000
  individual oaks 24.60, 24.63 and 24.54 ms (0.30.0: 24.54, 24.45 and 24.46), 100 000 pooled oaks
  7.55 and 7.61 ms (7.57 and 7.52): a tree without fruit costs what it did.
- **Found and fixed before release.** The first apple had 421 nodes and 247 000 foliage vertices,
  five times the cherry: fewer limbs and leaves now (212 nodes, 86 600 with the blossom). Apples in
  the crown's shade, where only the even ambient light reaches them, were flat dark discs: their
  underside is baked darker. A short-trunked, wide apple felled downhill on a 27° slope rested on
  nothing (its thin limbs give way under a falling tree) and stopped at 150°, 15 cm off the ground:
  thicker scaffold limbs (half the trunk's radius, on a 0.25 m trunk) now hold it on the slope.

## [0.30.0] - 2026-10-13

Rain. `FloraWetness` is how wet the trees are, for a game's weather to raise while it rains and
lower as they dry: wet bark darkens and turns glossy and the leaves' green grows richer, on every
way a tree is drawn, for one value in the shared buffer. New API only, hence a minor release.

### Added

- **`FloraWetness(f32)`**, a resource the plugin inserts: 0 dry (the default), 1 soaked
  (`FloraWetness::SOAKED`; `amount()` clamps to 0..1, NaN reads as dry). Wet bark keeps about half
  its albedo, its colour deepens and its roughness falls to at most 0.5; a wet leaf keeps its
  brightness and roughness and its colour moves a quarter further from its own grey. On
  `ProceduralTree`, `GpuGrowth`, pooled trees and their impostors (by the texel: wood or leaf),
  under the season's snow. Like the wind and the season it lives in the shared buffer (lane z of
  slot 2): changing it changes no material and costs nothing per tree; dry, the shaders skip it.
  In the crate root and the prelude.
- Examples: `inspect --wet <0..1>` and `forest --wet <0..1>`.
- README: "Rain: `FloraWetness`" under Seasons, with `docs/wet.jpg` (an oak dry and soaked).

### Measured

- **Look.** Dry, the 424 gallery scenarios render as in 0.29.0 on the CPU and with `--gpu` (a
  mean difference of 0.000 of 255; byte for byte but the painterly maple and oak, PSNR 100 and
  83 dB; no `ERROR`), and so do the growth, grove, seasons and fire clips frame for frame (push
  within PSNR 89 dB); the shake, fell and chop clips drop other leaves of the same trees.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.29.0): 1 000
  individual oaks 24.51, 24.52 and 24.55 ms median (0.29.0: 24.90, 24.45 and 24.65), 100 000
  pooled oaks 7.61 and 7.57 ms (7.74 and 7.59).
- **Found and fixed before release.** Wet leaves made glossy caught the sun and the sky on cards
  facing every way, a crown of silvery cards that read as frost, not rain; darkened but matte they
  still greyed, the same white sheen a bigger share of a darker leaf (the impostors' leaves too).
  A wet leaf now keeps its brightness and roughness and only its colour grows richer; the dark,
  glossy bark says rain.

## [0.29.0] - 2026-10-13

Finding trees. The plugin keeps a `FloraIndex` of every standing tree, pooled ones included, by
where its trunk stands: which tree is nearest the woodcutter, which ones stand in the blast, which
trunk the axe, the arrow or the player's look meets, in microseconds among 100 000 trees. New API
only, hence a minor release.

### Added

- **`FloraIndex`**, a resource the plugin inserts and keeps up to date: every `ProceduralTree` (a
  `ProceduralTreeAsset` tree once its asset has loaded) and every `PooledTree` (once its
  `TreePool` exists) as its trunk (`IndexedTree { entity, base, axis, height, base_radius,
  top_radius }`, a cone in world space), at the age it shows and where it stands after transform
  propagation. A tree moved, rebuilt or grown is updated that frame; a felled or despawned one
  leaves it. Queries: `nearest(at, max)` and `within(at, radius)` by the bark at the trunk's foot
  (world x and z), `cast_ray(ray, max)` for the first trunk a ray meets (`TreeHit { tree, point,
  normal, distance }`; crowns and leaves do not stop it), `get`, `iter`, `len`. A grid of 8 m
  cells (`FloraIndex::CELL`): a query costs the trees near it. `FloraIndex`, `IndexedTree` and
  `TreeHit` in the crate root, `FloraIndex` in the prelude.
- Examples: `capture_growth --chop`, a woodcutter felling a row of trees one after another, each
  the nearest standing tree `FloraIndex` gives it, a ring on the tree it is after
  (`docs/chop.gif`).
- README: "Finding trees: `FloraIndex`", with `docs/chop.gif`.

### Measured

- **Look.** The 424 gallery scenarios render as in 0.28.0 on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and so do the growth, grove, seasons, fire and push
  clips frame for frame; the shake and fell clips drop other leaves of the same trees.
- **Queries** (100 000 pooled oaks on a 2.5 m grid, release, Apple M4 Pro): `nearest` within 5 m
  1.6 µs, `cast_ray` over 50 m 0.55 µs, `within` 20 m (about 210 trees) 8 µs.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.28.0): 1 000
  individual oaks 24.50 and 24.64 ms (0.28.0: 23.97 and 24.54), 100 000 pooled oaks 7.57 and
  7.75 ms (7.66 and 7.65), the CPU time per frame alike: keeping the index costs nothing a frame
  measures once the trees stand still.
- **Found and fixed before release.** The clip's first take framed the whole grove from the
  shake clip's camera: the woodcutter started at the bottom edge and its first tree was the small
  Japanese maple there, half out of the picture; the clip now has a row of four trees across the
  view, each falling to the left.

## [0.28.0] - 2026-10-13

Before the saw, the axe. A `TreeNotch` carves an axe's wedge out of a standing trunk, deeper with
each blow, its floor and face showing the end grain; a notched tree is felled at the notch's floor,
the floor staying on the stump and the face going with the tree. `mesh::cut` now cuts across any
plane. Nothing that existed changes, but the new API makes this a minor release.

### Added

- **`TreeNotch { direction, height, depth }`**, a component (+x, 0.5 m, 0.25): a wedge out of the
  trunk on the side `direction` points to, its floor level `height` above the base (with a
  `FloraGround`, above the uphill side, as `FellTree::cut_height`), its face rising from the floor
  at 45°, reaching `depth` of the way through the trunk (at most 0.9). Each change of `depth`
  carves the trunk again on the CPU; removing the component makes it whole. Only the trunk is
  carved: a limb is its own piece of the wood, its root buried in the trunk, and is left whole
  however near the notch it passes. The end grain's rings centre on the trunk's pith on both
  faces. A pooled tree becomes its variant's `ProceduralTree` and a `GpuGrowth` tree moves to the
  CPU path, as for felling; a notched tree stops growing; with `TreeLod` only its nearest level is
  notched. In the crate root and the prelude.
- `FellTree` on a notched tree cuts at the notch's floor whatever `cut_height` says: the stump's
  face is the whole cut, the falling tree carries the notch's face and the cut behind its back.
- `mesh::cut`: `Plane` and `cut_plane` (a cut across any plane; `cut` is the level one), `Notch`,
  `carve_notch` and `fell_notched` (the carving itself, for a mesh of your own), `clip_outline`
  and `fill` (cut faces of any outline, fanned where a fan holds, else ear-clipped).
- Examples: `inspect --notch <bearing>,<depth>[,<height>]`; `showcase`'s `X` key, an axe's blow
  (the fourth fells the tree toward the notch); the fell clip notches the oak with four blows,
  each shivering its crown, before the saw goes in (`docs/fell.gif`).
- README: "Notching: `TreeNotch`", with `docs/notch.jpg`.

### Measured

- **Look.** The 424 gallery scenarios render as in 0.27.0 on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and so do the growth, grove, seasons, shake, fire and
  push clips frame for frame; level cuts (felling, bucking) come out vertex for vertex as before.
  The notch, photographed and zoomed: an oak from a quarter and head on, pine, birch, baobab,
  spruce, willow, cypress, a Japanese maple, a pooled maple, a deep notch a metre up a spruce,
  an oak and a pine felled at their notches, and the fell clip's blows frame by frame.
- **Robustness.** Carved into every preset at 3 seeds, 3 ages, 2 heights and 3 depths (900
  trees), the trunk stays closed in 879; 12 of the other 21 are the shrub, whose trunk is a
  0.5 m stub its stems leave from, and the rest a willow, young baobabs and dragon trees, a bonsai
  and a redwood, left with 1 to 7 open edges (one baobab also with 4 faces wound inward). A carve
  takes at most 1.7 ms (release, Apple M4 Pro).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.27.0): 1 000
  individual oaks 24.51 and 24.64 ms (0.27.0: 23.95 and 24.47), 100 000 pooled oaks 7.70 and
  7.80 ms (7.69 and 7.73): no notch, no cost.
- **Found and fixed before release.** Each face fanned from its own middle put the end grain's
  pith and checks in the middle of every notch face, spokes on the slanted one: the rings now
  centre on the trunk's pith. Lobed cross-sections (a redwood's buttresses, a baobab's root
  lobes, a deep notch) cross the notch's back more than twice and closed into overlapping faces:
  the cut's runs are now joined along the back in order, as a polygon cut by a line closes, and a
  face whose fan would fold is ear-clipped. The floor's ring spanned the point where the face
  split it (a crack along the back): the wood below is split there too. The wedge cut everything
  in it, so a deep notch a metre up a spruce took slabs out of its lowest limbs: only the trunk is
  carved. The trunk was the cut's longest outline and the piece with the most triangles, either
  of which can be a willow's long hanging twig: it is now the outline round the tree's base and
  the piece whose cut surrounds the axis.

## [0.27.0] - 2026-10-13

Undergrowth gives way. A `FoliagePusher` on the player or an NPC pushes the leaves and thin stems
it walks through out of its way, shadows included, and they close up behind it; trunks stay put.
The flora materials gain a `push` flag, hence the minor bump.

### Added

- **`FoliagePusher { radius, height, strength }`**, a component (0.45 m round, 1.8 m tall centred on
  the entity's translation, strength 1): leaves and wood thinner than 5 cm nearer its axis than
  its radius go out of its way, a little down, as a pushed stem bends, and the plant stays rooted
  at its foot. The plugin writes the `MAX_PUSHERS` (4) bodies nearest the camera into the shared
  buffer when they move, each where it is and where it was 0.15, 0.3 and 0.45 s ago, ever weaker,
  so what it parted closes up behind it. The four mesh vertex stages carry it (main pass and
  shadows, `ProceduralTree`, `TreeLod`, pools, `GpuGrowth`), only while an entity has one
  (`FLORA_PUSH`). `FoliagePusher` and `MAX_PUSHERS` in the crate root, `FoliagePusher` in the
  prelude.
- Examples: `inspect --push <x>,<z>[,<radius>[,<vx>,<vz>]]` (a red body standing in the tree or
  walking through it), `forest --push <x>,<z>`, `capture_growth --push` (a body walking through a
  patch of shrubs; `docs/push.gif`).
- README: "Walking through undergrowth: `FoliagePusher`", with `docs/push.gif`.

### Changed

- `FloraKey`, `FloraExtension` and `GrowthExtension` gain `push: bool` (the plugin sets it while an
  entity has a `FoliagePusher`; a material built by hand starts with `false`).
- The shared buffer (`FloraWindBuffer`) holds 33 more vectors after the see-through fade, the
  pushers'; the fire's header and grid now start at slot 58.

### Measured

- **Look.** With no pusher the 424 gallery scenarios render as in 0.26.0, on the CPU and with
  `--gpu` (a mean difference of 0.000 of 255, no `ERROR`), and so do the growth, grove, seasons
  and fire clips; the shake and fell clips drop other leaves of the same trees. The push,
  photographed: a shrub with the body standing in its middle, the body walking through a shrub
  (16 frames a quarter of a second apart, from the side and along its path, zoomed where it is
  inside) and past an oak's trunk, and the push clip.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.26.0). With no
  pusher: 1 000 individual oaks 24.50 and 24.47 ms (0.26.0: 24.49 and 24.47), 100 000 pooled oaks
  7.54 and 7.64 ms (7.41 and 7.54). With one standing in the forest: 24.94 and 24.95 ms against
  24.53, and 7.56 and 7.58 ms against 7.50 and 7.53.
- **Found and fixed before release.** Pushing each vertex from the body's axis straight to its
  surface, by the strongest sample, tore leaf cards across the body into streaks (a card across
  the axis, or across the line where two samples push alike): the map from the axis is now
  continuous and the samples are blended, the strongest most. Every leaf vertex of every tree
  looped over the samples, 7% on 1 000 oaks with one pusher: the header now carries the circle
  everything the pushers reach lies in, and a vertex outside it stops there (2%).

## [0.26.0] - 2026-10-13

Trees burn. A `FloraFire` over the game's world spreads a fire from where the game lights it
through the trees, faster downwind, as far as there is something to burn; the trees it reaches
scorch, flare and lose their leaves, and their wood chars from the ground up to charcoal with
glowing seams, on every way a tree is drawn. The flora materials gain a `fire` flag, hence the
minor bump.

### Added

- **Fire.** `FloraFire`, a resource: a square grid of `FireCell { char, heat }` over the world
  (`new`: 128 cells a side; `with_cells`, up to 512; `with_spread`). `ignite(at, radius)` lights
  it, `extinguish` puts it out keeping the char, `scorch` lays a burn down with no fire;
  `char_at` and `heat_at` read it back as the shaders do (between the cell centres), `cells` and
  `is_burning` as it is. The plugin steps it ten times a second while anything burns: fuel lies
  within `FireSpread::reach` (3 m) of every `ProceduralTree` and `PooledTree` trunk (and
  `ground` everywhere), the front reaches a neighbour `distance / speed` after a cell was lit
  (`speed` 1 m/s, faster downwind by `wind`, a quarter more or less by cell, from `seed`), a cell
  burns through in `burn_time` (12 s) and its embers cool in `cool_time` (20 s).
- **The look of fire**, compiled into the flora shaders only while a `FloraFire` exists
  (`FLORA_FIRE`): a leaf scorches brown as the burning front nears it, flares orange and is gone
  (in the shadow maps too); wood chars from the ground up (the trunk by char 0.4, the crown's
  wood by 1) to dull charcoal split into blocks whose seams glow while it burns (900 nits times
  the heat squared, flickering) and cool after; impostors burn their leaves away in patches,
  scorch and char. `ProceduralTree`, `TreeLod`, pools and impostors, `GpuGrowth` trees and the
  toon look; a burnt tree drops none of the leaves it lost (shakes, felling, the season).
- `FloraFire`, `FireSpread`, `FireCell` in the crate root; `FloraFire` in the prelude.
- Examples: `inspect --burn <after>[,<every>]` (a fire lit at the tree's foot, photographed as
  with `--drop`), `forest --fire <x>,<z>[,<radius>]` and `--fire armed`, `capture_growth --fire`
  (the grove at dusk burning, with bloom, its ground scorched by `char_at`; `docs/fire.gif`).
- README: "Fire: `FloraFire`", with `docs/fire.gif` and `docs/fire_burn.jpg`.

### Changed

- `FloraKey`, `FloraExtension`, `GrowthExtension`, `ImpostorMaterial` and `ImpostorKey` gain
  `fire: bool` (the plugin sets it while a `FloraFire` exists; a material built by hand starts
  with `false`).
- The shared buffer (`FloraWindBuffer`) holds one more vector after the weather, the fire's
  header, and the fire's grid while one exists. `sync_wind` writes only the weather's slots.

### Measured

- **Look.** With no fire the 424 gallery scenarios render as in 0.25.0, on the CPU and with
  `--gpu` (a mean difference of 0.000 of 255, no `ERROR`), and so do the growth, grove and
  seasons clips; the shake and fell clips drop other leaves of the same trees (deterministically:
  two runs alike). The fire, photographed: `inspect --burn` series of an oak from 1 to 23 s in
  daylight, all 17 presets at 4 and 8 s, a pooled maple's impostor, the oak in toon, a mixed
  forest of 3 000 pooled trees 10, 25 and 45 s after a fire was lit in it, and the fire clip,
  whose `--gpu` run matches the CPU run frame for frame (a mean difference of 0.000).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.25.0). With
  no `FloraFire`: 1 000 individual oaks 24.50 and 24.48 ms (0.25.0: 24.50 and 24.46), 100 000
  pooled oaks 7.51 and 7.60 ms (7.59 and 7.71). With one and nothing burning (`--fire armed`):
  medians 24.67 and 24.63 ms, and 7.52 ms. With every tree within 60 m of the middle alight,
  1 000 oaks 25.02 and 25.09 ms; within 300 m, 100 000 pooled oaks 7.63 and 7.59 ms.
- **Found and fixed before release.** The spread first lit neighbours by chance each step, and
  its front ran at a speed set by the cell size (24 m in 8 s at half-metre cells against a
  nominal 1 m/s: more cells, more paths racing). It is now an arrival time: at 1 m/s a tree 24 m
  away catches in 10 to 36 s with half-metre and two-metre cells alike, within 1.6 times of each
  other (a test). The first embers glowed over the whole char and painted trunks copper in
  daylight; they glow in the seams only.

## [0.25.0] - 2026-10-13

Autumn's leaves fall. As the season's leaf fall rises, the leaves the shaders stop drawing on a
deciduous tree near a camera let go from where they hung and fall, tumble and come to lie on the
ground. `LeafBurst::tree` becomes an `Option` and `FallingLeaves` gains a field, hence the minor
bump.

### Added

- **The season's falling leaves.** While `FloraSeason::leaf_fall` rises, every tenth of a second
  the leaves whose moment it has passed meanwhile (the very leaves it hides) let go on the trees
  within `FallingLeaves::autumn_distance` (50 m) of a `Camera3d`, each from the time the leaf fall
  passed it, and fall like a shake's, in one burst per look of leaves. A rise of more than 0.1 in
  one frame (a season set, such as `FloraSeason::WINTER`) sheds without letting fall, and spring
  drops nothing. CPU and pooled trees; `GpuGrowth` trees and plain-material trees shed where the
  leaves hang, as before.
- `FallingLeaves::autumn_distance` (0 lets none fall).
- `inspect --autumn <seconds>[,<after>[,<every>]]`: the leaf fall rises from the `--season` value
  to 1 over `seconds` once the tree is ready, photographed as with `--drop`.
- `forest --leaf-fall-animate <seconds>`: an autumn every `seconds`, over and over.
- README: the autumn in "Falling leaves", with `docs/autumn_fall.jpg`.

### Changed

- `LeafBurst::tree` is `Option<Entity>`: `None` for the season's leaves, which come from several
  trees with one look.
- The seasons clip (`capture_growth --seasons`, a year in eight seconds) sets
  `autumn_distance` to 0: leaves falling at their own speed would still hang in the air in its
  winter. It stays as it was.

### Measured

- **Look.** The 424 gallery scenarios render as in 0.24.0, on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and so do the growth, grove and seasons clips; the
  shake and fell clips drop other leaves of the same trees (deterministically: two runs of 0.25.0
  are frame for frame alike). `inspect --autumn` series of a maple
  and an oak (the leaf fall 0 to 1 over 6 s, 14 frames from 0.5 to 10.25 s, and one from 7 m up at
  9 s): the crown streams leaves and thins, the branches stand bare while the last leaves fall, and
  the leaves lie in the crown's colours around the trunk.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.24.0). With
  nothing falling: 1 000 individual oaks 24.45 and 24.49 ms (0.24.0: 24.53 and 24.46); 100 000
  pooled oaks 7.65, 7.64 and 7.72 ms (7.46, 7.45 and 7.48), and as much (7.61, 7.58 and 7.68)
  with only 0.24.0's systems registered, so the 2% is not the new systems'. An autumn every six
  seconds (`--leaf-fall-animate 6`) against the same autumn standing still (`--season 1,0.5`):
  1 000 individual oaks 23.16 and 23.14 ms (22.89 and 22.90), 100 000 pooled oaks 7.82 and 8.06 ms
  (7.70 and 7.74).

## [0.24.0] - 2026-10-13

Falling leaves. Hit a tree and leaves come off it; a felled tree sheds them as it lands; a game
drops them from any tree whenever it wants. They tumble down, drift with the wind, lie on the
ground a while and shrink away, all on the GPU, in the colours the tree wears this season.
`FloraExtension` and `FloraKey` gain a field, which breaks struct literals, hence the minor bump.

### Added

- **Falling leaves** (`falling` module). A drop copies some of the tree's own leaf cards into a
  burst (`LeafBurst`: one entity, one small mesh) whose vertex stage moves every card: it hangs
  where it grew until its moment (a drop lets go over half a second), falls from rest to between
  0.9 and 1.5 m/s, tumbles about an axis of its own, swings from side to side, drifts with
  `FloraWind`, turns flat over its last 0.3 s (a leaf of crossed planes or a star folds flat),
  lies 3 cm above the ground (`FloraGround`, or the height the tree stands at) for about
  `FallingLeaves::rest` seconds and shrinks away; its shadow falls with it. The fragment stage is
  the tree's leaves' own: scan, tint, veins, the season's colour, snow, toon, the see-through
  fade. A deciduous tree drops none of the leaves it has shed, and no flowers. The tree keeps its
  own leaves.
- **`FallingLeaves`** (a resource, in the prelude): `per_shake` (12 leaves a tree for a shake of
  strength 1 where it happens, fewer toward its radius), `per_landing` (80 for a felled tree as it
  lands), `rest` (6 s), `max_leaves` (5 000 falling or lying at once).
- **`DropLeaves { tree, count }`** (a message, in the prelude): drop leaves from any
  `ProceduralTree` or `PooledTree` now. The plugin writes it for every new `TreeShake` and every
  `TreeLanded`. `GpuGrowth` trees and trees drawn with plain `StandardMaterial`s drop none.
- `FloraExtension::falling` and `FloraKey::falling`: a burst's material is a copy of its tree's
  leaf material with `falling` set, whose pipelines take `falling.wesl` (and
  `falling_prepass.wesl` for prepasses and shadows) as their vertex stage.
- `inspect --drop <count>[,<after>[,<every>]]`: drop leaves once the tree is ready and photograph
  them `after` seconds later, then every `every` seconds.
- README: "Falling leaves: `DropLeaves`", with `docs/falling_leaves.jpg`; the shake and fell clips
  show leaves falling.

### Measured

- **Look.** The 424 gallery scenarios render as in 0.23.0, on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and so do the growth, grove and seasons clips, frame
  for frame; the shake and fell clips differ by their falling leaves. `inspect --drop` series of an
  oak in summer and a maple in autumn from 0.25 to 11.5 s after the drop, from eye height and from
  above: the leaves leave the crown, tumble, land flat around the trunk with their shadows and
  shrink away, in the crown's colours.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.23.0). With
  nothing falling no system of it runs: 100 000 pooled oaks take 7.52, 7.54 and 7.53 ms (0.23.0:
  7.41, 7.50 and 7.46), 1 000 individual oaks 24.49 and 24.46 ms (24.56 and 24.48). With every
  shake slot busy (`--shake-animate`, thousands of leaves always falling) 100 000 pooled oaks take
  7.59 and 7.66 ms (7.50 and 7.53), 1 000 individual oaks 28.30 and 28.31 ms (27.27 and 27.28).
  A material type of its own for falling leaves cost the pooled forest 0.08 ms a frame with
  nothing falling (Bevy's render systems run per material type over every visible mesh), which is
  why they are drawn with `FloraMaterial`.

## [0.23.0] - 2026-10-13

Levels of detail for individual trees. A forest of `ProceduralTree`s, which grow, change and
fall, draws at a quarter of the cost: 1 000 oaks take 6.5 ms a frame instead of 24. It only adds
to the API.

### Added

- **`TreeLod(LodSettings)`** (a component, in the prelude): a `ProceduralTree` drawn as a pool
  draws its trees, at the same levels (`build_lods`): in full up close, then level 1 from
  `distances.lod1` and level 2 from `distances.lod2` (30 and 80 m by default), cross-fading over
  2 m at each switch. There is no impostor: level 2 is drawn at every distance beyond `lod2`. The
  coarse levels are rebuilt with the tree from the skeleton it keeps (a `Growing` tree's every
  age, a `BuildInBackground` tree's builds included), and the component may come, go or change
  at any time. A `GpuGrowth` tree ignores it, and a felled tree drops its coarse levels at the cut
  and falls in full.
- `plugin::TreeLodMesh(level)` marks the coarse children: a wood and a leaf child per level,
  drawn with the tree's materials; a flowering tree's flowers ride in the leaves, as on a pool's.
- `LodSettings` and `LodDistances` derive `Reflect`, and `TreeLod` is registered for reflection.
- `forest --tree-lod`; `inspect --mode tree` gives the tree a `TreeLod` with `--lod` or `--level`.
- README: "Distant trees for less: `TreeLod`", with `docs/tree_lod.jpg`.

### Measured

- **Look.** A tree's coarse levels are a pool's (a test holds every child's meshes to
  `build_lods` and its band to the distances), and `inspect` dollies from 15 to 150 m show the
  `TreeLod` tree and the pooled tree frame for frame alike through both switches. Without
  `TreeLod` the 424 gallery scenarios render as in 0.22.0, on the CPU and with `--gpu` (a mean
  difference of 0.000 of 255, no `ERROR`), and the five clips frame for frame.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.22.0, two runs
  each): 1 000 individual oaks take 24.35 and 24.46 ms without `TreeLod` (0.22.0: 23.83 and 24.49)
  and 6.49 and 6.50 ms with it. 200 oaks growing on the CPU (`--mode cpu-live`) take 10.08 and
  10.14 ms without and 14.19 and 14.10 ms with it, every new age rebuilding the coarse levels too.
  100 000 pooled oaks take 7.41 and 7.43 ms (0.22.0: 7.39 and 7.43).

## [0.22.0] - 2026-10-13

Seeing the player through the trees. A third-person or top-down camera keeps the player in
view in a forest: the trees between them dither out. Nothing is drawn differently while no
camera does. `FloraExtension`, `GrowthExtension`, `FloraKey` and `ImpostorMaterial` gain a field,
which breaks struct literals, hence the minor bump.

### Added

- **`SeeThroughTrees { target, radius }`** (a component for the camera, in the prelude): the
  parts of trees between the camera and `target` dither out, in a cone around the line of sight
  `radius` wide at the target and a third of that at the camera, which closes over the last tenth
  of the way so nothing behind the target is cut. Only that camera's views cut: shadow maps keep
  the whole trees (the cut tree's shadow still falls on the player), and so do other cameras.
  Every way a tree is drawn takes part (`ProceduralTree`, `GpuGrowth`, pooled levels, impostors),
  and so do the camera's prepasses: with a `DepthPrepass`, or depth, normal and motion-vector
  prepasses, the image is the same as without them, so TAA, SSAO and depth of field leave no
  holes. One camera at a time. With plain `StandardMaterial`s there is no fade.
- `FloraExtension::see_through`, `GrowthExtension::see_through`, `ImpostorMaterial::see_through`
  and `FloraKey::see_through`: set by the plugin on every flora material while a camera has
  `SeeThroughTrees`, so that otherwise no pipeline carries the fade's discard. `ImpostorMaterial`
  has pipeline data of its own now, `ImpostorKey`.
- `forest --see-through <x,y,z>[,<radius>]` (a stand-in for the player and the component on the
  camera) and `forest --prepass <none|depth|all>`.
- README: "Seeing the player: `SeeThroughTrees`", with `docs/see_through.jpg`.

### Changed

- The shared buffer (`FloraWindBuffer`) holds two more vectors, the fade's, after the shakes.

### Measured

- **Look.** While no camera sees through, the 424 gallery scenarios render as in 0.21.0 (a mean
  difference of 0.000 of 255) and the five clips frame for frame. With the fade, a forest seen by
  a third-person camera (`ProceduralTree`, `GpuGrowth` and pooled, each with no prepass, a depth
  prepass, and depth, normal and motion-vector prepasses) shows the same image with and without
  the prepasses, and differs from the image without the fade only inside the cone.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.21.0, two runs
  each): without the fade 100 000 pooled oaks take 7.41 and 7.50 ms (0.21.0: 7.49 and 7.64) and
  1 000 individual oaks 24.51 and 24.53 ms (24.52 and 24.48). With a camera seeing through,
  100 000 pooled oaks 7.43 and 7.52 ms, 1 000 individual oaks 27.06 and 27.03 ms (+10%: a discard
  keeps the GPU from rejecting hidden fragments early), and 33.39 and 33.12 ms with a depth
  prepass (30.53 and 30.55 without the fade).

## [0.21.0] - 2026-10-13

Trees built in the background. A game that plants trees while it runs (a streamed chunk, a
sapling the player plants) no longer stalls the frame they are spawned in. Nothing is drawn
differently. `rebuild_trees` takes a new system parameter, which only matters to code that
names its signature; otherwise this adds to the API.

### Added

- **`BuildInBackground`** (a marker component, in the prelude): the tree is built on Bevy's async
  compute pool instead of in the frame it is spawned or changes, and appears the frame its build
  is done, usually a few later (`TreeGenerated` is sent then; until its first build it has no
  `TreeRender` and draws nothing). The meshes are the ones the frame would have built: a forest
  screenshot is the same image, byte for byte, either way, for `ProceduralTree` and `GpuGrowth`
  trees, leaves and flowers alike. A change of parameters while a build runs starts a new one in
  its place and the tree keeps its old meshes until the new ones are in; a change of age waits
  for the build and is caught up after. A `Growing` tree is rebuilt in the background at every
  age too (each age shows a frame or two late); a `GpuGrowth` tree builds its mature meshes in
  the background and ages in the frame. Despawning a tree drops its build.
- `forest --background` builds its `ProceduralTree`s that way.
- README: "Planting without a hitch: `BuildInBackground`", with the measurements below.

### Changed

- `rebuild_trees` and `rebuild_gpu_trees` test for changed parameters and ages themselves
  instead of filtering their queries by `Changed`, since a background build is applied in a
  frame its tree did not change; trees not built in the background are rebuilt exactly when they
  were.

### Measured

- **The frame trees are spawned in** (a headless `App` doing 8 ms of other work a frame, release,
  Apple M4 Pro): 10, 50 and 200 oaks cost 4.0, 8.2 and 22.8 ms built in the frame and 0.31, 0.42
  and 0.47 ms with `BuildInBackground`, all drawn 2, 2 and 3 frames later; 200 cherries 37.7 ms
  against 0.45 ms, drawn 5 frames later. The GPU uploads of their meshes come on top, in the
  frames their builds land.
- **Look.** The 424 gallery scenarios render as in 0.20.0 (a mean difference of 0.000 of 255).
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.20.0, two runs
  each): 100 000 pooled oaks 7.61 and 7.66 ms (0.20.0: 7.63 and 7.61); 1 000 individual oaks 24.50
  and 24.46 ms (24.49 and 24.48); 200 oaks growing on the CPU 10.36 and 10.41 ms (10.29 and 10.22);
  1 000 growing on the GPU 61.71 and 60.69 ms (60.91 and 60.72). Testing for changes in the
  systems instead of in their queries costs nothing measurable. The five clips are frame for
  frame 0.20.0's.

## [0.20.0] - 2026-10-12

Spring blossom. Flowering trees come into blossom in spring, one flower at a time, on every way a
tree is drawn, and a new `Cherry` preset blossoms pale pink on bare branches before its leaves
come back. Nothing else is drawn differently, and a bare tree's fallen leaves cost less.
`FloraSeason`, `FoliageParams`, `TreeMeshes` and `FloraUniform` gain a field and `TreePreset` a
variant, which breaks struct literals and exhaustive matches, hence the minor bump.

### Added

- **`FoliageParams::blossom: Option<Color>`**: a flowering tree has a five-petalled flower at
  every leaf's stem, 0.8 of the leaf's length across, facing out and up, in this colour (with
  the leaf's own variation and a darker centre). The flowers sway and rustle with the leaves,
  cast shadows, grow on `GpuGrowth` and thin out on a pool's coarser levels, where their petals
  are simpler; impostors paint them over their picture in patches. Petals are not given the leaf
  scan, veins or gloss, and light through them takes their own colour.
- **Flowers out of season cost nothing.** They are a mesh of their own, `TreeMeshes::flowers`,
  drawn on a third child of a `ProceduralTree` or `GpuGrowth` tree (`TreeRender::flower_entity`)
  with the leaf material, and the plugin hides that child while `FloraSeason::blossom` is 0.
  Hidden in the vertex shader instead, as fallen leaves are, they made 1 000 individual cherries
  cost 42 ms a frame all year against 23 ms without flowers. A pool keeps them in its leaf meshes
  (the shaders hide them there): its trees are mostly coarse levels and impostors, and a fourth
  entity per level would cost a big forest more than that. With plain `StandardMaterial`s a
  tree's flowers are all out while the blossom is above 0 (a pool's all year).
- **`FloraSeason::blossom`** (0 to 1): the share of the flowers out; they open one by one and
  fall in the reverse order. `FloraSeason::BLOSSOM` is flowering trees in full blossom on bare
  branches. It lives in the shared buffer's free slot, so it costs nothing per tree.
- **`TreePreset::Cherry`**: a small spreading cherry, about 5 m high and wider than tall, with
  the Sakura bark, birch-like leaves that turn orange-red, and pale pink blossom (17 presets; the
  gallery renders 424 scenarios, the cherry's 17 among them, and its spring: in blossom, flowering
  among half its leaves, the last flowers).
- `inspect` and `forest` take the blossom as a fourth `--season` value.
- README: blossom in "Seasons", with `docs/blossom.jpg`, a grove of cherries in blossom.

### Changed

- **`FloraSeason::of_year`'s spring**: flowering trees open their blossom from the year's turn
  (after the snow has melted) to 0.015 and drop it from 0.03 to 0.07, and the leaves come back
  from 0.02 to 0.07 (they came back from 0 to 0.05), so the blossom shows on bare branches.
- **Fallen leaves cost less.** The vertex stages (main pass, prepass and both `GpuGrowth`
  shaders) test for a fallen leaf or a closed flower first and return before the shake, the
  flutter, the growth and the wind's bend; before, a bare tree did all of it and was clipped
  after. Nothing is drawn differently.
- `foliage_mesh` and `foliage_mesh_styled` give a flowering tree's flowers after its leaves, in
  one mesh; `TreeMeshes::foliage` stays the leaves alone.
- The grove clips (`docs/grove.gif`, `docs/seasons.gif`, `docs/shake.gif`) have a cherry in
  place of the second birch, and the seasons clip spends a fifth of its length on the melt and
  the blossom; its GIF adds 16 of the blossom's pinks to its palette, which had turned the
  flowers white (the commands are in `capture_growth`'s docs and `AGENTS.md`). The fell clip's
  trees stand a few centimetres further apart (the clips space their trees by the grove's mean
  crown width, which the cherry widens).
- `FloraUniform` has a `blossom` field: the blossom colour over the material's base colour, and
  1, or zero for a tree without flowers and for wood.

### Measured

- **Look.** The 399 scenarios of 0.19.0 render as they did (a mean difference of 0.000 of 255),
  the 25 cherry scenarios have no `stats.tsv` flag, and CPU against `--gpu` differs by a mean of
  0.077 over all 424 (worst the young `AllBranches` shrub at 2.1, as before). The growth clip is
  frame for frame 0.19.0's.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, interleaved with 0.19.0, two runs
  each): 100 000 pooled oaks 7.43 and 7.63 ms (0.19.0: 7.52 and 7.60); 1 000 individual oaks 24.52
  and 24.44 ms (24.25 and 24.19, +1%); 1 000 bare oaks in winter (`--season 1,1`) 18.86 and 18.86
  ms (19.92 and 19.77, -5%: the early return). 1 000 individual cherries take 22.2 ms out of
  season and 41.1 ms in full blossom (`blossom: None`: 22.8 ms), and 100 000 pooled cherries 7.56
  and 7.59 ms. The cherry has 18 400 leaf and 28 500 flower vertices at level 0.

## [0.19.0] - 2026-10-12

Bucking. A felled tree's trunk can be cut into logs that lie on the ground, ready to become
pickups or colliders. Nothing is drawn differently until a tree is bucked; every change is an
addition.

### Added

- **`BuckTree { log_length, min_radius }`** (in the prelude): insert it on a felled tree, with
  its `FellTree` or later, and once the tree lies still the plugin cuts its trunk into logs
  `log_length` long (2 m by default), up to where the trunk gets thinner than `min_radius`
  (8 cm). The logs lie on the ground in a row from the stump, 15 cm apart, the way the tree fell,
  each with the trunk's bark and end grain at both ends, on the `FloraGround` if there is one;
  the tree's entity, crown included, is despawned and the stump stays.
- **`TreeLog { stump, index, capsule }`**, a component on each log: `index` counts up the trunk
  from the butt, and `capsule` is a `WoodCapsule` in the log's own space for its collider
  (`WoodCapsule::to_primitive`).
- **`TreeBucked { tree, stump, logs }`**, a message sent the frame a tree is bucked, with its
  logs butt first.
- `inspect --buck <m>` (with `--fell`: the photographs wait for the logs), and `B` in `showcase`
  (buck the felled tree; any key then plants a new one).
- README: bucking in "Felling", with an example that fells a tree away from the player, bucks it,
  and gives each new log a collider; `docs/logs.jpg`.

### Measured

- **Look.** The 399 gallery scenarios and the five README clips render as 0.18.0's do (mean
  difference 0.000 of 255). Bucked trees were looked at: an oak as a `ProceduralTree` and pooled,
  from the side and close up at a log's end, lit and in shade (the sapwood band, heartwood, pith
  and checks on the eight-sided section); the row with 5 cm and 15 cm between the logs (5 cm read
  as a crack in one trunk, so the logs lie 15 cm apart); an oak on a 20° slope (its logs down the
  hillside, their shadows meeting them); a maple, spruce, birch and redwood. The unit tests buck
  an oak on flat ground and on a 17° slope, falling both ways (every log 2 m long, its butt end
  on the ground and its top end above it, in order from the stump with a gap, its wood and its
  end faces), and every preset.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, 1080p, interleaved with the
  0.18.0 binary, two runs each): 100 000 pooled oaks 7.8 and 7.7 ms a frame against 7.5 and
  7.8, 1 000 `ProceduralTree` oaks 24.2 ms on both; with no tree being bucked the new system
  matches nothing.

## [0.18.0] - 2026-10-12

Terrain. Trees stand on hills without their trunks floating over the slope, and felled trees
land on the hills. Nothing is drawn differently on flat ground. Every change is an addition;
the minor bump is for the new API.

### Added

- **`FloraGround`** (in the prelude): the height of the game's ground at every point (x, z),
  from a closure (your terrain's height function or a heightmap lookup). `footing(at, radius)`
  is the lowest ground under a trunk's base, where a tree stands so that its trunk meets a slope
  all round: stood at the height under its centre, the downhill side of a trunk hangs over the
  ground with its flat base showing (`docs/footing.jpg`). `slope(at)` and `normal(at)` are for
  keeping trees off cliffs; `FloraGround::flat(h)` and `Default` are flat ground.
- **`TreeParams::base_radius()`**, the grown trunk's radius at the ground, flare and root ridges
  included: the footprint a footing is taken under.
- **`ScatterPoint::transform_on(&ground, &params)`**: a scattered tree's transform at its
  footing, at its own scale.
- **Felling on terrain**: with a `FloraGround` resource the saw goes in `cut_height` above the
  ground on the trunk's uphill side (so a stump on a slope is taller downhill), and the tree
  comes to rest where its trunk or limbs meet the terrain, further over than on flat ground when
  it falls down the slope, less when it falls against it. Without the resource a tree falls as in
  0.17.0, onto flat ground at its own height.
- `forest --terrain <relief>`: rolling hills about `relief` metres high, drawn as a heightfield
  and used as the `FloraGround`; every tree and shrub stands at its footing and none on ground
  steeper than 30°, and `--eye`/`--at` heights count from the ground (`docs/terrain.jpg`).
  `inspect --slope <deg>` stands the tree on ground falling toward +x (`--on-centre`: where a
  plain placement would put it) and fells it onto the slope with `--fell`.
- README: "On terrain", with an example that plants a pool on hills, keeps it off steep ground
  and lets felled trees land on them.

### Changed

- A felled tree may turn up to 150° from upright (110° before), so one felled down a steep
  slope can lie along it; on flat ground every preset lands before 105°, as before.
- README: the felling clip's oak lands about 2.7 s after the cut (it said a little over two).

### Measured

- **Look.** The 399 gallery scenarios and the five README clips render as 0.17.0's do (mean
  difference 0.000 of 255; the fell clip too: without a `FloraGround` the fall is computed as
  before), and a flat `forest --scatter` screenshot is byte for byte 0.17.0's. On a 25° slope an
  oak's trunk was looked at close up at its footing and at the height under its centre (side by
  side, the same camera), the oak felled down, up and across the slope (the uphill stump half a
  metre tall on its uphill side, the log's butt on the stump's front edge), the spruce, willow,
  shrub, maple, pine and baobab standing on it, and a 3 000-tree forest with undergrowth on hills
  14 m high from a clearing, from above and with trunk bases zoomed in. The unit tests fell every
  preset down, up and across a 27° slope (each lands once, lies still and touches the slope),
  and the oak on a 22° slope against flat ground.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, 1080p, interleaved with the
  0.17.0 binary, two runs each): 100 000 pooled oaks 7.7 ms a frame against 7.6 in the same
  session (0.17.0 measured 7.5 on a quieter machine), 1 000 `ProceduralTree` oaks 24.2 ms on
  both; nothing a frame runs changed. Placing trees costs a footing (17 height samples) each,
  and with `forest --terrain` the slope test 4 more per candidate: 9 418 trees and shrubs on
  hills were placed in 35 ms.

## [0.17.0] - 2026-10-12

Felling. Any tree can be cut down: it leaves a stump, falls about the hinge of the cut, lands on
its limbs and lies there. Nothing is drawn differently until a tree is felled. `TreePool` gains a
public field, which breaks building one with a struct literal, hence the minor bump.

### Added

- **`FellTree { direction, cut_height, speed }`** (in the prelude): insert it on a tree and the
  plugin cuts its wood at `cut_height` (0.5 m by default). The wood below stays as a
  **`TreeStump`** entity; the tree keeps its entity and the wood above, tips over toward
  `direction` about the front edge of the cut (a felled trunk's hinge, so it never cuts into its
  stump), slowly at first and faster as a rod of its height would, and comes to rest where its
  trunk or a big limb meets the ground, after a small bounce. Twigs and leaves on the near side
  sink into the ground, as a crushed crown does; a conifer's whorls do not prop it up.
- **`TreeLanded { tree, stump, at }`**, a message sent the frame a felled tree hits the ground,
  with the point it hit (for sound, dust or a camera shake). The landing shakes the crowns it
  falls through and rustles its own leaves through `TreeShakes`.
- **`Felled { stump, angle, at_rest }`** on a falling or fallen tree. Once it lands it no longer
  sways in the wind (its leaves still flutter); it is not rebuilt when its `ProceduralTree` or
  `TreeAge` changes, and stays until you despawn it.
- Cut faces on both sides: the cut is closed with faces that show end grain (a pale sapwood
  band, rings, the pith and a few radial checks), a procedural image built once and shared.
  Branch stubs inside the trunk add no face.
- Pooled and GPU-grown trees fell too: a `PooledTree` becomes the `ProceduralTree` of its variant
  (the same tree as the pool's finest level) and a `GpuGrowth` or `Growing` tree is moved to the
  CPU path before the cut. `TreePool::tree_params(variant)` gives a variant's parameters.
- `inspect --fell <bearing>` (photographs wait until the tree lies still), `capture_growth --fell`
  (`docs/fell.gif`: an oak felled among four other trees, from a still camera), and `F` in
  `showcase` (fell the tree; the next key brings a new one).
- README: a "Felling" section with the clip and an example of an axe that fells a tree away from
  the player and listens for the landing.

### Changed

- **`TreePool` has a public `age` field**, the age its variants were built at (`build_pool`
  fills it), so a pooled tree can be felled as the tree it shows. Code that builds a `TreePool`
  with a struct literal must set it.

### Measured

- **Look.** The 399 gallery scenarios render as the 0.16.0 binary's do (mean difference 0.000 of
  255), on the CPU and `--gpu` paths; the growth, grove, seasons and shake clips are frame for
  frame those of 0.16.0. Felled trees were looked at lying still (oak, spruce, birch, redwood,
  willow, shrub), close up at the stump and the log's end, as a pooled birch, and frame by frame
  in the clip (the cut, the landing, a per-frame difference series); the unit tests fell every
  preset at three seeds and check that each lands once and lies still, and that seven presets
  of every shape rest on their trunk and limbs, neither sinking into the ground nor floating.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, 1080p, interleaved with the
  0.16.0 binary, three runs each): 100 000 pooled oaks 7.5 ms a frame against 7.4 (+1%), 1 000
  `ProceduralTree` oaks 24.2 ms on both; CPU time per frame unchanged. With nothing being
  felled, the two felling systems match no entity.

## [0.16.0] - 2026-10-12

Undergrowth. A new preset, `TreePreset::Shrub`, and a recipe for planting it under the trees.
Every other preset draws what 0.15.0 drew. `TreePreset` gains a variant, which breaks an
exhaustive `match` on it, hence the minor bump.

### Added

- **`TreePreset::Shrub`**: a low, dense, rounded bush, a dozen stems from a stub of a trunk that
  lean out and arch down, each with a few side shoots, in poplar leaves down to near the ground:
  about 1.4 m high and 1.9 m across. 5 900 wood and 17 600 leaf triangles at level 0, 5 200 at
  level 1 and 2 100 at level 2. It grows, turns orange-red in autumn and sheds its leaves, takes
  snow, and draws in the painterly and toon looks, on `GpuGrowth`, in pools and as an impostor
  like every preset.
- `forest --undergrowth <preset>[:<spacing>]`: a second `Scatter` of that species, 2.5 m apart
  by default and at least 1.5 m from every trunk, thinned in patches about 30 m across, its pool
  switching levels of detail at half the trees' distances.
- README: undergrowth under "Planting a forest", with the recipe (a second scatter that keeps
  clear of the trunks, the shrub pool's nearer levels of detail) and `docs/undergrowth.jpg`.

### Changed

- `TreePreset::ALL` has 16 presets; the gallery renders 399 scenarios (the shrub in all 17 of a
  preset's).
- **Bevy's `file_watcher` is this crate's `file_watcher` feature**, off by default, instead of
  being on for every example through the `bevy` dev-dependency (0.15.0): with it every example
  without an `assets` folder logged two warnings at startup. `tree_file` rebuilds on save with
  `--features ron,file_watcher`, and says so on screen without it.

### Measured

- **Look.** The 382 scenarios of 0.15.0 render as they did (mean difference 0.000 of 255); the
  17 shrub scenarios raise no `stats.tsv` flag; CPU against `--gpu` at a mean of means 0.078,
  the worst the young shrub at 2.1 (young `AllBranches` leaves keep their mature places on the
  GPU, as documented). The shrub was looked at from eye height at 3 and 6 m, through every level
  of detail into the impostor, through the year, in both looks, and in woods from the ground and
  from 14 m up; it was designed by editing `.tree.ron` files with the `tree_file` example.
- **Cost** (`forest --offscreen --deterministic`, Apple M4 Pro, 1080p, the default camera, two
  runs each, within 0.3 ms): 1 372 trees of three species 2.7 ms a frame; with 4 863 shrubs under
  them 7.9 ms when the shrub pool uses the trees' distances, 3.3 ms at half of them. With the
  wall clock instead, the half-distance forest ran at 3.4 ms in some runs and 6.6 ms in others,
  switching part way through a 900-frame run with the same scene and wind or no wind; the
  trees-only and trees'-distance runs did not. The fixed clock gave 3.3 ms in all six runs.

## [0.15.0] - 2026-10-12

Trees from files. `TreeParams` is an asset, a tree can be drawn from a handle to one, and with
the new `ron` feature trees load from `.tree.ron` files that rebuild them when saved. Nothing is
drawn differently. The minor bump is for the feature and for what the `serde` feature now accepts
(see Changed).

### Added

- **`ProceduralTreeAsset(Handle<TreeParams>)`** (in the prelude): the plugin inserts the
  entity's `ProceduralTree` once the asset has loaded and replaces it whenever the asset changes,
  so every tree that shares the handle follows an edit, from code, an inspector or a file. Until
  the asset has loaded the entity draws nothing; a change that leaves the parameters equal
  rebuilds nothing.
- `TreeParams` derives `Asset`; the plugin registers `Assets<TreeParams>` and its reflection
  (`register_asset_reflect`), whichever order the plugins were added in.
- **The `ron` feature**: `TreeParamsLoader` loads `.tree.ron` files, `TreeParams::to_ron` writes
  any tree as one (every field, pretty-printed) and `TreeParams::from_ron` reads one;
  `TreeFileError` says what went wrong, with the line and column and, for a misspelt field, the
  fields that exist. With Bevy's `file_watcher`, saving a file rebuilds its trees while the game
  runs, and a file that fails to load keeps the last good tree.
- Example `tree_file` (`--features ron`): a tree from `examples/assets/trees/maple.tree.ron`
  that rebuilds on save, with the load state or error on screen; `--export <preset>` prints a
  preset as a tree file, `--screenshot` saves the tree. The README shows the maple file before and
  after three edits (`docs/tree_file.jpg`).

### Changed

- With the `serde` feature every parameter struct is `#[serde(default, deny_unknown_fields)]`: a
  field left out takes its default (so files written before a field existed keep loading) where
  it used to be an error, and a field the crate does not know is an error where it used to be
  ignored.
- The `bevy` dev-dependency enables `file_watcher`, for the example.

### Measured

- **Look.** The 382 gallery scenarios render as 0.14.0's (mean difference 0.000 of 255) and the
  four README clips are frame-identical. Every preset survives `to_ron` and `from_ron` unchanged
  (a test), and renders from its exported file as it does from code (oak, maple, spruce and
  willow looked at).
- **Hot reload** (a throwaway probe with the file watcher, Apple M4 Pro): a saved edit reached
  `ProceduralTree` 0.44 s after the save; a save with a misspelt field left the tree as it was,
  with the load state saying why; saving the fix rebuilt it 0.35 s later.

## [0.14.0] - 2026-10-12

Trees that react. A new resource, `TreeShakes`, swings the trees near a hit, a passing character
or a blast. While no shake is under way every tree draws what 0.13.0 drew. The API only grows (two
types, in the prelude); the minor bump is for the feature.

### Added

- **`TreeShake`** and **`TreeShakes`** (in the prelude): `TreeShakes::shake(TreeShake { at,
  radius, strength, direction, duration }, &time)` swings every tree whose trunk stands within
  `radius` metres of `at`, the nearest the most, first away from `at` (or along `direction`), then
  back and forth, dying away over `duration` seconds, while its leaves rustle. Each tree swings at
  its own pace, taller trees slower, and bends as it does in the wind, on every way a tree is
  drawn: `ProceduralTree`, `GpuGrowth`, pooled trees' levels of detail and their impostors, and
  the shadows. Up to eight run at once; another takes the place of the one that ends soonest. A
  shake is drawn, not simulated: `TreeShape` colliders stay where they are.
- Examples: `showcase` hits the tree with `H`; `capture_growth --shake` records the new README
  clip, `docs/shake.gif` (an axe on the oak, one on a birch, a blast); `forest --shake-animate`
  knocks the trees every sixth frame, so every shake slot is busy, for timing.

### Changed

- The shared wind buffer (`FloraWindBuffer`) holds 23 `vec4`, up from 6: the shakes ride in it,
  so a shake writes a few hundred bytes and touches no material, and a header with the time the
  last shake ends lets the shaders skip the shakes while none is under way. The plugin drops a
  shake once it has died away (`TreeShakes::iter` lists the ones under way).

### Measured

- **Look.** With no shake the 382 gallery scenarios render as 0.13.0's (mean difference 0.000 of
  255). The shake clip is frame-identical on the CPU and `--gpu` paths. Its hits and blast were
  looked at in strips of consecutive frames and in motion masks: each hit moves its own tree and
  that tree's shadow, the blast every tree, trunk bases stay put, and a trunk tracked through the
  blast swings smoothly and settles. In a pooled forest the impostor band swings with the meshes.
  The clip's oak hit needed a strength of 3 to read at the clip's scale (the oak's sway is 0.8).
- **Cost** (medians of interleaved runs, Apple M4 Pro, 1080p, offscreen): 100 000 pooled oaks
  7.36 ms in 0.13.0, 7.40 to 7.45 ms now with no shake, 7.48 ms with all eight shakes busy every
  frame; 1 000 individual oaks 23.7 ms in 0.13.0, 24.2 ms now (+2%, the header check in every
  vertex), 26.8 ms with all eight busy over the whole forest; 1 000 spruces 38.4 and 39.3 ms. The
  shaders work out a tree's shakes once per vertex: calling it from both the bend and the leaf
  flutter cost 28.9 ms with all eight busy.

## [0.13.0] - 2026-10-12

Snow. `FloraSeason` gains `snow`, laid on every tree in the fragment stage, and
`FloraSeason::of_year` now snows on the bare trees in winter. Summer draws what 0.12.0 drew. A
struct literal of `FloraSeason` needs the new field (`..FloraSeason::SUMMER` covers it), hence the
minor bump.

### Added

- **`FloraSeason::snow`** (0..1) and **`FloraSeason::SNOW`** (bare deciduous trees, every tree
  under snow). Snow settles first on what faces straight up and, the more of it there is, on
  steeper faces: the tops of branches and limbs, and on leaf and needle cards the side the camera
  sees when that side faces up, so a tier of needles is white from above and green from below,
  scaled by how much of the sky the leaf sees, with a faint dusting on every exposed needle. A
  leaf under snow lets no light through. Pools' levels of detail, `GpuGrowth` and the three looks
  take it alike; impostors take it by their baked normals and sky share.
- `FloraSeason::of_year` snows from 0.88 to 0.92 of the year (the last leaves are falling) and
  melts from 0.98, before the leaves come back; the year test now also holds that snow lies only
  on trees that have shed 90% of their leaves and never jumps.
- Examples: `--season <autumn>,<leaf fall>[,<snow>]` in `inspect` and `forest` (whose ground
  whitens with it), a `snow` stage in the gallery's season scenarios (382 in all), the seasons
  clip spends a quarter of its frames in the snow and whitens its ground, `showcase` shows the
  snow in its HUD, and the README has a winter forest (`docs/snow.jpg`).

### Changed

- API: `FloraSeason` has a third field, `snow`.

### Measured

- **Look.** The 377 scenarios of 0.12.0 render as they did (mean difference 0.000 of 255; a few
  pixels of a few clip frames move by one level, the shader compiler's rounding); CPU against
  `--gpu` as before. Snow was tuned by eye on spruce, pine, redwood, oak and maple at 12 m from
  below and from 9 m up, on pooled and scattered forests from the air and at eye level, toon
  included: the snow first followed the crown-bent leaf normal, which whitened the top of a
  conifer and nothing below; then a dusting of a fifth of the snow on every needle, which turned
  a spruce seen from below grey-olive (mixing in linear light triples a dark needle's albedo;
  0.03 is a hint); and snow on needle cards deep in the crown, lit only by the little light that
  gets in, rendered charcoal, so it now needs at least a third of the sky (all of it from 0.85).
  A scout read the five snow scenarios against their winter ones and two forests.
- **Cost** (medians of three interleaved runs, Apple M4 Pro, 1080p, offscreen): 100 000 pooled
  oaks 7.31 ms in 0.12.0, 7.39 ms now, 7.38 ms under snow; 1 000 individual oaks 23.5 and 23.7
  ms; 1 000 spruces 38.3 ms in summer and under snow.

## [0.12.0] - 2026-10-12

Planting forests. A new `scatter` module says where a forest's trees stand, so a game no longer
places them on a grid. Nothing is drawn differently. The API only grows (a new module and two
types in the prelude); the minor bump is for the feature.

### Added

- **`Scatter`** and **`ScatterPoint`** (`scatter` module, in the prelude): trunks at least
  `spacing` apart and otherwise irregular (Bridson's Poisson-disc sampling; every spot of the
  area within about two spacings of a trunk), species by weight (`species`), in groves about
  `grove_size` wide whose borders bend (the nearest grove centre after a smooth value-noise warp)
  or tree by tree at 0, and every tree its own yaw, uniform scale (`scale`, 0.85 to 1.15 by
  default) and variant draw (taken modulo the pool's variants by `PooledTree`).
  `Scatter::points_where` takes a density per spot (0 for a road, a lake or a clearing, a
  fraction for open woodland) and only ever removes trees: a tree it keeps is the one `points`
  gives. `ScatterPoint::transform(ground)` gives the tree's `Transform`. Pure CPU, deterministic
  per seed, `Reflect`, and serde with the `serde` feature. **`poisson_disc`** gives the bare
  points.
- `forest --scatter <preset[:weight]>,...` plants one pool per species with `Scatter`, about
  `--trees` of them, with `--grove <m>` (default 50; 0 mixes them tree by tree) and
  `--clearing <m>`; it logs how long the placement took. The README shows one (`docs/scatter.jpg`).

### Measured

- **Placement** (Apple M4 Pro, release): 90 733 trees over a 3 km square, four species in 60 m
  groves, take 0.18 to 0.20 s, once; the trunks' disc is most of it (one species without groves,
  0.19 s).
- **Nothing else moves.** All 377 gallery renders are byte-identical to 0.11.0's, the three README
  clips frame-identical, and 100 000 pooled oaks on the benchmark's grid take 7.37 ms against
  0.11.0's 7.35 (interleaved pairs).
- **Looks.** A 3 000-tree forest of oak, birch, maple and pine (3:2:2:1) in 60 m groves around a
  25 m clearing was looked at from the air, from 45 m and at eye level in summer and autumn: the
  stands read as groves with curved borders and no grid rows; mixing tree by tree reads as salt
  and pepper, as expected.

## [0.11.0] - 2026-10-12

Seasons. Deciduous trees turn their autumn colour and shed their leaves, one leaf at a time, from
one global resource that costs nothing per tree to change every frame. Summer, the default, draws
exactly what 0.10.0 drew. The API grows (`FoliageParams` and `FloraUniform` gain a field, the
shared wind buffer a sixth vector), hence the minor bump.

### Added

- **`FloraSeason`** (a resource, in the prelude): `autumn`, how far the leaves have turned (0
  summer green, 1 every leaf in its autumn colour), and `leaf_fall`, the share fallen (1 bare).
  `SUMMER`, `AUTUMN` and `WINTER`, and `FloraSeason::of_year(t)`, which drives a year from one
  number (green until 0.55, turning to 0.75, falling from 0.7 to 0.9, bare through the winter, in
  leaf again and green by 0.05; a test walks the year so no leaf turns green on the tree or comes
  back in its autumn colour).
- **`FoliageParams::autumn`** (`Option<Color>`, default `None`: evergreen). The oak (russet), willow
  and birch (gold), bonsai (orange-red), maple (scarlet-orange), poplar (yellow), baobab (gold) and
  Japanese maple (crimson) presets carry one; the pine, spruce, cypress, palm, redwood, dragon tree
  and dead tree ignore the season.
- Every leaf turns at its own moment and falls in the order the leaves turned. A fallen leaf is
  clipped in the vertex shader, so it leaves the shadows too; this works on growing trees,
  `GpuGrowth`, pools' levels of detail and their shadows. Impostors keep their summer bake and
  turn in patches of a few texels, blending to the patches' average where a pixel spans several
  (so a far forest does not shimmer), and thin their leaves' coverage as they fall, keeping the
  wood.
- The season lives in the shared wind buffer (`FloraWindBuffer`, now six `Vec4`s), so changing it
  writes 96 bytes in place and modifies no material, as the wind does.
- Examples: `inspect --season <autumn>,<leaf fall>`, `forest --season`, `capture_growth --seasons`
  (the grove through a year: `docs/seasons.gif` in the README), `showcase` key `Y` runs the year,
  and 20 gallery scenarios (maple, oak, birch, willow and pine, each turning, turned, half bare and
  bare): 377 in all.

### Changed

- A leaf's flutter phase (`ATTRIBUTE_LEAF_SWAY.w`) is a hash of its colour and hue draws instead of
  its position. The position moves while a tree grows, so a growing tree's leaves changed phase
  every age step and did not match `GpuGrowth`'s; now a leaf keeps its phase at every age, which
  also keeps its autumn moment. Flutter looks the same; each leaf just flutters at another moment.
- API: `FloraUniform` gains `autumn`; `FoliageParams` gains `autumn`.

### Measured

- **Look.** The 357 scenarios of 0.10.0 render with the trees' pixels identical to 0.10.0's (only
  the overlay's "n/377" counter differs); the pine's four season renders equal its summer one. CPU
  against `--gpu` over all 377: mean of means 0.069 of 255, worst `toonyoung_maple_5` at 1.31, as
  in 0.10.0; the 20 season scenarios are 0.000 apart. The impostor's mid-autumn far field changed
  by 13 to 15 levels between two camera positions 0.3 m apart with one patch's moment per pixel,
  7 to 8 with the blend, which is what a summer or fully turned forest gives (6.6 to 8.5).
- **Cost** (medians of three interleaved runs, Apple M4 Pro, 1080p, offscreen): 100 000 pooled
  oaks 7.33 ms in 0.10.0, 7.40 ms now; 1 000 individual `ProceduralTree` oaks 23.4 and 23.6 ms.
  Half bare, the individual oaks take 22.1 ms (fallen leaves draw no pixels and no shadow).

## [0.10.0] - 2026-10-12

Gameplay. A tree can now carry a simple shape for colliders, picking and line of sight that
follows it as it grows, a growing tree announces when it is done, and one `use` brings in what a
game needs. Nothing is drawn differently. The API changes (`PoolVariant` gains fields,
`grow_trees` takes a message writer), hence the minor bump.

### Added

- **`TreeShape`** (`shape` module) and **`WoodCapsule`**: a tree's gameplay shape in its local
  space, made from its aged skeleton. Three capsules up the trunk that taper with it (one under
  3 m; the lowest has at least the trunk's radius at 1 m, so a walking character meets what it
  sees), one capsule along every stretch of wood whose mean radius reaches `limb_radius` (6 cm by
  default; two along a branch gravity bends), a box around the leaves (`crown`) and the bounds of
  it all. `TreeShape::from_skeleton` builds one from any `TreeMeshes::skeleton`;
  `WoodCapsule::to_primitive` gives Bevy's `Capsule3d` and its `Isometry3d`, what physics
  engines' capsule colliders take; `capsules()` iterates the trunk's and the limbs'.
- As a component it is opt-in: insert `TreeShape::default()` (or `with_limb_radius`) next to a
  `ProceduralTree` and the new system `update_tree_shapes` keeps it up to date on every rebuild,
  every age step of a growing tree included, on the CPU and the `GpuGrowth` path alike, writing it
  only when it changed (`Changed<TreeShape>` is the cue to rebuild colliders). On a `PooledTree`
  it is filled once from its variant: `PoolVariant::shape` (default limb radius) and
  `PoolVariant::skeleton` (the variant's aged skeleton, for shapes or gameplay of your own).
- **`TreeMature`**, a message sent once when a `Growing` tree reaches its mature age.
- **`bevy_flora::prelude`**: the plugin, the tree, pool and growth components, the presets and
  parameters, the wind, the shapes and the messages.
- `examples/showcase.rs` draws the shape over the tree (`C`) and the HUD counts its capsules;
  `inspect --colliders` photographs it (`docs/shape.jpg` in the README).

### Changed

- API: `PoolVariant` gains `shape` and `skeleton`; `grow_trees` takes a
  `MessageWriter<TreeMature>` (systems that call it by name are unaffected).

### Measured

- **Shapes.** A test builds every preset at ages 2, 5, 9 and mature and checks the crown box holds
  every leaf vertex (it is within about half a metre of the leaves' own bounds on 14 presets, 1.8
  m on the palm, whose fronds are long cards), the trunk stack rests on the ground, reaches the
  trunk's top, tapers and has no gaps, and limbs respect their radius. Renders of the oak, maple,
  willow, pine and baobab with the shape drawn over them, mature and at age 5, were looked at
  (the trunk capsules sit inside the wood, a little inside the root flare, which they leave out).
- **Cost.** Nothing changes for a tree without a `TreeShape`; the gallery renders as in 0.9.1.
  With one, each rebuild or age step also ages the skeleton and builds the shape: 9.0, 7.7 and 5.8
  microseconds for an oak, a maple and a willow (Apple M4 Pro, release, 400 ages each), of which
  ageing is 3.1, 6.7 and 4.1. A growing `GpuGrowth` tree otherwise costs a few microseconds of CPU
  a frame, so a shape doubles or triples that, which is why it is opt-in.

## [0.9.1] - 2026-10-12

The birch looks like a birch. The scan 0.7.0 gave it (ambientCG `Bark004`) was scaly, pine-like
bark, and the preset's fade radii (0.10 / 0.40 m) kept the slim trunk smooth anyway, so the birch
was a plain pale pole. It now wears TextureCan's `wood_0027` (CC0 1.0): white papery bark with
dark horizontal lenticels and branch scars, on a 0.6 m tile, and the fade radii most presets use
(0.03 / 0.15 m). No API change.

### Changed

- `BarkScan::Birch` is TextureCan's `wood_0027` (a procedural Substance Designer material, colour
  and normal map at 512 px) on a 0.6 x 0.6 m tile; it was ambientCG `Bark004` on 1 x 1 m.
  `tools/prepare_textures.py` gains a TextureCan source (it reads the 1K zip's link, which carries a
  hash, off the asset's page) and `src/textures/CREDITS.md` has the row. The bark set is 3.15 MB
  (3.2) and the bundled textures 3.6 MB in all.
- The birch preset's `bark` radii go from `smooth_below` 0.10 / `fissured_above` 0.40 to 0.03 /
  0.15 m: with the old ones no scan would have shown on the trunk (QA proposal D-058).
- Nothing else changes. Of the 357 gallery scenarios, 12 (the mature and older birches, plain and
  styled) differ from 0.9.0's, by 0.39 to 0.49 of 255 on average; every other one by under 0.001.

### Measured

Apple M4 Pro, release builds, against 0.9.0 built from the same source tree.

- **Colour.** The region mean colour of the birch at 22 m and at 60 m differs from 0.9.0's by 0.01
  and 0.00 levels of 255; the trunk's own pixels are 2.7% darker (bump-mapped lenticels take a
  little light).
- **CPU against GPU growth** (`gallery` and `gallery --gpu`, `compare_gallery`): over all 357
  scenarios the mean difference is 0.073 of 255, the worst a toon maple at age 5 at 1.31, as in
  0.9.0. No `stats.tsv` flag, no warning or error in either log.
- **Growth.** A 192-frame growth clip of the birch on both paths: the largest frame-to-frame step
  is at most 1.10 times its neighbours' (limit 2), so the bark thickens in place.
- **Levels of detail and impostors** (`lod_fidelity.sh`, the birch, two bearings, 12 distances):
  the impostor's error against level 0 is 0.074 (region mean colour, of 255; 0.071 in 0.9.0).
- **Cost.** Only the image bytes changed (same sizes, same decode), so frame times were not
  re-measured.

## [0.9.0] - 2026-10-12

Looks. `TreeParams::style` (`RenderStyle`) draws a tree natural (what 0.8.0 draws, pixel for
pixel), painterly or toon. A style changes the leaf normals and the lighting, not a vertex
position, colour or UV, so pools, levels of detail, impostors and `GpuGrowth` follow it. The API
changes (new fields and a new parameter, below), hence the minor bump.

### Added

- **`RenderStyle`** (`Natural`, the default; `Painterly`; `Toon`) and **`TreeParams::style`**;
  every preset is `Natural`.
  - *Painterly:* every leaf takes the normal that points out of the centre of its own clump,
    fully (`normal_bend` toward the crown does not apply), where a clump is the leaves of one
    primary branch, the leader or one half of a split with everything that grows from it. The
    lighting is the natural one, so a crown shades as a cluster of rounded masses, each with a lit
    and a shaded side, instead of as one soft volume.
  - *Toon:* the clump normals, and the lighting replaced: each directional light's `N.L` is cut
    into three bands (0.15, 0.55 and 1, from 0 and 0.5) times its shadow, its colour and the
    surface colour, plus the ambient light; the light through backlit leaves is kept; and an ink
    rim darkens every surface toward its silhouette (`mix(0.35, 1, smoothstep(0.05, 0.3, |N.V|))`).
    The bands use the geometric normal, not the one the bark and leaf detail maps perturb, so
    they stay clean. Point and spot lights do not light a toon.
- `LeafPoint::clump` (the `Node::id` of the leaf's clump, the nearest ancestor of its node at
  recursion depth 0; stable as the tree ages) and `foliage_mesh_styled(points, foliage, style)`
  (`foliage_mesh` is the natural style).
- `FloraExtension::style`, `GrowthExtension::style` and `FloraKey::style`: toon is the shader def
  `FLORA_TOON`, so a toon and a natural tree never share a pipeline, and `SharedFlora` shares
  materials per style. The impostor shader reads the style from its `params.y`
  (`ImpostorMaterial::new` takes it after `views`) because its pipeline is shared by every pool.
- `--style natural|painterly|toon` for the `inspect`, `forest` and `capture_growth` examples, and
  38 `gallery` scenarios (every preset mature in each look, the oak and the maple at ages 2 and
  5): 357 in all.

### Changed

- API: struct literals of `TreeParams`, `LeafPoint`, `FloraExtension`, `GrowthExtension` and
  `FloraKey` need the new fields; `ImpostorMaterial::new` takes a `RenderStyle`.
- Without the renderer's plugins (the plain-material fallback) a toon tree is lit as a painterly
  one: the plain material has no bands.

### Measured

Apple M4 Pro, release builds, against 0.8.0 built from the same source tree. bmb was loaded by
other builds (load average 15-110 during the last runs), so cost is interleaved runs and the
level-of-detail numbers carry that noise.

- **The natural look is unchanged.** All 319 scenarios of 0.8.0's gallery render with a mean
  difference of 0.000 of 255 and 100% close pixels from 0.9.0's.
- **CPU against GPU growth** (`gallery` and `gallery --gpu`, `compare_gallery`): over all 357
  scenarios the mean difference is 0.073 of 255, the worst a toon maple at age 5 at 1.31 (the
  limit is 2.5). 48 PNGs are byte-identical. No `stats.tsv` flag, no warning or error in either log.
- **Growth.** A 192-frame growth clip of the oak and of the maple, painterly and toon, on both
  paths: the largest frame-to-frame step is at most 1.11 times its neighbours' (limit 2).
- **Levels of detail and impostors** (`lod_fidelity.sh`, 15 presets, two bearings, 12 distances):
  the impostor's error against level 0 (region mean colour, of 255) is 0.212 natural, 0.23-0.27
  painterly and 0.27-0.32 toon (two runs each; the two runs of the same painterly code differ by
  0.04, the second one with the machine at a load average over 80). In the first run the excess is
  mostly the oak's and the baobab's, the other 13 presets within 0.1 of their natural error. Why
  the toon's is the larger is not established: softening its bands in the impostor shader was
  tried and changed nothing.
- **How far a look departs from natural** (15 presets at 22 m, mean absolute luminance
  difference over the tree's pixels, as a share of the natural luminance spread): painterly 0 to
  19% (the oak 19%, willow 12%, palm 10%, maple 7%, needle trees under 5%), toon 2 to 32% (oak
  32%, baobab 30%). Painterly is subtle: it shades by clump, and a crown's clumps are a few leaves
  wide at game zoom (proposal D-065 in `qa/visual_findings.md`).
- **Cost** (`forest --offscreen`, 1080p, sun with shadows, wind on, interleaved runs, medians): 100
  000 pooled oaks 12.1-15.3 ms in 0.8.0, 13.1-14.8 natural (one run 20.7), 13.1-15.7 painterly and
  14.9-15.8 toon; 1 000 individual oaks 32-35 ms in 0.8.0, 32-39 natural, 31-48 painterly and
  32-48 toon, the spread being the load on the machine (the CPU time per frame was 18-21 ms in
  the runs that were not disturbed).

## [0.8.0] - 2026-10-12

Leaves. Seven presets cut their leaf cards to the real outline of a bundled CC0 leaf scan and wear
its colour and normal map, and every other leaf-shaped card has veins grown by space colonisation
instead of drawn by a formula. Crowns keep their colour, impostors and pools their cost, and a
forest of individual oaks stays within 10% of 0.7.0's frame time (the outlines carry 10 points for
that). The API changes (new fields and parameters, and a renamed library, below), hence the minor
bump.

### Added

- **`LeafScan`** (7 scans: `Oak`, `Willow`, `Birch`, `Bonsai`, `Maple`, `Poplar`, `JapaneseMaple`;
  `ALL`) and **`FoliageParams::scan: Option<LeafScan>`**. A scan is an atlas of the four largest
  leaves of an ambientCG leaf set (512 px colour and normal maps, 2x2 cells; sources in
  `src/textures/CREDITS.md`, all CC0 1.0). Each leaf's outline is cut at alpha 0.5, reduced to the
  best 10 vertices of a 24-point outline and triangulated ahead of time (`src/textures/leaves.rs`,
  generated by `tools/prepare_textures.py`, which is not part of the package). A leaf is one of the
  four, picked by a hash of its own random draws, which never change as the tree ages: it keeps its
  shape while the tree grows, `GpuGrowth` included. The card is as wide as the leaf is (its aspect)
  and one `size` long. Oak, willow, birch, bonsai, maple, poplar and Japanese maple set `scan`;
  the other presets and `FoliageParams::default` have `None`, and `NeedleCluster` and
  `ClusterSphere` cards ignore it.
- **`venation`**: Runions et al.'s open space colonisation (SIGGRAPH 2005): a vein network grows
  from the petiole toward auxin sources scattered by dart throwing, widths by Murray's law, drawn
  anti-aliased into a 128 x 256 `R8` image (`grow`, `rasterize` and the constants). Every leaf
  card without a scan draws its veins from one such image, made once from one fixed seed and
  shared (with a mipmap chain), so trees keep sharing their leaf materials whatever their seeds;
  `LeafShading::veins` still sets how strongly.
- `TextureLibrary` (the system parameter that replaces `BarkLibrary`), `LeafTexture` (an atlas's
  images and its mean colour), `FloraTextures::leaves` and `FloraTextures::venation`.
- `FloraExtension` and `GrowthExtension` gain `venation` (texture 106, sampler 107); `FloraKey`
  gains `venation`. `SharedFlora` shares one material per leaf scan and colour.

### Changed

- A scanned card is cut to the leaf's outline in the geometry (no alpha-mask `discard`) and wears
  the atlas's colour and normal map, applied in the fragment shader with the same cotangent frame
  as bark. The base colour is the preset over the atlas's mean over the leaf's own pixels, so a
  crown keeps its preset colour. The atlas has the real leaf's veins, so `LeafShading::veins` has no
  effect on a scanned card.
- **The vein formula is gone from the leaf shader**: a card without a scan samples the grown image
  (`FLORA_VENATION`) with its UVs. The veins are the same strength and branch the way veins do.
- Distant levels of detail cut scanned cards to a coarser outline (6 to 10 points,
  a subset of the full one), so the pool's levels cost what they did.
- A scanned card side has 10 vertices and 8 triangles where the procedural fan has 9 and 8: the
  oak at level 0 has 16 760 foliage vertices and 13 408 triangles against 15 084 and 13 408. The
  count is a measured budget (see below), not the outline's limit: `tools/prepare_textures.py
  --max-outline-points N --out-rs <file>` makes a table with more.
- The `textures` feature carries the 14 leaf images too: about 3.7 MB of JPEG in all, 3.7 MiB for
  the whole package.
- API: struct literals of `FoliageParams`, `FloraExtension`, `GrowthExtension` and `FloraKey` need
  the new fields; `BarkLibrary` is now `TextureLibrary`.

### Measured

Apple M4 Pro, release builds, against 0.7.0 built from the same source tree. bmb was loaded by
other builds most of the time (load average 15-30), so every comparison is interleaved runs of the
two binaries.

- **Colour.** The region mean colour (the padded box of `lod_fidelity.py`) of every preset at
  level 0 and game zoom (22 m), 0.7.0 against 0.8.0: within 0.65 levels of 255 in every channel on
  14 of 15 presets; the willow is 2.8 apart because its leaves are narrower and cover 0.916 of the
  area they did.
- **Levels of detail and impostors** (`lod_fidelity.sh`, 15 presets, two bearings, 12 distances):
  the impostor's error against level 0 goes from 0.251 to 0.212 (region mean colour) and from
  7.55 to 7.39 (mean pixel difference); level 1 from 0.248 to 0.210 and level 2 from 0.378 to
  0.350.
- **CPU against GPU growth** (`gallery` and `gallery --gpu`, `compare_gallery`): over all 319
  scenarios the mean difference is 0.066 of 255 (0.7.0: 0.076), the worst `extreme_lagthick_6` at
  1.25 (a willow at 1.83). 39 PNGs are byte-identical. No `stats.tsv` flag, no warning or error in
  either log.
- **Growth.** A 192-frame growth clip of the oak and of the maple, on both paths: the largest
  frame-to-frame step is at most 1.11 times its neighbours' (limit 2), so leaves neither pop nor
  change shape as the tree grows.
- **Cost** (`forest --offscreen`, 1080p, one shadow-casting sun, wind on, interleaved runs):
  1 000 individual oaks, median frame time. A card's outline is what costs here (the GPU's vertex
  work, in the main pass and every shadow cascade): 0.7.0 took 29-31 ms while 0.8.0 with
  24-point Douglas-Peucker outlines took 54-57 ms (+80%), with 16 points about +60%, with 12
  points 35-37 ms (+13 to +19%) and with the 10 best vertices it ships with 32 ms (+4 to +10%),
  which is the cap the 10% rule asks for (proposal D-063 in `qa/visual_findings.md`). 100 000
  pooled oaks: on a quiet machine the development build with 24-point outlines drew them in
  10.0-10.6 ms against 0.7.0's 11.2-12.5 (five runs each); the shipped build's later runs were
  made at a load average of 40-70 and say nothing about frame time, but its CPU time per frame
  (the `cpu` column, which contention does not move) was 57-65 ms against 0.7.0's 62-65. The
  500 `GpuGrowth` oaks' CPU time was 97-106 ms against 99-105. The README's frame-time table was
  not re-measured: the machine was never quiet long enough to run it.

## [0.7.0] - 2026-10-11

Bark. Trunks and branches wear bundled CC0 bark scans that follow the wood's growth: smooth young
wood that turns fissured as it thickens, with the scan laid out in metres on the mature tree so a
growing tree's bark never slides. A textured surface averages the preset colour, so far trees,
impostors and the README clips keep their colour, and a pooled forest costs what it did. The API
changes (new fields and parameters, below), hence the minor bump.

### Added

- **`BarkScan`** (14 scans: `Oak`, `Pine`, `Willow`, `Birch`, `Palm`, `Cedar`, `Zelkova`, `Maple`,
  `KnottedPine`, `Sakura`, `PalmTree`, `Hackberry`, `Brown`, `Metasequoia`; `tile_size()` is the
  real size of a scan's tile) and **`BarkParams`** (`scan`, `smooth_below`, `fissured_above`, radii
  in metres: wood thinner than `smooth_below` shows the scan's average colour and no relief, wood
  thicker than `fissured_above` the whole scan, a smoothstep in between), as
  **`TreeParams::bark`**. Every preset picks the scan of its species; birch (0.10 / 0.40), palm
  (0 / 0.05) and baobab (0.20 / 0.80) set their own radii, the rest 0.03 / 0.15. `BarkParams`'
  default is no scan, so a `TreeParams` of your own stays plain colour until you set one.
- **The `textures` feature (on by default)** compiles the scans in: 14 colour and normal maps,
  512 px wide, 3.2 MB of JPEG, from Poly Haven and ambientCG, all CC0 1.0 (sources and authors in
  `src/textures/CREDITS.md`, produced by `tools/prepare_textures.py`, which is not part of the
  package). Without it every scan is absent and wood is plain colour as in 0.6. The crate now
  asks `bevy` for its `jpeg` feature.
- `FloraTextures` (a resource that decodes each scan on first use, builds the mipmaps on the CPU
  and keeps the handles), `BarkTexture` (a scan's images and mean colour; `base_color` divides the
  mean out of a preset colour) and `BarkLibrary` (the system parameter `rebuild_trees` and the
  pool systems take). A scan that fails to decode logs one `warn!` naming the file and its trees
  are plain colour.
- `FloraExtension` and `GrowthExtension` gain `detail_color` and `detail_normal` (bindings 103,
  104 and a sampler at 105); `FloraKey` gains `detail`; `FloraUniform` gains `bark` (the two radii)
  and `bark_mean`. `SharedFlora` shares one material per bark scan and colour.

### Changed

- **Wood UVs are metric and anchored to the mature tree.** UV0 is in tiles of the scan: `v` is the
  distance along the fully grown wood over the tile's length, `u` runs a whole number of tiles
  around (at least one, so the seam is invisible). Every vertex keeps its texture coordinate at
  every age, and the aged mesh the CPU path builds agrees with the mature mesh `GpuGrowth` shrinks
  (a new test compares them, vertex by vertex, over every preset, age and piece). Wood meshes also
  carry a second UV set: `uv1.x` is the vertex's own radius in metres (the shaders fade the bark in
  with it; `GpuGrowth` computes it from the growth state in the vertex shader, so the bark fades in
  as the wood thickens), `uv1.y` is 1. Collars carry on the tube's bark across the junction.
- The normal map is applied in the fragment shader with a cotangent frame from screen-space
  derivatives (the meshes have no tangents). The relief fades out toward the silhouette
  (`smoothstep(0.05, 0.4, N.V)`): without that, crevices at grazing angles turned to black
  speckle along trunk edges. Shadow and prepass shaders are unchanged.
- API: `emit_ring` takes a `RingUv`, `emit_cap` the size of its disc in tiles, and `trunk_mesh`,
  `tube_mesh`, `collar_mesh` and `root_segment` the tile size; struct literals of `TreeParams`,
  `FloraExtension`, `GrowthExtension`, `FloraKey` and `FloraUniform` need the new fields.
- Under the pipe model (`MeshParams::pipe_radius_enabled`) an aged node's radii are not its mature
  radii scaled, so the tile count around a branch and the size of its cap follow the aged radii
  over their growth: the bark of such a tree may re-tile by a tile while it grows and differs a
  little from `GpuGrowth`'s. No preset uses the pipe model.

### Measured

Apple M4 Pro, release builds, against 0.6.2 built from the same source tree. bmb was loaded by
other builds the whole time (load average 20+), so every comparison is interleaved runs of the two
binaries, and the frame-time tables of earlier releases are not re-measured.

- **Colour.** The region mean colour (the padded box of `lod_fidelity.py`) of every preset at
  level 0, 60 m and at game zoom (22 m), 0.6.2 against 0.7.0: at most 0.04 levels of 255 apart in
  any channel. On the wood pixels alone (those that changed) the mean is within 3% of 0.6.2's on
  13 of 15 presets, 9% darker on the pine and 2% lighter on the spruce: bump-mapped crevices take
  a little light.
- **Levels of detail and impostors** (`lod_fidelity.sh`, 15 presets, two bearings, 12 distances):
  the impostor's error against level 0 goes from 0.249 to 0.251 (region mean colour) and from
  7.43 to 7.55 (mean pixel difference): the impostor is baked from the untextured preset colour
  and level 0 now has bark. Levels 1 and 2 are unchanged to the third decimal.
- **CPU against GPU growth** (`gallery` and `gallery --gpu`, `compare_gallery`): over all 319
  scenarios the mean difference is 0.076 of 255 (0.6.2: 0.08), the worst a willow at 1.83 (2.2).
  No `stats.tsv` flag, no warning or error in either log.
- **Growth.** A 192-frame growth clip of the oak and of the maple, on both paths: the largest
  frame-to-frame step is at most 1.10 times its neighbours' (limit 2), so bark neither pops nor
  slides as the tree grows.
- **Cost** (`forest --offscreen`, 1080p, one shadow-casting sun, wind on, 0.6.2 and 0.7.0
  interleaved): 100 000 pooled oaks 7.45 ms against 7.41 ms (three runs each, within 1% of each
  other); 500 `GpuGrowth` oaks 35.7 / 116 / 116 ms against 35.6 / 118 / 116 ms (the machine
  changed state between runs, both binaries alike); 1 000 individual oaks 46.3 against 47.2 ms
  (medians of 14 runs each, which ranged from 22 to 69 ms in both).

## [0.6.2] - 2026-10-11

Trees keep their colour against the sun. Nothing changes where the sun is beside or behind the
camera: those views are byte-identical to 0.6.1 (318 of the 319 gallery scenarios, the other
one differs by at most 2 levels of 255 in any pixel; all 80 `gallery --gpu` renders of the
`mature_*` and `age_*` scenarios; every front-lit forest view). No API change.

### Fixed

- **Treetops turned white when the camera faced the sun.** A leaf card is flat, and seen against
  the sun its glossy upper side mirrored the sun straight into the camera; the pine's and the
  spruce's upper whorls, the maple's and oak's crown tops and the birch's apex read as snow,
  frost or blossom, at every level of detail and on the impostors. A card stands for a leaf or a
  cluster of needles whose many small surfaces would scatter that glint, so a leaf's specular
  reflectance now fades as the camera turns toward a directional light (all of it kept until
  the view and the light are 114 degrees apart, none left from 143 degrees), and the crown glows
  green through its leaves instead, as the leaf shader already lets it. The impostor does the
  same for its leaf texels; wood keeps its gloss. Measured on an Apple M4 Pro with the
  `inspect` example, near-white pixels on a tree photographed against the sun (bearing 135):

  | preset | level 0 | level 1 | level 2 | impostor |
  |---|---|---|---|---|
  | pine | 262 -> 0 | 278 -> 0 | 407 -> 0 | grey top -> green |
  | spruce | 369 -> 0 | 406 -> 0 | 592 -> 0 | grey top -> green |
  | maple | 95 -> 0 | 126 -> 0 | 49 -> 0 | grey top -> green |
  | oak | 129 -> 0 | 240 -> 0 | 32 -> 0 | grey top -> green |

  An orbit from 30 to 240 degrees in 3-degree steps changes as smoothly from frame to frame as
  before (the largest step against its neighbours 1.68 in both versions), with no white pixel at
  any bearing (37 on the oak and 164 on the pine at 129 degrees before). The cost is one loop
  over the directional lights per leaf pixel: 100 000 pooled oaks take the same time
  (7.23-7.25 ms against 7.21-7.30 ms), 1 000 individual oaks 0.2-0.4 ms more (22.1-22.3 ms).

## [0.6.1] - 2026-10-11

Far trees read like the trees they stand for. Up close nothing changes: all 319 gallery scenarios
are byte-identical to 0.6.0, and so is every level of detail of a pool; only the impostors look
different, and only with MSAA (without it they are byte-identical to 0.6.0). No API change.

### Fixed

- **Impostors were darker, more saturated and blockier than the trees they replace.** Beyond the
  last level of detail a tree is a few dozen pixels high and its crown is leaves and sky mixed
  inside each pixel; the meshes show that mix (MSAA blends each pixel's samples), and the
  impostor's atlas keeps it too, as fractional alpha averaged down its mipmaps. The impostor cut
  that alpha at one half, so a far crown became solid leaf colour or nothing: the oak's
  impostor at 197 m was 30 levels of 255 darker than the oak and 68 off in colour, and lost 16%
  of its pixels by 320 m. `ImpostorMaterial` now draws with alpha to coverage: with MSAA a pixel
  draws the share of its samples the baked tree covers, unsharpened (Bevy's own sharpening would
  turn a far mip's partial coverage back into all or nothing). Measured on an Apple M4 Pro with
  the new `inspect` example, every level forced at the same distance and compared with level 0
  at game zoom:

  | impostor against level 0, beyond 200 m | 0.6.0 | 0.6.1 |
  |---|---|---|
  | luminance error, mean over 6 presets and 2 bearings (of 255) | 5.44 | 1.50 |
  | colour error, same | 13.21 | 2.84 |
  | oak at 197 m, colour error in a box around the tree | 7.1 | 3.4 |

  A forest of 6 400 pooled oaks against the same forest drawn with level 0 everywhere, in
  16-pixel blocks: from the air 4.16 -> 1.70, from 320 m straight up 3.37 -> 2.83, from the
  example's corner 30 m up 4.35 -> 3.95; at ground level and through a telephoto lens the far
  trees overlap so much that both versions cover the horizon alike. Standing still in a wind,
  an impostor's pixels change frame to frame by 0.030 levels on average (0.024 before), the
  oak's own meshes at that distance by 0.224. The cost is small: 100 000 pooled oaks take 7.30 ms a
  frame against 7.31 ms, 20 000 take 3.41 ms against 3.31 ms (medians of three interleaved runs). Without MSAA (TAA users) the impostor is cut at one half as before.

### Added

- `examples/inspect.rs`: photographs one tree under controlled conditions to look at how it
  reads: a camera that backs off through every level of detail (`--distance a,b --frames n`),
  optionally zooming so the tree keeps its size on screen (`--constant-size`, which shows a
  level switch as a jump), any bearing, height and field of view, one level forced at every
  distance (`--level 0|1|2|3`), the empty scene (`--empty`) and no MSAA (`--no-msaa`), at a fixed
  1/60 s clock so runs repeat.
- `forest --eye <x,y,z> --at <x,y,z> --fov <deg>`: put the forest's camera anywhere.
- `capture_growth --grove` records a second README clip, `docs/grove.gif`: a grove of eight trees
  of six species (oak, two maples, two birches, pine, cypress, willow, Japanese maple), each
  growing from its own start frame, with the camera circling it and a `GustFront` crossing it
  in the last seconds.
- `FloraMaterialSupport` and `SharedFlora` are re-exported at the crate root, like
  `GpuGrowthSupport`.

### Changed

- `capture_growth` steps a fixed clock, one 1/24 s step per captured frame, so the wind sways
  smoothly and two runs write the same frames (with the wind at its default strength, 0.3, the
  clip used to catch whatever the wall clock gave each frame). The clips are lit by a light wind
  (strength 0.5), the single tree is a maple by default (it was an oak) and starts as a visible
  seedling instead of bare ground. `docs/growth.gif` is re-recorded (480x360, 24 fps, 192
  frames, 5.0 MB).
- The README forest table is measured again on one machine, an Apple M4 Pro, for 0.6.0; before,
  its columns mixed the 0.5.0 numbers (Apple M5) with 0.6.0 prose.
- Documentation brought up to 0.6.0: the Leaf look section of `ARCHITECTURE.md` (the mesh feeds
  the leaf shader, it is not a plain `StandardMaterial`), the three `Paint` variants, what
  trees need to share a material ("equal looks"), the WebGL2 fallback in the `PooledTree`,
  `PoolMaterials`, `GpuGrowth` and `Paint::Standard` docs and the no-impostors warning, the gust
  examples (launch a gust once, not every frame), `MeshData`'s fields, the `forest` flags, and
  `CLAUDE.md` and `AGENTS.md` (both clips, CI's commands, committing `Cargo.lock`).
- `docs/growth.gif` and `docs/grove.gif` re-recorded for this release (byte-identical: the clips draw
  no impostors). The README forest table's impostor column is measured again for 0.6.1.

## [0.6.0] - 2026-10-11

Draws faster, looks the same. All 319 gallery scenarios, rendered by the CPU path and with
`--gpu`, are byte-identical to 0.5.0 (the 638 PNGs and both `stats.tsv`). The forest screenshots
(wind on, fixed time step) are identical too, apart from one pixel of a 1920x1080 frame that is one
level of 255 darker in the red channel (the shader compiler's arithmetic: the wind is read from a
buffer now). `FloraUniform` and the three flora materials changed, so a program that builds them by
hand needs a small change (below).

### Changed

- **The wind no longer lives in the materials.** 0.5.0 copied the global wind into the uniform of
  every flora material and rewrote all of them whenever `FloraWind` changed. Bevy re-specializes
  every entity of a material whenever the material asset is modified, so one change cost time
  proportional to the number of trees: a pooled forest of 20 000 trees took 212 ms a frame, and 1.2
  s of CPU, while the wind changed every frame (a storm that builds, a wind that turns) against
  3.3 ms still. The wind is now one small storage buffer, `FloraWindBuffer`, that `FloraExtension`,
  `GrowthExtension` and `ImpostorMaterial` bind at binding 102 and the shaders read; changing
  `FloraWind` or launching a `GustFront` writes it in place and modifies no material.
  - `FloraUniform` loses `wind`, `motion`, `rate`, `front` and `front_shape`; it holds `tree`,
    `response`, `leaf` and `tint`, what belongs to a tree and its species.
  - `FloraExtension`, `GrowthExtension` and `ImpostorMaterial` gain `wind: Handle<ShaderBuffer>`,
    and `ImpostorMaterial::new` takes the `&FloraWindBuffer`.
  - Reading the buffer needs storage buffers in the vertex stage. A device without them (WebGL2)
    now gets plain `StandardMaterial` trees, pools without impostors, and one warning, as when the
    plugin is added before the renderer's plugins; in 0.5.0 it had the flora material there.
- **Trees with equal looks share their materials.** A `ProceduralTree` used to own two flora
  materials, so no two trees shared a bind group and no draw could reuse the last one's state.
  Trees whose colours and species-level shading (`TreeParams::wind`, `LeafShading`, trunk height)
  are equal now resolve the same two materials from `SharedFlora`, which a tree never writes in
  place: a tree whose look changes is given other shared ones. A thousand oaks that differ by seed
  take 21.5 ms a frame instead of 58 ms, at about a third of the CPU. Give every tree its own colour,
  or its own `trunk.height`, and each is its own material again, as in 0.5.0. This replaces the
  "known cost" listed under 0.5.0, whose suspected cause (a non-bindless extension) was only part
  of it.
- **Trees built in the same frame are built on all cores.** `rebuild_trees` and
  `rebuild_gpu_trees` collect the trees whose parameters or age changed, build their meshes on the
  compute task pool in batches, and apply the results in order; the meshes are bit-identical to
  the serial build. Spawning 1 000 oaks takes 0.3 s instead of 0.6 s, 4 000 take 1.0 s instead of
  2.4 s. A frame that rebuilds one or two trees (a hero tree growing) runs inline.

### Added

- `FloraWindBuffer` and `SharedFlora`, public because the plugin's systems take them.
- `forest` example: `--mode gpu`, `--mode gpu-live` and `--mode cpu-live` (`GpuGrowth` trees,
  fully grown or growing for the whole run, and CPU-rebuilt growing trees), `--wind-animate`
  (the wind's strength changes every frame), `--deterministic` (a fixed 1/60 s step and a warmup
  counted in frames, so two builds' `--screenshot`s can be compared byte for byte), and the
  result line gains the median, the minimum and `cpu`, the CPU time of all threads per measured
  frame, which stays comparable when the machine is busy.

### Measured

`forest --offscreen`, 1920x1080, a shadow-casting sun, an oak forest on an 8 m grid seen from 30 m
up, release build, Apple M4 Pro (14 cores) with nothing else running, three runs interleaved
against 0.5.0:

| scene | 0.5.0 | 0.6.0 |
|---|---|---|
| 1 000 `ProceduralTree`s, ms per frame (CPU ms per frame) | 57-59 (74-76) | 21.4-22.0 (17-26) |
| pool of 20 000 trees, wind changed every frame | 211.6 ms (1 158 ms CPU) | 3.3 ms (12.7 ms CPU) |
| pool of 20 000 trees, wind still | 3.3 ms (12.7 ms CPU) | 3.3 ms (12.7 ms CPU) |
| 1 000 trees spawned, until all exist | 0.6 s | 0.3 s |
| 4 000 trees spawned (6 m grid) | 2.4 s | 1.0 s |
| 500 `GpuGrowth` trees | 36 ms | 36 ms |

### Notes

- `GpuGrowth` trees are bound by the GPU, not by their materials: sharing them (one node buffer
  with a slice per tree, the slice's start passed in `MeshTag`, one material per look) was built
  and measured at 36.4 ms against 35.5 ms for 500 oaks, so it is not in this release. Most of the
  cost is the shadow cascades: with the sun's shadows off 500 trees take 15 ms a frame instead of
  36 ms.
- Impostors and pooled trees were already sharing their materials; the pooled forest's cost is
  unchanged.

## [0.5.0] - 2026-10-11

Forest scale, leaf shading and wind. The meshes are unchanged (the `stats.tsv`
geometry columns of all 319 gallery scenarios are identical to 0.4.1), but every tree the plugin
draws now uses a flora material, so leaves and wood look different (259 of the 319 gallery images
differ) and move in the wind. `TreeParams` and `FoliageParams` gain fields (`wind`,
`hue_variation`, `occlusion`, `shading`), so a struct literal without `..Default::default()` stops
compiling.

### Added

- **Leaf shading**: `FloraMaterial` (`ExtendedMaterial<StandardMaterial, FloraExtension>`) is what
  `ProceduralTree`, `GpuGrowth`, `TreePool` and the impostors are drawn with, where the renderer's
  plugins are there. Leaves get an upper side that is darker and glossier than the paler, matte
  lower side (`LeafShading::roughness`, `underside`), a midrib and side veins on leaf-shaped cards
  (`veins`; never on needle or sphere clusters), a hue of their own (`FoliageParams::hue_variation`),
  darker interiors of the crown (`FoliageParams::occlusion`) and light that comes through a leaf lit
  from behind (`translucency`, `transmission_tint`: Unreal's two-sided foliage model), so a crown
  glows yellow-green toward the sun and stays dark where other leaves shade it. The inputs are baked
  into the leaf mesh (`MeshData::uv1`: upper-side flag and crown visibility; `MeshData::sway`:
  `ATTRIBUTE_LEAF_SWAY`, the flutter vector). Trees drawn with a plain `StandardMaterial` ignore
  them.
- **Wind**: the `FloraWind` resource bends every tree as a whole, more toward its top, and flutters
  each leaf, in the vertex stage of the main pass and of the shadow maps, so shadows follow the
  motion and the CPU does nothing per frame. Every knob is a field: `direction`, `strength`, `speed`,
  `flutter`, `bend`, `gust_strength`, `gust_frequency`, `flutter_frequency`, `turbulence` and `gust`.
  `GustFront` (`FloraWind::launch_gust`) sends a band of stronger wind across the world along a
  direction at a speed. Each species answers in its own way, `TreeParams::wind: WindResponse` (`sway`,
  `flutter`, `sway_frequency`, `flutter_frequency`, all 1 by default; the 15 presets set their own, so
  a willow moves more than an oak and a bonsai hardly at all). Changing `FloraWind` rewrites every
  flora material once. Wind writes no motion vectors, and branches do not sway separately.
- **`TreePool` and `PooledTree`**, for forests. `build_pool(params, variants, age, &LodSettings,
  &mut Assets<Mesh>)` builds a few seed variants of a tree once (on all cores), each at three levels
  of detail with its meshes; the plugin makes the two materials the variants share (`PoolMaterials`)
  and `PooledTree { pool, variant }` places one. The plugin gives it a child per level and a
  `VisibilityRange` band (30 m, 80 m and 200 m by default, each switch a 2 m dithered cross-fade),
  and Bevy draws the entities that share a mesh and a material in one instanced call. Pooled trees
  are static; trees that grow stay `ProceduralTree` (with `GpuGrowth`).
- **`build_lods` and `LodSettings`**, levels of detail built from the skeleton, not by simplifying a
  mesh: level 0 is exactly `build_tree`; levels 1 and 2 drop branches thinner than a radius (and what
  grows from them) except the ones that reach furthest in each direction, mesh the rest with fewer
  sides and rings, no collars and a flat disc where a steep branch meets the trunk, and keep every
  3rd / 8th leaf (plus the leaves that reach furthest where the others fall short), scaled up by the
  square root of that as far as the room to level 0's bounding box allows, to cover about as much.
  A coarse level reaches no more than a centimetre beyond level 0's bounding box and is within 10%
  of its size on every axis, for every preset, and level 2 has at most 15% of level 0's triangles (or 500, for the palm, the
  bonsai and the dead tree, which are that small to begin with). `LodDistances` sets where the levels
  switch.
- **Impostors**: beyond the last level a pooled tree is one quad that turns about the trunk and shows
  the nearest of eight baked views (`ImpostorMaterial`, `ImpostorAtlas`). Two pictures are baked in
  the running app, once per variant, from the level 1 meshes into a 2048x512 image (eight 256x256
  tiles for the colour, eight for the surface: the normal in the tree's own frame, how much of the
  sky each leaf sees, how much light it lets through; 10° above the horizon), read back, given
  mipmaps (averaged in linear light, weighted by coverage) and stored with the pool. They are not lit
  when baked: the scene's own lights, ambient light, shadow maps (where the cascades reach) and fog
  shade them every frame, so impostors follow the time of day, glow with the sun behind them, and
  sway with the wind. They cast no shadows. Without a renderer, or with the plugin added before the
  renderer's, a pool has none and level 2 is drawn at every distance, with one warning. The bake
  runs four variants at a time: eight at once is 128 cameras in a frame, which loses the Metal device.
- `examples/forest.rs`, the forest benchmark (`--mode naive|pool`, `--offscreen`, `--no-impostors`,
  `--lod`, `--shadows`, `--sun`, `--wind`, `--wind-dir`, `--gust`, `--screenshot`).

### Measured

`forest --offscreen`, 1920x1080, a shadow-casting sun, an oak forest on an 8 m grid seen across it
from 30 m up, Apple M5, release build:

| trees | one `ProceduralTree` each | pool, levels of detail only | pool, with impostors |
|---|---|---|---|
| 1 000 | 114 ms | 7.5 ms | 6.2 ms |
| 5 000 | 1 280 ms | 19.0 ms | 7.0 ms |
| 20 000 | not run | 40.7 ms | 9.0 ms |
| 100 000 | not run | 118 ms | 15.3 ms (p95 16.2) |

The columns are the median of three runs, except the first column's 5 000 trees (one run) and
20 000 and 100 000 separate meshes, which would take an estimated 15 and 75 GB of GPU memory and
were not run. 100 000 trees run at 65 fps with the impostors, 8.5 fps without. Timings on this
machine drift by 40% and more between runs of the same binary (100 000 trees with impostors read
11.5 to 17 ms in one session), so read them as that wide; the budget was 16.6 ms, and the same
forest took 9.9 ms before the leaf shader and the wind. The shadow-map pass is about a third of the
pooled forest's time (5 000 trees: 6.3 ms, 4.1 ms with the sun's shadow maps off).

### Notes

- The `forest` timings need `--offscreen`: a windowed app is paced by the compositor, and Bevy's
  default `WinitSettings` update an unfocused window only 60 times a second, which made every
  windowed run read "60 fps".
- Known cost: a forest of one `ProceduralTree` each is about four times slower with the flora
  material than with a plain `StandardMaterial` (the plugin added before the renderer's, so no
  flora material: 1 000 trees took 45-56 ms against 172-181 ms with it, interleaved in one session).
  A pool shares two materials among all its trees and does not pay that. The suspected cause, not
  verified: `ExtendedMaterial` is bindless only when both halves are, and `FloraExtension` is not,
  so every tree's materials need bind groups of their own. Not fixed.
- Internal: `mesh::branch::skeleton_mesh_coarse` (the coarse wood mesher) and an optional ring cap
  in `append_tube`; the level 0 meshers are unchanged.
- The impostor bake's cameras write with `BlendState::REPLACE`: Bevy's default for a `None` blend
  state alpha-blends every camera after the first of a target, which dropped the wood (alpha 0)
  from the surface picture and left every impostor trunk black.

## [0.4.1] - 2026-10-10

Bug fix for 0.4.0; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.4.0).

### Fixed

- `FloraPlugin` panicked at startup (`Requested resource ... does not exist`) when it was added
  before `AssetPlugin` / `DefaultPlugins`, as in `App::new().add_plugins(FloraPlugin)
  .add_plugins(DefaultPlugins)`. 0.3.5 accepted any order and so does 0.4.1. The plugin now
  registers `GrowthMaterial` with the renderer only when the renderer's plugins (and `PbrPlugin`)
  were added first, and otherwise registers the asset types in `Plugin::finish`; trees with
  `GpuGrowth` then grow on the CPU, and one warning says to add the plugin after `DefaultPlugins`
  to use GPU growth. Checked in a fresh project against both orders (CPU fallback and GPU growth
  both render) and by a regression test that fails on 0.4.0.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.4.0] - 2026-10-10

A new opt-in growth path. Without the new component a tree is exactly as in 0.3.5 (all 319
gallery images byte-identical to 0.3.5, `growth.gif` unchanged).

### Added

- **`GpuGrowth`**, a component that grows a tree in the vertex shader. The mature tree is built
  and uploaded once; each frame the plugin ages the skeleton and writes a few kilobytes of
  per-node state into a storage buffer, and nothing else is uploaded. Main-world cost per frame
  with every tree growing (headless `App`, release build, Apple M-series, median of three runs):

  | 200 growing trees | CPU rebuild | `GpuGrowth` |
  |---|---|---|
  | oak | 17.6 ms | 0.81 ms |
  | willow | 25.4 ms | 0.90 ms |
  | oak, 2 smoothing passes | 69.8 ms | 0.70 ms |

  That is 0.004 ms of CPU per tree per frame against 0.09-0.35 ms. The wood and the leaves use
  `GrowthMaterial` (`StandardMaterial` extended by `GrowthExtension`), the prepass shares the
  vertex shader so shadows follow the growth, and trees can gain or lose `GpuGrowth` at any time.
- `GpuGrowthSupport`, a resource the plugin sets from the device once the renderer is ready:
  `true` where the vertex stage can read a storage buffer, `false` on WebGL2. While it is `false`
  (and before the renderer exists) trees with `GpuGrowth` grow on the CPU. Insert it yourself only
  in a headless test.
- `gallery --gpu`, `capture_growth --gpu` and the `showcase` key `U` render the GPU path; the new
  `compare_gallery` example diffs two render directories (mean difference and share of close pixels
  per PNG, non-zero exit past the limits).

### What differs from the CPU path

Documented on `GpuGrowth`. Over all 319 gallery scenarios the two paths' renders differ by 0.08 of
255 on average (105 PNGs byte-identical; the worst is a willow at 2.2, the worst share of close
pixels 93.0%); the 192 frames of the README clip, grown by age-only updates, differ by 0.07 on
average (worst frame 0.75). The wood (trunk, collars, tubes, bases) matches the CPU mesh to within
0.2 mm at every age when smoothing and adaptive resolution are off (checked vertex by vertex
against the CPU meshers at hundreds of ages on every preset). Smoothing is computed once on the mature mesh and
deformed; side counts stay at the mature count; `AllBranches` leaves keep their mature positions
along a branch and appear in order; tip-cluster leaves keep the mature mirroring; a growing pointed
trunk ends in a flat cap; the `Mesh` assets (picking, physics, culling) are the mature tree;
growth does not feed motion vectors.

### Changed

- `rebuild_trees` skips trees that grow on the GPU and takes the `GpuGrowthSupport` resource;
  `TreeRender` also exists for GPU trees (its mesh handles are the mature meshes). `FloraPlugin`
  adds a `MaterialPlugin` for `GrowthMaterial` when a renderer exists, so add it after
  `DefaultPlugins`.
- The `bevy` dependency enables `bevy_log` (for one warning when a device lacks storage buffers).
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.5] - 2026-10-10

Performance only; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.3.4). The target is a growing tree, which rebuilds both meshes every frame.

### Changed

- A growing tree no longer regenerates its full-grown skeleton every frame. The skeleton is kept
  and regenerated only when `ProceduralTree` changes; a frame that only advances `TreeAge` ages
  the kept one.
- A growing tree leaves its two materials alone unless a colour changed. Writing them every frame
  made Bevy re-prepare both on the render thread each frame.
- Wood pieces (tubes, collars, primary bases, root domes) are emitted straight into the tree's
  mesh instead of into a temporary mesh that was then copied in.
- Measured on a headless `App` with 20 trees growing at once (release build, Apple M-series,
  median of three runs, milliseconds per frame): oak 5.4 to 2.0, willow 8.8 to 3.4, oak with 2
  smoothing passes 14.5 to 10.9. Building the unsmoothed wood of one growing oak went from
  0.135 ms to 0.044 ms, the willow's from 0.228 ms to 0.078 ms.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.4] - 2026-10-10

Performance only; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.3.3).

### Changed

- Smoothing (`MeshParams::smooth_iterations`) is 3-5 times faster: the willow with 2 passes builds
  in 3.0 ms instead of 14.7 ms, the oak in 1.6 ms instead of 4.7 ms (release build, Apple
  M-series). Vertex welding probes one grid cell instead of 27 with a cheap hasher, adjacency is a
  flat array built from one integer sort, smoothing passes reuse one buffer, and the "keep this
  vertex put" spheres are bucketed on a grid instead of tested one by one for every vertex. The
  15 presets with smoothing on build in 16 ms together instead of 55 ms.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.3] - 2026-10-10

Bug fixes found by a parameter sweep; no API changes.

### Fixed

- With `MeshParams::pipe_radius_enabled`, a growing tree's just-born branches (1 mm long) were
  floored to the full `pipe_radius_min` radius: wide, paper-thin discs whose faces folded inside
  out (oak and willow at intermediate ages). The floor now grows with the branch's radius.
  Mature trees are unchanged.
- A sub-branch that `keep_out_of_trunk` shortened to nothing (its parent runs inside a wobbly or
  steep trunk) still grew leaves, which sat inside the trunk. Branches without wood carry no
  leaves now; the default presets are unchanged.

### Changed

- The `gallery` example renders 319 scenarios: pipe radii at age 5 (`pipeyoung_<preset>_5`) and a
  wobbly, twisted trunk (`wobble_<preset>`) on every preset.
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

## [0.3.2] - 2026-10-10

Appearance only; no API changes.

### Changed

- The willow weeps in strands instead of sitting as a leafy mound: its twigs are fewer, longer and
  thinner (`branches.sub_branch_count` 3 -> 2, `sub_branch_scale` 0.7 -> 0.9, `length` 0.8 -> 0.7),
  and its leaves follow them (`foliage.orientation` `RandomUpward` -> `FollowBranch`, `density`
  6 -> 10), so they hang in rows from the arching limbs. 151 nodes instead of 401, and 18,500
  wood vertices instead of 48,000 for the mature tree.
- The closed, rounded base of a branch that hugs the trunk is rounder: four rings and a cap with
  twice the branch's side count, joined to the branch's own base ring, instead of three rings with
  the branch's side count. On a hexagonal cypress or poplar branch the knot is no longer a
  six-sided pebble. The wood has more vertices as a result (the mature cypress about 8,000
  instead of 5,800, the poplar 10,300 instead of 8,200, the birch 2,900 instead of 2,500).
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

### Fixed

- A sub-branch's wood could end up inside the trunk after it was seated on a bent parent and
  bent by gravity (found by the longer willow twigs). Its bend is now relaxed, or the branch is
  shortened, until it clears the trunk.

## [0.3.1] - 2026-10-10

Appearance only; no API changes.

### Changed

- A primary branch that hugs the trunk (less than half of its direction pointing away from it:
  steep upward or downward branches on the birch, cypress, poplar and some extremes) now ends in
  a closed, rounded base that sits half in the trunk, instead of a flat, sheared cut toward the
  trunk axis. Branches that leave the trunk at a wider angle are unchanged.
- The willow's limbs now start all the way up to the top of the trunk (`branches.end` 0.9 -> 1.0),
  so the crown drapes from the trunk top instead of sitting low under a bare end (401 nodes instead
  of 321).
- The spruce grows slower, so a young spruce is a sapling instead of a 5 m pole of hair-thin
  2 m branches: `growth.trunk_fraction` 0.4 -> 0.6, `branch_fraction` 0.35 -> 0.6,
  `thickening_lag` 2 -> 1.2 and `leaf_threshold` 0.5 -> 0.15. The mature spruce is unchanged; it is
  fully grown at about age 16 instead of 12.
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

## [0.3.0] - 2026-10-10

Thicker twigs, smoother drooping branches and a weeping willow. `BranchParams` gains a field, hence
the minor bump.

### Added

- `BranchParams::min_twig_radius` (metres, default 0.005; `0` turns it off). No branch is thinner
  than this at its base, or thinner than half of it at its tip, so twigs taper but no longer shrink
  to the 0.4 mm hairlines (0.0 mm on the willow) the radius ratios used to give them, and a twig is
  never thicker than the wood it grows from. The willow uses 3 mm and the bonsai and Japanese maple
  2.5 mm. Gallery close-ups show twigs as sticks instead of wires; at full-tree distance they are
  still thin lines.
- `Node::axis_points()` and `skeleton::MAX_AXIS_POINTS`: the number of mesh rings a branch's wood
  gets.

### Changed

- Drooping branches are meshed on more rings: `BENT_AXIS_POINTS` (6) is now the minimum, and a
  branch that gravity turns more than about 4.6 degrees between two rings gets more, up to 16, so
  long arching limbs are smooth instead of faceted. Wood, children, leaves and the floor test all
  use the same count. The willow's wood grows from about 10k to 38k vertices; presets with mild
  droop barely change.
- Sub-branches leave a drooping parent along its bent tangent at the attachment point, not along
  the direction the parent left the trunk with, so twigs hang from arching limbs. This shifts
  sub-branch directions slightly on every preset with `gravity_strength > 0`.
- The willow preset weeps: `branches.start` 0.4 -> 0.5, `density` 2 -> 4, `length` 0.7 -> 0.8,
  `up_attraction` -0.75 -> -0.5, `gravity_strength` 0.5 -> 2.5 and `stiffness` 0.2 -> 0.1. It has 8
  primaries instead of 5, branch tips hang nearly straight down, and the crown is a wide skirt that
  reaches the ground.
- `BranchParams` literals need the new field: end them with `..Default::default()`. Serialized
  params from 0.2.x (the `serde` feature) lack `min_twig_radius` and no longer deserialize as they
  are.
- `docs/growth.gif`, the README clip, is re-recorded for this release.

## [0.2.2] - 2026-10-10

Appearance and docs only; no API changes.

### Changed

- The poplar preset is now a leafy column. It had 14 short, nearly vertical branches hugging a 12 m
  trunk, so it read as a bare pole with about 70 leaves and a 2.4 m bare leader above them; it now
  has about 125 nodes and about 800 leaves in a narrow crown that starts near the ground and
  reaches the tip. `branches.start` 0.2 -> 0.12, `density` 1.6 -> 2.5, `length` 0.3 -> 0.35,
  `angle` 15 -> 25, `up_attraction` 0.92 -> 0.6, `sub_branch_count` 2 -> 4, `foliage.density`
  3.5 -> 10, `trunk.leader_length` 0.2 -> 0.12 and `leader_has_branches` false -> true.
- `docs/growth.gif`, the README clip, is re-recorded. It still showed the square leaves of 0.1.0.
  Re-recording it is now part of every release (see `AGENTS.md`).

## [0.2.1] - 2026-10-10

Appearance only; no API changes.

### Changed

- `FoliageParams::default().color_variation` is now 0.3 (was 0.15), so the per-leaf brightness
  differences added in 0.2.0 are visible on the dark leaf greens. Set it back to 0.15 to keep the
  0.2.0 look.
- `leaf_size` is 1.3 times larger on the nine presets that use leaf-shaped cards, to make up for
  cards covering about half the area of the old squares: oak 0.33, willow 0.26, birch 0.26,
  palm 1.05, bonsai 0.33, maple 0.39, poplar 0.29, dragon tree 0.46 and Japanese maple 0.23.
  Needle (pine, spruce, redwood) and octahedron (cypress, baobab) presets are unchanged.

## [0.2.0] - 2026-10-10

A leaf look pass, done in the mesh with no custom shader. The API gains public fields and
`foliage_mesh` changes signature, hence the minor bump.

### Added

- Leaf-shaped cards: `CrossedPlanes`, `SingleQuad` and `StarBurst` now emit a pointed oval instead
  of a square, with `CrossedPlanes` and `SingleQuad` seating the card's base on the leaf point so
  leaves grow out of their twigs.
- Crown-bent normals: `FoliageParams::normal_bend` (default 0.8; 0 = the old flat cards) bends
  every leaf normal toward pointing out of the crown, so crowns shade as a soft volume.
- Per-leaf tint: `FoliageParams::color_variation` (default 0.15, 0-0.5) scales each leaf's colour
  by a stable per-leaf factor, stored as a vertex colour. `LeafPoint::tint` holds the draw; it
  comes from its own random stream, so leaf positions, sizes and rotations are unchanged and
  nothing re-rolls as the tree ages.
- `MeshData::colors` (linear RGBA vertex colours, empty for none). `MeshData::extend` keeps it
  aligned with the positions and `into_bevy_mesh` adds `Mesh::ATTRIBUTE_COLOR` when it is set.
- Gallery scenarios `extreme_flatleaves` (the old flat look), `extreme_leafvary`,
  `smoothyoung_<preset>_4` and `pipe_<preset>`; the gallery now renders 289 scenarios.

### Changed

- `foliage_mesh(points, &FoliageParams)` replaces `foliage_mesh(points, style)`, because it now
  needs the normal bend and the colour variation as well as the style.
- `LeafPoint` has a new `tint` field, and `FoliageParams` has `normal_bend` and
  `color_variation`; build them with `..Default::default()` or `..FoliageParams::default()`.
- A leaf-shaped card covers about half the area of the square it replaces, so crowns are airier
  at the same `leaf_size`. `FoliageParams::leaf_size` is now the leaf's length from base to tip.
- The gallery's `INSIDE_OUT` flag counts only the wood, because bent foliage normals deliberately
  disagree with face winding; foliage winding is covered by a unit test.

## [0.1.1] - 2026-10-10

Documentation only; no API or behaviour changes.

### Fixed

- `MeshParams::smooth_iterations` described plain Laplacian smoothing; it now documents the
  volume-preserving (Taubin) smoothing the crate does and what it keeps in place.
- `FoliagePlacement::TerminalBranches` and `TipClusters` now state that "terminal" means terminal
  in the full-grown tree, which is why young branches that will grow twigs stay leafless.
- `ARCHITECTURE.md` explains how aged skeletons keep full-grown child counts.

## [0.1.0] - 2026-10-10

The first release.

### Added

- `TreeParams` grouped by subsystem (`TrunkParams`, `BranchParams`, `SplitParams`, `MeshParams`,
  `FoliageParams`, `GrowthParams`) and 15 `TreePreset`s: oak, pine, willow, birch, palm, cypress,
  bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree and redwood.
- `build_tree(params, age)`: one deterministic call from parameters and age to CPU meshes (wood and
  leaves) plus the aged skeleton. The same seed always gives the same tree.
- A continuous growth model. Every node has a birth age and a duration, so a tree can be shown at
  any age from a bare seedling to maturity; leaves never re-roll or disappear as it grows.
- `FloraPlugin` with the `ProceduralTree`, `TreeAge` and `Growing` components. It rebuilds the
  meshes when parameters or age change and reports each rebuild with a `TreeGenerated` message.
- Branch shape: gravity bend scaled by each branch's lever arm, Y-splits, recursive sub-branches,
  crown envelopes, phyllotaxis, twist, floor avoidance that holds for bent wood.
- Wood mesh: tapered, wobbling, twisted trunk with root flares; flat, pointed or leader-branch
  trunk tops; branch collars and root segments, so every junction is closed; optional pipe-model
  radii, adaptive ring resolution and volume-preserving (Taubin) smoothing.
- Leaves: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster),
  three placements (terminal branches, all branches, tip clusters) and four orientations, with
  translucent leaf material.
- Optional `serde` feature for `Serialize`/`Deserialize` on every parameter type.
- Examples: `showcase` (interactive, with growth and preset cycling), `gallery` (offscreen
  four-view renders and per-scenario statistics for visual QA) and `capture_growth` (the README
  clip).

[Unreleased]: https://github.com/Ladvien/bevy_flora/compare/v0.47.0...HEAD
[0.47.0]: https://github.com/Ladvien/bevy_flora/compare/v0.46.1...v0.47.0
[0.46.1]: https://github.com/Ladvien/bevy_flora/compare/v0.46.0...v0.46.1
[0.46.0]: https://github.com/Ladvien/bevy_flora/compare/v0.45.0...v0.46.0
[0.45.0]: https://github.com/Ladvien/bevy_flora/compare/v0.44.0...v0.45.0
[0.44.0]: https://github.com/Ladvien/bevy_flora/compare/v0.43.0...v0.44.0
[0.43.0]: https://github.com/Ladvien/bevy_flora/compare/v0.42.0...v0.43.0
[0.42.0]: https://github.com/Ladvien/bevy_flora/compare/v0.41.0...v0.42.0
[0.41.0]: https://github.com/Ladvien/bevy_flora/compare/v0.40.0...v0.41.0
[0.40.0]: https://github.com/Ladvien/bevy_flora/compare/v0.39.1...v0.40.0
[0.39.1]: https://github.com/Ladvien/bevy_flora/compare/v0.39.0...v0.39.1
[0.39.0]: https://github.com/Ladvien/bevy_flora/compare/v0.38.1...v0.39.0
[0.38.1]: https://github.com/Ladvien/bevy_flora/compare/v0.38.0...v0.38.1
[0.38.0]: https://github.com/Ladvien/bevy_flora/compare/v0.37.0...v0.38.0
[0.37.0]: https://github.com/Ladvien/bevy_flora/compare/v0.36.0...v0.37.0
[0.36.0]: https://github.com/Ladvien/bevy_flora/compare/v0.35.0...v0.36.0
[0.35.0]: https://github.com/Ladvien/bevy_flora/compare/v0.34.0...v0.35.0
[0.34.0]: https://github.com/Ladvien/bevy_flora/compare/v0.33.0...v0.34.0
[0.33.0]: https://github.com/Ladvien/bevy_flora/compare/v0.32.0...v0.33.0
[0.32.0]: https://github.com/Ladvien/bevy_flora/compare/v0.31.0...v0.32.0
[0.31.0]: https://github.com/Ladvien/bevy_flora/compare/v0.30.0...v0.31.0
[0.30.0]: https://github.com/Ladvien/bevy_flora/compare/v0.29.0...v0.30.0
[0.29.0]: https://github.com/Ladvien/bevy_flora/compare/v0.28.0...v0.29.0
[0.28.0]: https://github.com/Ladvien/bevy_flora/compare/v0.27.0...v0.28.0
[0.27.0]: https://github.com/Ladvien/bevy_flora/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/Ladvien/bevy_flora/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/Ladvien/bevy_flora/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/Ladvien/bevy_flora/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/Ladvien/bevy_flora/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/Ladvien/bevy_flora/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/Ladvien/bevy_flora/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/Ladvien/bevy_flora/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/Ladvien/bevy_flora/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/Ladvien/bevy_flora/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/Ladvien/bevy_flora/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/Ladvien/bevy_flora/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/Ladvien/bevy_flora/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/Ladvien/bevy_flora/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/Ladvien/bevy_flora/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/Ladvien/bevy_flora/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/Ladvien/bevy_flora/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/Ladvien/bevy_flora/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/Ladvien/bevy_flora/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/Ladvien/bevy_flora/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/Ladvien/bevy_flora/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/Ladvien/bevy_flora/compare/v0.6.2...v0.7.0
[0.6.2]: https://github.com/Ladvien/bevy_flora/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/Ladvien/bevy_flora/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/Ladvien/bevy_flora/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/Ladvien/bevy_flora/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/Ladvien/bevy_flora/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Ladvien/bevy_flora/compare/v0.3.5...v0.4.0
[0.3.5]: https://github.com/Ladvien/bevy_flora/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/Ladvien/bevy_flora/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Ladvien/bevy_flora/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Ladvien/bevy_flora/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Ladvien/bevy_flora/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Ladvien/bevy_flora/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/Ladvien/bevy_flora/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Ladvien/bevy_flora/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Ladvien/bevy_flora/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/Ladvien/bevy_flora/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Ladvien/bevy_flora/releases/tag/v0.1.0
