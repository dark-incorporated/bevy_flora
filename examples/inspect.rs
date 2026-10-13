//! Photographs one tree under controlled conditions, to look at how readable it is: from a
//! distance, from every side, at several zooms and in a row of frames.
//!
//! ```text
//! cargo run --release --example inspect -- --out <dir> [--preset <name>] [--mode pool|tree|deadfall]
//!     [--age <a>] [--seed <n>] [--frames <n>] [--distance <m>[,<m>]] [--constant-size [<share>]]
//!     [--fov <deg>] [--yaw <deg>] [--eye <m>] [--look <share>] [--sun <x,y,z>] [--wind <s>]
//!     [--size <w>x<h>] [--lod <a,b,c>] [--no-impostors] [--level <0|1|2|3>] [--empty] [--no-msaa]
//!     [--style <natural|painterly|toon>] [--colliders] [--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]] [--wet <0..1>]
//!     [--fell <deg> [--buck <m>] [--lying]] [--slope <deg>] [--on-centre] [--drop <count>[,<after>[,<every>]]]
//!     [--autumn <seconds>[,<after>[,<every>]]] [--burn <after>[,<every>]]
//!     [--push <x>,<z>[,<radius>[,<vx>,<vz>]]] [--notch <deg>,<depth>[,<height>]]
//!     [--hull <tiers>,<sides>[,<lift>[,<jitter>]]] [--prism <sides>] [--branches]
//! ```
//!
//! The tree stands at the origin; the camera stands `--distance` metres away on the bearing
//! `--yaw` (0 is +z), `--eye` metres above the ground, and looks at `--look` (a share of the
//! tree's height, 0.5 by default) on the trunk's axis. With `--frames n` and two distances the
//! camera backs off from the first to the second in `n` steps, spaced geometrically, so the
//! tree passes through every level of detail and into the impostor. Each frame is written to
//! `<dir>/frame_<nnnn>.png` and the camera's distance and field of view to `<dir>/frames.tsv`.
//!
//! `--mode deadfall` draws pooled deadfall instead of one tree ([`PooledDeadfall`](bevy_flora::PooledDeadfall)):
//! two variants each of a stump, a tree lying where it fell and a bucked one, in rows 12 m apart, the
//! fallen toward +x (down a `--slope`); `--preset`, `--hull` and `--prism` choose the tree.
//!
//! `--mode pool` (the default) draws the tree from a [`TreePool`](bevy_flora::TreePool), as a
//! forest would, so the level of detail follows the distance (`--lod` moves the switches,
//! `--no-impostors` leaves the impostor out). `--mode tree` draws a `ProceduralTree` at `--age`
//! (a growth age; mature by default), in full at every distance, or with `--lod` or `--level` at
//! the same levels of detail (`TreeLod`; an individual tree has no impostor).
//! `--level n` draws level `n` at every distance (3 is the pool's impostor), so two levels
//! can be compared at one distance; `--empty` photographs the same scene with no tree, so the
//! tree's pixels are the ones that differ from it. `--no-msaa` turns the camera's multisampling
//! off (it is on, 4 samples, by default).
//!
//! `--constant-size` zooms as the camera backs off, so the tree keeps filling `<share>` of the
//! frame's height (0.6 by default): consecutive frames then differ only by what the distance
//! changes, and a switch of level of detail shows as a jump in the picture. Without it the field
//! of view stays `--fov` and the tree shrinks as it would in a game.
//!
//! `--fell <deg>` cuts the tree half a metre up (`FellTree`) and lets it fall toward that bearing
//! (degrees from +x toward +z), fast; the photographs are taken once it lies still. `--buck <m>`
//! then cuts its trunk into logs `m` metres long (`BuckTree`), photographed once they lie there.
//! `--notch <deg>,<depth>[,<height>]` cuts an axe's notch (`TreeNotch`) into the trunk from that
//! bearing's side, `depth` of the way through, its floor `height` metres up (0.5 by default); with
//! `--fell` the tree is then felled at the notch.
//!
//! `--slope <deg>` tilts the ground so it falls toward +x and makes it the `FloraGround`: the
//! tree stands at its footing (`FloraGround::footing`), so the downhill side of its trunk meets
//! the ground, or with `--on-centre` at the height under its centre, as a plain placement would
//! put it; a felled tree lands on the slope. `--eye` is then the camera's height above the
//! ground under it, and `--look` measures from the ground under the trunk, so `--on-centre`
//! and the footing are photographed from the same place.
//!
//! `--drop <count>[,<after>[,<every>]]` makes the tree drop `count` leaves (`DropLeaves`) once it is
//! ready, and takes the first photograph `after` seconds later and each further one `every` seconds
//! after the one before; with one `--distance`, the frames follow the leaves as they fall and lie.
//! `--autumn <seconds>[,<after>[,<every>]]` instead lets the season's leaf fall rise from the
//! `--season` value to 1 over `seconds` once the tree is ready, photographed the same way: the
//! tree's own leaves let go as the season passes them. `--burn <after>[,<every>]` lights the
//! tree's spot of a [`FloraFire`](bevy_flora::FloraFire) once it is ready (the fire is there from
//! the start, 64 m in 1 m cells), photographed the same way: the leaves scorch and burn away, the
//! wood chars from the ground up and its embers glow, then cool. `--push <x>,<z>[,<radius>[,<vx>,<vz>]]`
//! stands a red body 1.8 m tall with a [`FoliagePusher`](bevy_flora::FoliagePusher) there
//! (radius 0.45 m by default), and walks it at (vx, vz) m/s once the tree is ready, photographed
//! every quarter of a second: the leaves and thin stems it walks through give way.
//!
//! `--hull <tiers>,<sides>[,<lift>[,<jitter>]]` (with `--branches`: every branch drawn, not the trunk
//! chain only, `CrownStyle::Hull::trunk_only`) draws the crown as a faceted hull of that many
//! stacked cones ([`CrownStyle::Hull`](bevy_flora::CrownStyle), lift 0.2 and jitter 1 by default)
//! and `--prism <sides>` the wood as flat-shaded prisms ([`TrunkStyle::Prism`](bevy_flora::TrunkStyle)):
//! `--preset spruce --hull 5,7 --prism 5` is the stylised winter conifer, and every other option
//! (`--fell`, `--notch`, `--burn`, `--season`, `--drop`, levels of detail) applies to it.
//!
//! The clock steps a fixed 1/60 s per frame, the wind is off unless `--wind` says otherwise, and
//! nothing depends on the wall clock, so two runs write the same frames.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy_flora::{
    BuckTree, CrownStyle, DropLeaves, FellTree, Felled, FloraFire, FloraGround, FloraPlugin, FloraSeason,
    FloraWetness, FloraWind, FoliagePusher, LodDistances, LodSettings, PooledTree, ProceduralTree, RenderStyle, TreeAge,
    TreeLod, TreeNotch, TreePool, TreePreset, TreeShape, TrunkStyle, build_pool, build_pool_with_deadfall, build_tree,
    DeadfallKind, DeadfallSettings, PooledDeadfall,
};

