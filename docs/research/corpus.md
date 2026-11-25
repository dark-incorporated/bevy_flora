# Research corpus

The reading list per domain, by home-still stem (`distill_search` / `markdown_read`), or by URL for
a web source the corpus does not hold. Each system's own note (`docs/research/<system>.md`) quotes
what it rests on; this file only says what exists. Newest additions last in each section.

## Tree architecture and biomechanics

- `10.1371_journal.pone.0093535`: Minamino and Tateno 2014, "Tree branching: Leonardo da Vinci's
  rule versus biomechanical models" (the uniform stress and elastic similarity hypotheses).
- `10.1038_s41467-017-00995-6`: Eloy, Fournier, Lacointe and Moulia 2017, "Wind loads and
  competition for light sculpt trees into self-similar structures" (MECHATREE; wind-induced
  bending stress per branch).
- `10.3389_fpls.2022.971690`: "Stronger wind, smaller tree: testing tree growth plasticity through a
  modeling approach" (2022).
- `10.3389_fpls.2016.01095`: "Why be a shrub? A basic model and hypotheses for the adaptive values of
  a common growth form" (2016).
- `10.48044_jauf.2024.020`: "Tree trimming effects on 3-dimensional crown structure and tree
  biomechanics" (2024; measured sway frequency and displacement).
- `10.1038_s41467-025-60262-x`: "The global spectrum of tree crown architecture" (2025).
- `10.1038_s41467-023-44004-5`: "Edge effects on tree architecture exacerbate biomass loss of
  fragmented Amazonian forests" (2023).
- `10.1111_cgf.12282`: "Inverse procedural modelling of trees".
- `10.1145_3502220`: "Procedural urban forestry".
- `10.1111_cgf.13106`: "Interactive modeling and authoring of climbing plants" (2017; wind
  bending and breaking of branches).
- `10.48550_arxiv.2204.03237`: Yang, Wang and Huo 2022, "Rule-based Procedural Tree Modeling
  Approach" (survey of rule-based tree models; branch surfaces as generalized cylinders swept along
  the skeleton).
- `sig2024_Interactive_Invigoration_Volumetric_Modeling_of_Trees_with_Strands`: "Interactive
  Invigoration: Volumetric Modeling of Trees with Strands" (SIGGRAPH 2024; strand-based branch
  meshes against generalized cylinders, mesh generation times).
- `10.48550_arxiv.2306.09310`: Raistrick et al. 2023, "Infinite Photorealistic Worlds using
  Procedural Generation" (Infinigen: procedural trees and plants in a whole-scene generator).

## Plant growth and branching

- `10.1038_s41580-025-00851-2`: "Mechanisms of auxin action in plant growth and development".
- `10.1038_s41467-020-15893-7`: "Arabidopsis FHY3 and FAR1 integrate light and strigolactone
  signaling to regulate branching".
- `10.1038_s41598-017-14831-w`: "Climate change accelerates growth of urban trees in metropolises
  worldwide".
- `10.1111_cgf.13263`: "Tree Growth Modelling Constrained by Growth Equations" (2017; added 2026-10-30).
- `10.14214_sf.a8529`: "A functional model of tree growth and tree architecture" (1997; added 2026-10-30).
- `The Computational Beauty of Nature -- Anna's Archive`: Flake, "The Computational Beauty of Nature"
  (a general text on fractals and growth models; found in `catalog_recent` 2026-10-31, not read).
- `10.1145_378456.378505`: de Reffye, Edelin, Francon, Jaeger and Puech 1988, "Plant Models Faithful to
  Botanical Structure and Development" (SIGGRAPH '88; the architectural models of Halle, Oldeman and
  Tomlinson as a quantitative simulation of meristems: growth, ramification and mortality over a
  per-plant time unit; was in the catalog, listed 2026-10-31).
- `10.1145_3306346.3323039`: Makowski et al. 2019, "Synthetic silviculture" (SIGGRAPH 2019;
  ecosystems of up to 500K plants, a per-type maturity age; was in the catalog, listed 2026-10-31).
