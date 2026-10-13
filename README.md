# bevy_flora

> ⚠️ **Vibe Coded.** This crate was written by an AI agent working from a human's direction. It is covered by tests and by a rendered visual QA loop (`qa/visual_findings.md` in the repository), but it has had no line-by-line human audit. Read it before you trust it.

[![crates.io](https://img.shields.io/crates/v/bevy_flora.svg)](https://crates.io/crates/bevy_flora) [![docs.rs](https://img.shields.io/docsrs/bevy_flora)](https://docs.rs/bevy_flora) [![CI](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml/badge.svg)](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Ladvien/bevy_flora/blob/main/LICENSE-MIT)

**Parametric, stylized procedural trees for Bevy that grow over time.** Pick one of 18 presets (17 trees and a shrub) or tune every parameter, spawn a component, and watch a seedling grow into a mature tree. The same seed always gives the same tree.

![A maple growing from a bare trunk into a full crown while the camera circles it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif)

*`cargo run --release --example capture_growth`: the maple preset from age 0 to maturity, one frame per age step, in a light wind.*

![A grove of eight trees of eight species growing in turn while the camera circles them and a gust of wind sweeps through](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/grove.gif)

*`cargo run --release --example capture_growth -- --grove`: a grove of an oak, a maple, a Japanese maple, a birch, a cherry, a pine, a cypress and a willow, each at its own age, with a gust front crossing it in the last seconds. Every tree is a `ProceduralTree` with its own seed.*

![The same grove fully grown through a year: the leaves turn red, orange and gold and fall until the broadleaf trees stand bare beside the green pine and cypress, snow settles on every tree and melts, the cherry blossoms pink on its bare branches, and the leaves come back green](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/seasons.gif)

*`cargo run --release --example capture_growth -- --seasons`: the grove from late summer through autumn, leaf fall, snow and winter into spring, driven by `FloraSeason::of_year`. The deciduous presets carry an autumn colour, the cherry blossoms before its leaves come back, and the pine and the cypress are evergreen.*

## Compatibility

| `bevy_flora` | `bevy` | Rust |
|---|---|---|
| 0.35 | 0.20 | 1.97.1+ (edition 2024) |

## Install

```toml
[dependencies]
bevy_flora = "0.35"
```

| feature | default | what it does |
|---|---|---|
| `textures` | yes | The bundled CC0 bark and leaf scans (about 3.6 MB of JPEG compiled into the crate). Without it every scan is absent: wood is plain colour and leaves keep their procedural outline |
| `serde` | — | `Serialize` / `Deserialize` on every parameter type and on `Scatter`, so trees and forests can be authored in RON or JSON. A tree's fields may be left out (they keep their defaults); an unknown field is an error |
| `ron` | — | `serde`, plus the `.tree.ron` asset loader and `TreeParams::to_ron` / `from_ron` (see [Trees from files](#trees-from-files-proceduraltreeasset-and-treeron)) |
| `file_watcher` | — | Bevy's `file_watcher`: assets, `.tree.ron` trees included, reload when their files are saved. An app without an `assets` folder then logs a warning at startup |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_log`, `bevy_mesh`, `bevy_pbr`, and `jpeg` to decode the bundled textures), so it adds no windowing or audio to a build.

## Quickstart

Add the plugin and spawn a tree. `Growing` advances its age every frame until it is mature; leave it off and set `TreeAge` yourself (the default, `f32::INFINITY`, is the mature tree).

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreePreset};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    let mut params = TreePreset::Maple.params();
    params.seed = 7;
    commands.spawn((ProceduralTree(params), TreeAge(0.0), Growing { rate: 1.0 }));

    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 20.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Y),
    ));
}
```

The plugin keeps two child entities on each tree (wood and leaves) and rebuilds their meshes whenever `ProceduralTree` or `TreeAge` changes. It sends a `TreeGenerated` message after each rebuild.

### Trees from files: `ProceduralTreeAsset` and `.tree.ron`

`TreeParams` is an asset. Spawn `ProceduralTreeAsset(handle)` in place of `ProceduralTree(params)` and the plugin gives the entity its `ProceduralTree` once the asset has loaded, and replaces it whenever the asset changes, so every tree that shares the handle follows an edit, made in code (`Assets<TreeParams>::get_mut`), in an inspector (the asset is registered for reflection) or in a file. With the `ron` feature the plugin loads `.tree.ron` files, and with Bevy's `file_watcher` (this crate's `file_watcher` feature turns it on) saving one rebuilds its trees while the game runs, a few tenths of a second after the save: tune a tree without recompiling. `TreeParams::to_ron` writes any tree as a file to start from (`cargo run --example tree_file --features ron -- --export oak`). A file may leave fields out, which keep their defaults; a misspelt field is an error that gives the line and the fields that exist; and a file that fails to load keeps the last good tree.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

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

![The maple preset as exported to a tree file, and the same file with three values changed: a fuller, lighter crown](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/tree_file.jpg)

*`cargo run --release --example tree_file --features ron` (add `file_watcher` to rebuild on save): left, `examples/assets/trees/maple.tree.ron` as exported; right, the same file with the branch density raised from 3 to 7, the leaf density from 4.5 to 7 and the leaf colour lightened.*

Pools are built from a value (`build_pool(assets.get(&handle).unwrap(), ..)`), so a pool does not follow later edits to the asset.

### Growing many trees: `GpuGrowth`

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

Shadows follow the growth (the prepass shares the vertex shader), and the grown frames match the CPU path: over all 357 gallery scenarios the mean difference between the two renders is 0.07 of 255 (48 PNGs are byte-identical, the worst is a toon maple at 1.31). What differs, and why, is listed on `GpuGrowth` in the docs: smoothing is computed once on the mature mesh and deformed, side counts stay at the mature count, `AllBranches` leaves keep their mature positions, and the `Mesh` assets (picking, physics, culling) are the mature tree. It needs storage buffers in the vertex stage, so not WebGL2; there the trees grow on the CPU.

### Planting without a hitch: `BuildInBackground`

A tree is built in the frame it is spawned (or its `ProceduralTree` changes), on all cores, and that frame waits for it: a game that plants a chunk of trees as the player walks feels it. Add `BuildInBackground` and the tree is built on Bevy's async compute pool instead; the frame goes on, and the tree appears the frame its build is done, usually a few later (`TreeGenerated` is sent then; until its first build a tree has no `TreeRender` and draws nothing). The meshes are the same: a screenshot of a forest built either way is the same image, byte for byte.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

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

### Distant trees for less: `TreeLod`

A `ProceduralTree` is drawn in full at every distance: an oak 150 m away costs as many vertices as the one beside the camera. Add `TreeLod` and it is drawn the way a pool draws its trees, from the same levels (`build_lods`): in full up close; from 30 m (`LodDistances::lod1`) without the branches thinner than 1.5 cm and with a third of the leaves, made larger to cover the same crown; from 80 m (`lod2`) with the limbs thicker than 5 cm and an eighth of the leaves; cross-fading over 2 m at each switch. There is no impostor: level 2 is drawn at every distance beyond `lod2`. The trees stay individual trees, which grow, change their parameters, keep their own looks and fall.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn plant(mut commands: Commands) {
    for i in 0..1000u32 {
        let at = Vec3::new((i % 40) as f32 * 8.0, 0.0, (i / 40) as f32 * 8.0);
        commands.spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default(), Transform::from_translation(at)));
    }
}
```

![One oak at its three levels of detail, zoomed to the same size: the full tree, then fewer and larger leaves on the limbs and branches, then the limbs alone with a few large leaves](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/tree_lod.jpg)

*The same oak at level 0 (25 m), level 1 (33 m) and level 2 (91 m), each zoomed to the same size.*

Measured with `forest --mode naive --offscreen --deterministic` (1920x1080, Apple M4 Pro, interleaved runs of 0.23.0):

| | without `TreeLod` | with `TreeLod` |
|---|---|---|
| 1 000 oaks, fully grown | 24.4 ms | 6.5 ms |
| 200 oaks growing on the CPU (`--mode cpu-live`) | 10.1 ms | 14.1 ms |

Every rebuild builds the coarse levels too, from the same skeleton, so a tree growing on the CPU pays for them with every new age; a `GpuGrowth` tree ignores `TreeLod`. Each coarse level adds two children to the tree, a wood and a leaf child (marked `plugin::TreeLodMesh`), drawn with the tree's own materials; a flowering tree's flowers ride in their leaves, as on a pool's. A felled tree drops them at the cut and falls in full. `forest --tree-lod` draws its trees this way, and `inspect --mode tree --lod a,b,c` photographs one through the switches.

### Drawing a forest: `TreePool`

`ProceduralTree` gives every tree its own meshes, which a GPU can draw a few thousand of. Trees with equal looks (the same colours, trunk height, wind response and leaf shading; seeds and every other parameter may differ) share their two materials, so a thousand oaks cost 22 ms a frame, not 58; give each tree its own colour or trunk height and each gets its own materials again. For a forest build a `TreePool` once, a few seed variants of one tree, and place `PooledTree`s from it. Entities that share a mesh and a material are drawn in one instanced call, and the distance to the camera picks what is drawn: the full tree up close, two coarser levels of detail (30 m and 80 m by default, cross-fading) and, beyond 200 m, one camera-facing quad per tree that shows a view of the variant baked in the running app.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, LodSettings, PooledTree, TreePool, TreePreset, build_pool};