/// Frames rendered before the first photograph so the pool's impostor is baked and shadows exist.
const WARMUP_FRAMES: u32 = 30;
/// Frames between moving the camera and photographing (visibility ranges, shadows).
const SETTLE_FRAMES: u32 = 3;

/// Frames in `seconds`, at the fixed 1/60 s a frame.
fn ticks(seconds: f32) -> u32 {
    (seconds * 60.0).round().max(0.0) as u32
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pool,
    Tree,
    /// Pooled deadfall (`PooledDeadfall`): a stump, a lying tree and a bucked one, two variants of each.
    Deadfall,
}

struct Args {
    out: PathBuf,
    preset: TreePreset,
    mode: Mode,
    age: f32,
    seed: i32,
    frames: u32,
    distance: [f32; 2],
    /// `Some(share)`: zoom so the tree fills this share of the frame's height.
    constant_size: Option<f32>,
    fov: f32,
    yaw: f32,
    eye: f32,
    look: f32,
    sun: Vec3,
    wind: f32,
    size: UVec2,
    lod: Option<[f32; 3]>,
    impostors: bool,
    /// Draw only this level of detail of the pool (3: the impostor).
    level: Option<u32>,
    /// No tree: the background the tree's pixels are measured against.
    empty: bool,
    style: RenderStyle,
    msaa: bool,
    /// Give the tree a `TreeShape` and draw it over the picture.
    colliders: bool,
    /// How far deciduous trees have turned and shed their leaves.
    season: FloraSeason,
    /// How wet the trees are (`FloraWetness`).
    wet: f32,
    /// Fell the tree toward this bearing (degrees from +x toward +z) and photograph it lying.
    fell: Option<f32>,
    /// With `--fell`, cut the trunk into logs this long once it lies still.
    buck: Option<f32>,
    /// With `--fell`, lay it down at once where it would come to rest (`FellTree::lying`).
    lying: bool,
    /// The ground falls toward +x at this angle, degrees.
    slope: f32,
    /// Stand the tree at the ground's height under its centre, not at its footing.
    on_centre: bool,
    /// Drop this many leaves from the tree once it is ready, photograph `.1` seconds later and every
    /// `.2` seconds after that.
    drop: Option<(u32, f32, f32)>,
    /// Let the leaf fall rise to 1 over `.0` seconds once the tree is ready, photograph `.1` seconds
    /// later and every `.2` seconds after that.
    autumn: Option<(f32, f32, f32)>,
    /// Light the tree's cell of a `FloraFire` once it is ready, photograph `.0` seconds later and
    /// every `.1` seconds after that.
    burn: Option<(f32, f32)>,
    /// A body pushing foliage aside: where it stands (x, z), its radius and the velocity it walks at
    /// once the tree is ready.
    push: Option<(Vec2, f32, Vec2)>,
    /// An axe's notch: the bearing of the side it is cut from (degrees from +x toward +z), its
    /// depth (share of the trunk's width) and the height of its floor.
    notch: Option<(f32, f32, f32)>,
    /// A faceted hull crown: tiers, sides, lift and jitter (`CrownStyle::Hull`).
    hull: Option<(u32, u32, f32, f32)>,
    /// A prism trunk with this many sides (`TrunkStyle::Prism`).
    prism: Option<u32>,
    /// With `--hull`: draw every branch, not the trunk chain only.
    branches: Option<()>,
}

