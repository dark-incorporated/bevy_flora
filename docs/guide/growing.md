# Growing a tree over time

How a tree changes with age, from the CPU rebuild to the vertex shader, and how to keep those rebuilds off the frame.

## Trees from files: `ProceduralTreeAsset` and `.tree.ron`

`TreeParams` is an asset. Spawn `ProceduralTreeAsset(handle)` in place of `ProceduralTree(params)` and the plugin gives the entity its `ProceduralTree` once the asset has loaded, and replaces it whenever the asset changes, so every tree that shares the handle follows an edit, made in code (`Assets<TreeParams>::get_mut`), in an inspector (the asset is registered for reflection) or in a file. With the `ron` feature the plugin loads `.tree.ron` files, and with Bevy's `file_watcher` (this crate's `file_watcher` feature turns it on) saving one rebuilds its trees while the game runs, a few tenths of a second after the save: tune a tree without recompiling. `TreeParams::to_ron` writes any tree as a file to start from (`cargo run --example tree_file --features ron -- --export oak`). A file may leave fields out, which keep their defaults; a misspelt field is an error that gives the line and the fields that exist; and a file that fails to load keeps the last good tree.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, ProceduralTreeAsset, TreeParams, TreePreset};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // assets/trees/maple.tree.ron, written with `TreePreset::Maple.params().to_ron()` and edited.
    let maple: Handle<TreeParams> = assets.load("trees/maple.tree.ron");
    for x in [-8.0, 0.0, 8.0] {
        commands.spawn((ProceduralTreeAsset(maple.clone()), Transform::from_xyz(x, 0.0, 0.0)));
    }
}
```

![The maple preset as exported to a tree file, and the same file with three values changed: a fuller, lighter crown](../media/tree_file.jpg)

*`cargo run --release --example tree_file --features ron` (add `file_watcher` to rebuild on save): left, `examples/assets/trees/maple.tree.ron` as exported; right, the same file with the branch density raised from 3 to 7, the leaf density from 4.5 to 7 and the leaf colour lightened.*

Pools are built from a value (`build_pool(assets.get(&handle).unwrap(), ..)`), so a pool does not follow later edits to the asset.

### A file can say anything: `TreeParams::sanitized`

A `.tree.ron` is typed by a person, and an editor's number field passes through every value on the way to the one meant: `-`, `1e`, `1e6`, `NaN`. Every numeric field of `TreeParams` has a range, and the tree is built from the parameters held inside it: `TreeParams::sanitized()` reads a `NaN` as the field's default and any other value outside its range as the nearest end (a trunk `height` of `1e9` is 500 m, `branches.start` of `-1` is 0, `radial_segments` of `u32::MAX` is 64). The ranges are wide, at least ten times what the presets and the gallery's extreme scenarios use, so a preset is never touched and `sanitized()` borrows it as it is. `build_tree`, `build_lods`, `generate_skeleton` and `skeleton_mesh` do it for you, and `FloraPlugin` writes the result back into a `ProceduralTree` that is added or changed (and reads a `TreeAge` of `NaN` as 0), so the component shows the tree that is drawn; an editor that wants the value in force asks `params.sanitized()`.

What the ranges allow can still multiply to more wood or leaves than any tree has, so a tree has at most `MAX_NODES` (5 000) branches and `MAX_LEAVES` (60 000) leaves: generation stops adding wood at the first and drops the last nodes (the top of the crown) to fit the second. The 20 presets have at most 588 nodes and 4 630 leaves, the gallery's 40 m giant oak 893 nodes and about 21 000. The worst tree the ranges allow, a 500 m trunk with 5 000 branches or one with 20 000 leaves, builds in about a second and about 300 MB on an Apple M5, where unbounded parameters hung the build or ran the machine out of memory.

## Growing many trees: `GpuGrowth`

A growing tree normally rebuilds both meshes every frame, 0.1 to 0.4 ms of CPU per tree. Add `GpuGrowth` and the tree is built once, at maturity, and grown in the vertex shader: each frame the plugin ages the skeleton and writes a few kilobytes of per-node state, and nothing else is uploaded.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, GpuGrowth, Growing, ProceduralTree, TreeAge, TreePreset};

fn forest(mut commands: Commands) {
    for i in 0..200 {
        commands.spawn((
            ProceduralTree(TreePreset::Oak.params()),
            TreeAge(0.0),
            Growing::default(),
            GpuGrowth,
            Transform::from_xyz((i % 20) as f32 * 8.0, 0.0, (i / 20) as f32 * 8.0),
        ));
    }
}
```

Main-world cost per frame with every tree growing (a headless `App`, release build, Apple M-series, median of three runs):

| 200 growing trees | CPU rebuild | `GpuGrowth` |
|---|---|---|
| oak | 17.6 ms | 0.81 ms |
| willow | 25.4 ms | 0.90 ms |
| oak, 2 smoothing passes | 69.8 ms | 0.70 ms |