fn forest(
    mut commands: Commands,
    mut pools: ResMut<Assets<TreePool>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let pool = build_pool(
        &TreePreset::Oak.params(),
        8, // variants: seeds 1, 2, ... 8
        f32::INFINITY,
        &LodSettings::default(),
        &mut meshes,
    );
    let pool = pools.add(pool);
    for i in 0..100_000u32 {
        commands.spawn((
            PooledTree { pool: pool.clone(), variant: i },
            Transform::from_xyz((i % 316) as f32 * 8.0, 0.0, (i / 316) as f32 * 8.0),
        ));
    }
}
```

Measured for 0.6.1 with `cargo run --release --example forest -- --offscreen` (1920x1080, one shadow-casting sun, wind on, an oak forest on an 8 m grid seen across it from 30 m up, Apple M4 Pro, release build; the median of three runs on a machine with nothing else running, the runs within 1% of each other; the larger forests are not run with a mesh per tree, which would take an estimated 15 and 75 GB of GPU memory):

| trees | one `ProceduralTree` each | pool, levels of detail only | pool, with impostors |
|---|---|---|---|
| 1 000 | 22 ms | 4.2 ms | 3.3 ms |
| 5 000 | 60 ms | 8.0 ms | 3.3 ms |
| 20 000 | not run | 18.7 ms | 3.4 ms |
| 100 000 | not run | 64.7 ms | 7.3 ms |

Bark (0.7.0) and leaf scans (0.8.0) leave these figures where they were, within what could be measured: the machine these were taken on has been loaded by other builds since, so the table above was not re-measured, and the releases were compared by interleaved runs instead (see the `CHANGELOG`). In those, 100 000 pooled oaks cost no more in 0.8.0 than in 0.7.0 (0.6.2 and 0.7.0 took 7.41 and 7.45 ms), and a thousand individual `ProceduralTree` oaks cost 4 to 10% more, because a leaf card cut to a real outline has more vertices than the old fan. The looks of 0.9.0 do not move them: the natural look draws the same pixels as 0.8.0 with the same shaders, and in interleaved runs 100 000 pooled oaks took 12 to 16 ms in all three looks on a machine under load, a thousand individual ones 31 to 48. Seasons (0.11.0) add under 1%: 100 000 pooled oaks took 7.33 ms in 0.10.0 and 7.40 in 0.11.0, a thousand individual oaks 23.4 and 23.6 ms (medians of three interleaved runs on a quiet machine), and half-bare trees are cheaper (22.1 ms).

The pool's trees are static: they have the age the pool was built at, and growing trees stay `ProceduralTree` (with `GpuGrowth`), which suits the few that are close. Impostors are baked once per variant by a few frames of offscreen rendering, so a tree appears when that is done, and they need `FloraPlugin` added after `DefaultPlugins` (before it, with no renderer, or on a device without storage buffers in the vertex stage such as WebGL2, the coarsest level is drawn at every distance and one warning says so). They are baked unlit, as a neutral grey colour picture (a pool's looks tint it, below) and a surface picture, and shaded by the scene's own lights every frame: they follow the time of day, glow with the sun behind them and sway with the wind. They cast no shadows, and they show one of eight views, at one elevation. With MSAA an impostor keeps the gaps of its crown: each pixel draws the share of its samples the baked tree covers, so a far crown is as airy, and as light, as the leaves it stands for. Level 2 and the impostors replace thin twigs and most leaves, so a forest seen from above or close up should keep its near trees at level 0.

Shadows are a large part of a forest's cost: every shadow cascade draws every tree that casts into it. Put `NotShadowCaster` on a `PooledTree` (or a `ProceduralTree`) and none of what draws it casts; take it off and it casts again, so a game can keep the shadow maps to the trees near the player, re-marking them as the player moves. `forest --shadow-radius 150` lets only the trees within 150 m of the camera cast: 1 000 individual oaks take 20.3 ms a frame instead of 24.6, 100 000 pooled oaks 7.2-7.3 ms instead of 7.5 (0.35.0, Apple M4 Pro, `--offscreen --deterministic`).

#### One forest, many looks: `PoolColors`

A pool can be drawn in more than one colouring without a second set of meshes. `TreePool::looks` starts with look 0, the colours of the parameters; `add_look` adds more (before the pool goes into the assets), and a `PooledTree` picks one with `PooledLook(n)`. Three looks of one forest draw the same meshes with three pairs of materials, impostors included: they are baked neutral grey and take a look's colours as a tint. A look can be `unlit`: its wood, leaves, impostors and cut faces then show their colour as it is, with no lighting, shadows or light through the leaves (the rain, the snow and the fire still mark it), for a map, a diagram or a dream. `RecolorPool` changes a look's colours at runtime, every tree in it at once, by writing the pool's materials in place where a value changed (the pool asset is never touched, no mesh is rebuilt, so a palette that glides through the day costs a few material writes a frame); scans keep their pattern and the autumn and blossom colours follow the new leaf colour. A pooled tree that is felled keeps its look (`TreePool::tree_params_for`), and `RecolorTree` recolours it, its stump, its cut faces and its logs afterwards.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn looks(
    mut commands: Commands,
    mut pools: ResMut<Assets<TreePool>>,
    mut meshes: ResMut<Assets<Mesh>>,
) -> Handle<TreePool> {
    let mut pool = build_pool(&TreePreset::Oak.params(), 8, f32::INFINITY, &LodSettings::default(), &mut meshes);
    // Look 0 is the oak's own colours; the others are added before the pool goes into the assets.
    let dusk = pool.add_look(PoolColors { wood: Color::srgb(0.20, 0.10, 0.10), leaves: Color::srgb(0.40, 0.15, 0.30), unlit: false });
    let map = pool.add_look(PoolColors { wood: Color::BLACK, leaves: Color::srgb(0.10, 0.80, 0.40), unlit: true });
    let pool = pools.add(pool);
    commands.spawn((PooledTree { pool: pool.clone(), variant: 3 }, PooledLook(dusk)));
    commands.spawn((PooledTree { pool: pool.clone(), variant: 4 }, PooledLook(map)));
    pool
}

// Later: dawn. Every tree in look 1 follows, and so does every impostor of it.
fn dawn(pool: &Handle<TreePool>, mut recolor: MessageWriter<RecolorPool>) {
    let colors = PoolColors { wood: Color::srgb(0.35, 0.22, 0.12), leaves: Color::srgb(0.55, 0.60, 0.25), unlit: false };
    recolor.write(RecolorPool { pool: pool.id(), look: 1, colors });
}
```

`forest --looks` draws a forest in three looks side by side (the oak's own, a lit autumn one, a flat unlit one), and `--looks --recolor` recolours two of them after the warmup.

### Planting a forest: `Scatter`

A grid of trees reads as an orchard. `Scatter` spreads them the way a forest stands: trunks never closer than `spacing` and otherwise irregular (Poisson-disc sampling, Bridson 2007), species in groves about `grove_size` wide whose borders bend instead of salt and pepper, and every tree its own yaw, size (`scale`, 0.85 to 1.15 by default) and pool variant. `points_where` takes a density for each spot, 0 for a road, a lake or a village and a fraction for open woodland, and only ever removes trees, so carving a clearing never moves the trees around it. It is pure CPU and deterministic per seed: 90 000 trees in groves take 0.2 s (Apple M4 Pro), once.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn plant_forest(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut pools: ResMut<Assets<TreePool>>) {
    let lod = LodSettings::default();
    let species: Vec<Handle<TreePool>> = [TreePreset::Oak, TreePreset::Birch, TreePreset::Pine]
        .iter()
        .map(|preset| pools.add(build_pool(&preset.params(), 8, f32::INFINITY, &lod, &mut meshes)))
        .collect();
    let forest = Scatter {
        species: vec![3.0, 2.0, 1.0], // three oaks to two birches to one pine
        grove_size: 60.0,
        ..Scatter::new(Rect::new(-500.0, -500.0, 500.0, 500.0), 7.0)
    };
    // A road along the x axis, kept clear.
    for tree in forest.points_where(|at| if at.y.abs() < 6.0 { 0.0 } else { 1.0 }) {
        commands.spawn((
            PooledTree { pool: species[tree.species].clone(), variant: tree.variant },
            tree.transform(0.0), // on terrain: `tree.transform_on(&ground, &params)`, below
        ));
    }
}
```

`forest --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25` plants one, here in autumn so the groves show:

![A scattered forest in autumn seen from above one edge: groves of scarlet maples, russet oaks, pale birches and green pines with bending borders, around a round clearing](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/scatter.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25 --season 1,0 --eye 0,45,-200 --at 0,0,-20 --wind 0 --deterministic --screenshot scatter.png`*

`points_with` also weighs each species at each spot, for stands the game places: snags in a dead patch, saplings along the forest's edge, old giants deep inside. Its second closure, `weight(at, species)`, multiplies a species' share where a tree stands: 0 shuts the species out there, and where every weight is 1 the forest is `points_where`'s, tree for tree. Inside a stand of two species their groves still meet along bent borders, and a weight that fades out over a rim bends the stand's edge with the groves:

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// The last species alone within 35 m of `centre`, fading out over the next 20 m, and nowhere else.
fn with_a_stand(forest: &Scatter, centre: Vec2) -> Vec<ScatterPoint> {
    let last = forest.species.len() - 1;
    forest.points_with(
        |_| 1.0,
        |at, species| {
            let out = ((at.distance(centre) - 35.0) / 20.0).clamp(0.0, 1.0);
            if species == last { 1.0 - out } else { out }
        },
    )
}
```

![A pine forest from 60 m up, a stand of scarlet maples in its middle whose edge bends with the groves, pines mixing into its rim](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/stand.jpg)

*`cargo run --release --example forest -- --trees 3000 --scatter pine:1,maple:1 --grove 30 --stand 0,0,45 --season 0.8,0 --eye 0,60,-150 --at 0,0,0 --wind 0 --deterministic --screenshot stand.png`*

Undergrowth is a second, closer scatter that keeps clear of the trunks, drawn from the `Shrub` preset (a mound of arching stems about 1.4 m high and 1.9 m across: 5 900 wood and 17 600 leaf triangles up close, 2 100 in all at its coarsest level):

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// Shrubs at least 2.5 m apart and 1.5 m from every trunk. For a big forest, look the trunks up in a
// grid instead of all of them (the `forest` example does).
fn plant_undergrowth(commands: &mut Commands, trees: &[ScatterPoint], area: Rect, shrubs: &Handle<TreePool>) {
    let layer = Scatter { seed: 2, ..Scatter::new(area, 2.5) };
    for shrub in layer.points_where(|at| if trees.iter().any(|t| t.position.distance(at) < 1.5) { 0.0 } else { 1.0 }) {
        commands.spawn((PooledTree { pool: shrubs.clone(), variant: shrub.variant }, shrub.transform(0.0)));
    }
}
```

![Eye level in a birch grove with oaks beyond: shrubs stand between the trunks in thickets and gaps](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/undergrowth.jpg)

*`cargo run --release --example forest -- --trees 1500 --spacing 7 --scatter oak:3,birch:2,maple:2 --grove 30 --clearing 14 --undergrowth shrub:2.5 --eye -14,1.8,-30 --at 6,1,0 --fov 55 --sun 20,25,-30 --wind 0 --deterministic --screenshot undergrowth.png`: the example's undergrowth also thins out in patches about 30 m across.*

A shrub stands about a tenth as tall as a tree, so its pool can hand over to coarser levels much nearer: build it with half the default distances, `LodSettings { distances: LodDistances { lod1: 15.0, lod2: 40.0, impostor: 100.0 }, ..default() }`, as the example does. In the forest above (1 372 trees and 4 863 shrubs, `--offscreen --deterministic`, the default camera), the trees alone take 2.7 ms a frame; with the shrubs at the trees' distances 7.9 ms, at half of them 3.3 ms (Apple M4 Pro).

### On terrain: `FloraGround`

A tree stood at the ground's height under its centre floats on a slope: the downhill side of its trunk hangs in the air over the ground, its flat base showing. `FloraGround` is the height of your ground at every point (x, z), your terrain's height function or a lookup into its heightmap, and `footing(at, radius)` is the lowest ground under a trunk's base, where a tree stands so that its trunk meets the ground all round. `TreeParams::base_radius()` is that base, flare and root ridges included, and `ScatterPoint::transform_on(&ground, &params)` does it for a scattered tree at its own scale. `slope(at)` and `normal(at)` keep trees off cliffs and screes.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn height(at: Vec2) -> f32 {
    6.0 * (at.x * 0.03).sin() * (at.y * 0.02).cos() // your terrain, or a heightmap lookup
}

