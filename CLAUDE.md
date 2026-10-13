# CLAUDE.md

Guidance for Claude Code in this repository.

## Project

`bevy_flora`: a pure-Rust Bevy 0.20 crate that generates parametric, stylized procedural trees which can grow over time. See `ARCHITECTURE.md`.

## Commands

```bash
cargo build --release
cargo build --release --features serde
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo build --examples
cargo package --locked
cargo run --example showcase   # orbit: right-drag/wheel; [ ] presets; N reseed; Space grow; R restart; M mature; G rebuild
cargo run --release --example gallery -- --out target/gallery/latest [--gpu] [--warmup-secs <s>] [--filter <substr>]... [--no-shadows]   # 4-view PNG + stats.tsv per scenario
cargo run --release --example compare_gallery -- target/gallery/cpu target/gallery/gpu   # compares two gallery renders
cargo run --release --example forest -- --offscreen [--mode pool|naive|gpu|gpu-live|cpu-live] [--deterministic] [--looks [--recolor]]   # forest benchmark, real frame times; --looks: three looks of one pool in three bands, --recolor: recolours two of them mid-run
cargo run --release --example tree_file --features ron,file_watcher [-- --export <preset> | --screenshot <path>]   # a tree from a .tree.ron file, rebuilt on save
```

## Layout

