# Media

Every image, clip and video in this repository, with the command that regenerates it, the date it was
captured and the crate version it shows. Nothing here is in Git LFS. The README, the guide pages and
[the gallery](../gallery.md) reference these by a relative path, so crates.io serves them from
`https://github.com/dark-incorporated/bevy_flora/raw/HEAD/docs/media/<file>` too.

Frames come from the crate's own capture paths: `capture_growth` (one tree, a grove, a year, a shake, a
fell, a fire, a push, a chop or a harvest), `playground --demo`, `isometric --demo`, or `inspect` for a
single staged photograph. A clip is encoded from numbered PNG frames; a still is one of those frames, or
one `inspect` render.

The clips are deterministic (the clock steps a fixed 1/24 s a frame), so a re-record of unchanged code
comes out frame-identical on one machine; another GPU may differ by a level here and there. `AGENTS.md`
has the full release recipe, including the palette each clip is quantised to.

## Budgets

House style: a clip is one idea, 3 to 8 seconds, looping, at most 960 px wide, 15 fps and 4 MB; a still
is a PNG or JPEG at most 1280 px on its long side. Two clips here are over the 4 MB budget because the
palette they need is larger — see the table. A video over 8 seconds belongs in the book, not here.

## Clips

| File | Shows | Capture | Date | Version |
|---|---|---|---|---|
| `growth.gif` | The maple preset growing from a bare trunk to a full crown; 480x360, 24 fps, 8 s, 5.0 MB | `cargo run --release --example capture_growth -- --preset maple --out target/capture/growth` | 2026-10-31 | 0.47.0 |
| `grove.gif` | Eight species growing in turn, a gust front crossing them; 560x315, 20 fps, 8 s, 6.0 MB | `cargo run --release --example capture_growth -- --grove --out target/capture/grove` | 2026-10-31 | 0.47.0 |
| `seasons.gif` | The grove through autumn, leaf fall, snow and spring blossom; 560x315, 20 fps, 8 s, 5.8 MB | `cargo run --release --example capture_growth -- --seasons --out target/capture/seasons` | 2026-10-31 | 0.47.0 |
| `shake.gif` | Two axe hits swing one tree each, then a blast swings the grove; 560x315, 20 fps, 8 s, 4.8 MB | `cargo run --release --example capture_growth -- --shake --out target/capture/shake` | 2026-10-31 | 0.47.0 |
| `fell.gif` | An oak notched by four blows, cut, and falling to its limbs; 560x315, 20 fps, 8 s, 3.4 MB | `cargo run --release --example capture_growth -- --fell --out target/capture/fell` | 2026-10-31 | 0.47.0 |
| `fire.gif` | The grove at dusk with a fire running through it on the wind; 560x315, 20 fps, 8 s, 3.7 MB | `cargo run --release --example capture_growth -- --fire --out target/capture/fire` | 2026-10-31 | 0.47.0 |
| `push.gif` | A body walking through shrubs, the stems bending aside and closing; 560x315, 20 fps, 8 s, 4.5 MB | `cargo run --release --example capture_growth -- --push --out target/capture/push` | 2026-10-30 | 0.46.1 |
| `chop.gif` | A woodcutter felling a row of four trees, each found by `FloraIndex`; 560x315, 20 fps, 8 s, 5.1 MB | `cargo run --release --example capture_growth -- --chop --out target/capture/chop` | 2026-10-30 | 0.46.1 |
| `harvest.gif` | An apple tree shaken, another picked into a basket; 560x315, 20 fps, 8 s, 2.9 MB | `cargo run --release --example capture_growth -- --harvest --out target/capture/harvest` | 2026-10-30 | 0.46.1 |
| `playground.gif` | First person: shake, pick, notch, fell, buck, rain; 560x315, 16 fps, 8 s, 7.0 MB | `cargo run --release --example playground -- --demo --out target/capture/playground` | 2026-10-30 | 0.46.1 |
| `isometric.gif` | An orthographic camera zooming over a pooled forest, the walker seen through the trees; 560x315, 16 fps, 8 s, 7.0 MB | `cargo run --release --example isometric -- --demo --out target/capture/isometric` | 2026-10-31 | 0.47.0 |