fn plant_on_hills(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut pools: ResMut<Assets<TreePool>>) {
    let ground = FloraGround::new(height);
    let oak = TreePreset::Oak.params();
    let pool = pools.add(build_pool(&oak, 8, f32::INFINITY, &LodSettings::default(), &mut meshes));
    let forest = Scatter::new(Rect::new(-200.0, -200.0, 200.0, 200.0), 8.0);
    // Nothing on ground steeper than 30°.
    for tree in forest.points_where(|at| if ground.slope(at) > 30f32.to_radians() { 0.0 } else { 1.0 }) {
        commands.spawn((PooledTree { pool: pool.clone(), variant: tree.variant }, tree.transform_on(&ground, &oak)));
    }
    // Felled trees are cut above the uphill side of their trunk and land on the hills.
    commands.insert_resource(ground);
}
```

![Two oak trunks on a 25° slope, side by side: on the left, stood at the height under its centre, its flat base juts out over the downhill ground; on the right, stood at its footing, the trunk meets the slope all round](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/footing.jpg)

*`cargo run --release --example inspect -- --out target/footing --preset oak --mode tree --slope 25 --distance 4 --eye 0.6 --look 0.04` (right), with `--on-centre` (left).*

As a resource it is also the ground felling uses: the saw goes in `cut_height` above the trunk's uphill side, so a stump on a slope is taller on its downhill side, and a felled tree comes to rest where its trunk or limbs meet the terrain, turning further than on flat ground when it falls down the slope and less when it falls against it. Without it the ground is flat at each tree's own height, as before. The crate does not draw the ground: `FloraGround` is only what trees stand and fall on, so it must match the terrain you draw. Branches and leaves are not bent to the slope: on steep ground a spruce's lowest whorls and a willow's strands on the uphill side run into the hillside.

![A wooded valley of rolling hills from a clearing: oaks, maples, birches and pines in groves climb the slopes, shrubs between them](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/terrain.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,pine:2,maple:1 --grove 50 --clearing 30 --terrain 14 --undergrowth shrub:3 --eye 0,2,0 --at -80,8,0 --fov 50 --wind 0.3 --deterministic --screenshot terrain.png`: `--terrain 14` draws hills about 14 m high and plants every tree and shrub at its footing, none on ground steeper than 30°.*

### A world that is not in metres: `FloraUnits`

The crate builds trees in metres, and a game scales them to its world with a `Transform`. A world whose unit is a foot, say, scales its trees by about 3.28, and everything the crate then measures in the world itself has to follow: how fast a tree falls (gravity is 9.81 m/s²), how far a falling leaf drifts, how wide a gust is, how far a fire reaches, how big a pusher is, how far away the levels of detail change. Insert `FloraUnits` with the number of world units to a metre and all of it does:

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn setup(mut commands: Commands) {
    // A world in feet: trees are placed with `Transform::from_scale(Vec3::splat(3.2808))`.
    commands.insert_resource(FloraUnits { units_per_metre: 3.2808 });
}
```

The rule, written on every field it touches: a field that sizes or times the crate's own simulation is in **metres** and is multiplied by `units_per_metre` where it meets the world (`FellTree::cut_height`, `TreeNotch::height`, `BuckTree`, `FireSpread::speed` and `reach`, `FoliagePusher::radius` and `height`, `SeeThroughTrees::radius`, `TreeShake::radius`, `GustFront::speed` and `width`, `LodDistances`, `FallingLeaves::autumn_distance`); a position, or a size that addresses the world, is in **world units** (every `at`, `target`, `centre` and `origin`; the size of `FloraFire` and the radius of `ignite`, `extinguish` and `scorch`; `FloraIndex`'s radii and distances; `Scatter`'s area and spacing; `FloraGround`'s heights). The default, 1.0, is a world in metres and changes nothing. Set it before planting: a pooled tree's bands of distance are made when it is spawned. A felled oak lands on the same update in a world in feet as in metres, a fire spreads the same number of cells a second and the default pusher covers the same share of a shrub (tests in `felling.rs`, `fire.rs` and `push.rs`).

### Leaf shading and wind

Trees drawn by the plugin move in the wind, with no cost to the CPU per frame. `FloraWind` is a global resource: how hard (`strength`), which way (`direction`), how fast (`speed`), how far trees bend (`bend`), how much leaves flutter (`flutter`, `flutter_frequency`), how much the wind comes in gusts (`gust_strength`, `gust_frequency`) and sways across itself (`turbulence`). `TreeParams::wind` is how a species answers it, and the presets set their own: a willow moves more than an oak, a bonsai hardly at all. A `GustFront` is a band of stronger wind that sweeps across the world:

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraWind, GustFront, TreePreset, WindResponse};

// Call this once, when the storm begins (from an event or a timer), not every frame: launching a
// gust restarts its front at `origin`.
fn start_storm(mut wind: ResMut<FloraWind>, time: Res<Time>) {
    wind.strength = 0.6; // 0 is calm, 1 a strong wind
    wind.direction = Vec2::new(1.0, 0.0); // blowing toward +x
    // A band of wind, 30 m deep, crossing the world along +x at 20 m/s from x = -50.
    wind.launch_gust(
        GustFront { origin: Vec2::new(-50.0, 0.0), speed: 20.0, ..default() },
        &time,
    );
}

fn stiff_willow() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Willow.params();
    params.wind = WindResponse { sway: 0.4, ..default() };
    params
}
```

Changing `FloraWind` writes one small shared buffer (`FloraWindBuffer`) and touches no material, so you can animate it every frame (a storm that builds, a wind that turns): a pooled forest of 20 000 trees takes 3.3 ms a frame with the wind changing every frame, where 0.5 took 212 ms (Apple M4 Pro). Launching a gust front writes the same buffer. Wind writes no motion vectors, and branches do not sway on their own: the whole tree bends and the leaves flutter. Leaf shading and wind need storage buffers in the vertex stage; on a device without them (WebGL2) trees are drawn with plain materials and one warning says so.

### Bark

Trunks and branches wear a bundled CC0 bark scan (a colour map and a normal map, 512 px wide, about 3.1 MB for all 14). Each preset picks the scan of its species (`TreeParams::bark`); `BarkParams` sets how it follows the wood's growth. The scan is laid out in metres on the *mature* tree, so a growing tree's bark never slides; wood thinner than `smooth_below` is smooth, and by `fissured_above` it shows the whole scan, so a young branch gains relief as it thickens. The scan's average colour is divided out of the material's base colour, so a textured tree keeps its preset colour, and far trees, impostors and the plain `StandardMaterial` fallback look as they always did.

```rust,no_run
use bevy_flora::{BarkParams, BarkScan, TreePreset};

fn gnarled_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    // Fissured sooner than the preset: smooth under 1 cm of radius, the whole scan from 8 cm.
    params.bark = BarkParams { scan: Some(BarkScan::Brown), smooth_below: 0.01, fissured_above: 0.08 };
    params
}

fn plain_pine() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Pine.params();
    params.bark.scan = None; // flat colour, as before 0.7
    params
}
```

The textures are compiled in by the default `textures` feature (about 3.6 MB of JPEG for the bark and the leaf scans); with `default-features = false` every scan is absent, wood is plain colour and leaves keep their procedural outline. Their sources and licences (all CC0 1.0, no attribution required) are in [`src/textures/CREDITS.md`](src/textures/CREDITS.md).

### Leaves: real outlines and grown veins

Seven presets (oak, willow, birch, bonsai, maple, poplar and Japanese maple) cut their leaf cards to the real outline of a scanned leaf and wear it. `FoliageParams::scan` (`LeafScan`) picks one of seven bundled CC0 atlases of four photographed leaves each; every leaf is one of the four (chosen by a hash of its own random draws, which never change as the tree ages, so a leaf keeps its shape while the tree grows and on `GpuGrowth` too), the card is as wide as that leaf is, and its length is the leaf's `size`. The atlas's colour and normal maps are applied in the fragment shader and its average colour is divided out of the leaf colour, so a crown keeps its preset colour. Distant levels of detail cut the cards to a coarser outline.

Leaves with no scan (the other presets and any `TreeParams` of your own, since `scan` defaults to `None`) keep the procedural outline, and their veins are no longer a formula: a vein network is grown from the petiole toward auxin sources scattered over the card by space colonisation (Runions et al., SIGGRAPH 2005), widths by Murray's law, drawn into one small image that every such leaf shares (`bevy_flora::venation`; `LeafShading::veins` sets how strongly the shader draws them). Needle clusters and cluster spheres are not leaf-shaped and have none.

```rust,no_run
use bevy_flora::{LeafScan, TreePreset};

fn maple_with_oak_leaves() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Maple.params();
    params.foliage.scan = Some(LeafScan::Oak);
    params
}

fn procedural_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    params.foliage.scan = None; // the procedural leaf, with grown veins
    params
}
```

### Looks: natural, painterly, toon

`TreeParams::style` (`RenderStyle`) changes how a tree is lit, not its shape; every preset is `Natural`. `Painterly` gives each leaf the normal that points out of the centre of its own clump (the leaves of one primary branch, the leader or one half of a split) instead of out of the crown's centre, so a crown reads as a cluster of rounded masses, each with a lit side and a shaded side; the lighting is the same physically based lighting as `Natural`. `Toon` keeps those normals, cuts the light of each directional light into three bands (0.15, 0.55 and 1 at `N·L` of 0 and 0.5) times its shadow and the surface colour, keeps the light that comes through backlit leaves, and darkens every surface toward its silhouette with an ink rim. Point and spot lights do not light a toon. Pools, impostors, `GpuGrowth` and growing trees draw them all; without the renderer's plugins (the plain-material fallback) a toon is lit as a painterly tree.

```rust,no_run
use bevy_flora::{RenderStyle, TreePreset};

fn painted_maple() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Maple.params();
    params.style = RenderStyle::Painterly;
    params
}

fn toon_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    params.style = RenderStyle::Toon;
    params
}
```

### Seasons

`FloraSeason` is a global resource like `FloraWind`: how far the leaves have turned (`autumn`, 0 summer green to 1 every leaf in its autumn colour), how many have fallen (`leaf_fall`, 0 to 1, bare), how much snow lies on the trees (`snow`, 0 to 1) and how far flowering trees are in blossom (`blossom`, 0 to 1). The deciduous presets (oak, willow, birch, bonsai, maple, poplar, baobab, Japanese maple and cherry) carry an autumn colour, `FoliageParams::autumn`; the evergreens have `None` and keep their needles. Leaves turn one at a time, each at its own moment, and fall in the order they turned. A fallen leaf is dropped in the vertex shader, so it leaves the shadows with it, and a leaf keeps its moment as the tree grows, on `GpuGrowth`, and on a pool's levels of detail; an impostor turns in patches of a few texels and thins its crown's coverage. Snow settles on every tree, first on what faces straight up and, as more falls, on steeper faces: the tops of branches, and on leaf and needle cards the side the camera sees when that side faces up (a tier of needles is white from above and green from below), as much as the leaf sees of the sky, with a light dusting on every exposed needle so a snowy conifer reads as snowy from below too. Impostors take it by their baked normals. The season shares the wind's buffer, so changing it every frame writes 96 bytes in place and touches no material: a year costs nothing per tree.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// A year every two minutes: green until 0.55 of the way through, turning to 0.75, falling from
// 0.7 to 0.9, snowing from 0.88 to 0.92, melting from 0.98, the cherries in blossom from the
// year's turn to 0.07, and in leaf again by 0.07.
fn seasons(time: Res<Time>, mut season: ResMut<FloraSeason>) {
    *season = FloraSeason::of_year(time.elapsed_secs() / 120.0);
}