- `src/params.rs`, `crown.rs`, `presets.rs`: `TreeParams` and the 20 presets (17 trees, the cherry with spring blossom and the apple with fruit among them, and three undergrowth plants: a shrub and the evergreen juniper and fern); `TreeParams::crown` (`CrownStyle`) and `trunk_style` (`TrunkStyle`) draw a low-poly conifer, off in every preset
- `src/deadfall.rs`: `DeadfallSettings`, `Deadfall`, `PoolLog`, `PooledDeadfall`, `DeadfallKind`: stumps, fallen trees and bucked logs drawn from a `TreePool` built with `build_pool_with_deadfall` (shares `felling.rs`'s `lay_log`, `trunk_alone`, `hinge_point`, `rest_angle`)
- `src/crown_hull.rs`: the faceted hull of `CrownStyle::Hull` (pure CPU, `bevy::math` only): a stack of cones round the leaf cloud; `foliage.rs`, `lod.rs` and `pipeline.rs` dispatch to it, `MeshData::flat_shaded` and `mesh/branch.rs` make the prism trunk, `falling::hull_chips` is what a hull sheds
- `src/skeleton.rs`, `growth.rs`: node tree and age model
- `src/mesh/`: ring helpers (winding lives in `rings.rs`), trunk, branches, smoothing
- `src/foliage.rs`, `pipeline.rs`, `plugin.rs`, `background.rs`: leaves (scanned outlines included), `build_tree`, Bevy components/systems, building trees on the async compute pool (`BuildInBackground`)
- `src/shape.rs`: `TreeShape`, the gameplay capsules and crown box (opt-in component kept up to date by the plugin)
- `src/see_through.rs`: `SeeThroughTrees`, the camera's see-through fade (arming the flora materials, its slots in the shared buffer); its shaders are `see_through` in `shading/common.wesl` and `shading/see_through_prepass.wesl`
- `src/push.rs`: `FoliagePusher`, bodies whose foliage gives way: the nearest `MAX_PUSHERS` to the camera, each with its trail of the last half second, in the shared buffer's push slots; the shaders (`FLORA_PUSH`, `push_offset` and `wood_flex` in `shading/common.wesl`) bend leaves and thin wood aside in the vertex stages
- `src/felling.rs`: `FellTree`, `Felled`, `TreeStump`, `TreeLanded`, `BuckTree`, `TreeLog`, `TreeBucked`: the cut, the fall about the hinge, the landing, bucking the trunk into logs; `src/mesh/cut.rs`: cutting a mesh at a height and closing it with end-grain faces
- `src/falling.rs`: `FallingLeaves`, `DropLeaves`, `LeafBurst`: leaves knocked off by shakes, shed by felled trees and let go by the season's leaf fall near a camera, copied from the tree's leaf mesh into bursts drawn with a copy of its leaf material (`FloraExtension::falling`), whose vertex stage `shading/falling*.wesl` moves them on the GPU
- `src/harvest.rs`: `TreeFruit`, `Fruit`, `FallingFruit`, `FallenFruit`, `FruitFell`: a fruiting `ProceduralTree`'s fruit read from its leaf mesh (vertex alpha 0.25, one phase per fruit), picks marked in its leaf meshes (alpha 0.375, hidden by `leaf_hidden`), fruit a shake knocks loose falling as entities onto the ground
- `src/fire.rs`: `FloraFire`, `FireSpread`, `FireCell`: a grid of char and heat over the world that a fire spreads through (arrival-time front, fuel from tree trunks), written into the shared buffer after the weather; the shaders (`FLORA_FIRE`, `fire_at` and the leaf/wood helpers in `shading/common.wesl`) burn leaves away and char wood with glowing seams
- `src/tree_asset.rs`: `TreeParams` as an asset, `ProceduralTreeAsset` (a tree from a handle), and with the `ron` feature the `.tree.ron` loader and `to_ron`/`from_ron`
- `src/venation.rs`: leaf veins by space colonisation (pure CPU), rasterised into the image `FloraTextures` shares
- `src/shading/` (`FloraWind`, `FloraSeason`, `TreeShakes`, `FloraMaterial`, leaf look, wind, shakes and seasons in `common.wesl`/`vertex.wesl`/`prepass.wesl`/`fragment.wesl`; `share.rs`: `SharedFlora`, materials shared by trees with equal looks; `textures.rs`: `FloraTextures`, the bark images and their mean colours)
- `src/gpu/` (`GpuGrowth`: growth in the vertex shader, `growth*.wesl`)
- `src/lod.rs` (levels of detail from the skeleton; `TreeLod` draws a `ProceduralTree` at them, through the plugin's `sync_levels`), `src/pool.rs` (`TreePool`, `PooledTree`, its looks: `PoolColors`, `PooledLook`, `RecolorPool`; materials per look, `RecolorPool` writes them in place and never takes the pool mutably), `src/impostor/` (baked far-field quads, `impostor.wesl`; baked neutral grey, one `ImpostorMaterial` per look tints them), `src/scatter.rs` (`Scatter`: where a forest's trees stand, Poisson disc and groves, pure CPU): forest scale
- `src/recolor.rs`: `RecolorTree`, `TreeColors`, `CutWood`: recolouring trees that are not pooled (a `ProceduralTree`, its stump, cut faces and logs) with `SharedFlora` materials, no mesh rebuilt; an unlit look is `StandardMaterial::unlit` through `Surface`, `SharedFlora`'s `Look` key, `fragment.wesl`, `impostor.wesl` and `CutFace`'s unlit twin
- `src/ground.rs`: `FloraGround`, the height of the game's terrain: footings on slopes, the ground felled trees land on
- `src/units.rs`: `FloraUnits { units_per_metre }`, the world's unit: every field documented as metres is multiplied by it where it meets the world (gravity in `felling.rs` and `harvest.rs`, `falling.rs`, `fire.rs`, `push.rs`, `pool.rs` and `plugin.rs` bands, the see-through radius, gusts and shake radii when `shading::wind_vectors` packs them); the shaders read it from the shared buffer (`world_per_metre`, slot 2 lane w)
- `examples/showcase.rs`, `examples/gallery.rs` (visual QA renderer), `examples/compare_gallery.rs` (diffs two gallery renders), `examples/inspect.rs` (one tree photographed through every level of detail, at any bearing, height, zoom, season or forced level, a `ProceduralTree` with `--mode tree` (`TreeLod` with `--lod`/`--level`), felled with `--fell <bearing>` (`--lying`: laid down at once, as from a save), on a slope with `--slope <deg>` (`--on-centre`: the naive placement), dropping leaves with `--drop <count>[,<after>[,<every>]]`, shedding them as an autumn passes with `--autumn <seconds>[,<after>[,<every>]]`, burning with `--burn <after>[,<every>]` or with a red `FoliagePusher` body standing or walking through it with `--push <x>,<z>[,<radius>[,<vx>,<vz>]]` (time series): readability checks), `examples/capture_growth.rs` (records `docs/growth.gif`, `docs/grove.gif` with `--grove`, `docs/seasons.gif` with `--seasons` (a time-lapse: its leaves are shed where they hang, `FallingLeaves::autumn_distance` 0), `docs/shake.gif` with `--shake`, `docs/fell.gif` with `--fell`, `docs/fire.gif` with `--fire` (the grove at dusk, the fire sped up), `docs/push.gif` with `--push` (a body walking through shrubs), `docs/chop.gif` with `--chop` (a woodcutter using `FloraIndex`) and `docs/harvest.gif` with `--harvest` (shaking and picking apples), for the README), `examples/forest.rs` (forest benchmark: `--offscreen` for real frame times, never a windowed run; `--mode`, `--tree-lod` (`TreeLod` on the `ProceduralTree` modes), `--sun`, `--wind`, `--wind-dir`, `--wind-animate`, `--shake-animate`, `--leaf-fall-animate <s>` (an autumn every `s` seconds: leaves always falling), `--fire <x,z[,radius]|armed>` (a `FloraFire` lit there, or one that burns nothing), `--push <x,z>` (a `FoliagePusher` standing there), `--gust`, `--season`, `--scatter`/`--grove`/`--clearing`/`--undergrowth`, `--terrain <relief>` (hills and a `FloraGround`), `--deterministic`, `--eye`/`--at`/`--fov`), `examples/tree_file.rs` (`--features ron`: a tree from `examples/assets/trees/*.tree.ron`, rebuilt on save; `--export <preset>`, `--screenshot`; run the binary directly with `BEVY_ASSET_ROOT` set to the crate)
- `inspect --mode deadfall` draws pooled deadfall instead of one tree (two variants each of a stump, a lying tree and a bucked one, 12 m apart, fallen toward +x, down a `--slope`; `--preset`, `--hull` and `--prism` choose the tree): `--preset spruce --hull 5,7 --prism 5 --mode deadfall --slope 15 --distance 26 --eye 11 --look 0 --yaw 160 --fov 55`
- `inspect --hull <tiers>,<sides>[,<lift>[,<jitter>]]` and `--prism <sides>` set `TreeParams::crown` (`--branches`: not `trunk_only`) and `trunk_style` on the preset (`--preset spruce --hull 5,7 --prism 5` is the stylised winter conifer), so every other `inspect` option (`--fell`, `--notch`, `--burn`, `--season`, `--drop`, levels of detail) can photograph one; the gallery's `hull_spruce`, `hull_fir` and `hull_cypress` scenarios (mature, ages 2 and 5, snow) flag `HULL_TRIS` when a crown's triangles are not `tiers * sides * 2`
- `examples/playground.rs`: a first-person sandbox where the gameplay APIs meet (axe, buck, shake, pick, fire, rain, year, save and load), every action read from a `Controls` resource so `--demo` plays a script through the same systems and records `docs/playground.gif`
- `examples/isometric.rs`: an orthographic isometric camera over a pooled forest, following a walker with `SeeThroughTrees` (cylinder, share kept, `in_front`, `floor`) and zooming while the levels of detail follow (`LodScale`); `--demo` records `docs/isometric.gif`
- `src/textures/` (bundled CC0 bark scans and leaf atlases, `CREDITS.md`, the generated outline table `leaves.rs`; compiled in by the default `textures` feature) and `tools/prepare_textures.py` (downloads and processes them and generates `leaves.rs`; repo only, not packaged)
- Publishing: `Cargo.toml` `include` keeps `qa/`, `slop/`, `docs/`, `tools/` out of the package; README media use absolute GitHub URLs
- Releasing: follow `AGENTS.md`. Every release, patch releases included, re-records the README clips (`docs/growth.gif`, `grove`, `seasons`, `shake`, `fell`, `fire`, `push`, `chop` and `harvest`) with `capture_growth`, `docs/playground.gif` with `playground --demo` and `docs/isometric.gif` with `isometric --demo`; the clips are not updated automatically
- `qa/visual_findings.md`: visual QA defect log (open / fixed / proposed / iterations)

## Conventions

- Prefer `TreeParams` fields over magic numbers.
- Core modules depend only on `bevy::math` and `bevy::color`; only `mesh/mod.rs`, `plugin.rs`, `felling.rs`, `falling.rs`, `harvest.rs`, `shading/`, `gpu/`, `pool.rs` and `impostor/` touch render/ECS types (`mesh/cut.rs` and `crown_hull.rs` are pure CPU; `lod.rs`, `shape.rs` and `ground.rs` are pure CPU but derive `Component` on `TreeLod` and `TreeShape` and `Resource` on `FloraGround`).
- Docs: https://docs.rs/bevy/0.20.0, https://bevy.org/news/bevy-0-20/