fn usage() -> ! {
    eprintln!(
        "usage: inspect --out <dir> [--preset <name>] [--mode pool|tree] [--age <a>] [--seed <n>] \
         [--frames <n>] [--distance <m>[,<m>]] [--constant-size [<share>]] [--fov <deg>] [--yaw <deg>] \
         [--eye <m>] [--look <share>] [--sun <x,y,z>] [--wind <s>] [--size <w>x<h>] [--lod <a,b,c>] \
         [--no-impostors] [--level <0|1|2|3>] [--empty] [--no-msaa] [--style <natural|painterly|toon>] \
         [--colliders] [--season <autumn>,<leaf fall>[,<snow>[,<blossom>]]] [--wet <0..1>] [--fell <deg> [--buck <m>] [--lying]] [--slope <deg>] \
         [--on-centre] [--drop <count>[,<after>[,<every>]]] [--autumn <seconds>[,<after>[,<every>]]] \
         [--burn <after>[,<every>]] [--push <x>,<z>[,<radius>[,<vx>,<vz>]]] [--notch <deg>,<depth>[,<height>]] \
         [--hull <tiers>,<sides>[,<lift>[,<jitter>]]] [--prism <sides>] [--branches]"
    );
    std::process::exit(2);
}

fn floats(list: &str) -> Vec<f32> {
    list.split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect()
}

