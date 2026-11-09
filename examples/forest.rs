//! Forest-scale benchmark: how many trees can be drawn at what frame rate.
//!
//! ```text
//! cargo run --release --example forest -- [--trees <n>] [--mode <naive|pool|gpu|gpu-live|cpu-live>]
//!     [--preset <name>] [--frames <n>] [--warmup-secs <s>] [--spacing <m>]
//!     [--no-impostors] [--lod <lod1,lod2,impostor>]
//!     [--sun <x,y,z>] [--wind <strength>] [--wind-dir <x,z>] [--gust <speed>]
//!     [--eye <x,y,z>] [--at <x,y,z>] [--fov <deg>] [--ortho <view height>]
//!     [--impostor-elevation <deg>] [--impostor-shadows] [--background-pools]
//!     [--shadows <off|distance,cascades>] [--wind-animate] [--shake-animate] [--leaf-fall-animate <s>] [--deterministic]
//!     [--offscreen] [--screenshot <path>] [--style <natural|painterly|toon>]
//!     [--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]] [--wet <0..1>]
//!     [--scatter <preset[:weight]>,... [--grove <m>] [--clearing <m>] [--stand <x,z,radius>]]
//!     [--undergrowth <preset>[:<spacing>]] [--terrain <relief>] [--background] [--tree-lod] [--shadow-radius <m>]
//!     [--see-through <x,y,z>[,<radius>]] [--prepass <none|depth|all>] [--fire <x,z[,radius]|armed>]
//!     [--push <x,z>] [--looks [--recolor [--glide <s>]]] [--ages <n>]
//! ```
//!
//! Trees stand on a square grid with a deterministic jitter and a random yaw, or, with
//! `--scatter oak:3,birch:1,...`, where `Scatter` puts them: trunks at least `--spacing` apart,
//! the species in groves `--grove` metres wide (0 mixes them tree by tree), one pool per species,
//! about `--trees` of them, around a clearing `--clearing` metres wide. `--stand <x>,<z>,<radius>`
//! grows the last `--scatter` species there alone, fading out over a rim half the radius wide,
//! and nowhere else (`Scatter::points_with`), as a game places a stand of snags. `--undergrowth
//! shrub:2.5` adds a second `Scatter` of that species, 2.5 m apart (the default) and at least
//! 1.5 m from every trunk, under the trees (it counts on top of `--trees`). A
//! fixed camera 30 m above one corner looks across the grid toward its centre,
//! so near, mid and far trees are all on screen. The window is 1920x1080 with
//! vsync off and one shadow-casting directional light.
//!
//! `--mode naive` gives every tree its own meshes (`ProceduralTree`, today's
//! path without growth). `--mode pool` draws the same forest from a
//! `TreePool`: a few seed variants shared by every tree, with mesh levels of
//! detail and, beyond the last of them, instanced impostors (`--no-impostors`
//! leaves those out; `--lod 30,80,200` sets the distances, in metres, at which
//! each takes over). The impostors are lit by the scene's own sun, so `--sun <x,y,z>` (the
//! sun's position, looking at the origin; default `-20,25,20`) moves their light too, and
//! `--wind <strength>` (0 still to 1 strong; the library's default if not given) sways the
//! trees, leaves and impostors alike; `--wind-dir <x,z>` turns the wind. `--gust <speed>`
//! sends a gust front across the grid along +x, from its edge, when the trees are ready.
//! `--wind-animate` changes the wind's strength every frame, as a game with weather would.
//! `--deterministic` steps the clock a fixed 1/60 s per frame and counts `--warmup-secs` as
//! 60 frames a second, so a `--screenshot` of two versions of the code is the same image
//! wherever they draw the same (compare the PNGs byte for byte).
//! `--eye <x,y,z>` and `--at <x,y,z>` move the camera and what it looks at (world metres, the
//! grid centred on the origin), and `--fov <deg>` sets its vertical field of view (45 by default).
//! `--ortho <view height>` makes the camera orthographic, its view that many metres tall: the
//! levels of detail and impostors follow the zoom (`LodScale`). `--impostor-elevation <deg>` bakes
//! the impostors from that far above the horizon (`LodSettings::impostor_elevation`, 10 by
//! default) and `--impostor-shadows` lets them cast shadows (`LodSettings::impostor_shadows`).
//! `--background-pools` builds the pools with `BackgroundPools` instead of `build_pool` at
//! Startup; either way the loading report gives the longest frame while the forest came in.
//! `--mode gpu` and `--mode gpu-live` draw the trees as `ProceduralTree` with `GpuGrowth`,
//! fully grown or still growing through the whole run; `--mode cpu-live` grows them on the
//! CPU, rebuilding every mesh each frame. `--background` builds those trees off the main thread
//! (`BuildInBackground`): they appear a few frames after the start instead of in its first frame.
//! `--tree-lod` draws them at levels of detail (`TreeLod`, at the `--lod` distances; there is no
//! impostor), as a pool draws its trees.
//! `--see-through <x,y,z>[,<radius>]` stands a red stand-in for the player at that point (its
//! height above the ground) and puts `SeeThroughTrees` on the camera; `--prepass depth` or
//! `--prepass all` gives the camera a depth prepass, or depth, normal and motion-vector ones.
//! `--fire <x>,<z>[,<radius>]` puts a `FloraFire` over the forest (cells of about 2 m) and lights
//! it there, `radius` metres round (4 by default), once the trees are placed; it spreads through
//! the forest from there. `--fire armed` puts the fire there and lights nothing: what a game pays
//! for having one before anything burns. `--push <x>,<z>` stands a body with a `FoliagePusher`
//! there (no mesh): every flora shader then carries the push. `--shadow-radius <m>` lets only the
//! trees within `m` metres of the camera cast shadows (`FloraShadows`), as a game keeps the shadow
//! maps to the trees near the player. `--looks` (pool mode) gives every pool two
//! looks beyond its own (`TreePool::add_look`: a lit autumn one, and a flat unlit one in two other
//! colours) and stands the forest in three bands along x, one in each (`PooledLook`): the meshes
//! are the same, the impostors take the colours as a tint. `--recolor` then, after the warmup,
//! changes the colours of looks 1 and 2 (`RecolorPool`) and stands another warmup in them, so a
//! `--screenshot` shows the forest after a recolour; with `--glide <s>` the looks glide to the new
//! colours over `s` seconds (measured and shot while they glide when `s` is longer than the
//! warmup). `--ages <n>` (pool mode) builds every
//! species' pool at n ages (4, 8, ... years, the last mature) and spreads the trees among them, as
//! a game plants young and old trees: pools whose surfaces are equal share their materials.
//!
//! `--terrain <relief>` replaces the flat ground with rolling hills about `relief` metres high
//! (a heightfield mesh) and makes them the `FloraGround`: every tree and shrub stands at its
//! footing (`ScatterPoint::transform_on`), none on ground steeper than 30°, and `--eye` and
//! `--at` heights count from the ground under them.
//!
//! After the trees exist and `--warmup-secs` have passed (shaders compile in the
//! background), the example times `--frames` frames and prints one line:
//!
//! ```text
//! forest pool oak 100000 trees: 12.3 ms/frame (81.3 fps), p95 14.0 ms, median 11.9 ms, min 9.0 ms, cpu 18.2 ms/frame
//! ```
//!
//! `cpu` is the CPU time of every thread per measured frame. It does not grow when other
//! programs compete for the machine, and it is far below the frame time when the GPU is the
//! limit, so it tells the two apart and compares runs on a machine that is not quiet.
//!
//! With `--screenshot <path>` the camera renders into an offscreen image
//! instead, which is saved to `path` once the forest is ready; nothing is timed.
//!
//! `--shadows off` turns shadow maps off; `--shadows 120,2` limits them to 120 m
//! in 2 cascades (Bevy's default reaches 150 m in 4), which keeps the far trees
//! out of the shadow maps.
//!
//! Display note: the window runs flat out (`WinitSettings::continuous`), but on
//! macOS the compositor still paces a windowed app to the display's refresh rate,
//! so frame times below one refresh interval read as the interval. `--offscreen`
//! has no window: the camera renders into an image, nothing is presented, and the
//! frame time is what the CPU and the GPU actually take.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy_flora::{
    BackgroundPools, BuildInBackground, FloraFire, FloraGround, FloraPlugin, FloraSeason, FloraShadows, FloraWetness, FloraWind,
    FoliagePusher,
    GpuGrowth, Growing,
    GustFront, LodDistances, LodSettings, PoolColors, PooledLook, PooledTree, ProceduralTree, RecolorPool, RenderStyle,
    Scatter, SeededRng, SeeThroughTrees, TreeAge, TreeLod, TreeParams, TreePool, TreePreset, TreeShake, TreeShakes,
    build_pool,
};