// Or set it outright: `FloraSeason::AUTUMN` is every leaf turned, `WINTER` bare trees, `SNOW`
// bare trees and every tree under snow, and `BLOSSOM` flowering trees in blossom on bare branches.
fn first_frost(mut season: ResMut<FloraSeason>) {
    *season = FloraSeason::AUTUMN;
}

// A preset's autumn colour, or one for a tree of your own; and flowers for any tree.
fn golden_oak() -> TreeParams {
    let mut params = TreePreset::Oak.params();
    params.foliage.autumn = Some(Color::srgb(0.85, 0.6, 0.1));
    params.foliage.blossom = Some(Color::srgb(0.98, 0.97, 0.92)); // white, like a pear's
    params
}
```

A flowering tree (`FoliageParams::blossom`, the cherry's pale pink) has a five-petalled flower at every leaf's stem, which opens as `blossom` rises, each at its own moment, and falls in the reverse order; `of_year` brings the cherry into blossom on bare branches as the snow goes and drops the flowers while the leaves come back. The flowers sway and rustle with the leaves, cast shadows, grow on `GpuGrowth` and thin out on a pool's coarser levels (with simpler petals there); an impostor paints them over its picture in patches. They are a mesh of their own (`TreeMeshes::flowers`), on a third child of the tree (`TreeRender::flower_entity`) that the plugin does not draw while `blossom` is 0, so out of season a flowering tree costs what it would without flowers. In blossom they are most of its vertices (the cherry: 18 400 leaf and 28 500 flower vertices at level 0, 4 900 and 3 400 at level 1, 1 900 and 1 300 at level 2): a thousand individual cherries take 22 ms a frame out of season and 41 ms in full blossom, where a thousand oaks take 24 ms (Apple M4 Pro). A pool keeps its flowers in its leaf meshes and the shaders hide them out of season, so a pooled tree drawn at its finest level still fetches them; but a pool draws most of its trees at the coarser levels and as impostors, and 100 000 pooled cherries take what 100 000 oaks do (7.6 and 7.5 ms).

![A grove of cherry trees in full blossom on bare branches, pale pink crowns receding over green grass](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/blossom.jpg)

*`cargo run --release --example forest -- --trees 700 --spacing 5 --scatter cherry:1 --clearing 7 --season 0,1,0,1 --wind 0.3 --sun -20,25,20 --eye 2,4.5,2 --at -25,1.5,-18 --fov 50 --deterministic --screenshot blossom.png`, cropped.*

A fruiting tree (`FoliageParams::fruit`: a `FruitParams` colour, radius and share of the leaves that bear one; the `Apple` preset's red apples on one leaf in eight) hangs a round fruit below the stem of some of its leaves. The fruit is in the leaf mesh: it sways with its leaf, stays round in the crown's shade (its underside is baked darker), grows with its leaf on a young tree and on `GpuGrowth`, and a pool's coarser levels carry fewer, larger fruit; an impostor, a few pixels across, leaves it out. The season brings it: no fruit while the blossom is out, then fruit all summer, dropping one by one in the first two thirds of the leaf fall, so `FloraSeason::of_year` gives an apple tree white blossom in spring, red apples through summer and bare branches by winter. A fruit is 63 vertices (35 on the coarser levels).

![On the left an orchard of round-headed apple trees receding over green grass, red apples among their leaves; on the right the crown of one apple tree up close, glossy red apples hanging among the leaves](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/fruit.jpg)

*`cargo run --release --example forest -- --mode pool --trees 400 --spacing 6.5 --scatter apple:1 --clearing 6 --wind 0.3 --sun -20,25,20 --eye 2,2.6,2 --at -20,1.8,-15 --fov 50 --deterministic --screenshot orchard.png`, cropped, and `cargo run --release --example inspect -- --out target/apple --preset apple --mode tree --distance 3.2 --yaw 300 --eye 1.6 --look 0.55 --fov 50`.*

A scattered winter forest under `FloraSeason::SNOW`: the spruces and pines keep their needles with snow on the tiers that face up, the oaks and birches stand bare with snow along their branches (the white ground is the example's, not the crate's):

![A winter forest: dark spruces and pines with snow on the upper sides of their branch tiers, bare oaks and pale birches, on white ground](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/snow.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter spruce:3,pine:2,birch:2,oak:1 --grove 50 --clearing 25 --season 1,1,1 --eye 25,9,-75 --at 0,6,-20 --fov 50 --wind 0 --deterministic --screenshot snow.png`*

#### Rain: `FloraWetness`

`FloraWetness` is how wet the trees are, 0 (dry, the default) to 1 (soaked), for the game's weather to raise while it rains and lower as the trees dry. Wet bark darkens to about half, its colour deepens and it turns glossy; a leaf keeps its brightness and only its green grows richer (a darker or glossier leaf lets the sky's white sheen grey the crown, which then reads as frost, not rain). It reaches every way a tree is drawn, pools and impostors included, snow lies over it, and changing it writes one value of the shared buffer, no material: dry, every tree is exactly as before.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

/// The game's weather.
#[derive(Resource)]
struct Raining(bool);

// A shower: soaked in a minute, dry again ten minutes after it stops.
fn rain(raining: Res<Raining>, time: Res<Time>, mut wetness: ResMut<FloraWetness>) {
    let rate = if raining.0 { 1.0 / 60.0 } else { -1.0 / 600.0 };
    wetness.0 = (wetness.0 + rate * time.delta_secs()).clamp(0.0, 1.0);
}
```

![The foot and lower crown of an oak, twice: on the left dry, its bark a warm mid brown; on the right soaked, its bark a darker, deeper brown with a soft sheen up the trunk and its leaves a little richer green](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/wet.jpg)

*`cargo run --release --example inspect -- --out target/wet --preset oak --mode tree --wet 1 --distance 6.5 --yaw 35 --eye 1.7 --look 0.3 --fov 55 --size 1200x1200` (and `--wet 0`), side by side.*

### Trees that react: `TreeShakes`

A game hits trees. `TreeShakes` is a resource like `FloraWind`: `shake(TreeShake { at, radius, strength, direction, duration }, &time)` swings every tree whose trunk stands within `radius` metres of `at` (world x and z), the nearest the most, first away from `at` (or along `direction`), then back and forth, dying away over `duration` seconds, while its leaves rustle. About a metre of radius singles out the tree that was hit; tens of metres reach a whole grove. Each tree swings at its own pace, taller trees slower, and the whole tree bends as it does in the wind, on every way a tree is drawn: `ProceduralTree`, `GpuGrowth`, pooled trees and their impostors. Up to eight shakes run at once (`TreeShakes::MAX`; a new one takes the place of the one that ends soonest, so a knock always shows). They live in the wind's shared buffer, so a shake writes a few hundred bytes and touches no material, and while none is under way the shaders compare one number a vertex. A shake is drawn, not simulated: colliders (`TreeShape`) stay where they are.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// An axe on the tree standing at `trunk`, swung along `swing`: a metre of radius singles it out.
fn axe_hit(shakes: &mut TreeShakes, time: &Time, trunk: Vec3, swing: Vec3) {
    let hit = TreeShake { at: trunk.xz(), radius: 1.0, strength: 2.0, direction: swing.xz(), ..default() };
    shakes.shake(hit, time);
}

// A blast: every tree within 30 m swings away from it.
fn blast(shakes: &mut TreeShakes, time: &Time, at: Vec3) {
    shakes.shake(TreeShake { at: at.xz(), radius: 30.0, strength: 3.0, duration: 3.5, ..default() }, time);
}

#[derive(Component)]
struct Player;

// A character pushing through a thicket: a small knock where it is, a few times a second.
fn brush(mut shakes: ResMut<TreeShakes>, time: Res<Time>, player: Single<&Transform, With<Player>>, mut wait: Local<f32>) {
    *wait -= time.delta_secs();
    if *wait <= 0.0 {
        *wait = 0.3;
        let knock = TreeShake { at: player.translation.xz(), radius: 2.0, strength: 0.6, duration: 1.5, ..default() };
        shakes.shake(knock, &time);
    }
}
```

![The grown grove from a still camera: an axe hit swings the oak alone and knocks a few leaves off it, another swings the birch, then a blast swings every tree outward, leaves fall from them all, and they settle back](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/shake.gif)

*`cargo run --release --example capture_growth -- --shake`: an axe on the oak, one on the birch, then a blast at the grove's middle, in a light wind; leaves fall from every tree hit.*

### Falling leaves: `DropLeaves`

Hit a tree and leaves come off it. Every new shake knocks `FallingLeaves::per_shake` leaves (12 by default) off each tree it reaches, times its strength and fewer toward its radius, and a felled tree sheds `per_landing` (80) as it lands. `DropLeaves { tree, count }` drops leaves from any tree whenever the game wants: a bird lands, an arrow hits, a gust. The leaves are copies of the tree's own cards: each hangs where it grew until its moment (a drop lets go over half a second), falls, picking up speed to between 0.9 and 1.5 m/s, tumbles about an axis of its own and swings from side to side, drifts with `FloraWind`, turns flat as it reaches the ground (`FloraGround`, or the height the tree stands at), lies there for about `FallingLeaves::rest` seconds (6) and shrinks away. They wear the tree's scan, tint and season (a red maple drops red leaves, and a deciduous tree drops none of those it has already shed), cast shadows, and a leaf of crossed planes or a star folds flat when it lands.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// A bird lands in the oak.
fn bird_lands(mut drops: MessageWriter<DropLeaves>, oak: Single<Entity, With<ProceduralTree>>) {
    drops.write(DropLeaves { tree: *oak, count: 6 });
}