fn parse_args() -> Args {
    let mut args = Args {
        out: PathBuf::new(),
        preset: TreePreset::Oak,
        mode: Mode::Pool,
        age: f32::INFINITY,
        seed: 1,
        frames: 1,
        distance: [30.0, 30.0],
        constant_size: None,
        fov: 45.0,
        yaw: 45.0,
        eye: 1.7,
        look: 0.5,
        sun: Vec3::new(-20.0, 25.0, 20.0),
        wind: 0.0,
        size: UVec2::new(960, 540),
        lod: None,
        impostors: true,
        level: None,
        empty: false,
        style: RenderStyle::Natural,
        msaa: true,
        colliders: false,
        season: FloraSeason::SUMMER,
        wet: 0.0,
        fell: None,
        buck: None,
        lying: false,
        slope: 0.0,
        on_centre: false,
        drop: None,
        autumn: None,
        burn: None,
        push: None,
        notch: None,
        hull: None,
        prism: None,
        branches: None,
    };
    let mut have_out = false;
    let mut it = std::env::args().skip(1).peekable();
    fn value<T: std::str::FromStr>(it: &mut impl Iterator<Item = String>) -> T {
        it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage())
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                args.out = it.next().unwrap_or_else(|| usage()).into();
                have_out = true;
            }
            "--preset" => {
                let name = it.next().unwrap_or_else(|| usage());
                args.preset = TreePreset::ALL.into_iter().find(|p| p.name() == name).unwrap_or_else(|| usage());
            }
            "--mode" => {
                args.mode = match it.next().as_deref() {
                    Some("pool") => Mode::Pool,
                    Some("tree") => Mode::Tree,
                    Some("deadfall") => Mode::Deadfall,
                    _ => usage(),
                }
            }
            "--age" => args.age = value(&mut it),
            "--seed" => args.seed = value(&mut it),
            "--frames" => args.frames = value(&mut it),
            "--distance" => {
                let d = floats(&it.next().unwrap_or_else(|| usage()));
                args.distance = match d[..] {
                    [a] => [a, a],
                    [a, b] => [a, b],
                    _ => usage(),
                };
            }
            "--constant-size" => {
                let share = it.peek().and_then(|v| v.parse::<f32>().ok());
                if share.is_some() {
                    it.next();
                }
                args.constant_size = Some(share.unwrap_or(0.6));
            }
            "--fov" => args.fov = value(&mut it),
            "--yaw" => args.yaw = value(&mut it),
            "--eye" => args.eye = value(&mut it),
            "--look" => args.look = value(&mut it),
            "--sun" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                let [x, y, z]: [f32; 3] = p.try_into().unwrap_or_else(|_| usage());
                args.sun = Vec3::new(x, y, z);
            }
            "--wind" => args.wind = value(&mut it),
            "--size" => {
                let s = it.next().unwrap_or_else(|| usage());
                let (w, h) = s.split_once('x').unwrap_or_else(|| usage());
                args.size = UVec2::new(
                    w.parse().unwrap_or_else(|_| usage()),
                    h.parse().unwrap_or_else(|_| usage()),
                );
            }
            "--lod" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.lod = Some(p.try_into().unwrap_or_else(|_| usage()));
            }
            "--no-impostors" => args.impostors = false,
            "--level" => args.level = Some(value(&mut it)),
            "--empty" => args.empty = true,
            "--style" => {
                args.style = match it.next().as_deref() {
                    Some("natural") => RenderStyle::Natural,
                    Some("painterly") => RenderStyle::Painterly,
                    Some("toon") => RenderStyle::Toon,
                    _ => usage(),
                }
            }
            "--no-msaa" => args.msaa = false,
            "--colliders" => args.colliders = true,
            "--season" => {
                let v = floats(&it.next().unwrap_or_else(|| usage()));
                let (autumn, leaf_fall, snow, blossom) = match v[..] {
                    [autumn, leaf_fall] => (autumn, leaf_fall, 0.0, 0.0),
                    [autumn, leaf_fall, snow] => (autumn, leaf_fall, snow, 0.0),
                    [autumn, leaf_fall, snow, blossom] => (autumn, leaf_fall, snow, blossom),
                    _ => usage(),
                };
                args.season = FloraSeason { autumn, leaf_fall, snow, blossom };
            }
            "--wet" => args.wet = value(&mut it),
            "--fell" => args.fell = Some(value(&mut it)),
            "--buck" => args.buck = Some(value(&mut it)),
            "--lying" => args.lying = true,
            "--slope" => args.slope = value(&mut it),
            "--on-centre" => args.on_centre = true,
            "--drop" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.drop = match p[..] {
                    [count] => Some((count as u32, 0.0, 0.0)),
                    [count, after] => Some((count as u32, after, 0.0)),
                    [count, after, every] => Some((count as u32, after, every)),
                    _ => usage(),
                };
            }
            "--autumn" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.autumn = match p[..] {
                    [seconds] => Some((seconds, 0.0, 0.0)),
                    [seconds, after] => Some((seconds, after, 0.0)),
                    [seconds, after, every] => Some((seconds, after, every)),
                    _ => usage(),
                };
            }
            "--push" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.push = match p[..] {
                    [x, z] => Some((Vec2::new(x, z), 0.45, Vec2::ZERO)),
                    [x, z, radius] => Some((Vec2::new(x, z), radius, Vec2::ZERO)),
                    [x, z, radius, vx, vz] => Some((Vec2::new(x, z), radius, Vec2::new(vx, vz))),
                    _ => usage(),
                };
            }
            "--hull" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.hull = match p[..] {
                    [tiers, sides] => Some((tiers as u32, sides as u32, 0.2, 1.0)),
                    [tiers, sides, lift] => Some((tiers as u32, sides as u32, lift, 1.0)),
                    [tiers, sides, lift, jitter] => Some((tiers as u32, sides as u32, lift, jitter)),
                    _ => usage(),
                };
            }
            "--branches" => args.branches = Some(()),
            "--prism" => args.prism = Some(value(&mut it)),
            "--notch" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.notch = match p[..] {
                    [degrees, depth] => Some((degrees, depth, 0.5)),
                    [degrees, depth, height] => Some((degrees, depth, height)),
                    _ => usage(),
                };
            }
            "--burn" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.burn = match p[..] {
                    [after] => Some((after, 0.0)),
                    [after, every] => Some((after, every)),
                    _ => usage(),
                };
            }
            _ => usage(),
        }
    }
    if !have_out || args.frames == 0 || args.distance.iter().any(|d| *d <= 0.0) {
        usage();
    }
    args
}