const WINDOW: UVec2 = UVec2::new(1920, 1080);
/// Seeds the forest cycles through; the naive forest uses the same ones as the pool.
const VARIANTS: u32 = 8;
/// Height of the camera above the ground, in metres.
const CAMERA_HEIGHT: f32 = 30.0;
/// The undergrowth's pool switches levels of detail at this share of the trees' distances
/// (15, 40 and 100 m by default): a shrub stands about a tenth as tall as a tree.
const UNDERGROWTH_LOD_SCALE: f32 = 0.5;
/// With `--terrain`, no tree or shrub stands on ground steeper than this, in degrees.
const MAX_SLOPE_DEGREES: f32 = 30.0;
/// `--looks`: the colours of looks 1 and 2 (the pools' own look is 0), and what `--recolor`
/// changes them to: a lit autumn look and a flat, unlit one.
const LOOKS: [(PoolColors, PoolColors); 2] = [
    (
        PoolColors { wood: Color::srgb(0.38, 0.17, 0.08), leaves: Color::srgb(0.85, 0.22, 0.08), unlit: false },
        PoolColors { wood: Color::srgb(0.25, 0.10, 0.30), leaves: Color::srgb(0.55, 0.15, 0.75), unlit: false },
    ),
    (
        PoolColors { wood: Color::srgb(0.12, 0.10, 0.40), leaves: Color::srgb(0.30, 0.90, 0.80), unlit: true },
        PoolColors { wood: Color::srgb(0.55, 0.40, 0.08), leaves: Color::srgb(1.00, 0.85, 0.15), unlit: true },
    ),
];

/// The look of a tree at `x`: with `--looks` the forest's three bands along x (`extent` wide, centred),
/// else look 0.
fn look_at(x: f32, extent: f32, looks: bool) -> u32 {
    if !looks {
        return 0;
    }
    (((x / extent.max(1.0) + 0.5) * 3.0).floor() as i32).clamp(0, 2) as u32
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Naive,
    Pool,
    /// `ProceduralTree` with `GpuGrowth`, fully grown.
    Gpu,
    /// `ProceduralTree` with `GpuGrowth`, still growing every frame of the run.
    GpuLive,
    /// `ProceduralTree` without `GpuGrowth`, still growing every frame of the run.
    CpuLive,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Naive => "naive",
            Mode::Pool => "pool",
            Mode::Gpu => "gpu",
            Mode::GpuLive => "gpu-live",
            Mode::CpuLive => "cpu-live",
        }
    }
}

/// How the directional light's shadow maps are set up.
#[derive(Clone, Copy, PartialEq)]
enum Shadows {
    /// Bevy's defaults.
    Default,
    Off,
    /// Out to this many metres, in this many cascades.
    Reach(f32, usize),
}

/// `--fire`: where the fire is lit, or nowhere (armed: the fire exists and nothing burns).
#[derive(Clone, Copy, Debug)]
enum FireStart {
    Armed,
    At(Vec2, f32),
}