// More leaves to a hit, lying longer.
fn setup(mut leaves: ResMut<FallingLeaves>) {
    leaves.per_shake = 30.0;
    leaves.rest = 15.0;
}
```

![An autumn maple dropping red leaves: they tumble through the air around its trunk at every angle, the lowest about to reach the grass](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/falling_leaves.jpg)

*`inspect --mode tree --preset maple --season 1,0.3 --drop 40,2.6`: forty leaves of an autumn maple, 2.6 seconds after they let go.*

In autumn the falling leaves are the tree's own. The shaders stop drawing a deciduous tree's leaf once `FloraSeason::leaf_fall` passes the leaf's moment, so raising it a little at a time (a season passing, as `FloraSeason::of_year` does) thins the crown leaf by leaf; on the trees within `FallingLeaves::autumn_distance` (50 m) of a camera each of those leaves now lets go from where it hung and falls like the others (every tenth of a second, those passed meanwhile, in one burst per look of leaves). A rise of more than 0.1 in one frame, such as setting `FloraSeason::WINTER`, is a season set, not one passing: those leaves are gone at once, as before, and spring brings leaves back without dropping any. Leaves fall at their real speed, so a season sped up to a few seconds leaves them in the air into winter: the seasons clip above, a year in eight seconds, sets `autumn_distance` to 0.

![A maple's autumn in three moments: its red crown streaming leaves, its branches bare while the last leaves fall around the trunk, and the bare tree with its leaves lying around its foot](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/autumn_fall.jpg)

*`inspect --mode tree --preset maple --season 1,0 --autumn 6,0.5,0.75 --frames 14`, frames 3, 9 and 13: the leaf fall rises from 0 to 1 over six seconds; 2.75, 7.25 and 10.25 s in.*

Nothing moves on the CPU: a drop is one entity (`LeafBurst`) with one small mesh, drawn with a copy of its tree's leaf material that moves the cards in its vertex stage (`FloraExtension::falling`), and despawned when its last leaf is gone. Making one reads the tree's leaf mesh (a pool's variants once, then kept), and `FallingLeaves::max_leaves` (5 000) caps how many fall or lie at once. On an Apple M4 Pro (`forest --offscreen`): knocking trees every few frames with every shake slot busy (`--shake-animate`), so that thousands of leaves are always falling, cost 2% more frame time over 100 000 pooled oaks than before falling leaves and 4% over 1 000 individual ones; an autumn every six seconds, over and over (`--leaf-fall-animate 6`), costs 1 000 individual oaks 1% more than the same autumn standing still, and 100 000 pooled oaks 2 to 4%. With nothing falling no system runs. Trees drawn on the CPU and from pools drop leaves; `GpuGrowth` trees and trees drawn with plain `StandardMaterial`s do not. The see-through fade cuts falling leaves as it cuts the trees. A shake's or a drop's leaves are copies: the tree keeps its own, and a few falling copies are not missed from a crown of thousands.

### Picking fruit: `TreeFruit`

A fruiting tree (`FoliageParams::fruit`, the `Apple` preset) drawn as a `ProceduralTree` gets a `TreeFruit` once it is built: the fruit it grew, each with a key that holds as the tree grows, where it hangs in the tree's own space and its radius. `on_tree(&season)` lists the fruit the season hangs on it, `nearest(&season, tree, at, max)` finds the one nearest a hand, and `pick(key)` takes it off the tree: the shaders stop drawing it, on every level of detail, and it stays picked when the tree is rebuilt, until the next blossom brings the fruit back. A shake (`TreeShakes`) knocks loose a share of the fruit of the trees it reaches (`FallingFruit::per_shake`, 0.3 of it at strength 1, less the further the tree stands): each falls from where it hung as a `FallenFruit` entity (a sphere of its colour; its `Transform`'s scale is its radius), bounces and lies on the ground (`FloraGround`, or the height the tree stands at) for `FallingFruit::rest` seconds (60) for the game to pick up, and a `FruitFell` message tells of each. Pooled trees share their meshes, so their fruit hangs as the season says, and a tree grown on the GPU has no `TreeFruit`.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// The player picks the apple nearest their hand, and gathers the fallen ones within reach.
fn pick(
    mut commands: Commands,
    hand: Single<&GlobalTransform, With<Camera3d>>,
    season: Res<FloraSeason>,
    mut trees: Query<(&GlobalTransform, &mut TreeFruit)>,
    fallen: Query<(Entity, &GlobalTransform), With<FallenFruit>>,
) {
    let at = hand.translation();
    for (tree, mut fruit) in &mut trees {
        if let Some((apple, _)) = fruit.nearest(&season, tree, at, 1.5) {
            fruit.pick(apple.key);
        }
    }
    for (apple, lying) in &fallen {
        if lying.translation().distance(at) < 1.5 {
            commands.entity(apple).despawn();
        }
    }
}
```

![Two apple trees on grass: the left one is shaken and its apples fall and scatter in the grass, then a red stand-in beyond the right one picks its apples one by one, a yellow line marking each, into a basket at its feet](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/harvest.gif)

*`cargo run --release --example capture_growth -- --harvest`: `TreeShakes`, `FallenFruit` and `TreeFruit::nearest` and `pick`.*

### Felling: `FellTree`

Insert `FellTree { direction, cut_height, speed }` on a tree and it comes down. The plugin cuts its wood at `cut_height`: what stays becomes a `TreeStump` entity, and what falls keeps the tree's entity, both closed by a cut face that shows the end grain (pale sapwood, rings, the pith and a few checks). The tree tips over toward `direction` about the front edge of the cut, as a felled trunk turns on its hinge, slowly at first and then faster as a rod of its height would (the oak below lands about 2.7 seconds after the cut), until its trunk or one of its big limbs reaches the ground; it lands there with a small bounce, thin branches and leaves on the near side giving way into the ground. `TreeLanded` is sent the frame it hits, with the point it hit, for the crash, the dust and the camera shake, and the landing shakes the crowns of the trees it fell through and rustles its own leaves. `Felled` says how far it has tipped and when it lies still. A `PooledTree` becomes the `ProceduralTree` of its variant before it is cut (the same tree as the pool's finest level), and a `GpuGrowth` tree is moved to the CPU path first, so any tree in a forest can be felled. On a hillside, a `FloraGround` resource (see "On terrain" above) is the ground it is cut above and lands on.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// The player's axe finished the job: the tree falls away from them.
fn fell(commands: &mut Commands, tree: Entity, player: Vec3, trunk: Vec3) {
    let away = (trunk - player).xz();
    commands.entity(tree).insert(FellTree { direction: away, ..default() });
}

// When it lands: a crash and dust where it hit; a minute later, swap it for logs.
fn crash(mut landed: MessageReader<TreeLanded>) {
    for landing in landed.read() {
        info!("tree {:?} hit the ground at {}", landing.tree, landing.at);
    }
}
```

Once it has landed the tree no longer sways (its leaves still flutter), it is not rebuilt if its `ProceduralTree` or `TreeAge` changes, and it stays until you despawn it or buck it (below); its `TreeShape`, if it has one, turns with it and is not cut. There is no physics: the fall is drawn, as a shake is.

![An oak among a few trees is notched by four blows of an axe, each shivering its crown, then cut; it leans, falls to the left with gathering speed, lands on its limbs with a small bounce and lies there by its stump](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/fell.gif)

*`cargo run --release --example capture_growth -- --fell`*

#### Notching: `TreeNotch`

Before the saw, the axe. Insert `TreeNotch { direction, height, depth }` on a standing tree (a `ProceduralTree`, or a `PooledTree`, which becomes the tree of its variant as for felling) and the plugin carves an axe's notch out of its trunk: a wedge on the side `direction` points to, its floor level `height` above the base (0.5 m by default; with a `FloraGround`, above the uphill side, as the saw's cut), its face rising from the floor at 45°, reaching `depth` of the way through the trunk (0.25 by default, at most 0.9). Both faces show the end grain, its rings centred on the trunk's pith. Change `depth` with each blow and the notch is carved again; remove the component and the trunk is whole. Fell a notched tree and the saw goes in at the notch's floor whatever `cut_height` says: the stump keeps the floor and the falling trunk takes the face, as a real felling notch splits. A notched tree stops growing, a change of its `ProceduralTree` or `TreeAge` rebuilds it notch and all, and with `TreeLod` only its nearest level is notched. Pair each blow with a `TreeShake` and the crown shivers and drops a few leaves, as in the clip above.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// One blow of the player's axe on the side facing them: the notch deepens; once it is a third of
// the way through, the tree falls toward the notch.
fn chop(commands: &mut Commands, notches: &mut Query<&mut TreeNotch>, tree: Entity, toward_player: Vec2) {
    let Ok(mut notch) = notches.get_mut(tree) else {
        commands.entity(tree).insert(TreeNotch { direction: toward_player, depth: 0.1, ..default() });
        return;
    };
    if notch.depth >= 0.3 {
        commands.entity(tree).insert(FellTree { direction: notch.direction, ..default() });
    } else {
        notch.depth += 0.1;
    }
}
```

![Left: the foot of an oak with an axe's notch cut into its side, a pale level floor and a slanted face meeting at the back of the notch, the end grain's rings on both. Right: a pine felled at its notch beside its stump, the stump's top a full disc of end grain, the fallen trunk's butt showing the cut behind the notch with the pith and, below it, the slanted face of the notch](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/notch.jpg)

*`cargo run --release --example inspect -- --out target/notch --preset oak --mode tree --notch 90,0.3 --distance 3.1 --yaw 60 --eye 1.5 --look 0.042 --fov 50 --sun 20,25,30 --size 1200x1200` and `-- --out target/notch_fell --preset pine --mode tree --notch 0,0.3,0.6 --fell 0 --distance 3.1 --yaw 300 --eye 1.5 --look 0.03 --fov 50 --size 1200x1200`, side by side.*

Insert `BuckTree { log_length, min_radius }` on a felled tree, with its `FellTree` or later, and once it lies still the plugin bucks it: its trunk is cut into logs `log_length` long (2 m by default), which lie on the ground in a row from the stump, the way the tree fell, on the `FloraGround` if there is one, each with its bark and end grain at both ends; the tree's entity and its crown are gone. The logs stop where the trunk gets thinner than `min_radius` (8 cm). Each is a `TreeLog` entity: `index` counts up the trunk from the butt, and `capsule` is a `WoodCapsule` in its own space, for its collider. `TreeBucked { tree, stump, logs }` is sent the same frame.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// Fell the tree away from the player, and buck it into 2 m logs once it lies still.
fn chop_down(commands: &mut Commands, tree: Entity, away: Vec2) {
    commands.entity(tree).insert((FellTree { direction: away, ..default() }, BuckTree::default()));
}

// A collider for every new log, for the player to pick up.
fn log_colliders(logs: Query<&TreeLog, Added<TreeLog>>) {
    for log in &logs {
        let (capsule, place) = log.capsule.to_primitive(); // in the log's own space
        info!("log {}: {:.2} m across, its middle at {}", log.index, 2.0 * capsule.radius, place.translation);
    }
}
```

![An oak bucked into three logs lying in a row from its stump, the pale end grain showing in the gaps between them](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/logs.jpg)

*`cargo run --release --example inspect -- --out target/logs --preset oak --mode tree --fell 0 --buck 1.5 --distance 11 --eye 3.2 --look 0 --yaw 25 --fov 40 --sun 25,22,-15 --size 1600x900`, cropped.*

### Fire: `FloraFire`

Trees burn. Insert a `FloraFire` over the game's world, a square grid of cells that each hold how charred they are and how hot they burn, and light it where the game says (`ignite(at, radius)`): a lightning strike, a fire arrow, a campfire left burning. The plugin spreads it from cell to cell through the trees at `FireSpread::speed` (1 m/s by default) whatever the size of the cells, faster downwind (`FloraWind`), as far as there is something to burn: the cells within `reach` (3 m) of a trunk, so a clearing, a road or a firebreak stops it. Each tree it reaches scorches its leaves brown and burns them away one by one, each flaring as it goes (in the shadow maps too), and chars its wood from the ground up to black charcoal split into blocks whose seams glow while it burns and cool after. Every way a tree is drawn takes part: individual trees, `TreeLod` levels, pools and their impostors, `GpuGrowth` trees, and the leaves a burnt tree no longer has do not fall from it.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

#[derive(Component)]
struct Player;

// A 512 m square around the origin, 128 cells a side (4 m each). Insert it while the level
// loads: the flora materials are prepared again once, with the fire in their shaders.
fn setup(mut commands: Commands) {
    commands.insert_resource(FloraFire::new(Vec2::ZERO, 512.0));
}

fn lightning(mut fire: ResMut<FloraFire>) {
    fire.ignite(Vec3::new(40.0, 0.0, -12.0), 3.0);
}

// The grid is the game's too: read it where the shaders do.
fn burn_the_player(fire: Res<FloraFire>, players: Query<&Transform, With<Player>>) {
    for player in &players {
        if fire.heat_at(player.translation) > 0.5 {
            info!("the player stands in the fire");
        }
    }
}

// Where it burns now, for the game's lights and its AI.
fn watch_the_fire(fire: Res<FloraFire>) {
    let hottest = fire.burning().max_by(|a, b| a.1.heat.total_cmp(&b.1.heat));
    if let (Some((at, cell)), Some(burnt)) = (hottest, fire.touched_bounds()) {
        info!("hottest at {at} ({:.2}), all of it within {burnt:?}", cell.heat);
    }
}
```