#[derive(Resource)]
struct Shoot {
    args: Args,
    target: Handle<Image>,
    /// Height of the tree, for the camera's aim and zoom.
    height: f32,
    frame: u32,
    wait: u32,
    in_flight: bool,
    /// Whether the scene (the pool's impostor, the tree's children) is ready.
    ready: bool,
    log: String,
    /// The ground (flat at 0 unless `--slope`) and the height the tree stands at.
    ground: FloraGround,
    base: f32,
}

#[derive(Component)]
struct Photographer;

fn main() {
    let args = parse_args();
    fs::create_dir_all(&args.out).expect("create the output directory");
    let wind = args.wind;
    let season = args.season;
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        }),
        FloraPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
    .insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.6, 0.6, 0.6),
        brightness: 300.0,
        ..default()
    })
    .insert_resource(FloraWind { strength: wind, ..default() })
    .insert_resource(season)
    .insert_resource(FloraWetness(args.wet))
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
    .insert_resource(Shoot {
        args,
        target: Handle::default(),
        height: 1.0,
        frame: 0,
        wait: WARMUP_FRAMES,
        in_flight: false,
        ready: false,
        log: String::from("frame\tdistance\tfov\n"),
        ground: FloraGround::default(),
        base: 0.0,
    })
    .add_systems(Startup, setup)
    .add_systems(Update, (drive, draw_colliders, autumn, walk))
    .add_systems(Startup, gizmos_on_top);
    app.run();
}

/// With `--autumn`: once the tree is ready, the leaf fall rises from the `--season` value to 1 over
/// the given seconds, so the leaves fall as a season passing lets them.
fn autumn(shoot: Res<Shoot>, time: Res<Time>, mut season: ResMut<FloraSeason>, mut start: Local<Option<(f32, f32)>>) {
    let Some((seconds, ..)) = shoot.args.autumn else { return };
    if !shoot.ready {
        return;
    }
    let (t0, from) = *start.get_or_insert((time.elapsed_secs(), season.leaf_fall));
    let share = ((time.elapsed_secs() - t0) / seconds.max(0.01)).clamp(0.0, 1.0);
    let leaf_fall = from + (1.0 - from) * share;
    if season.leaf_fall != leaf_fall {
        season.leaf_fall = leaf_fall;
    }
}

/// The collider lines are drawn over the tree, not hidden inside its wood, and thick enough to
/// read in a downscaled picture.
fn gizmos_on_top(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line.width = 3.0;
}