struct Args {
    trees: u32,
    mode: Mode,
    preset: TreePreset,
    /// How the trees are drawn.
    style: RenderStyle,
    frames: usize,
    warmup: Duration,
    spacing: f32,
    impostors: bool,
    /// Where the levels of detail and the impostor take over, if not the defaults.
    lod: Option<[f32; 3]>,
    /// How the sun's shadow maps are set up.
    shadows: Shadows,
    /// Where the sun stands; it looks at the origin.
    sun: Vec3,
    /// Where the camera stands and what it looks at, if not the default corner view.
    eye: Option<Vec3>,
    at: Option<Vec3>,
    /// Vertical field of view, degrees.
    fov: f32,
    /// An orthographic camera whose view is this many metres tall, instead of a perspective one.
    ortho: Option<f32>,
    /// How far above the horizon the pools' impostor pictures are taken, degrees.
    impostor_elevation: f32,
    /// Whether the pools' impostors cast shadows.
    impostor_shadows: bool,
    /// Whether the pools are built in the background (`BackgroundPools`) instead of at Startup.
    background_pools: bool,
    /// Strength of the wind, if not the library's default.
    wind: Option<f32>,
    /// Where the wind blows toward (world x and z), if not the library's default.
    wind_dir: Option<Vec2>,
    /// Speed of a gust front to send across the grid when the trees are ready, if any.
    gust: Option<f32>,
    /// Change the wind's strength every frame, as a game with weather would.
    wind_animate: bool,
    /// Knock the trees every few frames (`TreeShakes`), so every shake slot is always busy.
    shake_animate: bool,
    /// Let the leaves fall over and over, an autumn every this many seconds.
    leaf_fall_animate: Option<f32>,
    /// A `FloraFire` over the forest, lit where it says.
    fire: Option<FireStart>,
    /// A `FoliagePusher` standing there (x, z).
    push: Option<Vec2>,
    /// Step the clock a fixed 1/60 s per frame and count the warmup in frames, so two runs of
    /// different code render the same image at the same frame.
    deterministic: bool,
    /// Build the `ProceduralTree`s in the background (`BuildInBackground`).
    background: bool,
    /// Draw the `ProceduralTree`s at levels of detail (`TreeLod`, the `--lod` distances).
    tree_lod: bool,
    /// Render into an image, with no window.
    offscreen: bool,
    screenshot: Option<PathBuf>,
    /// How far deciduous trees have turned and shed their leaves.
    season: FloraSeason,
    /// How wet the trees are (`FloraWetness`).
    wet: f32,
    /// Species and their shares, to spread with `Scatter` instead of the grid.
    scatter: Option<Vec<(TreePreset, f32)>>,
    /// How wide a grove of one species grows (`Scatter::grove_size`), with `--scatter`.
    grove: f32,
    /// Radius of a clearing at the centre, with `--scatter`.
    clearing: f32,
    /// A stand of the last `--scatter` species: its centre (world x, z) and radius
    /// (`Scatter::points_with`).
    stand: Option<(Vec2, f32)>,
    /// Only the trees within this many metres of the camera cast shadows (`FloraShadows`).
    shadow_radius: Option<f32>,
    /// A second `Scatter`, of this species this far apart, clear of the trunks.
    undergrowth: Option<(TreePreset, f32)>,
    /// Rolling hills about this many metres high instead of flat ground (`FloraGround`).
    terrain: Option<f32>,
    /// A player stand-in at this point (above the ground) that the camera sees through the trees
    /// to (`SeeThroughTrees`), and the radius of the clear view.
    see_through: Option<(Vec3, f32)>,
    /// The camera's prepasses: 0 none, 1 depth, 2 depth, normal and motion vectors.
    prepass: u8,
    /// The pools get two more looks and the forest stands in three bands along x, one in each.
    looks: bool,
    /// With `--looks`: after the warmup, looks 1 and 2 change colour (`RecolorPool`).
    recolor: bool,
    /// With `--recolor`: the looks glide to their new colours over this many seconds.
    glide: f32,
    /// Pool mode: every species' pool built at this many ages, the trees spread among them.
    ages: u32,
}

