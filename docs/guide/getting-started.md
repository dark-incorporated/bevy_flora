# Getting started

The crate features, the plugin's own behaviour, presets by name, and a tree's growing time.

## Install

```toml
[dependencies]
bevy_flora = "0.48"
```

| feature | default | what it does |
|---|---|---|
| `textures` | yes | The bundled CC0 bark and leaf scans (about 3.6 MB of JPEG compiled into the crate). Without it every scan is absent: wood is plain colour and leaves keep their procedural outline |
| `serde` | — | `Serialize` / `Deserialize` on every parameter type and on `Scatter`, so trees and forests can be authored in RON, YAML or JSON. A tree's fields may be left out (they keep their defaults); an unknown field is an error |
| `ron` | — | `serde`, plus the `.tree.ron` asset loader and `TreeParams::to_ron` / `from_ron` (see [Trees from files](growing.md#trees-from-files-proceduraltreeasset-and-treeron)) |
| `yaml` | — | `serde`, plus the `.tree.yaml` asset loader and `TreeParams::to_yaml` / `from_yaml`: the file the Cauldron saves a species in. Independent of `ron`; a game that reads YAML compiles no RON |
| `file_watcher` | — | Bevy's `file_watcher`: assets, `.tree.ron` and `.tree.yaml` trees included, reload when their files are saved. An app without an `assets` folder then logs a warning at startup |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_log`, `bevy_mesh`, `bevy_pbr`, and `jpeg` to decode the bundled textures), so it adds no windowing or audio to a build.

## What the plugin does on its own

`FloraPlugin` is the whole of it: `DefaultPlugins` then `FloraPlugin`, and the types are `ProceduralTree(params)` for a tree and `TreeAge` for its age (`f32::INFINITY`, the default, is the mature tree). Add `Growing { rate }` and the plugin advances the age every frame until the tree is mature.

- It keeps two child entities on each tree, wood and leaves, and rebuilds their meshes whenever `ProceduralTree` or `TreeAge` changes. A flowering tree gets a third, for its flowers.
- It sends a `TreeGenerated` message after each rebuild, and a `TreeMature` message the frame a growing tree reaches the age `TreeParams::mature_at` gives.
- The camera, the light, the terrain, the clock and the physics are the game's. The crate draws trees and answers questions about them; it does not own the world.

## Presets, and how long a tree takes to grow

A preset has a name for a command line, an editor or a data file: `"oak".parse::<TreePreset>()` (or `TreePreset::from_name`) is the inverse of `TreePreset::name`, exact and snake_case, so `"Oak"`, `"oak "` and the `serde` spelling `"DragonTree"` are not presets. Its error, `UnknownPreset`, names what was asked for and lists every preset — a sentence a command line can show as it is.

A tree's growing time is in its own growth units, and an oak and a palm do not take the same number of them:

```rust
use bevy_flora::TreePreset;

for preset in TreePreset::ALL {
    let params = preset.params();
    // The age `Growing` runs the tree to, and above which `TreeAge` changes nothing:
    let grown = params.mature_at();
    assert!(grown.is_finite() && grown > 0.0, "{}", preset.name());
}
```

`TreeParams::mature_at()` reads the skeleton and builds no mesh, so it is about eight times cheaper than `build_tree` over the 20 presets — cheap enough to call while a level loads.


## Without the plugin

`build_tree` is a pure function from parameters and age to CPU meshes and the aged skeleton, so you can bake trees offline, feed your own renderer, or build colliders from the skeleton:

```rust
use bevy_flora::{TreePreset, build_tree};

let params = TreePreset::Pine.params();
let tree = build_tree(&params, f32::INFINITY);
let wood = tree.trunk.clone().into_bevy_mesh();  // bevy::mesh::Mesh
let leaves = tree.foliage.into_bevy_mesh();
assert!(tree.flowers.is_empty()); // a flowering tree's flowers (`FoliageParams::blossom`); a pine has none
assert!(tree.skeleton.nodes.len() > 1);
```
