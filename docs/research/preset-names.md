# Naming and ageing a tree without hand-rolling it

Iteration 11, 2026-10-31. No issue is open on this repository (`gh issue list`, 0 open). The item comes from
the section 4 fallback, "what the crate's consumers hand-roll": the same two lines are written by
bevy_grimoire (family issue F6, `dark-incorporated/bevy_dark_factory#102`), by Sanctuary and by this crate's own examples.

## What the consumers write today

- A preset from its name: `TreePreset::ALL.iter().find(|t| t.name() == name)`, in bevy_grimoire's
  `ops_tree.rs` (the tree editor's `preset` field, with an error message built by joining
  `TreePreset::ALL.iter().map(|t| t.name())`) and again in `forest_view.rs` (a species is a preset name or a
  `.tree.ron` path); in this repository `examples/capture_growth.rs`, `forest.rs` (three times),
  `inspect.rs` and `tree_file.rs` (the last one case-insensitively). Eight copies, and `FromStr`, the
  language's own name for it, is missing: `"oak".parse::<TreePreset>()` does not compile.
- The age at which a tree stops changing, to place a tree at a fraction of its growth: Sanctuary's
  `src/prototype/trees.rs` (`grown_age`, fed by `build_tree(&params, f32::INFINITY).mature_at`) builds a whole
  mature tree, wood and leaves, once per kind to read one number; bevy_grimoire's `place` takes an `age_years`
  and has no way to say how many years a species takes to mature. The number is already public
  (`generate_skeleton(&params).mature_at(&params.growth)`) but nothing says so, and nothing in `TreeParams`
  or `TreeAge` points at it.

## Sources

- Model: de Reffye, Edelin, Francon, Jaeger and Puech 1988, "Plant Models Faithful to Botanical Structure and
  Development" (SIGGRAPH '88, doi `10.1145/378456.378505`, stem `10.1145_378456.378505`): the unit of a
  plant's clock belongs to the plant. Makowski et al. 2019, "Synthetic silviculture" (SIGGRAPH 2019, doi
  `10.1145/3306346.3323039`, stem `10.1145_3306346.3323039`): maturity is a per-species age.
- Shipping-game tooling: Epic Games, "Procedural Vegetation Editor in Unreal Engine" (UE 5.8 documentation,
  <https://dev.epicgames.com/documentation/unreal-engine/procedural-vegetation-editor-in-unreal-engine>, read
  2026-10-31, not indexed): the age unit of a shipped grower is not a calendar year, and a preset is a named,
  saved asset a tool loads.