impl Args {
    /// Whether the camera draws into an image instead of a window.
    fn is_offscreen(&self) -> bool {
        self.offscreen || self.screenshot.is_some()
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: forest [--trees <n>] [--mode <naive|pool|gpu|gpu-live|cpu-live>] [--preset <name>] [--frames <n>] \
         [--warmup-secs <s>] [--spacing <m>] [--no-impostors] [--lod <a,b,c>] \
         [--shadows <off|distance,cascades>] [--sun <x,y,z>] [--wind <strength>] \
         [--eye <x,y,z>] [--at <x,y,z>] [--fov <deg>] [--ortho <view height>] \
         [--impostor-elevation <deg>] [--impostor-shadows] [--background-pools] \
         [--wind-dir <x,z>] [--gust <speed>] [--wind-animate] [--shake-animate] [--leaf-fall-animate <s>] [--deterministic] \
         [--offscreen] [--screenshot <path>] [--style <natural|painterly|toon>] \
         [--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]] [--wet <0..1>] \
         [--scatter <preset[:weight]>,... [--grove <m>] [--clearing <m>] [--stand <x,z,radius>]] \
         [--undergrowth <preset>[:<spacing>]] [--terrain <relief>] [--background] [--tree-lod] [--shadow-radius <m>] \
         [--see-through <x,y,z>[,<radius>]] [--prepass <none|depth|all>] [--fire <x,z[,radius]|armed>] [--push <x,z>] \
         [--looks [--recolor [--glide <s>]]] [--ages <n>]"
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args {
        trees: 10_000,
        mode: Mode::Pool,
        preset: TreePreset::Oak,
        style: RenderStyle::Natural,
        frames: 600,
        warmup: Duration::from_secs(5),
        spacing: 8.0,
        impostors: true,
        lod: None,
        shadows: Shadows::Default,
        sun: Vec3::new(-20.0, 25.0, 20.0),
        eye: None,
        at: None,
        fov: 45.0,
        ortho: None,
        impostor_elevation: 10.0,
        impostor_shadows: false,
        background_pools: false,
        wind: None,
        wind_dir: None,
        gust: None,
        wind_animate: false,
        shake_animate: false,
        leaf_fall_animate: None,
        fire: None,
        push: None,
        deterministic: false,
        background: false,
        tree_lod: false,
        offscreen: false,
        screenshot: None,
        season: FloraSeason::SUMMER,
        wet: 0.0,
        scatter: None,
        grove: 50.0,
        clearing: 0.0,
        stand: None,
        shadow_radius: None,
        undergrowth: None,
        terrain: None,
        see_through: None,
        prepass: 0,
        looks: false,
        recolor: false,
        glide: 0.0,
        ages: 1,
    };
    let mut it = std::env::args().skip(1);
    fn value<T: std::str::FromStr>(it: &mut impl Iterator<Item = String>) -> T {
        it.next()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| usage())
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--trees" => args.trees = value(&mut it),
            "--mode" => {
                args.mode = match it.next().as_deref() {
                    Some("naive") => Mode::Naive,
                    Some("pool") => Mode::Pool,
                    Some("gpu") => Mode::Gpu,
                    Some("gpu-live") => Mode::GpuLive,
                    Some("cpu-live") => Mode::CpuLive,
                    _ => usage(),
                }
            }
            "--preset" => {
                let name = it.next().unwrap_or_else(|| usage());
                args.preset = TreePreset::ALL
                    .into_iter()
                    .find(|p| p.name() == name)
                    .unwrap_or_else(|| usage());
            }
            "--style" => {
                args.style = match it.next().as_deref() {
                    Some("natural") => RenderStyle::Natural,
                    Some("painterly") => RenderStyle::Painterly,
                    Some("toon") => RenderStyle::Toon,
                    _ => usage(),
                }
            }
            "--frames" => args.frames = value(&mut it),
            "--warmup-secs" => args.warmup = Duration::from_secs_f32(value(&mut it)),
            "--spacing" => args.spacing = value(&mut it),
            "--no-impostors" => args.impostors = false,
            "--offscreen" => args.offscreen = true,
            "--shadows" => {
                args.shadows = match it.next().as_deref() {
                    Some("off") => Shadows::Off,
                    Some(reach) => {
                        let parts: Vec<&str> = reach.split(',').collect();
                        match parts[..] {
                            [distance, cascades] => Shadows::Reach(
                                distance.parse().unwrap_or_else(|_| usage()),
                                cascades.parse().unwrap_or_else(|_| usage()),
                            ),
                            _ => usage(),
                        }
                    }
                    None => usage(),
                }
            }
            "--sun" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, y, z]: [f32; 3] = parts.try_into().unwrap_or_else(|_| usage());
                args.sun = Vec3::new(x, y, z);
            }
            "--eye" | "--at" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, y, z]: [f32; 3] = parts.try_into().unwrap_or_else(|_| usage());
                if arg == "--eye" {
                    args.eye = Some(Vec3::new(x, y, z));
                } else {
                    args.at = Some(Vec3::new(x, y, z));
                }
            }
            "--fov" => args.fov = value(&mut it),
            "--ortho" => args.ortho = Some(value(&mut it)),
            "--impostor-elevation" => args.impostor_elevation = value(&mut it),
            "--impostor-shadows" => args.impostor_shadows = true,
            "--background-pools" => args.background_pools = true,
            "--wind" => args.wind = Some(value(&mut it)),
            "--wind-dir" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, z]: [f32; 2] = parts.try_into().unwrap_or_else(|_| usage());
                args.wind_dir = Some(Vec2::new(x, z));
            }
            "--gust" => args.gust = Some(value(&mut it)),
            "--wind-animate" => args.wind_animate = true,
            "--shake-animate" => args.shake_animate = true,
            "--leaf-fall-animate" => args.leaf_fall_animate = Some(value(&mut it)),
            "--deterministic" => args.deterministic = true,
            "--background" => args.background = true,
            "--tree-lod" => args.tree_lod = true,
            "--scatter" => {
                let list = it.next().unwrap_or_else(|| usage());
                let species = list
                    .split(',')
                    .map(|item| {
                        let (name, weight) = item.split_once(':').unwrap_or((item, "1"));
                        let preset = TreePreset::ALL.into_iter().find(|p| p.name() == name).unwrap_or_else(|| usage());
                        (preset, weight.parse::<f32>().unwrap_or_else(|_| usage()))
                    })
                    .collect();
                args.scatter = Some(species);
            }
            "--grove" => args.grove = value(&mut it),
            "--clearing" => args.clearing = value(&mut it),
            "--stand" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                args.stand = match parts[..] {
                    [x, z, radius] => Some((Vec2::new(x, z), radius)),
                    _ => usage(),
                };
            }
            "--shadow-radius" => args.shadow_radius = Some(value(&mut it)),
            "--undergrowth" => {
                let item = it.next().unwrap_or_else(|| usage());
                let (name, spacing) = item.split_once(':').unwrap_or((&item, "2.5"));
                let preset = TreePreset::ALL.into_iter().find(|p| p.name() == name).unwrap_or_else(|| usage());
                args.undergrowth = Some((preset, spacing.parse::<f32>().unwrap_or_else(|_| usage())));
            }
            "--terrain" => args.terrain = Some(value(&mut it)),
            "--see-through" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                args.see_through = match parts[..] {
                    [x, y, z] => Some((Vec3::new(x, y, z), 1.5)),
                    [x, y, z, radius] => Some((Vec3::new(x, y, z), radius)),
                    _ => usage(),
                };
            }
            "--prepass" => {
                args.prepass = match it.next().as_deref() {
                    Some("none") => 0,
                    Some("depth") => 1,
                    Some("all") => 2,
                    _ => usage(),
                };
            }
            "--push" => {
                let item = it.next().unwrap_or_else(|| usage());
                let v: Vec<f32> = item.split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect();
                let [x, z]: [f32; 2] = v.try_into().unwrap_or_else(|_| usage());
                args.push = Some(Vec2::new(x, z));
            }
            "--fire" => {
                let item = it.next().unwrap_or_else(|| usage());
                args.fire = Some(if item == "armed" {
                    FireStart::Armed
                } else {
                    let v: Vec<f32> = item.split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect();
                    match v[..] {
                        [x, z] => FireStart::At(Vec2::new(x, z), 4.0),
                        [x, z, radius] => FireStart::At(Vec2::new(x, z), radius),
                        _ => usage(),
                    }
                });
            }
            "--season" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let (autumn, leaf_fall, snow, blossom) = match parts[..] {
                    [autumn, leaf_fall] => (autumn, leaf_fall, 0.0, 0.0),
                    [autumn, leaf_fall, snow] => (autumn, leaf_fall, snow, 0.0),
                    [autumn, leaf_fall, snow, blossom] => (autumn, leaf_fall, snow, blossom),
                    _ => usage(),
                };
                args.season = FloraSeason { autumn, leaf_fall, snow, blossom };
            }
            "--wet" => args.wet = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            "--lod" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                args.lod = Some(parts.try_into().unwrap_or_else(|_| usage()));
            }
            "--looks" => args.looks = true,
            "--recolor" => args.recolor = true,
            "--glide" => args.glide = value(&mut it),
            "--ages" => args.ages = value(&mut it),
            "--screenshot" => args.screenshot = Some(it.next().unwrap_or_else(|| usage()).into()),
            _ => usage(),
        }
    }
    if args.trees == 0 || args.frames == 0 || args.spacing <= 0.0 {
        usage();
    }
    args
}

#[derive(PartialEq, Eq)]
enum Phase {
    /// Waiting for every tree to get its render entities.
    Loading,
    /// Trees exist; rendering until the warmup time has passed.
    Warmup,
    Measuring,
    /// A screenshot was requested and has not come back yet.
    Capturing,
    /// The result was printed; the app is exiting.
    Done,
}

#[derive(Resource)]
struct Bench {
    args: Args,
    phase: Phase,
    /// When the trees all existed.
    loaded: Instant,
    /// Start of the frame being loaded, and the longest frame while loading, seconds.
    loading_frame: Instant,
    longest_loading: f32,
    /// Start of the previous frame while measuring.
    last: Option<Instant>,
    /// Process CPU seconds (all threads) when measuring began.
    cpu_start: Option<f64>,
    /// Frames rendered in the warmup so far.
    warm_frames: u32,
    /// Seconds per measured frame.
    frames: Vec<f32>,
    /// Offscreen target for `--screenshot`.
    target: Handle<Image>,
    /// The side of the square the trees cover, in metres.
    extent: f32,
    /// How many trees were placed.
    placed: u32,
    /// `--recolor` has been done.
    recolored: bool,
}

