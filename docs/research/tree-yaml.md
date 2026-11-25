# A species as a YAML file: `.tree.yaml` beside `.tree.ron` (issue #95)

Iteration 13, 2026-11-25. The item is the one open issue, #95 "A YAML loader for TreeParams (.tree.yaml)", labelled
`enhancement`, `consumer` and `consumer:bevy_dark_factory`: the Grimoire editor and game moved every project file from
RON to YAML (`bevy_dark_factory#365`, GD-03), writes a species as `trees/<name>.tree.yaml` through this crate's serde
derives, and a game cannot load what the editor writes because this crate's asset loader and `TreeParams::from_ron` /
`to_ron` know only `.tree.ron`. It outranks every candidate of the backlog (section 3: a `consumer` issue is the pass's
item). No bug is open and no other enhancement is filed. The parked branch `wiggum/parked-pass-9-20261102-054127` is a
160-line throwaway probe already adopted by iteration 12; it is stale.

## What the consumer does today

`bevy_dark_factory` (`crates/bevy_dark_factory/src/ops_tree.rs`, `forest_view.rs`) reads and writes a `TreeParams`
with its own YAML layer: `blueprint_model::yaml::to_string` is `serde_yaml_ng::to_string(value)`, `from_str` is
`serde_yaml_ng::Deserializer::from_str` wrapped by `serde_path_to_error`, and the dependency is
`serde_yaml_ng = "0.10"`. It pins this crate at `25e956b` with `features = ["serde"]` only. It needs, on the game side,
the same file read the same way: an `AssetLoader` for `.tree.yaml` so `asset_server.load("trees/oak.tree.yaml")` is a
`Handle<TreeParams>` that `ProceduralTreeAsset` draws and that rebuilds when the file is saved, and `TreeParams::to_yaml`
/ `from_yaml` for code that holds the text (the editor, a test, an agent).

## Sources

- Model: Lintermann and Deussen 1999, "Interactive Modeling of Plants" (IEEE Computer Graphics and Applications 19(1),
  doi `10.1109/38.736469`, stem `10.1109_38.736469`): a plant type is a set of parameters a tool edits and a program
  turns into geometry, and a saved model is reused and changed.
- Shipping-game tooling: Unity Technologies, "Format of text serialized files" (Unity 6000.6 manual,
  <https://docs.unity3d.com/Manual/FormatDescription.html>, read 2026-11-25, not indexed); Epic Games, "Procedural
  Vegetation Editor in Unreal Engine" (UE 5.8, <https://dev.epicgames.com/documentation/unreal-engine/procedural-vegetation-editor-in-unreal-engine>,
  read 2026-10-31, quoted in `preset-names.md`, not indexed).
- Technical: `serde_yaml_ng` 0.10.0, crate documentation (<https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/>, read
  2026-11-25) and its source, `src/de.rs` (`RepetitionLimitExceeded`, `recursion_check`) and `src/ser.rs`
  (`serialize_f32`), read 2026-11-25 in the cargo registry; Bevy 0.20's `AssetLoader` (`extensions`, a multi-dot
  extension such as `tree.ron`), which `src/tree_asset.rs` already uses.

## Craft principle

Lintermann and Deussen describe the plant-modelling tools this crate descends from: "De Reffye et al. developed a
procedural model based on the birth and death of growing buds that lets users control the generation of plants by some
parameters. AMAP, a commercial library of plants (particularly trees) and generation procedures, builds on this idea.
The user edits a plant type's parameters and runs the simulation to produce the desired geometry." A species is
therefore data, a parameter set that lives outside the program, and its value is that it survives the session:
"models can easily be reused and changed in order to get new tree models". Which text the parameter set is written in
is a matter of the project that keeps it; what a model of this kind has to keep true is that the file and the value
are the same thing: what a tool saves is what the generator reads, and a tree is the same tree for the same file.
`TreeParams` already satisfies the second half (`from_ron(to_ron(p)) == p` for every preset); the YAML form must satisfy
it too.

## How shipping games do it