- Web, not indexed: Epic Games, "Procedural Vegetation Editor in Unreal Engine" (UE 5.8, the Grower's
  Cycles ("not equivalent to years") and Save Grower Preset),
  <https://dev.epicgames.com/documentation/unreal-engine/procedural-vegetation-editor-in-unreal-engine>.

## Phenology (leaf colour, leaf fall, blossom)

- `10.1038_s41586-022-05092-3`: "Warm springs alter timing but not total growth of temperate
  deciduous trees".
- `10.1038_s41559-022-01668-4`: "An earlier start of the thermal growing season enhances tree growth
  in cold humid areas but not in dry areas".
- `10.1038_s41467-022-28490-7`: "Forest fragmentation impacts the seasonality of Amazonian evergreen
  canopies".
- `10.1038_s41467-026-68989-x`: "Pantropical moist forests are converging towards a middle leaf
  longevity".
- `10.5194_bg-2015-619`: "Climate seasonality limits leaf carbon assimilation and wood productivity
  in tropical forests" (added 2026-10-31).
- `s2025-advances-materials-3`: SIGGRAPH 2025 "Advances in Real-Time Rendering in Games", a materials
  talk: a shipped renderer's vegetation materials take a season colour LUT and an alpha texture with two
  channels, one with and one without leaves, chosen by the season (8 season values; found in
  `catalog_recent` 2026-10-31). The 2015 to 2025 "Advances" notes (`s20NN-advances-*`) were indexed the
  same day; `distill_search` finds no other vegetation talk in them.

- `10.1093_treephys_tpaa058`: Dox et al. 2020, "Timeline of autumn phenology in temperate deciduous trees" (onset of foliar
  senescence of beech "in mid-late October" and of birch "in late September to mid-October" at 51 degrees N; listed
  2026-11-25 when `FloraSeason::of_date` was weighed and dropped, `docs/research/point-grid.md`).

## Tree condition and mortality (issue #87)

- `10.1038_s41467-020-18996-3`: "Tree mode of death and mortality risk factors across Amazon
  forests".
- `10.1038_s41559-017-0248-x`: "A multi-species synthesis of physiological mechanisms in
  drought-induced tree mortality".
- `10.1038_s41467-022-29289-2`: "Global field observations of tree die-off reveal hotter-drought
  fingerprint for Earth's forests".
- `10.1038_s41467-020-20455-y`: "Cross-scale interaction of host tree size and climatic water
  deficit governs bark beetle-induced tree mortality".
- `10.1038_s41467-019-12380-6`: "Tree height explains mortality risk during an intense drought".
- `10.3390_f8040137`: "Traditional and Novel Indicators of Climate Change Impacts on European Forest
  Trees" (Forests 2017; crown defoliation as the visual vitality indicator; added 2026-10-31).
- `10.1007_s10342-005-0085-3`: "Tree growth as indicator of tree vitality and of tree reaction to
  environmental stress: a review" (transparency and defoliation classes; mortality against
  transparency; added 2026-10-31).
- `10.3389_fpls.2026.1809811`: "Divergent climate-growth responses in radial growth of Chinese pine
  forests" (stands graded by crown dieback, about 10, 30, 50 and 80 per cent; added 2026-10-31).
- `10.1038_s41598-022-24833-y`: "Canopy dieback and recovery in Australian native forests following
  extreme drought" (a canopy health score per tree; added 2026-10-31).
- `10.3832_ifor0538-003`: Johnson and Jacob, "Monitoring the effects of air pollution on forest
  condition in Europe: is crown defoliation an adequate indicator?" (iForest 2010; in the catalog,
  not yet converted when added 2026-10-31).
- `10.1016_j.scitotenv.2020.141792`: "From the comfort zone to crown dieback: Sequence of physiological
  stress thresholds in mature European beech trees" (the sequence under progressive drought: early
  defoliation, embolism, crown dieback exceeding 20% in the year after; added 2026-10-31).
- `10.1111_plb.13467`: Frei et al., "European beech dieback after premature leaf senescence during the
  2018 drought in northern Switzerland" (mortality rising monotonously over time; added 2026-10-31).