`playground.gif` and `isometric.gif` are 7.0 MB, over the 4 MB budget: a first-person scene with a HUD
and a dither fade does not survive a 96-colour palette, so they are quantised to 64 colours at 16 fps.
They are the two clips a future release should shrink (a 12-second scene cut to 8, or a 480 px width)
rather than re-palette.

Encode a clip from its frames, once they are in `target/capture/<name>/`:

```sh
ffmpeg -y -framerate 24 -i target/capture/growth/frame_%04d.png \
  -vf "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" \
  docs/media/growth.gif
```

`AGENTS.md` has the exact ffmpeg line for each clip, and the sizes to check it against.

## Stills

| File | Shows | Command | Date | Version |
|---|---|---|---|---|
| `still.png` | The mature maple preset from the front: trunk, limbs, scanned bark, leaf cards | `cargo run --release --example inspect -- --out target/still --preset maple --mode tree --distance 15 --yaw 20 --eye 4 --look 0.5 --size 1280x900` | 2026-10-31 | 0.47.0 |
| `scatter.jpg` | A scattered autumn forest with groves and a clearing, from above one edge | `cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25 --season 1,0 --eye 0,45,-200 --at 0,0,-20 --wind 0 --deterministic --screenshot scatter.png` | 2026-10-12 | 0.12.0 |
| `stand.jpg` | A pine forest with a stand of scarlet maples whose edge bends with the groves | `forest --trees 3000 --scatter pine:1,maple:1 --grove 30 --stand 0,0,45 --season 0.8,0 --eye 0,60,-150 --wind 0 --deterministic --screenshot stand.png` | 2026-10-13 | 0.35.0 |
| `undergrowth.jpg` | Eye level in a birch grove, shrubs in thickets between the trunks | `forest --trees 1500 --spacing 7 --scatter oak:3,birch:2,maple:2 --grove 30 --clearing 14 --undergrowth shrub:2.5 --eye -14,1.8,-30 --at 6,1,0 --fov 55 --sun 20,25,-30 --wind 0 --deterministic --screenshot undergrowth.png` | 2026-10-12 | 0.16.0 |
| `footing.jpg` | Two oak trunks on a 25° slope: stood under its centre, and at its footing | `inspect --preset oak --mode tree --slope 25 --distance 4 --eye 0.6 --look 0.04` (and `--on-centre`) | 2026-10-12 | 0.18.0 |
| `terrain.jpg` | A wooded valley of rolling hills, every tree planted at its footing | `forest --trees 3000 --spacing 7 --scatter oak:3,birch:2,pine:2,maple:1 --grove 50 --clearing 30 --terrain 14 --undergrowth shrub:3 --eye 0,2,0 --at -80,8,0 --fov 50 --wind 0.3 --deterministic --screenshot terrain.png` | 2026-10-12 | 0.18.0 |
| `tree_lod.jpg` | One oak at level 0, 1 and 2, each zoomed to the same size | `inspect --mode pool --lod ... --constant-size` | 2026-10-12 | 0.23.0 |
| `tree_file.jpg` | A tree file as exported, and the same file with three values changed | `cargo run --release --example tree_file --features ron` | 2026-10-12 | 0.15.0 |
| `blossom.jpg` | A grove of cherry trees in full blossom on bare branches | `forest --trees 700 --spacing 5 --scatter cherry:1 --clearing 7 --season 0,1,0,1 --wind 0.3 --sun -20,25,20 --eye 2,4.5,2 --at -25,1.5,-18 --fov 50 --deterministic --screenshot blossom.png` | 2026-10-12 | 0.20.0 |
| `fruit.jpg` | An orchard of apple trees, and one apple tree's crown up close | `forest --mode pool --trees 400 --spacing 6.5 --scatter apple:1 --clearing 6 --wind 0.3 --sun -20,25,20 --eye 2,2.6,2 --at -20,1.8,-15 --fov 50 --deterministic --screenshot orchard.png` and `inspect --preset apple --mode tree --distance 3.2 --yaw 300 --eye 1.6 --look 0.55 --fov 50` | 2026-10-13 | 0.31.0 |
| `snow.jpg` | A winter forest: snow on the evergreens' tiers, bare oaks and birches | `forest --trees 3000 --spacing 7 --scatter spruce:3,pine:2,birch:2,oak:1 --grove 50 --clearing 25 --season 1,1,1 --eye 25,9,-75 --at 0,6,-20 --fov 50 --wind 0 --deterministic --screenshot snow.png` | 2026-10-12 | 0.13.0 |
| `wet.jpg` | An oak's foot and lower crown dry, and soaked by `FloraWetness` | `inspect --preset oak --mode tree --wet 1 --distance 6.5 --yaw 35 --eye 1.7 --look 0.3 --fov 55 --size 1200x1200` (and `--wet 0`) | 2026-10-13 | 0.30.0 |
| `falling_leaves.jpg` | An autumn maple dropping red leaves around its trunk | `inspect --mode tree --preset maple --season 1,0.3 --drop 40,2.6` | 2026-10-13 | 0.24.0 |
| `autumn_fall.jpg` | A maple's autumn in three moments, from a red crown to a bare tree | `inspect --mode tree --preset maple --season 1,0 --autumn 6,0.5,0.75 --frames 14`, frames 3, 9 and 13 | 2026-10-13 | 0.25.0 |
| `notch.jpg` | An axe's notch in an oak's side, and a pine felled at its notch | `inspect --preset oak --mode tree --notch 90,0.3 --distance 3.1 --yaw 60 --eye 1.5 --look 0.042 --fov 50 --sun 20,25,30 --size 1200x1200` and `--preset pine --notch 0,0.3,0.6 --fell 0 --yaw 300 --look 0.03` | 2026-10-13 | 0.28.0 |
| `logs.jpg` | An oak bucked into three logs lying in a row from its stump | `inspect --preset oak --mode tree --fell 0 --buck 1.5 --distance 11 --eye 3.2 --look 0 --yaw 25 --fov 40 --sun 25,22,-15 --size 1600x900` | 2026-10-12 | 0.19.0 |
| `fire_burn.jpg` | An oak burning, three moments from green crown to bare black tree | `inspect --mode tree --preset oak --burn 1,2 --frames 12 --distance 9 --size 1280x960`, frames 1, 3 and 6 | 2026-10-13 | 0.26.0 |
| `see_through.jpg` | An oak hiding a player, and the same oak dithered out by `SeeThroughTrees` | `forest --mode naive --trees 1500 --spacing 6 --scatter oak:2,birch:1,maple:1 --grove 30 --eye 0,7,-15 --at 0,1.5,0 --fov 50 --sun 20,30,-10 --wind 0 --deterministic --see-through 0,1,0,1.8 --screenshot see_through.png` (and without `--see-through`) | 2026-10-12 | 0.22.0 |
| `shape.jpg` | An oak with its `TreeShape` capsules and crown box drawn over it | `inspect --preset oak --mode tree --colliders --distance 13 --eye 2 --look 0.45 --yaw 30` | 2026-10-12 | 0.10.0 |

A still that shows a feature which has changed since its version was captured is re-recorded in that
release's pull request, by the same rule as a clip (`AGENTS.md`, step 2). None is a placeholder.

## Videos

None. A clip longer than 8 seconds belongs on [the book page](https://dark-incorporated.github.io/crates/bevy_flora.html),
as an MP4 with a poster.

## Copies at the old paths

Until 2026-10-31 these files lived directly under `docs/` (and some under other `docs/` folders). Every release published before then has a README on crates.io and docs.rs whose image URLs name those old paths, and a published README never changes. So each moved file also stays at its old path, as an identical copy: git stores the same content once, so a copy costs nothing in the repository's size. **Do not delete them as duplicates**; when one of the files here is re-recorded, the old-path copy is updated with it (`docs/media/` is the one new references use). The org's rule is in [DOCUMENTATION.md](https://github.com/dark-incorporated/.github/blob/main/DOCUMENTATION.md#image-and-link-urls-in-a-readme).