/// Marks a tree the benchmark spawned.
#[derive(Component)]
struct ForestTree;

fn main() {
    let args = parse_args();
    let mut app = App::new();
    // Offscreen: no window to present to, so nothing paces the frames.
    let window = if args.is_offscreen() {
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        }
    } else {
        WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(WINDOW.x, WINDOW.y)
                    .with_scale_factor_override(1.0),
                present_mode: PresentMode::AutoNoVsync,
                title: "bevy_flora forest".to_string(),
                ..default()
            }),
            ..default()
        }
    };
    if args.wind.is_some() || args.wind_dir.is_some() {
        let base = FloraWind::default();
        app.insert_resource(FloraWind {
            strength: args.wind.unwrap_or(base.strength),
            direction: args.wind_dir.unwrap_or(base.direction),
            ..base
        });
    }
    if args.deterministic {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    }
    app.insert_resource(args.season).insert_resource(FloraWetness(args.wet));
    let leaf_fall = args.leaf_fall_animate.is_some();
    let fire = args.fire.is_some();
    // Unfocused, Bevy's default settings update 60 times a second; a benchmark
    // must run flat out whatever has the focus. `continuous` waits for a window's redraw on Linux
    // (X11 and Wayland), so offscreen, with no window, the app would never update again: there a
    // reactive mode with no wait polls instead.
    let settings = if args.is_offscreen() {
        let flat_out = bevy::winit::UpdateMode::reactive(Duration::ZERO);
        bevy::winit::WinitSettings { focused_mode: flat_out, unfocused_mode: flat_out }
    } else {
        bevy::winit::WinitSettings::continuous()
    };
    app.insert_resource(settings)
        .add_plugins((DefaultPlugins.set(window), FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.6, 0.6, 0.6),
            brightness: 300.0,
            ..default()
        })
        .insert_resource(Bench {
            args,
            phase: Phase::Loading,
            loaded: Instant::now(),
            loading_frame: Instant::now(),
            longest_loading: 0.0,
            last: None,
            frames: Vec::new(),
            cpu_start: None,
            warm_frames: 0,
            target: Handle::default(),
            extent: 0.0,
            placed: 0,
            recolored: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, drive);
    // Only with the flag: an always-checked condition would put a wait on `Bench` in every frame.
    if leaf_fall {
        app.add_systems(Update, animate_leaf_fall);
    }
    if fire {
        app.add_systems(Update, light_fire);
    }
    app.run();
}