- `10.1051_forest:2006042`: Bréda, Huc, Granier and Dreyer, "Temperate forest trees and stands under severe
  drought: a review of ecophysiological responses, adaptation processes and long-term consequences"
  (Annals of Forest Science 2006; the 2003 drought's premature leaf shedding in beech, canopy leaf area
  index falling in August with the fallen leaves still green; added 2026-10-31).

## Game objects and entity state (`TreeCondition`, `FloraIndex`)

- `Game Engine Architecture, Third Edition` (Gregory; home-still doc id begins with that title): chapter
  16, the game object as attributes (state) and behaviours; component-based object models.
- `10.1145_3748522.3779910`: Tasnim and Zhao, "The Essence of Entity Component System" (components
  added to and removed from entities; archetype ECS semantics; added 2026-10-31).
- `10.1145_3763050`: Redmond et al., "Exploring the Theory and Practice of Concurrency in the
  Entity-Component-System Pattern" (added 2026-10-31).
- Web, not indexed: Timberborn's plant states (alive, dying, dead), as one mod's notes on the decompiled
  game record them, <https://github.com/timbermods/dam-good-maps/pull/742>.

## Wildfire

- `10.1038_s41598-019-55036-7`: "Short-interval wildfire and drought overwhelm boreal forest
  resilience".
- `10.1038_s41598-017-06747-2`: "Small fire refugia in the grassy matrix and the persistence of
  Afrotemperate forest in the Drakensberg mountains".
- `sig2024_Scintilla_Simulating_Combustible_Vegetation_for_Wildfires`: "Scintilla: Simulating
  Combustible Vegetation for Wildfires" (SIGGRAPH 2024; per-plant fuel and ignition; added 2026-10-30).

## Wind animation (shipping games and technique)

- `GPU Gems- Programming Techniques, Tips and Tricks for -- Randima Fernando; ...` (home-still doc
  id begins with this): GPU Gems 1, chapter 7, Pelzer, "Rendering Countless Blades of Waving Grass"
  (the Codecreatures Benchmark: per-cluster, per-vertex and per-object wind in the vertex shader).
- Web, not indexed: GPU Gems 3, chapter 16, Sousa, "Vegetation Procedural Animation and Shading in
  Crysis",
  <https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis>.
- Web, not indexed: GPU Gems 3, chapter 6, Zioma, "GPU-Generated Procedural Wind Animations for
  Trees" (Battlefield-era DICE),
  <https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-6-gpu-generated-procedural-wind-animations-trees>.

## Rendering, level of detail, impostors

- `book_real_time_rendering_4th_akenine`: Real-Time Rendering, 4th edition (vegetation instancing,
  chapter 18).
- `book_level_of_detail_3d_graphics_luebke`: Luebke et al., "Level of Detail for 3D Graphics".
- `s2008-advances-full-course-notes-5`: SIGGRAPH 2008 "Advances in Real-Time Rendering" course notes
  (CryEngine terrain and vegetation).
- Web, not indexed: SpeedTree Modeler manual (Unity), "Branch generator properties", Segments:
  Radial (Absolute, + Relative from the branch radius) and Dynamic LOD,
  <https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>.

## Spatial queries (`FloraIndex`)

- `book_real_time_collision_detection_ericson`: Ericson, "Real-Time Collision Detection", chapter 7
  (uniform grids: dense, sparse and hashed storage; a hashed grid holds only the occupied cells).
- `10.1145_1730804.1730814`: Laine and Karras 2010, "Efficient Sparse Voxel Octrees" (quotes the
  Amanatides and Woo regular-grid traversal: the next-plane `t` values per axis, step along the
  smallest).
- `eth-cgl-sim_anim-Sol14a`: "SPH Fluids in Computer Graphics" (the hashed grid's occupancy:
  "generally sparsely filled").
- `10.1111_j.1467-8659.2005.00829.x`: Teschner et al. 2005, "Collision Detection for Deformable
  Objects" (spatial subdivision and hashing for neighbourhood queries).
- Web, not indexed: Clark and Evans 1954, "Distance to Nearest Neighbor as a Measure of Spatial
  Relationships in Populations", doi `10.2307/1931034` (paywalled); the 1979 generalisation to k
  dimensions, doi `10.2307/1937660`.