/// With `--colliders`: the trees' gameplay shapes, capsules orange and the crown box cyan.
fn draw_colliders(mut gizmos: Gizmos, shapes: Query<(&GlobalTransform, &TreeShape)>) {
    for (transform, shape) in &shapes {
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let tree = Isometry3d::new(translation, rotation);
        for capsule in shape.capsules() {
            let (primitive, local) = capsule.to_primitive();
            gizmos.primitive_3d(&primitive, tree * local, Color::srgb(1.0, 0.45, 0.05));
        }
        if let Some((center, half)) = shape.crown {
            let cuboid = Cuboid { half_size: half };
            gizmos.primitive_3d(&cuboid, tree * Isometry3d::from_translation(center), Color::srgb(0.1, 0.9, 1.0));
        }
    }
}

/// The `--push` body.
#[derive(Component)]
struct Walker;

/// With `--push` and a velocity: once the tree is ready the body walks at it, on the ground.
fn walk(shoot: Res<Shoot>, time: Res<Time>, mut walkers: Query<&mut Transform, With<Walker>>) {
    let Some((_, _, velocity)) = shoot.args.push else { return };
    if !shoot.ready {
        return;
    }
    for mut at in &mut walkers {
        at.translation += Vec3::new(velocity.x, 0.0, velocity.y) * time.delta_secs();
        at.translation.y = shoot.ground.height(at.translation.xz()) + 0.9;
    }
}

fn setup(
    mut commands: Commands,
    mut shoot: ResMut<Shoot>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut pools: ResMut<Assets<TreePool>>,
) {
    let size = shoot.args.size;
    shoot.target = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_translation(shoot.args.sun).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let mut ground = commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4000.0, 4000.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.30),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));
    if shoot.args.slope != 0.0 {
        ground.insert(Transform::from_rotation(Quat::from_rotation_z(-shoot.args.slope.to_radians())));
    }
    let msaa = if shoot.args.msaa { Msaa::Sample4 } else { Msaa::Off };
    commands.spawn((
        Camera3d::default(),
        msaa,
        RenderTarget::Image(shoot.target.clone().into()),
        Transform::default(),
        Photographer,
    ));

    let mut params = shoot.args.preset.params();
    params.seed = shoot.args.seed;
    params.style = shoot.args.style;
    if let Some((tiers, sides, lift, jitter)) = shoot.args.hull {
        params.crown = CrownStyle::Hull { tiers, sides, lift, jitter, trunk_only: shoot.args.branches.is_none() };
    }
    if let Some(sides) = shoot.args.prism {
        params.trunk_style = TrunkStyle::Prism { sides };
    }
    let mature = build_tree(&params, shoot.args.age);
    shoot.height = mature
        .trunk
        .positions
        .iter()
        .chain(&mature.foliage.positions)
        .chain(&mature.flowers.positions)
        .fold(0.0f32, |h, p| h.max(p.y))
        .max(0.5);
    // `--slope`: the ground falls toward +x, and the tree stands at its footing, or with
    // `--on-centre` at the ground's height under its centre, as a plain placement would.
    if shoot.args.slope != 0.0 {
        let tan = shoot.args.slope.to_radians().tan();
        shoot.ground = FloraGround::new(move |at: Vec2| -at.x * tan);
        commands.insert_resource(shoot.ground.clone());
    }
    // `--burn`: a fire over the tree's ground from the start, so its shaders are made with the
    // others, in 1 m cells.
    if shoot.args.burn.is_some() {
        commands.insert_resource(FloraFire::with_cells(Vec2::ZERO, 64.0, 64));
    }
    // `--push`: a red body 1.8 m tall that pushes foliage aside, standing (or walking from) there.
    if let Some((at, radius, _)) = shoot.args.push {
        let base = shoot.ground.height(at);
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(radius * 0.8, 1.8 - 1.6 * radius))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.15, 0.12), ..default() })),
            Transform::from_xyz(at.x, base + 0.9, at.y),
            FoliagePusher { radius, height: 1.8, strength: 1.0 },
            Walker,
        ));
    }
    shoot.base = if shoot.args.on_centre {
        shoot.ground.height(Vec2::ZERO)
    } else {
        shoot.ground.footing(Vec2::ZERO, params.base_radius())
    };
    if shoot.args.empty {
        return;
    }
    let mut lod = LodSettings::default();
    if let Some([lod1, lod2, impostor]) = shoot.args.lod {
        lod.distances = LodDistances { lod1, lod2, impostor };
    }
    // One level everywhere: every other level's band lies below the camera's nearest position or
    // beyond its farthest.
    if let Some(level) = shoot.args.level {
        let (near, far) = (0.001, 1.0e6);
        lod.distances = match level {
            0 => LodDistances { lod1: far, lod2: far + 1.0, impostor: far + 2.0 },
            1 => LodDistances { lod1: near, lod2: far, impostor: far + 1.0 },
            2 => LodDistances { lod1: near, lod2: near * 2.0, impostor: far },
            _ => LodDistances { lod1: near, lod2: near * 2.0, impostor: near * 3.0 },
        };
    }
    if !shoot.args.impostors {
        lod.distances.impostor = f32::INFINITY;
    }
    let tree = match shoot.args.mode {
        Mode::Pool => {
            let pool = pools.add(build_pool(&params, 1, shoot.args.age, &lod, &mut meshes));
            commands.spawn((PooledTree { pool, variant: 0 }, Transform::from_xyz(0.0, shoot.base, 0.0))).id()
        }
        Mode::Deadfall => {
            let pool = pools.add(build_pool_with_deadfall(
                &params,
                2,
                shoot.args.age,
                &lod,
                &DeadfallSettings::default(),
                &mut meshes,
            ));
            // Down the slope (it falls toward +x) or toward +x on flat ground; a row of each kind.
            let direction = Vec2::X;
            let kinds = [
                DeadfallKind::Stump,
                DeadfallKind::Lying { direction },
                DeadfallKind::Bucked { direction },
            ];
            let mut first = None;
            for (row, kind) in kinds.into_iter().enumerate() {
                for variant in 0..2u32 {
                    let at = Vec2::new(variant as f32 * 14.0, (row as f32 - 1.0) * 12.0);
                    let y = shoot.ground.footing(at, params.base_radius());
                    let id = commands
                        .spawn((PooledDeadfall { pool: pool.clone(), variant, kind }, Transform::from_xyz(at.x, y, at.y)))
                        .id();
                    first.get_or_insert(id);
                }
            }
            first.expect("deadfall spawned")
        }
        Mode::Tree => {
            let at = Transform::from_xyz(0.0, shoot.base, 0.0);
            let mut tree = commands.spawn((ProceduralTree(params), TreeAge(shoot.args.age), at));
            if shoot.args.lod.is_some() || shoot.args.level.is_some() {
                tree.insert(TreeLod(lod));
            }
            tree.id()
        }
    };
    if shoot.args.colliders {
        commands.entity(tree).insert(TreeShape::default());
    }
    fell(&mut commands, tree, &shoot.args);
}