An engine that ships an editor and a game keeps one text form of an asset, so the two read the same bytes. Unity's
assets are YAML: "Unity's Scene format uses a custom subset of the YAML data serialization language. YAML is an open
format with documentation about it available on the YAML website." and, for numbers, "Unity uses the IEEE 754
representation for lossless encoding of values and to write floating point values which don't have a short decimal
representation": an asset written by the editor reads back to the same bits. Unreal's grower says the same of a
vegetation preset: "Save Grower Preset: The method for generating custom presets. The preset is written to a data
asset that can be loaded into a Grower". The practice: the editor's save and the runtime's load are one format, the
float round-trip is exact, and the loader sits behind the engine's asset system, so a saved file reloads the tree that
uses it.

## Model adopted

One addition behind one new feature, `yaml`, and a clean extension of the error type. No behaviour of a tree changes.

1. **Feature `yaml`** = `["serde", "dep:serde_yaml_ng"]`, `serde_yaml_ng = "0.10"`, the version the editor uses (the
   issue's own words), so a game that depends on both resolves one copy. It is independent of `ron`: a game that
   reads YAML does not compile RON, which is what the issue asks ("the editor no longer enables the `ron` feature").
2. **`TreeParams::to_yaml()` and `TreeParams::from_yaml(&str)`**: exactly `serde_yaml_ng::to_string` and
   `serde_yaml_ng::from_str` of the serde derives, no `singleton_map` or other attribute, so the text equals what the
   editor writes byte for byte (it calls `serde_yaml_ng::to_string` on the same type) and a file either side
   wrote reads on the other. Fields left out take their `Default` and an unknown field is an error that names it,
   as in RON (`#[serde(default, deny_unknown_fields)]` on every struct).
3. **`TreeParamsYamlLoader`**, an `AssetLoader` for extension `tree.yaml` registered by the plugin beside
   `TreeParamsLoader` (the RON one, unchanged), reading the file as UTF-8 and calling `from_yaml`. One extension: the
   editor writes `.tree.yaml`; `.tree.yml` is not claimed (a second spelling is a second path).
4. **`TreeFileError`** is the error of both loaders and of `from_ron` / `from_yaml`: its `Problem` gains a `Yaml`
   variant holding `serde_yaml_ng::Error`, whose message names the line and column and, for a misspelt field, the
   fields that exist, and an `EmptyYaml` variant (point 6). The type is exported under either feature (it was under
   `ron` only).
5. **What a file may say is bounded as before.** Nothing in the loader is new here: a `.tree.yaml` that says
   `trunk: {height: .nan}` or `1e9` reaches the generator as a `TreeParams` that `sanitized()` holds in range
   (`hostile-parameters.md`). A YAML-specific attack (an alias bomb, a deeply nested document) never reaches the
   parser's limits (`RepetitionLimitExceeded`, `RecursionLimitExceeded` in `serde_yaml_ng`'s `de.rs`), because a typed
   `TreeParams` refuses the first value that is not a field's type (measured: both texts are `invalid type: sequence,
   expected struct TrunkParams at line 1 column 8`); the test checks that such text is an `Err` at once.
6. **A text with no document is an error, not a tree of all defaults** (found by the first test run: `serde_yaml_ng`
   reads `""` as `TreeParams::default()`, because every struct has `#[serde(default)]`). `from_yaml` reads
   `Option<TreeParams>` and refuses `None` (nothing, whitespace, comments, `null`, `~`); `{}` is the all-defaults tree.
   An empty `.tree.ron` is an error already, and the reason is the hot reload: an editor that truncates and then
   writes lets a file watcher read the empty file between the two, and a default tree would replace the tree on
   screen for a frame. The writer is the editor and the reader is this crate, so the refusal is this crate's.

## Parameters

None. No default, preset literal or `Default` impl changes. New: Cargo feature `yaml`; dependency `serde_yaml_ng`.

## Validation targets

- `from_yaml(&p.to_yaml()) == p` for all 20 presets, and for the hull spruce (an enum with a struct variant, which YAML
  writes as a `!Hull` tag), so the tag form round-trips; no float changes by one bit (`serialize_f32` writes ryu's
  shortest round-trip text, `.nan` and `.inf` for the others).
- A file may leave fields out (`seed: 7` and `trunk: {height: 9.0}`) and may not misspell them: the error names the
  field and a line.