`char_at` and `heat_at` read the grid between the cell centres, as the shaders do: to hurt what stands in the fire, make a burnt tree worthless, or scorch the game's own ground (the clip below colours its ground by `char_at`). A tree the fire burns through (the char where its trunk stands passes `BURNT`, 0.95) is reported once with a `TreeBurnt` message and marked `Burnt`: insert `Burnt` on the burnt trees of a save, with the fire's restored grid, and loading reports none of them. `extinguish` puts the fire out where rain or water falls, keeping the char, and `scorch` lays an old burn down with no fire, for a level that starts burnt. `FireSpread` (`FloraFire::with_spread`) sets how fast the fire moves, how long a spot burns and its embers glow, how far a tree's fire reaches and whether the ground itself burns; `FloraFire::with_cells` sets the size of the cells (smaller cells burn along finer lines). The same fire lit the same way on the same clock burns the same way (`FloraFire::seed` makes its front ragged).

`burning()` hands out every cell burning now (heat above 0, cooling embers included) with its centre, for the game's lights and its AI: put lights where it burns hottest, keep the living out of it, feed a map of what the watch can see. It looks only at the cells inside `touched_bounds()`, the rectangle outside which nothing has burnt, so a small fire on a large grid is cheap to read, and the fire is marked changed only in a frame it steps (ten times a second while anything burns) or the game changed it, so a system run on `resource_changed::<FloraFire>` reads it just when there is something new.

![The grove at dusk from a still camera: a fire lit on its right runs through it on the wind, each crown scorching and flaring orange as its leaves burn away, the trunks blackening with glowing seams, the ground going to ash under them and lit orange by the fire, until bare black trees stand on a burnt clearing and their embers cool](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/fire.gif)

*`cargo run --release --example capture_growth -- --fire`: the grove at dusk, a fire lit on its upwind edge. Like the rest of a clip it is sped up (`FireSpread` 3.5 m/s, each spot burnt through in 2.5 s), the clip's own ground scorches by `char_at`, and six point lights over the fire's hottest 4 m squares (`burning`) light the ground and the trunks around it.*

![An oak burning in daylight, three moments: the char climbing the trunk from the ground in black blocks with orange glowing seams under a green crown; the trunk black and the crown scorching, leaves flaring orange as they burn away; the bare black tree, its seams still glowing](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/fire_burn.jpg)

*`inspect --mode tree --preset oak --burn 1,2 --frames 12 --distance 9 --size 1280x960`, frames 1, 3 and 6: 3, 7 and 13 s after the fire was lit at the oak's foot.*

While no `FloraFire` exists no shader carries any of it, and frames cost what they did before (1 000 individual oaks 24.5 ms, 100 000 pooled oaks 7.5-7.6 ms, as in 0.25.0; Apple M4 Pro, `forest --offscreen`). Inserting one prepares the flora materials again once, with the fire in their shaders; with nothing burning a shader stops at the grid's header (1 000 oaks 24.6-24.7 ms, `forest --fire armed`), and outside the burnt rectangle a tree reads two vectors and no cell. A forest on fire costs 1 000 individual oaks 2% (25.0-25.1 ms, every tree within 60 m of the middle lit at once) and 100 000 pooled oaks nothing measurable (7.6 ms, within 300 m). The grid rides in the buffer the wind is in, written ten times a second while anything burns or glows (8 bytes a cell: 128 KB at 128 cells a side), so a wind that changes every frame uploads it too; each step of the fire looks only at the cells inside the burnt rectangle. Leaf shading and wind need storage buffers, and so does fire: on WebGL2 trees do not burn.

### Seeing the player: `SeeThroughTrees`

A third-person or top-down camera in a forest loses the player behind trunks and crowns. Put `SeeThroughTrees` on the camera and keep its `target` on the player: the parts of trees between the camera and the target dither out, in a cone around the line of sight `radius` wide at the target and a third of that at the camera, which closes over the last tenth of the way so nothing behind the player is cut. Under an orthographic projection, where every ray is parallel, the shader cuts a cylinder `radius` wide along the view direction instead (and only what stands before the target). The shape is tunable to match a fade a game draws on its own meshes: `keep` is the share of a tree that stays drawn at the centre of the hole (0 cuts it all, 0.35 leaves a wall readable), `inner` where the feather starts as a share of the radius (0.7), `in_front` (metres) how far before the target the cut begins, in place of the closing over the last tenth, and `feet` with `above_feet` (metres) leave whatever stands lower than that over the player's feet whole, so the foot of a trunk beside him is not cut. `SeeThroughTrees::opacity_at` is the same maths on the CPU. Only that camera sees through: shadow maps keep the whole trees (the cut tree's shadow still falls on the player), and so do other cameras. Every way a tree is drawn takes part, pooled levels and impostors included, and so do the camera's prepasses, so a `DepthPrepass` (for TAA, SSAO or depth of field) leaves no holes where a tree was cut.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

#[derive(Component)]
struct Player;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera3d::default(), SeeThroughTrees { radius: 1.8, ..default() }));
}

// Keep the clear view on the player's chest as it moves.
fn follow(mut camera: Single<&mut SeeThroughTrees>, player: Single<&Transform, With<Player>>) {
    camera.target = player.translation + Vec3::Y;
}
```

![A forest from behind and above a red stand-in for the player: on the left an oak hides the player; on the right the parts of the oak and its neighbours between the camera and the player are dithered out and the player stands in clear view, the oak's shadow still on the ground](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/see_through.jpg)

*`cargo run --release --example forest -- --mode naive --trees 1500 --spacing 6 --scatter oak:2,birch:1,maple:1 --grove 30 --eye 0,7,-15 --at 0,1.5,0 --fov 50 --sun 20,30,-10 --wind 0 --deterministic --see-through 0,1,0,1.8 --screenshot see_through.png`, cropped; left without `--see-through`. The same images, byte for byte, with `--prepass depth` and `--prepass all`.*

While no camera has it, no shader carries the fade and nothing draws or costs differently (the 424 gallery scenarios render as before; 1 000 individual oaks take 24.5 ms a frame and 100 000 pooled oaks 7.5 ms, as without it). While one has it, every flora pipeline carries the discard, which keeps the GPU from rejecting hidden fragments early, and a depth-only prepass gets a fragment stage: in `forest --offscreen` (Apple M4 Pro) 1 000 individual oaks take 27.0 ms instead of 24.5 (33.2 instead of 30.5 with a depth prepass), and 100 000 pooled oaks 7.5 ms either way. The edge of the clear view is a 4x4 ordered dither: TAA smooths it, and without TAA it reads as a fine screen door up close. One camera sees through at a time, and there is no fade with plain `StandardMaterial`s.

### Walking through undergrowth: `FoliagePusher`

Shrubs and low branches that a character walks through as if they were not there break the illusion. Put `FoliagePusher` on the player, the NPCs or a cart (`radius`, 0.45 m by default; `height`, 1.8 m, centred on its translation; `strength`, 1) and the leaves and thin stems it walks through give way: what is nearer its axis than its radius goes out of its way, bending a little down as a pushed stem does, and closes up behind it over half a second as it walks on. Trunks and limbs thicker than 5 cm stay where they are, and a plant stays rooted at its foot. The shaders do it, in the main pass and the shadows alike, on every way a plant is drawn up close (`ProceduralTree`, `TreeLod`, pooled levels, `GpuGrowth`); the plugin writes the `MAX_PUSHERS` (4) bodies nearest the camera into the shared buffer when they move.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

#[derive(Component)]
struct Player;

fn spawn_player(mut commands: Commands) {
    // The translation is the middle of the body: 1.8 m tall, 0.45 m round, standing on the ground.
    commands.spawn((Player, Transform::from_xyz(0.0, 0.9, 0.0), FoliagePusher::default()));
}
```

![A row of shrubs from a still camera: a red stand-in for a character walks through them from left to right, the leaves and stems bending out of its way around its body and closing up again behind it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/push.gif)

*`cargo run --release --example capture_growth -- --push`: a `FoliagePusher` 0.45 m round walking at 1.4 m/s in a light wind.*

While no entity has a `FoliagePusher` no shader carries the push, and frames cost what they did before. Inserting the first prepares the flora materials again once, with the push in their vertex stages; a vertex outside the circle around everything the pushers reach reads one vector and goes on. With one body standing in the forest (`forest --push`, Apple M4 Pro) 1 000 individual oaks take 24.9 ms a frame against 24.5 without it (2%), and 100 000 pooled oaks 7.6 ms against 7.5. A still body writes nothing; a moving one rewrites the shared buffer every frame (under a kilobyte, and the fire's grid with it while there is a fire). The push is in the vertex stages, so a leaf card bends with it and a card across the body's axis stretches in its core, which the body hides; shake a plant the body runs into (`TreeShakes`) to make it rustle as well.

### Gameplay: colliders, growth events, the prelude

A game wants a tree's shape in a handful of primitives, not twenty thousand triangles. Insert `TreeShape::default()` next to a `ProceduralTree` (or a `PooledTree`) and the plugin keeps it up to date: three capsules up the trunk that taper with it (the lowest has the trunk's radius at hip height, so a walking character meets what it sees), one capsule for every stretch of wood at least `limb_radius` thick (6 cm by default; `TreeShape::with_limb_radius`), and a box around the leaves, all in the tree's local space. A growing tree's shape grows with it on both the CPU and the `GpuGrowth` path, and it is written only when it changes, so `Changed<TreeShape>` is the cue to rebuild colliders. `WoodCapsule::to_primitive` gives Bevy's `Capsule3d` and where it sits, which is what most physics engines' capsule colliders take. Building the shape costs about as much as ageing the skeleton (6 to 9 microseconds a tree on an Apple M4 Pro), which is why it is opt-in. `TreeMature` is sent once when a `Growing` tree reaches maturity, and `use bevy_flora::prelude::*;` brings in the plugin, the components, the presets and parameters, the wind and the shapes.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn plant(mut commands: Commands) {
    commands.spawn((
        ProceduralTree(TreePreset::Oak.params()),
        TreeAge(0.0),
        Growing::default(),
        TreeShape::default(), // kept up to date while the oak grows
    ));
}