/// With `--notch`: cut an axe's notch on that bearing's side. With `--fell`: cut the tree and let
/// it fall toward the bearing (degrees from +x toward +z), fast, and with `--buck` cut its trunk
/// into logs that long; the photographs wait until it lies still, or its logs do.
fn fell(commands: &mut Commands, tree: Entity, args: &Args) {
    if let Some((degrees, depth, height)) = args.notch {
        let (sin, cos) = degrees.to_radians().sin_cos();
        commands.entity(tree).insert(TreeNotch { direction: Vec2::new(cos, sin), depth, height });
    }
    if let Some(degrees) = args.fell {
        let (sin, cos) = degrees.to_radians().sin_cos();
        commands.entity(tree).insert(FellTree { direction: Vec2::new(cos, sin), speed: 4.0, lying: args.lying, ..default() });
        if let Some(log_length) = args.buck {
            commands.entity(tree).insert(BuckTree { log_length, ..default() });
        }
    }
}

/// The camera's distance and field of view (degrees) at frame `frame`.
fn pose(args: &Args, height: f32, frame: u32) -> (f32, f32) {
    let t = if args.frames > 1 { frame as f32 / (args.frames - 1) as f32 } else { 0.0 };
    let distance = args.distance[0] * (args.distance[1] / args.distance[0]).powf(t);
    let fov = match args.constant_size {
        Some(share) => (2.0 * (height / (2.0 * share * distance)).atan()).to_degrees().clamp(0.5, 120.0),
        None => args.fov,
    };
    (distance, fov)
}