- Technical: the Rust API Guidelines, "Types eagerly implement common traits (C-COMMON-TRAITS)"
  (<https://rust-lang.github.io/api-guidelines/interoperability.html>), and `std::str::FromStr`'s documentation
  (<https://doc.rust-lang.org/std/str/trait.FromStr.html>), both read 2026-10-31, not indexed.

## Craft principle

de Reffye et al. on what a growth clock is: "They heavily depend on time so that models of plant
architectures have to integrate time in growth, ramification and mortality processes. In de Reffye's
mathematical macroscopic model of plants' growth, the unit of discretized time is the time taken by the growth
of a growth unit, this length of time being supposed constant for the axes a given order of a given plant."
The clock is the plant's own: "age 6" means a different tree for an oak and for a palm, so a caller that
places trees by age must be able to ask the plant when it is grown. Makowski et al. give every plant type that
number: "We use a flowering age parameter to indicate when in the simulation a plant type reaches maturity",
and show it as 200 years for a baobab ("a 200 year old Baobab tree reaching maturity is seeding new plant
models"). `TreeParams` has it too, in this crate's age units, but only as a by-product of the skeleton
(`Skeleton::mature_at`: the latest `birth + duration * thickening_lag` of any node), because the last radius
to thicken depends on the topology the seed grows; there is no closed form from `GrowthParams` alone.

## How shipping games do it

Unreal's grower says in so many words that its age is not a year: "Cycles: How many cycles (iterations) the
Grower runs. This is not equivalent to years — the simulation emulates the output rather than running for 200+
cycles." A tool that sets a plant's maturity therefore sets it in the grower's own unit, per species, and
reads the result back from the grower. The same page treats a preset as something a tool names and loads:
"Save Grower Preset: The method for generating custom presets. The preset is written to a data asset that can be
loaded into a Grower — a good way to expand a preset library." A preset library is a table from a name to
parameters, and every tool and script that touches it needs the table's lookup.

## Model adopted

Two additions, no behaviour change.

1. `TreePreset::from_name(&str) -> Option<TreePreset>`, the inverse of `TreePreset::name` (exact, snake_case,
   the string the crate prints, `inspect --preset` takes and bevy_grimoire writes), `impl FromStr for TreePreset`
   whose error `UnknownPreset` names what was asked and, in its `Display`, every preset the crate has, and
   `impl Display for TreePreset` printing `name()`, so the two round-trip. The C-COMMON-TRAITS rule is the
   reason this belongs here: a downstream crate cannot add `FromStr` to a foreign type (the orphan rule the
   guideline spells out with `Url` and `Display`), so bevy_grimoire could only ever write the `find` itself.
   `FromStr` documents its input format ("Check the type's documentation for the input formats it knows
   how to parse"): this one's is `name()`'s output, not the `serde` form (`DragonTree`), which stays as saved
   files have it.
2. `TreeParams::mature_at(&self) -> f32` (the crate's own word: `TreeMeshes::mature_at`, `TreeRender::mature_at`,
   `Skeleton::mature_at`; not `mature_age`, which would sit beside `GrowthParams::maturity_age`, a different
   number): the age, in the growth units of `TreeParams::growth`, from which `TreeAge` no longer changes the
   tree (`generate_skeleton(self).mature_at(&self.growth)`; the skeleton only, no wood, no leaves). Documented
   on `TreeAge` and `GrowthParams::maturity_age`. A `TreeAge` above it draws the mature tree: that is the one
   claim the method makes, and the test checks it against the mesh.

## Parameters

None. No default, preset literal or `Default` impl changes.

## Validation targets

- `from_name(p.name()) == Some(p)` for all 20 presets (no two presets share a name: the existing test); the
  empty string, a prefix, a different case (`"Oak"`), a padded name (`"oak "`) and the serde form
  (`"DragonTree"`) give `None`; `p.to_string().parse() == Ok(p)`.
- The error names the input and lists all 20 names, so a caller can show it as it is.
- `mature_at()` equals `build_tree(&params, f32::INFINITY).mature_at` bit for bit for every preset and three
  seeds; it is finite and positive; the wood and leaves of a tree built at `mature_at()` are those of the tree
  built at `f32::INFINITY` (same triangles, vertices within 1e-6 m: the last sliver of thickening at that very
  age is a float's rounding short), and the wood built at half of it is not.
- `mature_at()` costs the skeleton, not the mesh: measured against `build_tree` in the log.
- Nothing a tree draws moves: the gallery's `stats.tsv` columns 1-10 for the scenarios the examples' argument
  parsing could touch are identical (they cannot move: no tree-building file is edited), and the forest
  bench does not change.

## What great looks like

`let params = name.parse::<TreePreset>()?.params();` turns a string from a file, a UI or an agent into a
tree, and the error is a sentence the editor can show. `TreeAge(params.mature_at() * 0.5)` is a tree half way
through its growing time, of any species, without building one to find out. Neither needs a line of the crate's source to be read.

## Deviations

None. `TreePreset::name` is unchanged. The `serde` form of `TreePreset` and the strings `name()` returns stay
two different spellings (`DragonTree`, `dragon_tree`); a design default, to not break files in the wild, said
in the docs of both.

## Pending index

- The Epic page, the Rust API Guidelines and `std::str::FromStr` are web pages, not in the corpus.
- de Reffye et al. 1988 and Makowski et al. 2019 are in the catalog; they were missing from `corpus.md`
  (added in `Research: corpus`).
- No source here measures how many years a real species takes to mature in a form a preset's `maturity_age`
  could be checked against; the presets' `GrowthParams` are design defaults (`src/presets.rs`) and this note
  does not change them.