- A `.tree.yaml` loaded through Bevy's asset server (a real `AssetServer` and file, an app with `FloraPlugin`) becomes a
  `Handle<TreeParams>` equal to the preset it was written from, the `ProceduralTreeAsset` tree follows it, and the
  existing `ron` path still loads `.tree.ron` in the same app (both features on).
- `examples/assets/trees/maple.tree.yaml`, shipped with the examples, parses to the same `TreeParams` as
  `maple.tree.ron` (one format cannot drift from the other; it is generated from the RON file, not from the preset:
  the RON example is an older export and is not equal to `TreePreset::Maple.params()`, and a test that pinned the
  YAML to the preset would fail on every preset change).
- A hostile text is an `Err` or a tree `sanitized()` accepts, never a panic or a hang: `.nan`, `1e9`, `-1`, an alias
  bomb (40 levels of doubling aliases), 10 000 levels of nesting, an empty text, a null, two documents, a sequence
  where a struct goes, a NUL, each in under two seconds together.
- An empty text, whitespace, a comment, `null`, `~` and `---` are errors that say "empty"; `{}` is the default tree.
- The build is unchanged without the feature: `cargo build` and `cargo build --features ron` do not pull
  `serde_yaml_ng`; the forest bench and the gallery are untouched (no tree-building file is edited), the median of
  three forest benches before and after within noise.

## What great looks like

`App::add_plugins(FloraPlugin)` with `features = ["yaml"]` and
`commands.spawn(ProceduralTreeAsset(asset_server.load("trees/old_oak.tree.yaml")))` draws the tree the editor saved,
and saving the file in the editor rebuilds it in the running game (with `file_watcher`), exactly as `.tree.ron` does.
A misspelt field is a sentence with a line number, not a silent default. A caller that holds the text calls
`TreeParams::from_yaml(text)?`. The README says so in one line and the guide shows the file.

## Deviations

None of a default or a preset. `to_ron`, `from_ron` and `TreeParamsLoader` are unchanged. Two design defaults, stated in
the docs: YAML tags the variants of the crown and trunk style enums (`crown: !Hull {tiers: 5, ...}`), where RON writes
`Hull(...)`, because that is how `serde_yaml_ng` writes an enum and what the editor's files contain, and a file with
the singleton-map form is not claimed; and `from_yaml` refuses a document-less text where `serde_yaml_ng::from_str`
alone would read the defaults (point 6), a stricter reading than a bare `serde_yaml_ng` call, for the hot reload's sake.

## Pending index

- Unity's and Unreal's pages and the `serde_yaml_ng` documentation are web pages, not in the corpus.
- No source in the corpus measures a text format's cost for a parameter file; a tree file is a few kilobytes read once
  per load, so the loader's cost is not on any frame this crate measures.
- Measured 2026-11-25, to settle a conflict between sources: `serde_yaml_ng` 0.10.0's crate page says "This version only
  support the YAML Specification 1.1", while `bevy_dark_factory`'s `Cargo.toml` says it "follows YAML 1.2, so only
  `true` and `false` are booleans". For a typed `bool` field the reader accepts `true`, `false`, `True` and `False`
  and refuses `yes`, `Yes`, `on`, `no` and `1` with `invalid type: string "yes", expected a boolean at line 1
  column 71`: the editor's description is the one that holds for this crate's types. The singleton-map form of an
  enum (`crown: {Hull: {...}}`) is refused (`invalid type: map, expected a YAML tag starting with '!'`); a unit
  variant reads as its name or as `!Name`; two documents are refused (`deserializing from YAML containing more than
  one document is not supported`).

## Assumed decisions

Assumed decision (owner may override), recorded on #95: (1) the feature is `yaml` and the loader `TreeParamsYamlLoader`
(not a rename of `TreeParamsLoader`, which would break `ron` users); (2) one extension, `.tree.yaml`; (3) an empty
file is an error rather than a tree of all defaults (point 6 of "Model adopted"); (4) the shipped example file is
`examples/assets/trees/maple.tree.yaml`, and the `tree_file` example keeps requiring `ron` (Cargo has no "either
feature" for `required-features`) and reads a YAML tree when `yaml` is on as well.