// Hand the capsules to your physics engine whenever the shape changes.
fn colliders(trees: Query<(Entity, &TreeShape), Changed<TreeShape>>) {
    for (tree, shape) in &trees {
        for capsule in shape.capsules() {
            let (capsule, place) = capsule.to_primitive(); // Capsule3d along the wood
            info!("{tree}: a capsule of radius {:.2} m at {:?}", capsule.radius, place.translation);
        }
    }
}

fn on_mature(mut mature: MessageReader<TreeMature>, mut commands: Commands) {
    for message in mature.read() {
        commands.entity(message.entity).remove::<Growing>();
    }
}
```

`examples/showcase.rs` draws the shape over the tree (`C`), and `inspect --colliders` photographs it:

![An oak with its gameplay shape drawn over it: three tapering capsules up the trunk, capsules along the thick limbs, and a box around the crown](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/shape.jpg)

*`cargo run --release --example inspect -- --out target/shape --preset oak --mode tree --colliders --distance 13 --eye 2 --look 0.45 --yaw 30`*

### Finding trees: `FloraIndex`

Which tree did the axe hit, which one is nearest the woodcutter, which ones stand in the blast? The plugin keeps a `FloraIndex` resource of every standing tree, `ProceduralTree` and `PooledTree` alike (a pooled one once its pool exists), each as its trunk: a cone from its foot to its top, at the age it shows and where it stands after transform propagation. A tree moved, rebuilt or grown is updated that frame, and a felled or despawned one leaves it. It is a grid of 8 m cells, so a query costs the trees near it, not all of them: with 100 000 pooled oaks, `nearest` takes about 1.6 µs and `cast_ray` over 50 m about 0.6 µs.

- `nearest(at, max)`: the tree whose bark at the foot is nearest a point (world x and z), within `max` metres.
- `within(at, radius)`: every tree whose bark at the foot is within `radius` metres.
- `cast_ray(ray, max)`: the first trunk a ray meets, as `TreeHit { tree, point, normal, distance }`: the player's look, an arrow, a falling branch. Crowns and leaves do not stop it.
- `get(entity)`, `iter()`, `len()`: each tree as an `IndexedTree { entity, base, axis, height, base_radius, top_radius }`.

```rust,no_run
use bevy::prelude::*;
use bevy::shape::Ray3d;
use bevy_flora::prelude::*;

// An axe's blow on the trunk the player looks at, within reach: the notch faces the player.
fn swing(mut commands: Commands, camera: Single<&GlobalTransform, With<Camera3d>>, index: Res<FloraIndex>) {
    let ray = Ray3d::new(camera.translation(), camera.forward());
    if let Some(hit) = index.cast_ray(ray, 2.5) {
        let toward_player = (camera.translation() - hit.point).xz();
        commands.entity(hit.tree).insert(TreeNotch { direction: toward_player, ..default() });
    }
}

// The woodcutter's next tree, and the trees a blast shakes.
fn next_tree(index: &FloraIndex, woodcutter: Vec3) -> Option<Entity> {
    index.nearest(woodcutter.xz(), 30.0).map(|tree| tree.entity)
}

fn in_the_blast(index: &FloraIndex, at: Vec2) -> Vec<Entity> {
    index.within(at, 8.0).map(|tree| tree.entity).collect()
}
```

![A row of four trees seen from the front: a red stand-in for a woodcutter walks to the birch on the left, a yellow ring at its foot, notches it, walks round it and fells it to the left; the ring jumps to the maple, then to the oak, each felled in turn, then to the pine](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/chop.gif)

*`cargo run --release --example capture_growth -- --chop`: the woodcutter asks `FloraIndex::nearest` for its next tree; a felled tree leaves the index at once, so the ring moves on while it falls.*

### Saving and loading the forest

A tree is its parameters and its seed, so a save holds what the game did to it:

- **The tree.** Its `TreeParams` (with the `serde` feature they serialize; `ron` writes `.tree.ron` files) or its preset and seed, its `Transform` and its `TreeAge`.
- **A notch.** `TreeNotch` is plain values; inserted again, it is carved again.
- **A felled tree.** How it was felled: on load, `FellTree { direction, lying: true, .. }` lays it down at once where it came to rest, with its stump and cut faces and no landing (no `TreeLanded`, no shake, no leaves shed). `BuckTree` then cuts it into logs as soon as it lies.
- **Picked fruit.** `TreeFruit::picked()` gives the keys; on load, spawn the tree with `TreeFruit::with_picked(keys)` and the same fruit stay off it.
- **A fire.** `FloraFire::cells()` with the grid's centre, size and cells per side; on load, `FloraFire::with_cells(centre, size, side)` and `restore(&cells)`: the char as it was, and the cells still burning burn on.
- **The weather and the year.** `FloraWind`, `FloraSeason` and `FloraWetness` are plain resources. `FallenFruit` entities are the game's to keep or let go.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

/// One tree as a game might save it.
struct SavedTree {
    params: TreeParams,
    at: Transform,
    felled_toward: Option<Vec2>,
    picked: Vec<u32>,
}

/// Plant the saved trees again, as the player left them.
fn restore(commands: &mut Commands, saved: &[SavedTree]) {
    for tree in saved {
        let fruit = TreeFruit::with_picked(tree.picked.iter().copied());
        let mut entity = commands.spawn((ProceduralTree(tree.params.clone()), tree.at, fruit));
        if let Some(direction) = tree.felled_toward {
            entity.insert(FellTree { direction, lying: true, ..default() });
        }
    }
}
```

### Everything at once: the playground

`examples/playground.rs` is a small first-person game built on the crate, to read as an example of wiring its gameplay APIs together: walk among an apple tree, an oak and a ring of mixed trees, strike a trunk with an axe (`FloraIndex::cast_ray` finds it, `TreeNotch` deepens, the fourth blow fells it with `FellTree`), buck the fallen tree (`BuckTree`), shake a tree (`TreeShakes`) and gather the apples that fall (`FallenFruit`), pick apples off it (`TreeFruit::nearest`, `pick`), light a fire (`FloraFire`, counting `TreeBurnt`), call the rain (`FloraWetness`), move the year on (`FloraSeason::of_year`), and save and load all of it (`FellTree::lying`, `TreeFruit::with_picked`, `Burnt`, `FloraFire::restore`). Every action reads a `Controls` resource, so `--demo` plays a script through the same systems and records it.

![First person among apple trees: walking up to an apple tree, shaking it so apples fall into the grass, picking apples off it, turning to an oak, striking it four times so it falls away, stepping back in the rain as it is bucked into logs beside its stump; a counter of apples and logs at the top](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/playground.gif)

*`cargo run --release --example playground -- --demo`; without `--demo` it is yours to play: `W` `A` `S` `D` walk, right mouse look, left click axe, `F` shake, `E` pick, `B` buck, `G` fire, `R` rain, `Y` a month on, `F5` save, `F9` load.*

### Without the plugin

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

## Features