## Parameter limits and hostile inputs (`TreeParams::sanitized`)

- `10.1111_cgf.12744`: Computer Graphics Forum, related work on Weber and Penn 1995 ("up to 80 parameters"; answers
  `distill_search`, no catalog entry).
- `10.1093_treephys_tpac065`: Aye, Brännström and Carlsson 2022, "Prediction of tree sapwood and heartwood profiles
  using pipe model and branch thinning theory" (the number of tips limited by light and space, `1 <= d <= 3`).
- `10.1111_cgf.13263`: Guo, Zhang, Li, Deussen and Yi 2017, "Tree Growth Modelling Constrained by Growth Equations"
  (the loop count `LoS` after which a tree stops growing).
- Web, not indexed: Rust standard library, `f32::clamp` ("returns NaN if the initial value was NaN"; panics if `min >
  max`), <https://doc.rust-lang.org/std/primitive.f32.html#method.clamp>. SpeedTree Modeler's branch Clamp group, in
  the manual page listed above.

## Tree files (`.tree.ron`, `.tree.yaml`)

- `10.1109_38.736469`: Lintermann and Deussen 1999, "Interactive Modeling of Plants" (a plant type is a parameter set a
  tool edits and a program turns into geometry; "models can easily be reused and changed in order to get new tree
  models").
- Web, not indexed: Unity, "Format of text serialized files" (Unity's assets are a subset of YAML; floats are written
  losslessly), <https://docs.unity3d.com/Manual/FormatDescription.html>. `serde_yaml_ng` 0.10.0, crate documentation
  (enums as `!Tag`, "This version only support the YAML Specification 1.1") and source (`src/de.rs`
  `RepetitionLimitExceeded`, `recursion_check`; `src/ser.rs` `serialize_f32`), <https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/>.
- The last 200 `catalog_recent` events (2026-11-17 to 2026-11-20) hold no tree, plant, foliage, L-system, phenology,
  fire-spread, impostor, wind or felling document that this file lacks.

## Parameter panels (`TreeParams::fields`)

- `10.1111_cgf.12744`: the related-work passage on Weber and Penn 1995 (no catalog entry; "requiring adjustment of up to
  80 parameters"; already listed above for the limits).
- Web, not indexed: Epic Games, "Metadata Specifiers in Unreal Engine" (`ClampMin`, `ClampMax`; "Metadata only exists in
  the editor"), <https://dev.epicgames.com/documentation/en-us/unreal-engine/metadata-specifiers-in-unreal-engine>.
  Unity, SpeedTree Modeler, "Branch generator properties" (a `Clamp` group of Min and Max per property),
  <https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>. `bevy_reflect` 0.20.0
  `src/attributes.rs` (a range declared on a field with `#[reflect(@RangeInclusive::<f32>::new(0.0, 1.0))]`, read from
  `TypeInfo`).
- The last 200 `catalog_recent` events were re-read on 2026-11-25 (the same 2026-11-17 to 2026-11-20 window as
  iteration 13: no new tree, plant, foliage, phenology, fire, impostor, wind or felling document).

## Placement rules and keeping clear (`PointGrid`)

- `10.1145_3502220`: Benes et al. 2022, "Procedural Urban Forestry" (planting rules are distances: "Poisson-disc distribution
  (minimal distance requirement) among plants and from the building envelope"; already listed above).
- `10.1890_1051-0761(2006)016[0540:naoctc]2.0.co;2`: Canham et al. 2006, "Neighborhood analyses of canopy tree competition
  along environmental gradients in New England forests" (a neighbourhood is a radius, 3.2 to 19.8 m for adult trees, and the
  radius belongs to the target species).
- `10.1111_cgf.12276`: a survey of procedural modelling for virtual worlds (the layers of a world interact: "the execution of
  each pm method might have to take into account the location, type and properties of other features"; the catalog title is
  empty).
- `book_real_time_collision_detection_ericson`: Ericson (hashed grids; listed above).
- Web, not indexed: Bridson 2007, "Fast Poisson Disk Sampling in Arbitrary Dimensions" (the background grid `scatter.rs` uses).