/// A tree that has no render children yet.
type Unfinished = (Or<(With<PooledTree>, With<ProceduralTree>)>, Without<Children>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut commands: Commands,
    mut shoot: ResMut<Shoot>,
    mut camera: Single<(&mut Transform, &mut Projection), With<Photographer>>,
    screenshots: Query<(), With<Screenshot>>,
    pools: Res<Assets<TreePool>>,
    trees: Query<(), Unfinished>,
    felling: Query<&Felled>,
    cutting: Query<(), With<FellTree>>,
    bucking: Query<(), With<BuckTree>>,
    standing: Query<Entity, Or<(With<ProceduralTree>, With<PooledTree>)>>,
    mut drops: MessageWriter<DropLeaves>,
    mut fire: Option<ResMut<FloraFire>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !shoot.ready {
        let baked = pools.iter().all(|(_, pool)| {
            !shoot.args.impostors || pool.variants.iter().all(|v| v.impostor.is_some())
        });
        if shoot.wait > 0 {
            shoot.wait -= 1;
            return;
        }
        let lying = felling.iter().all(|felled| felled.at_rest) && cutting.is_empty() && bucking.is_empty();
        if !baked || !trees.is_empty() || !lying {
            return;
        }
        shoot.ready = true;
        shoot.wait = SETTLE_FRAMES;
        // `--drop`: the leaves let go now, and the first photograph waits for them.
        if let Some((count, after, _)) = shoot.args.drop {
            for tree in &standing {
                drops.write(DropLeaves { tree, count });
            }
            shoot.wait = ticks(after).max(SETTLE_FRAMES);
        }
        // `--autumn`: the leaf fall starts rising now (`autumn`), and the first photograph waits.
        if let Some((_, after, _)) = shoot.args.autumn {
            shoot.wait = ticks(after).max(SETTLE_FRAMES);
        }
        // `--burn`: the tree's cell is lit now, and the first photograph waits.
        if let Some((after, _)) = shoot.args.burn {
            if let Some(fire) = fire.as_mut() {
                fire.ignite(Vec3::new(0.0, shoot.base, 0.0), 1.0);
            }
            shoot.wait = ticks(after).max(SETTLE_FRAMES);
        }
    }
    if shoot.in_flight {
        if !screenshots.is_empty() {
            return;
        }
        shoot.in_flight = false;
        shoot.frame += 1;
        let every = shoot.args.drop.map(|(_, _, every)| every).or(shoot.args.autumn.map(|(_, _, every)| every));
        let every = every.or(shoot.args.burn.map(|(_, every)| every));
        // A walking `--push` body: a photograph every quarter of a second.
        let walking = shoot.args.push.is_some_and(|(_, _, velocity)| velocity != Vec2::ZERO);
        let every = every.or(walking.then_some(0.25));
        shoot.wait = every.map_or(SETTLE_FRAMES, |every| ticks(every).max(SETTLE_FRAMES));
        if shoot.frame == shoot.args.frames {
            let path = shoot.args.out.join("frames.tsv");
            fs::write(&path, &shoot.log).expect("write frames.tsv");
            println!("inspect: wrote {} frames to {}", shoot.args.frames, shoot.args.out.display());
            exit.write(AppExit::Success);
            return;
        }
    }

    let (distance, fov) = pose(&shoot.args, shoot.height, shoot.frame);
    let yaw = shoot.args.yaw.to_radians();
    let (x, z) = (yaw.sin() * distance, yaw.cos() * distance);
    let eye = Vec3::new(x, shoot.ground.height(Vec2::new(x, z)) + shoot.args.eye, z);
    let aim = Vec3::new(0.0, shoot.ground.height(Vec2::ZERO) + shoot.height * shoot.args.look, 0.0);
    *camera.0 = Transform::from_translation(eye).looking_at(aim, Vec3::Y);
    *camera.1 = Projection::Perspective(PerspectiveProjection {
        fov: fov.to_radians(),
        near: 0.1,
        far: 4000.0,
        ..default()
    });

    if shoot.wait > 0 {
        shoot.wait -= 1;
        return;
    }
    let path = shoot.args.out.join(format!("frame_{:04}.png", shoot.frame));
    let frame = shoot.frame;
    let _ = writeln!(shoot.log, "{frame}\t{distance:.3}\t{fov:.3}");
    commands.spawn(Screenshot::image(shoot.target.clone())).observe(save_to_disk(path));
    shoot.in_flight = true;
}