- **18 presets**: oak, pine, willow, birch, palm, cypress, bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree, redwood, a cherry that blossoms in spring, an apple tree that bears fruit, and a shrub for undergrowth. Each is a plain `TreeParams` value you can edit.
- **Growth over time.** Every branch has a birth age and a growth duration, derived from where it sits on the tree. A tree can be shown at any age, branches thicken after they lengthen, and leaves grow to full size without re-rolling or disappearing as the tree ages.
- **Deterministic.** Each branch draws randomness from its own seeded stream, so the same `seed` gives the same tree on every run, and aging never re-rolls branches or leaves that already exist.
- **Branch shape**: crown envelopes, phyllotaxis, apical dominance, Y-splits, recursive sub-branches, twist, and a gravity bend that sub-branches and leaves follow. Floor avoidance keeps even strongly drooping wood above the ground.
- **Closed wood.** Branches meet the trunk through flared collars and root segments, forks taper into their halves, and trunk tops end in a flat cap, a cone or a leader branch. Optional pipe-model radii, radius-adaptive ring counts and volume-preserving (Taubin) smoothing that keeps seams, branch ends and twig seats in place.
- **Leaves**: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster), three placements (terminal branches, all branches, tip clusters) and four orientations. Leaf cards are leaf-shaped (cut into the geometry, so there is no alpha-mask `discard`) and grow out of their twigs, crowns shade as one soft volume through crown-bent normals (`normal_bend`), and each leaf gets its own brightness (`color_variation`), hue (`hue_variation`) and share of the sky (`occlusion`), all baked into the mesh. The plugin draws trees with `FloraMaterial`, a `StandardMaterial` with a leaf shader: a darker, glossier upper side, a paler matte lower side, veins, and light through a leaf lit from behind (`LeafShading`); facing the sun, the gloss gives way to that glow, so a backlit crown stays green instead of mirroring the sun white. The meshes also draw with a plain `StandardMaterial`, without those.
- **Wind.** `FloraWind` bends every tree and flutters its leaves in the vertex shader, shadows and impostors included, with a travelling `GustFront`, and each species answers in its own way (`TreeParams::wind`).
- **Bark.** Trunks and branches wear one of 14 bundled CC0 bark scans (colour and normal map; `TreeParams::bark`, `BarkScan`), laid out in metres on the mature tree so a growing tree's bark never slides, smooth on young wood and fissured as a branch thickens (`BarkParams`), and averaging the preset colour, so far trees, impostors and the plain-material fallback keep their colour. Compiled in by the default `textures` feature.
- **Leaf scans and grown veins.** Seven presets cut their leaf cards to the real outline of a bundled CC0 leaf scan and wear its colour and normal map (`FoliageParams::scan`, `LeafScan`); every other leaf-shaped card has veins grown by space colonisation (`venation`), drawn by the leaf shader from one small shared image.
- **Looks.** `TreeParams::style` draws a tree as `Natural`, `Painterly` (leaf normals point out of each clump's own centre: rounded masses) or `Toon` (banded light and an ink rim); impostors and pools follow.
- **Seasons.** `FloraSeason` turns deciduous trees to their autumn colour (`FoliageParams::autumn`), sheds their leaves one by one (near a camera they fall to the ground), lays snow on every tree, brings flowering trees into blossom in spring (`FoliageParams::blossom`: five-petalled flowers that open and fall one by one) and hangs fruit on fruiting trees through summer (`FoliageParams::fruit`, the apple), shadows, pools, impostors and `GpuGrowth` included, at no cost per tree; `FloraSeason::of_year` drives a whole year from one number.
- **Rain.** `FloraWetness` soaks every tree, pools and impostors included, from 0 (dry) to 1: wet bark darkens and turns glossy, the leaves' green grows richer; one value in the shared buffer, nothing per tree.
- **No hitches.** `BuildInBackground` builds a tree off the main thread: 200 oaks planted in one frame cost it 0.5 ms instead of 23 ms, and appear a few frames later.
- **Seeing the player.** `SeeThroughTrees` on the camera dithers out the trees between it and the player, prepasses included, while shadows keep the whole trees; nothing is compiled in while no camera has it.
- **Gameplay shapes.** `TreeShape`: capsules for the trunk and limbs and a box around the crown, kept up to date as a tree grows (CPU or GPU) and filled on pooled trees, ready to become colliders in any physics engine; `TreeMature` announces the moment a growing tree is done.
- **Finding trees.** `FloraIndex` holds every standing tree, pooled ones included, by where its trunk stands: the nearest tree to a point, the trees within a radius and the trunk a ray meets, in microseconds among 100 000 trees, kept up to date as trees are planted, grow, move, fall or go.
- **Saving and loading.** A felled tree comes back lying where it fell (`FellTree::lying`), picked fruit stays picked (`TreeFruit::with_picked`), and a fire's grid is restored as it was, still burning (`FloraFire::restore`).
- **Trees that react.** `TreeShakes` swings the trees near a hit, a passing character or a blast and rustles their leaves, on every way a tree is drawn, for one small buffer write and nothing per tree while nothing shakes. Hits knock leaves off and felled trees shed them as they land: they tumble down, drift with the wind and lie on the ground a while, moved on the GPU; `DropLeaves` drops them from any tree.
- **Picking fruit.** `TreeFruit` lists a fruiting tree's fruit, finds the one nearest a hand and picks it off the tree until the next blossom; shakes knock fruit loose to fall, bounce and lie on the ground as `FallenFruit` entities for the game to pick up.
- **Fire.** `FloraFire` spreads a fire through the trees from where the game lights it, faster downwind and stopped where nothing burns: leaves scorch, flare and burn away, wood chars from the ground up with glowing seams, on every way a tree is drawn; the game reads the fire back to hurt what stands in it, light it or scorch its own ground (`burning`, `touched_bounds`), and hears of each tree it burns through (`TreeBurnt`).
- **Walking through undergrowth.** `FoliagePusher` on the player or an NPC: the leaves and thin stems it walks through bend out of its way and close up behind it, shadows included, while trunks stay put.
- **Felling.** `TreeNotch` cuts an axe's notch into a standing trunk, deeper with each blow; `FellTree` cuts a tree, pooled ones included, at its notch if it has one: it falls about the hinge of the cut, lands on its limbs with a bounce (`TreeLanded`), and leaves a `TreeStump`; every cut face shows end grain. `BuckTree` then cuts its trunk into `TreeLog`s lying on the ground, each with a capsule for its collider.
- **Trees from files.** `TreeParams` is an asset: `ProceduralTreeAsset` draws a tree from a handle, and every tree sharing it follows edits; with the `ron` feature trees load from `.tree.ron` files and rebuild when the file is saved.
- **Forest scale.** `TreePool` builds a few variants of a tree once, each at three levels of detail made from the skeleton (thin wood dropped, fewer sides and rings, fewer and larger leaves) and a baked camera-facing impostor for the far field; `PooledTree` places them. 100 000 oak trees draw at 7.6 ms a frame, leaf shading, wind and seasons included (see above). A forest of individual `ProceduralTree`s is fine for a few thousand trees, because trees with equal looks share their materials (1 000 oaks: 25 ms a frame) and `TreeLod` draws each at the same three levels (6.5 ms), but each keeps its own meshes, so use a pool beyond that.
- **Planting forests.** `Scatter` says where the trees stand: trunks at least a spacing apart and otherwise irregular (Poisson disc), species in groves with bending borders, each tree its own yaw, size and variant, a density per spot for roads and clearings and a weight per species and spot for the stands a game places; deterministic per seed. A second scatter of the `Shrub` preset, kept clear of the trunks, makes the undergrowth.
- **Terrain.** `FloraGround` is your ground's height: trees stand at their footing, where the trunk meets a slope all round (`ScatterPoint::transform_on`), `slope` keeps them off cliffs, and felled trees are cut above the uphill side and land on the hills.
- **Any world unit.** `FloraUnits { units_per_metre }` tells the crate how many world units make a metre (a world in feet: 3.2808), and everything it measures in the world follows: gravity for felled trees and fallen fruit, how far falling leaves drift, gusts, fire speed and reach, the pusher's size, the see-through radius, the levels of detail's distances. The default, 1.0, changes nothing.

## Examples

| example | what it shows |
|---|---|
| `showcase` | An interactive growing tree. Right-drag / wheel orbits, `[` `]` cycle presets, `N` reseeds, `Space` toggles growth, `R` restarts, `M` jumps to mature, `G` rebuilds, `U` switches between growing in the vertex shader (`GpuGrowth`) and rebuilding on the CPU, `C` draws the gameplay shape, `Y` runs the year (`FloraSeason::of_year`, 40 s a year), `H` hits the tree (`TreeShakes`; leaves fall), `X` strikes it with an axe (`TreeNotch`, deeper with each blow; the fourth fells it), `F` fells it (`FellTree`) and then `B` bucks it into logs (`BuckTree`); any other key brings a new one. |
| `gallery` | Renders 441 scenarios (every preset at several ages and seeds, forced splits, smoothing, pipe radii (mature and young) and a wobbly twisted trunk on every preset, every leaf style × placement × orientation, every crown shape, every preset painterly and toon, parameter extremes, five deciduous presets and the pine turning, turned, half bare, bare and under snow, and the cherry's spring in blossom) offscreen to four-view PNGs plus a `stats.tsv` with mesh sanity flags. This is the visual QA harness. `--filter <substr>` narrows it; `--gpu` grows every tree in the vertex shader. |
| `compare_gallery` | Compares two renders directory against directory (CPU `gallery` against `gallery --gpu`, or two `capture_growth` runs): mean difference and share of close pixels per PNG, non-zero exit past the limits. |
| `inspect` | Photographs one tree under controlled conditions, to see how it reads: a camera that backs off through every level of detail and into the impostor (`--distance a,b --frames n`), optionally zooming so the tree keeps its size on screen and a level switch shows as a jump (`--constant-size`), any bearing, height and field of view, one level forced at every distance (`--level`), a `ProceduralTree` instead of a pooled tree (`--mode tree`, which `--lod` or `--level` give a `TreeLod`), the empty scene (`--empty`) to measure the tree's pixels against, with or without MSAA, in any look (`--style natural|painterly|toon`, which `forest` and `capture_growth` take too) and season (`--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]`, which `forest` takes too), notched (`--notch <bearing>,<depth>[,<height>]`: an axe's `TreeNotch`), felled (`--fell <bearing>`: the photographs wait until it lies still; with `--lying` it is laid down at once, as from a save) and bucked (`--buck <m>`: into logs `m` metres long), on a slope (`--slope <deg>`, the ground falling toward +x: the tree at its footing, or with `--on-centre` where a plain placement would put it), dropping leaves (`--drop <count>[,<after>[,<every>]]`: photographed `after` seconds after they let go, then every `every` seconds), shedding them as an autumn passes (`--autumn <seconds>[,<after>[,<every>]]`: the leaf fall rises to 1 over `seconds`, photographed the same way), burning (`--burn <after>[,<every>]`: a `FloraFire` lit at the tree's foot, photographed the same way), or with a body standing in it or walking through it (`--push <x>,<z>[,<radius>[,<vx>,<vz>]]`: a red `FoliagePusher`, photographed every quarter of a second while it walks). The clock is fixed, so runs repeat. |
| `capture_growth` | Records the clips above as numbered PNG frames: one tree growing (`--preset <name>`, default maple), with `--grove` a grove of mixed species with a gust front crossing it, with `--seasons` the same grove fully grown through a year, with `--shake` the grove knocked by `TreeShakes`, with `--fell` an oak notched by an axe and felled among a few trees, with `--fire` the grove burning at dusk, with `--push` a body walking through a patch of shrubs, with `--chop` a woodcutter felling a row of trees one after another, each found by `FloraIndex`, or with `--harvest` an apple tree shaken and another picked into a basket (`--gpu` grows the trees in the vertex shader). The clock steps a fixed 1/24 s per frame, so the wind sways smoothly and two runs write the same frames; the `ffmpeg` commands that make the GIFs are in the example's docs. |
| `forest` | The forest-scale benchmark: up to 100 000 trees under a shadow-casting sun, on a grid or, with `--scatter oak:3,birch:1,...`, where `Scatter` puts them (`--grove <m>`, `--clearing <m>`, `--stand <x,z,radius>`: the last species there alone; `--undergrowth shrub:2.5` adds a patchy layer of shrubs 2.5 m apart and clear of the trunks), on flat ground or on rolling hills (`--terrain <relief>`: every tree at its footing on a `FloraGround`), drawn from a `TreePool` per species (`--mode pool`, the default), as a `ProceduralTree` each (`--mode naive`), or as `GpuGrowth` trees, fully grown or growing for the whole run (`--mode gpu`, `gpu-live`; `--mode cpu-live` is the CPU-rebuilt equivalent), built in the frame or in the background (`--background`), in full or at levels of detail (`--tree-lod`: `TreeLod`), timed per frame or saved as a screenshot. `--offscreen` times the real frame cost without a window and the result line gives the mean, p95, median, minimum and CPU time per frame; `--no-impostors`, `--lod` and `--shadows` change the setup, and `--shadow-radius <m>` lets only the trees that near the camera cast (`NotShadowCaster` on the rest); `--sun <x,y,z>`, `--wind <strength>`, `--wind-dir <x,z>`, `--gust <speed>` and `--wind-animate` (the strength changes every frame) move the light and the weather, `--shake-animate` knocks the trees every few frames (every shake slot busy), `--leaf-fall-animate <s>` runs an autumn every `s` seconds (leaves always falling), `--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]` sets the season, `--fire <x>,<z>[,<radius>]` lights a `FloraFire` over the forest there (`--fire armed`: one that burns nothing), `--push <x>,<z>` stands a `FoliagePusher` there; `--see-through <x,y,z>[,<radius>]` stands a player in the forest for the camera to see through to (`SeeThroughTrees`) and `--prepass <none|depth|all>` gives the camera prepasses; `--deterministic` fixes the clock at 1/60 s, so two builds' `--screenshot`s compare byte for byte. |
| `tree_file` | (`--features ron`) A tree from `examples/assets/trees/maple.tree.ron` (`--tree <path>` for another file); with `file_watcher` too it rebuilds when the file is saved, and a load error shows on screen while the last good tree stays. `--export <preset>` prints a preset as a tree file, `--screenshot <path>` saves the tree and exits. |
| `playground` | A first-person sandbox where the gameplay APIs meet: chop, buck, shake, pick, burn, soak and save the trees (see "Everything at once: the playground"); `--demo` plays and records a scripted eight seconds. |

```sh
cargo run --release --example showcase
cargo run --release --example gallery -- --out target/gallery/latest --filter mature_
```

## How it works

`TreeParams` → skeleton (a node tree of trunk, leader, primaries, splits and sub-branches) → birth times → the skeleton at a given age → wood mesh and leaf points → `MeshData` → Bevy `Mesh`. `ARCHITECTURE.md` has the details and the invariants (winding, bend, junctions) the meshers rely on.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
