# What a parameter panel needs from `TreeParams`: every number's range, unit and default, and a checked edit

Iteration 14, 2026-11-25. No issue is open on this repository (`gh issue list`, 0 open; the search of the coven's and
Dark Incorporated's open issues for `flora` finds none that asks for this). The item is section 4's first fallback,
"what the crate's consumers hand-roll", and it is the item iteration 11 and 13 left at the top of their backlogs
(5 and 4: "per-field kind, unit and range for a panel ... and a dotted-path edit; wait for the requester to file it").
The requester has not filed it; what it writes today is read below and is general enough to build here.

## What the consumer does today

`bevy_dark_factory` (the editor, `crates/bevy_dark_factory/src/ops_tree.rs`, F6 `bevy-coven/bevy_grimoire#102`, closed)
registers one operation, `tree_edit`: a `preset` and a `set` map from a dotted path to a JSON value.

- `set_path` (`ops_tree.rs:73`) walks `serde_json::to_value(&TreeParams)` by dotted path and writes the value in, then
  `serde_json::from_value` turns it back into a `TreeParams`. It needs the `serde` feature, refuses a path that is not
  already a field with its own text, and **accepts any number**: `{"trunk.height": 1e9}` is written to the file and
  `TreeParams::sanitized` quietly draws a 500 m trunk; `{"branches.recursion": -1}` is refused by serde with a message
  that names no range. The file on disk and the tree on screen disagree and nothing says so.
- Nothing tells the editor which numbers exist, what range a field is held in (`limits.rs`'s table, private code), or
  what its unit is, so a panel (the family issue's "tune a `.tree.ron` and watch it regrow") has to guess a slider
  range per field, 87 times, and learn the crate's limits by hitting them.
- `TreePreset::ALL.iter().find(|t| t.name() == name)` is written there twice; `TreePreset::from_name` (iteration 11)
  replaces it once the editor moves its pin past `25e956b`.

## Sources

- Model: Weber and Penn 1995, "Creation and rendering of realistic trees", as the related-work passage of a later paper
  describes it (`10.1111_cgf.12744`, Computer Graphics Forum, indexed by chunk, lines 43-61); Lintermann and Deussen
  1999, "Interactive Modeling of Plants" (stem `10.1109_38.736469`, quoted in `tree-yaml.md`).
- Shipping-game tooling: Epic Games, "Metadata Specifiers in Unreal Engine" (<https://dev.epicgames.com/documentation/en-us/unreal-engine/metadata-specifiers-in-unreal-engine>,
  read 2026-11-25, not indexed); Unity's SpeedTree Modeler manual, "Branch generator properties" (<https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>,
  read 2026-11-25, not indexed).
- Technical: `bevy_reflect` 0.20.0, `src/attributes.rs` (custom attributes on a field, read from `TypeInfo`), read
  2026-11-25 in the cargo registry; `src/limits.rs` and `src/params.rs` of this crate; `serde_json`'s dotted-path walk in
  the consumer.

## Craft principle

A tree model is a handful of numbers a person turns, and the number of them is the cost of the tool. The survey that
describes Weber and Penn's method says: "The geometric method of Weber et al. [WP95] represents trees as collections of
narrow near-conical tubes. A tree structure is generated from transformations of the segments; a user adjusts
meaningful parameters, such as branching angles and segment lengths. Unfortunately, creating a new tree is cumbersome,
requiring adjustment of up to 80 parameters." This crate has 87 numbers plus the hull's and the prism's, so a person
tuning a species is in that position, and what makes it bearable is that each parameter is *meaningful*: it has a
unit (metres, degrees, a share of the trunk's height) and a range outside which it means nothing. Lintermann and
Deussen's workflow is the same loop: "The user edits a plant type's parameters and runs the simulation to produce the
desired geometry". A tool for that loop has to know, per parameter, what is a legal value.

## How shipping games do it

An engine's editor takes the legal range of a field from the field's declaration, not from the panel. Unreal's property
metadata: `ClampMin="N"` "Used for float and integer properties. Specifies the minimum value N that may be entered for
the property", with `ClampMax` for the other end, and the page's own warning, "Metadata only exists in the editor; do not
write game logic that accesses metadata." SpeedTree Modeler gives every branch property a `Clamp` group, "Min: Set the
minimum length of branches. Max: Set the maximum length of branches." The practice: the range and unit are declared once
beside the field, the panel and the validator read the declaration, and an out-of-range entry is stopped where it is
typed. The Unreal warning names the way this goes wrong: a range kept only for the editor is a second copy that the game
does not read, so the two drift. Here the generator already enforces the range (`TreeParams::sanitized`), so the
declaration the panel reads must be the one the generator uses, not a copy of it.

## Model adopted

One table in `src/limits.rs` is the single declaration of every number of a `TreeParams`: its dotted path, whether it
is a float or a count, its unit, the range the generator holds it in, and the value it reads as when it is `NaN`.
`TreeParams::sanitized` is rewritten to walk the table (the 87 `l.float(...)` and `l.count(...)` lines go), so the range
a panel shows is, by construction, the range the generator applies. The table is the public catalogue:

- `TreeParams::fields() -> &'static [ParamField]`: path, `ParamKind` (`Float` or `Count`), `ParamUnit`, `min`, `max`,
  `default`.
- `TreeParams::field(&self, path) -> Option<f32>`: the number now there; `None` for an unknown path or for a part the tree
  does not have (`foliage.fruit.radius` on a tree with no fruit, `crown.Hull.tiers` on a tree without a hull).
- `TreeParams::set_field(&mut self, path, value) -> Result<(), ParamError>`: refuses what `sanitized` would have to
  change. An unknown path (with the paths nearest to it), a part the tree does not have, a `NaN` or infinite value, a
  value outside the range (naming it and its unit), a count that is not a whole number. A refusal changes nothing.

The paths are the dotted paths of the serde form (`trunk.height`, `branches.strands.density`, `foliage.fruit.radius`,
`crown.Hull.tiers`, `trunk_style.Prism.sides`), so the editor's existing `{"trunk.radius": 0.6}` maps are the same keys.
Colours, enums, flags, the seed and the leaf and bark scans are not numbers in a range; they stay with serde and
reflection, which already give a panel their variants.

## Parameters

`ParamUnit`: `Metres`, `Degrees`, `PerMetre` (`branches.density`, `branches.strands.density`), `Age` (`growth.maturity_age`: `TreeAge`
counts "growth units", not years, as its doc says; the first draft called it `Years`) and `None` for a share, ratio, exponent or multiplier (the field's doc says of what). Hard limits only: the `min` and
`max` are `limits.rs`'s existing ranges, unchanged, wide by design (more than a factor of ten over the presets).
No default of any `TreeParams` field, no preset and no range moves.

## Validation targets

1. The table's paths are exactly the numeric fields found by reflection (`limits::fields`, a derivation that shares
   nothing with the table), with the fruit, hull and prism parts present, colour channels left out.
2. For every field, `field(path)` equals the reflected value at that path, for all 20 presets: each row's accessor
   reaches the field its path names. Every preset's value lies inside its row's range; `default` equals the reflected
   `TreeParams::default()` value (the explicit ones for the fruit, hull and prism parts).
3. With `serde`, the dotted path finds the same number in `serde_json::to_value(params)`: the keys are the editor's.
4. `sanitized()` returns the same parameters as before for every field at every hostile value (a throwaway
   differential against the previous implementation, run once, then deleted) and keeps its tests; its cost per call
   (94 to 110 ns before, `CHANGELOG.md`) stays within noise of a build that costs milliseconds.
5. `set_field` refusals each name the field and the range; success reads back through `field`; a refusal leaves the
   parameters unchanged.

## Measured findings (iteration 14)

- The reflection helper names an enum's fields without the variant (`.crown.tiers`, not `.crown.Hull.tiers`) and reads an
  optional part through its tuple field (`.foliage.fruit.0.radius`, not `.Some.0.`), and a colour is `Color::Srgba`'s
  tuple field `0` and then `red`, with no colour-space name in the path: the first draft of the tests assumed the
  serde-style paths and a space name, which reading `fields::numeric` showed were wrong before any run. The tests map
  the catalogue's serde-style paths to reflection's (`reflected`, `catalogued`) and name the six colour fields, so a
  seventh colour or a number of a new field fails `the_catalogue_lists_every_number_of_a_tree_and_only_those`.
- 92 numbers: 87 from the old `apply_limits` (two of them the fruit's), 4 of the hull crown, 1 of the prism trunk.
- Differential against the previous `apply_limits` (copied into a throwaway test, deleted): 40 166 cases (every numeric
  field of the 20 presets and of a birch with fruit, autumn, blossom, a hull and a prism, each at 19 hostile values:
  `NaN`, `inf`, `-inf`, `-1`, `1e30`, `-1e30`, `0`, `-0`, `1`, `0.5`, `2`, `63`, `65`, `255`, `65535`, `u32::MAX`, `1e-30`,
  `7199`, `7201`), the same tree and the same "moved" answer in every one.
- Cost of `sanitized()` (best of 20 x 50 000 calls, release, Apple M5, a clone included): the oak 277 ns against 219 ns
  for clone and the old `apply_limits`; a tree with every part 223 against 141 ns. A `build_tree` makes two calls
  (`pipeline.rs:34`, `skeleton_mesh`) and takes 29 µs (the dead tree) to 11.7 ms (the willow at maturity): at most 0.6%
  of the cheapest, and the forest bench does not call it per frame.

## What great looks like

An editor builds a slider for every number from `TreeParams::fields()` alone, labelled with its unit, and a typed
`1e9` in the height box is refused with "`trunk.height` is held between 0 m and 500 m; 1000000000 is out of range"
before anything is written; `tree_edit` shrinks to a loop over `set` that calls `set_field` for numbers. A path typed
wrong answers with the paths it is near. The same table keeps the generator's limits and the panel's in step because
there is only one.

## Deviations

None to a default, a preset or a range. The catalogue's range is the hard limit, not a "sensible" slider range (Unreal's
`UIMin`/`UIMax` against `ClampMin`/`ClampMax`): a panel would want both, and a slider range derived from the presets is
a design question left for the editor to ask for (recorded in the log's backlog).

## Pending index

The three web sources above (Epic's metadata specifiers, Unity's SpeedTree manual page, the `bevy_reflect` source) are
not in the corpus; `docs/research/corpus.md` lists them. `10.1111_cgf.12744` is indexed by chunk but has no catalog entry.

## Assumed decisions

Owner may override; there is no issue to record them on, so they are in the commit body and the log. (1) **Numbers only**:
enums, colours and flags stay with serde; options considered: a full value type (`ParamValue::{Float, Count, Bool,
Choice, Color}`) with a reflection-driven `set`, which is a second editing path beside the editor's serde one and the
largest option, or a table of numbers only. Chosen: numbers, the one part the crate alone knows (its limits).
(2) **`f32` for every value, a count included** (counts top out at 256, exact in an `f32`), so a panel has one number
type. (3) **Names**: `ParamField`, `ParamKind`, `ParamUnit`, `ParamError`, `TreeParams::{fields, field, set_field}`.
(4) **The seed is not in the table**: it has no range (any `i32`).