/// Grid side length (in trees) and the grid's extent in metres.
fn grid(trees: u32, spacing: f32) -> (u32, f32) {
    let side = (trees as f32).sqrt().ceil() as u32;
    (side, side as f32 * spacing)
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut pools: ResMut<Assets<TreePool>>,
    mut background: BackgroundPools,
) {
    let placing = Instant::now();
    let ground = ground(&bench.args);
    let (trees, extent) = placements(&bench.args, &ground);
    let placing = placing.elapsed().as_secs_f32() * 1000.0;
    bench.extent = extent;
    let half = extent * 0.5;
    // `--fire`: from the start, so its shaders are made while the trees load, in cells of about 2 m
    // (at most 512 a side).
    if bench.args.fire.is_some() {
        let cells = (extent / 2.0).ceil().clamp(2.0, FloraFire::MAX_CELLS as f32) as u32;
        commands.insert_resource(FloraFire::with_cells(Vec2::ZERO, extent + 8.0, cells));
    }
    // `--push`: a body pushing foliage aside, standing there (no mesh: what the shaders pay).
    if let Some(at) = bench.args.push {
        commands.spawn((Transform::from_xyz(at.x, ground.height(at) + 0.9, at.y), FoliagePusher::default()));
    }

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: bench.args.shadows != Shadows::Off,
            illuminance: 10_000.0,
            ..default()
        },
        match bench.args.shadows {
            Shadows::Reach(maximum_distance, num_cascades) => CascadeShadowConfigBuilder {
                maximum_distance,
                num_cascades,
                ..default()
            }
            .build(),
            _ => CascadeShadowConfig::default(),
        },
        // High, off to one side by default: the trees are lit from the side.
        Transform::from_translation(bench.args.sun).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let side = extent * 1.2 + 200.0;
    let ground_mesh = match bench.args.terrain {
        Some(_) => terrain_mesh(&ground, side),
        None => Plane3d::default().mesh().size(side, side).build(),
    };
    if bench.args.terrain.is_some() {
        commands.insert_resource(ground.clone());
    }
    commands.spawn((
        Mesh3d(meshes.add(ground_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Grass, or snow over it as far as `--season` says it snowed.
            base_color: {
                let grass = Vec3::new(0.35, 0.45, 0.30);
                let c = grass.lerp(Vec3::new(0.86, 0.88, 0.92), bench.args.season.snow);
                Color::srgb(c.x, c.y, c.z)
            },
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));

    // With `--terrain` the camera and its aim are given above the ground under them.
    let view = {
        let lift = |p: Vec3| p + Vec3::Y * ground.height(p.xz());
        let eye = lift(bench.args.eye.unwrap_or(Vec3::new(-half, CAMERA_HEIGHT, -half)));
        let at = lift(bench.args.at.unwrap_or(Vec3::new(0.0, 8.0, 0.0)));
        Transform::from_translation(eye).looking_at(at, Vec3::Y)
    };
    let mut camera = commands.spawn((
        Camera3d::default(),
        // `--ortho`: an orthographic view that many metres tall, which the levels of detail follow.
        match bench.args.ortho {
            Some(height) => Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical { viewport_height: height },
                far: extent * 2.0 + 500.0,
                ..OrthographicProjection::default_3d()
            }),
            None => Projection::Perspective(PerspectiveProjection {
                fov: bench.args.fov.to_radians(),
                far: extent * 2.0 + 500.0,
                ..default()
            }),
        },
        view,
    ));
    if bench.args.is_offscreen() {
        bench.target = images.add(Image::new_target_texture(
            WINDOW.x,
            WINDOW.y,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        camera.insert(RenderTarget::Image(bench.target.clone().into()));
    }
    // `--prepass`: the camera's prepasses, as TAA, SSAO or depth of field would add them.
    if bench.args.prepass >= 1 {
        camera.insert(bevy::core_pipeline::prepass::DepthPrepass);
    }
    if bench.args.prepass >= 2 {
        camera.insert((bevy::core_pipeline::prepass::NormalPrepass, bevy::core_pipeline::prepass::MotionVectorPrepass));
    }
    // `--see-through`: a stand-in for the player at the target, which the camera keeps in view.
    if let Some((target, radius)) = bench.args.see_through {
        let target = target + Vec3::Y * ground.height(target.xz());
        camera.insert(SeeThroughTrees { target, radius, ..default() });
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.3, 1.1))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.12, 0.08), ..default() })),
            Transform::from_translation(target),
        ));
    }

    // One species, or one per `--scatter` entry, then the undergrowth's: a pool each in pool mode.
    let bases: Vec<TreeParams> = bench
        .args
        .scatter
        .as_ref()
        .map_or_else(|| vec![bench.args.preset], |species| species.iter().map(|&(preset, _)| preset).collect())
        .into_iter()
        // The undergrowth is the last species.
        .chain(bench.args.undergrowth.map(|(preset, _)| preset))
        .map(|preset| {
            let mut base = preset.params();
            base.seed = 1;
            base.style = bench.args.style;
            base
        })
        .collect();
    // Levels of detail of each species: `--lod`'s distances, nearer for the undergrowth.
    let undergrowth = bench.args.undergrowth.map(|_| bases.len() - 1);
    let lods: Vec<LodSettings> = (0..bases.len())
        .map(|i| {
            let mut lod = LodSettings::default();
            if let Some([lod1, lod2, impostor]) = bench.args.lod {
                lod.distances = LodDistances { lod1, lod2, impostor };
            }
            if !bench.args.impostors {
                lod.distances.impostor = f32::INFINITY;
            }
            lod.impostor_elevation = bench.args.impostor_elevation;
            lod.impostor_shadows = bench.args.impostor_shadows;
            // A shrub is a tenth of a tree's height, so its coarser levels can take over much
            // nearer.
            if Some(i) == undergrowth {
                let d = lod.distances;
                let near = UNDERGROWTH_LOD_SCALE;
                lod.distances = LodDistances { lod1: d.lod1 * near, lod2: d.lod2 * near, impostor: d.impostor * near };
            }
            lod
        })
        .collect();
    let pools: Vec<Vec<Handle<TreePool>>> = if bench.args.mode == Mode::Pool {
        let started = Instant::now();
        let in_background = bench.args.background_pools;
        // `--ages <n>`: every species at n ages, 4, 8, ... years and the last one mature.
        let ages: Vec<f32> = (1..=bench.args.ages.max(1))
            .map(|k| if k == bench.args.ages.max(1) { f32::INFINITY } else { 4.0 * k as f32 })
            .collect();
        let built: Vec<Vec<_>> = bases
            .iter()
            .zip(&lods)
            .map(|(base, lod)| {
                ages.iter()
                    .map(|&age| {
                        if in_background {
                            background.build(base, VARIANTS, age, lod)
                        } else {
                            let mut pool = build_pool(base, VARIANTS, age, lod, &mut meshes);
                            if bench.args.looks {
                                // Looks 1 and 2 after the pool's own look 0.
                                for (colors, _) in &LOOKS {
                                    pool.add_look(*colors);
                                }
                            }
                            pools.add(pool)
                        }
                    })
                    .collect()
            })
            .collect();
        eprintln!(
            "forest: {} {} pool(s) of {VARIANTS} variants in {:.2} s",
            if in_background { "started" } else { "built" },
            built.iter().map(Vec::len).sum::<usize>(),
            started.elapsed().as_secs_f32()
        );
        built
    } else {
        Vec::new()
    };
    bench.placed = trees.len() as u32;
    eprintln!("forest: placed {} trees over {extent:.0} m in {placing:.1} ms", trees.len());
    // `--shadow-radius`: the trees farther from the camera cast no shadow.
    if let Some(radius) = bench.args.shadow_radius {
        commands.insert_resource(FloraShadows { radius });
    }
    for (transform, species, variant) in trees {
        let base = &bases[species];
        if let Some(pool) = pools.get(species).map(|ages| &ages[variant as usize % ages.len()]) {
            let look = PooledLook(look_at(transform.translation.x, bench.extent, bench.args.looks));
            commands.spawn((PooledTree { pool: pool.clone(), variant }, look, transform, ForestTree));
            continue;
        }
        let mut params: TreeParams = base.clone();
        params.seed = base.seed.wrapping_add(variant as i32);
        let mut tree = commands.spawn((ProceduralTree(params), transform, ForestTree));
        match bench.args.mode {
            Mode::Gpu => {
                tree.insert(GpuGrowth);
            }
            // Young enough to keep growing for as long as the run takes.
            Mode::GpuLive => {
                tree.insert((GpuGrowth, TreeAge(3.0), Growing { rate: 0.01 }));
            }
            // The same, rebuilding the meshes on the CPU every frame.
            Mode::CpuLive => {
                tree.insert((TreeAge(3.0), Growing { rate: 0.01 }));
            }
            _ => {}
        }
        if bench.args.background {
            tree.insert(BuildInBackground);
        }
        if bench.args.tree_lod {
            tree.insert(TreeLod(lods[species]));
        }
    }
}

/// The ground: flat at 0, or with `--terrain <relief>` rolling hills about `relief` metres high.
fn ground(args: &Args) -> FloraGround {
    match args.terrain {
        None => FloraGround::default(),
        Some(relief) => FloraGround::new(move |at: Vec2| hills(at, relief)),
    }
}

/// Rolling hills: crossed waves about 330, 160 and 75 m long whose crests a slow warp bends, so
/// they wander; they reach about `relief` metres above and below 0.
fn hills(at: Vec2, relief: f32) -> f32 {
    let q = at + Vec2::new((at.y * 0.011).sin(), (at.x * 0.013).cos()) * 25.0;
    relief
        * (0.5 * (q.x * 0.019).sin() * (q.y * 0.016).cos()
            + 0.3 * (q.x * 0.041 + 1.3).sin() * (q.y * 0.037 + 0.4).sin()
            + 0.2 * ((q.x + q.y) * 0.083).sin())
}