Shadows follow the growth (the prepass shares the vertex shader), and the grown frames match the CPU path: over 526 gallery scenarios the mean difference between the two renders is 0.03 of 255 (83 PNGs are byte-identical, the worst is a young willow at 1.16), and every leaf of an `AllBranches` tree is where the CPU tree puts it, at the size the CPU tree gives it. What differs, and why, is listed on `GpuGrowth` in the docs: smoothing is computed once on the mature mesh and deformed, side counts stay at the mature count, and the `Mesh` assets (picking, physics, culling) are the mature tree. It needs storage buffers in the vertex stage, so not WebGL2; there the trees grow on the CPU.

## Planting without a hitch: `BuildInBackground`

A tree is built in the frame it is spawned (or its `ProceduralTree` changes), on all cores, and that frame waits for it: a game that plants a chunk of trees as the player walks feels it. Add `BuildInBackground` and the tree is built on Bevy's async compute pool instead; the frame goes on, and the tree appears the frame its build is done, usually a few later (`TreeGenerated` is sent then; until its first build a tree has no `TreeRender` and draws nothing). The meshes are the same: a screenshot of a forest built either way is the same image, byte for byte.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{BuildInBackground, ProceduralTree, TreePreset};

// A streamed chunk of the world comes in: plant its trees without stalling the frame.
fn plant_chunk(commands: &mut Commands, spots: &[Vec3]) {
    for (i, &at) in spots.iter().enumerate() {
        let mut oak = TreePreset::Oak.params();
        oak.seed = i as i32;
        commands.spawn((ProceduralTree(oak), BuildInBackground, Transform::from_translation(at)));
    }
}
```

The frame the trees are spawned in, measured in a headless `App` that does 8 ms of other work a frame (release build, Apple M4 Pro); the GPU uploads of their meshes come on top, in the frames their builds land:

| spawned in one frame | built in the frame | `BuildInBackground`: worst frame | all drawn after |
|---|---|---|---|
| 10 oaks | 4.0 ms | 0.3 ms | 2 frames |
| 50 oaks | 8.2 ms | 0.4 ms | 2 frames |
| 200 oaks | 22.8 ms | 0.5 ms | 3 frames |
| 200 cherries | 37.7 ms | 0.5 ms | 5 frames |

A change of parameters while a build runs starts a new one in its place (the tree keeps its old meshes until the new ones are in), and a change of age waits for it and is caught up after. A `Growing` tree is rebuilt in the background at every age too, so it shows each age a frame or two late; `GpuGrowth` trees build their mature meshes in the background and age in the frame, as they do without it. `forest --background` builds its trees this way.

## Distant trees for less: `TreeLod`

A `ProceduralTree` is drawn in full at every distance: an oak 150 m away costs as many vertices as the one beside the camera. Add `TreeLod` and it is drawn the way a pool draws its trees, from the same levels (`build_lods`): in full up close; from 30 m (`LodDistances::lod1`) without the branches thinner than 1.5 cm and with a third of the leaves, made larger to cover the same crown; from 80 m (`lod2`) with the limbs thicker than 5 cm and an eighth of the leaves; cross-fading over 2 m at each switch. There is no impostor: level 2 is drawn at every distance beyond `lod2`. The trees stay individual trees, which grow, change their parameters, keep their own looks and fall.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{ProceduralTree, TreeLod, TreePreset};

fn plant(mut commands: Commands) {
    for i in 0..1000u32 {
        let at = Vec3::new((i % 40) as f32 * 8.0, 0.0, (i / 40) as f32 * 8.0);
        commands.spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default(), Transform::from_translation(at)));
    }
}
```

![One oak at its three levels of detail, zoomed to the same size: the full tree, then fewer and larger leaves on the limbs and branches, then the limbs alone with a few large leaves](../media/tree_lod.jpg)

*The same oak at level 0 (25 m), level 1 (33 m) and level 2 (91 m), each zoomed to the same size.*

Measured with `forest --mode naive --offscreen --deterministic` (1920x1080, Apple M4 Pro, interleaved runs of 0.23.0):

| | without `TreeLod` | with `TreeLod` |
|---|---|---|
| 1 000 oaks, fully grown | 24.4 ms | 6.5 ms |
| 200 oaks growing on the CPU (`--mode cpu-live`) | 10.1 ms | 14.1 ms |

Every rebuild builds the coarse levels too, from the same skeleton, so a tree growing on the CPU pays for them with every new age; a `GpuGrowth` tree ignores `TreeLod`. Each coarse level adds two children to the tree, a wood and a leaf child (marked `plugin::TreeLodMesh`), drawn with the tree's own materials; a flowering tree's flowers ride in their leaves, as on a pool's. A felled tree drops them at the cut and falls in full. `forest --tree-lod` draws its trees this way, and `inspect --mode tree --lod a,b,c` photographs one through the switches.