/// A square of ground `side` metres across, centred on the origin, following `ground`: cells of
/// 2 m (fewer, larger ones past 600 a side), smooth normals from the ground's slope.
fn terrain_mesh(ground: &FloraGround, side: f32) -> Mesh {
    let cells = ((side / 2.0).ceil() as u32).clamp(1, 600);
    let step = side / cells as f32;
    let n = cells + 1;
    let mut positions = Vec::with_capacity((n * n) as usize);
    let mut normals = Vec::with_capacity((n * n) as usize);
    let mut uvs = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let at = Vec2::new(i as f32 * step, j as f32 * step) - side * 0.5;
            positions.push([at.x, ground.height(at), at.y]);
            normals.push(ground.normal(at).to_array());
            uvs.push([i as f32 / cells as f32, j as f32 / cells as f32]);
        }
    }
    let mut indices = Vec::with_capacity((cells * cells * 6) as usize);
    for j in 0..cells {
        for i in 0..cells {
            let a = j * n + i;
            let (b, c) = (a + 1, a + n);
            // Counter-clockwise seen from above.
            indices.extend([a, c, b, b, c, c + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

/// Where the trees stand (transform, species, variant) and the side of the square they cover:
/// a jittered grid, or with `--scatter` a `Scatter` sized to hold about `--trees` trees, plus
/// with `--undergrowth` a second `Scatter` of the undergrowth species (the last pool) that keeps
/// clear of the trunks. Each stands at its footing on `ground`; with `--terrain` none stands on
/// ground steeper than [`MAX_SLOPE_DEGREES`].
fn placements(args: &Args, ground: &FloraGround) -> (Vec<(Transform, usize, u32)>, f32) {
    let (mut trees, extent) = tree_placements(args, ground);
    let Some((shrub, spacing)) = args.undergrowth else { return (trees, extent) };
    // Trunks by 2 m cell, so a shrub looks at the nine cells around it only.
    const CELL: f32 = 2.0;
    const TRUNK_CLEARANCE: f32 = 1.5;
    let cell = |p: Vec2| ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
    let mut trunks: std::collections::HashMap<(i32, i32), Vec<Vec2>> = Default::default();
    for (transform, ..) in &trees {
        let at = transform.translation.xz();
        trunks.entry(cell(at)).or_default().push(at);
    }
    let near_a_trunk = |at: Vec2| {
        let (cx, cz) = cell(at);
        (cx - 1..=cx + 1).any(|x| {
            (cz - 1..=cz + 1).any(|z| {
                trunks.get(&(x, z)).is_some_and(|ps| ps.iter().any(|p| p.distance(at) < TRUNK_CLEARANCE))
            })
        })
    };
    let species = args.scatter.as_ref().map_or(1, Vec::len);
    let half = extent * 0.5;
    let clearing = args.clearing * 0.75;
    // Thickets and gaps some 30 m across rather than an even carpet: two crossed waves.
    let patches = |at: Vec2| {
        let wave = (at.x * 0.19 + (at.y * 0.13).sin() * 2.0).sin() * (at.y * 0.17 + (at.x * 0.11).cos() * 2.0).cos();
        let t = ((wave + 0.6) / 1.2).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let shrub_params = shrub.params();
    let steep = |at: Vec2| args.terrain.is_some() && ground.slope(at) > MAX_SLOPE_DEGREES.to_radians();
    let shrubs = Scatter { seed: 2, ..Scatter::new(Rect::new(-half, -half, half, half), spacing) }
        .points_where(|at| if at.length() < clearing || near_a_trunk(at) || steep(at) { 0.0 } else { patches(at) });
    trees.extend(
        shrubs.into_iter().map(|shrub| (shrub.transform_on(ground, &shrub_params), species, shrub.variant % VARIANTS)),
    );
    (trees, extent)
}

/// The trees alone: see [`placements`].
fn tree_placements(args: &Args, ground: &FloraGround) -> (Vec<(Transform, usize, u32)>, f32) {
    let spacing = args.spacing;
    let steep = |at: Vec2| args.terrain.is_some() && ground.slope(at) > MAX_SLOPE_DEGREES.to_radians();
    let Some(species) = &args.scatter else {
        let (side, extent) = grid(args.trees, spacing);
        let radius = args.preset.params().base_radius();
        let half = extent * 0.5;
        let trees = (0..args.trees)
            .map(|i| {
                let mut rng = SeededRng::new(i as i32);
                let jitter = spacing / 3.0;
                let x = (i % side) as f32 * spacing - half + rng.range(-jitter, jitter);
                let z = (i / side) as f32 * spacing - half + rng.range(-jitter, jitter);
                let yaw = rng.range(0.0, std::f32::consts::TAU);
                let y = ground.footing(Vec2::new(x, z), radius);
                (Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(yaw)), 0, i % VARIANTS)
            })
            .collect();
        return (trees, extent);
    };
    // Poisson-disc trunks take about 1 / 0.68 spacing² each.
    let extent = (args.trees as f32 / 0.68).sqrt() * spacing;
    let half = extent * 0.5;
    let scatter = Scatter {
        seed: 1,
        species: species.iter().map(|&(_, weight)| weight).collect(),
        grove_size: args.grove,
        ..Scatter::new(Rect::new(-half, -half, half, half), spacing)
    };
    let clearing = args.clearing;
    let params: Vec<TreeParams> = species.iter().map(|&(preset, _)| preset.params()).collect();
    // `--stand`: the last species alone inside the disc, fading out over a rim half its radius
    // wide, and nowhere else.
    let last = species.len() - 1;
    let stand = |at: Vec2, kind: usize| match args.stand {
        None => 1.0,
        Some((centre, radius)) => {
            let out = ((at.distance(centre) - 0.75 * radius) / (0.5 * radius)).clamp(0.0, 1.0);
            if kind == last { 1.0 - out } else { out }
        }
    };
    let trees = scatter
        .points_with(|at| if at.length() < clearing || steep(at) { 0.0 } else { 1.0 }, stand)
        .into_iter()
        .map(|tree| (tree.transform_on(ground, &params[tree.species]), tree.species, tree.variant % VARIANTS))
        .collect();
    (trees, extent)
}

/// With `--fire <x>,<z>[,<radius>]`: light the fire once the trees are placed. (`--fire armed`
/// lights nothing; the fire is there, so its shaders are.)
fn light_fire(bench: Res<Bench>, fire: Option<ResMut<FloraFire>>, mut lit: Local<bool>) {
    if *lit || bench.phase == Phase::Loading {
        return;
    }
    *lit = true;
    if let (Some(FireStart::At(at, radius)), Some(mut fire)) = (bench.args.fire, fire) {
        fire.ignite(Vec3::new(at.x, 0.0, at.y), radius);
    }
}

/// `--leaf-fall-animate`: an autumn every `period` seconds, over and over, so the trees near the
/// camera are always letting leaves fall, the season's worst case. A system of its own, added only
/// with the flag, so that without it nothing else waits on the season or the bench.
fn animate_leaf_fall(bench: Res<Bench>, time: Res<Time>, mut season: ResMut<FloraSeason>) {
    if let Some(period) = bench.args.leaf_fall_animate
        && bench.phase != Phase::Loading
    {
        season.autumn = 1.0;
        season.leaf_fall = (time.elapsed_secs() / period.max(0.1)).fract();
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    unfinished: Query<(), (With<ForestTree>, Without<Children>)>,
    screenshots: Query<(), With<Screenshot>>,
    pools: Res<Assets<TreePool>>,
    mut exit: MessageWriter<AppExit>,
    mut recolor: MessageWriter<RecolorPool>,
    mut wind: ResMut<FloraWind>,
    time: Res<Time>,
    mut shakes: ResMut<TreeShakes>,
    mut knocks: Local<u32>,
) {
    let bench = &mut *bench;
    if bench.args.wind_animate && bench.phase != Phase::Loading {
        // Mutated every frame on purpose: the wind changes, so the shared wind buffer is written
        // every frame (and no material is touched).
        wind.strength = 0.6 + 0.3 * (time.elapsed_secs() * 1.7).sin();
    }
    if bench.args.shake_animate && bench.phase != Phase::Loading {
        // A knock every sixth frame, wandering over the near half of the forest, so all eight
        // shake slots are busy every frame: the worst case for the vertex stage.
        *knocks += 1;
        if knocks.is_multiple_of(6) {
            let a = *knocks as f32 * 0.37;
            let at = Vec2::new(a.cos(), a.sin()) * (bench.extent * 0.15);
            shakes.shake(TreeShake { at, radius: 20.0, strength: 1.5, duration: 3.0, ..default() }, &time);
        }
    }
    match bench.phase {
        Phase::Loading => {
            // The longest frame while loading: building the pools in the frame shows up here.
            let now = Instant::now();
            bench.longest_loading = bench.longest_loading.max((now - bench.loading_frame).as_secs_f32());
            bench.loading_frame = now;
            if unfinished.is_empty() {
                eprintln!(
                    "forest: {} trees ready after {:.1} s, the longest frame {:.0} ms",
                    bench.placed,
                    bench.loaded.elapsed().as_secs_f32(),
                    bench.longest_loading * 1000.0
                );
                bench.loaded = Instant::now();
                bench.phase = Phase::Warmup;
                if let Some(speed) = bench.args.gust {
                    // From the forest's -x edge, along +x, across every tree.
                    let extent = bench.extent;
                    wind.launch_gust(
                        GustFront {
                            origin: Vec2::new(-extent * 0.5, 0.0),
                            direction: Vec2::X,
                            speed,
                            width: 20.0,
                            strength: 1.5,
                            ..default()
                        },
                        &time,
                    );
                }
            }
        }
        Phase::Warmup => {
            bench.warm_frames += 1;
            let warm = if bench.args.deterministic {
                (bench.warm_frames as f32 / 60.0) < bench.args.warmup.as_secs_f32()
            } else {
                bench.loaded.elapsed() < bench.args.warmup
            };
            if warm {
                return;
            }
            // `--recolor`: once the forest has stood a while, two of its looks change colour (the
            // pool is not touched, no tree rebuilt), and it stands another warmup in them.
            if bench.args.recolor && !bench.recolored {
                bench.recolored = true;
                for (id, _) in pools.iter() {
                    for (look, (_, colors)) in LOOKS.iter().enumerate() {
                        recolor.write(RecolorPool { pool: id, look: look as u32 + 1, colors: *colors, glide: bench.args.glide });
                    }
                }
                eprintln!("forest: recoloured looks 1 and 2");
                bench.warm_frames = 0;
                bench.loaded = Instant::now();
                return;
            }
            // Trees wait for their impostors, so a pool without them here means the
            // bake went wrong (a shader that does not compile, a scene that never rendered).
            if bench.args.mode == Mode::Pool && bench.args.impostors {
                let baked = pools
                    .iter()
                    .all(|(_, pool)| pool.variants.iter().all(|v| v.impostor.is_some()));
                if !baked {
                    eprintln!("forest: the impostors were not baked");
                    exit.write(AppExit::error());
                    return;
                }
            }
            if let Some(path) = bench.args.screenshot.clone() {
                commands
                    .spawn(Screenshot::image(bench.target.clone()))
                    .observe(save_to_disk(path));
                bench.phase = Phase::Capturing;
            } else {
                bench.phase = Phase::Measuring;
                bench.cpu_start = cpu_seconds();
            }
        }
        Phase::Measuring => {
            let now = Instant::now();
            if let Some(last) = bench.last.replace(now) {
                bench.frames.push((now - last).as_secs_f32());
            }
            if bench.frames.len() >= bench.args.frames {
                report(bench);
                bench.phase = Phase::Done;
                exit.write(AppExit::Success);
            }
        }
        Phase::Capturing => {
            // `save_to_disk` despawns the screenshot entity once it has written the file.
            if screenshots.is_empty() {
                exit.write(AppExit::Success);
            }
        }
        Phase::Done => {}
    }
}

fn report(bench: &Bench) {
    let mut sorted = bench.frames.clone();
    sorted.sort_by(f32::total_cmp);
    let mean = bench.frames.iter().sum::<f32>() / bench.frames.len() as f32;
    let p95 = sorted[((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1)];
    let median = sorted[sorted.len() / 2];
    // CPU time of every thread per frame: unlike the frame time it does not grow when other
    // programs compete for the machine, and it is far below the frame time when the GPU is the
    // limit.
    let cpu = match (bench.cpu_start, cpu_seconds()) {
        (Some(start), Some(end)) => format!("{:.2}", (end - start) * 1000.0 / bench.frames.len() as f64),
        _ => "?".to_string(),
    };
    println!(
        "forest {} {} {} trees: {:.2} ms/frame ({:.1} fps), p95 {:.2} ms, median {:.2} ms, min {:.2} ms, cpu {} ms/frame",
        bench.args.mode.name(),
        bench.args.preset.name(),
        bench.placed,
        mean * 1000.0,
        1.0 / mean,
        p95 * 1000.0,
        median * 1000.0,
        sorted[0] * 1000.0,
        cpu,
    );
}

/// CPU time this process has used so far, all threads, in seconds (`ps`, 10 ms resolution).
fn cpu_seconds() -> Option<f64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "cputime=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    // `[hh:]mm:ss.cc`
    let mut seconds = 0.0;
    for part in text.trim().split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}
