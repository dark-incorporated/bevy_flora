//! An isometric camera over a forest: orthographic, following a walker along a path through the
//! trees, zooming from a close view out over the whole forest and back. Read it as an example of
//! the crate under such a camera. Nothing in it picks levels of detail: under an orthographic
//! camera the plugin hands the trees to coarser levels and impostors by the zoom (`LodScale`).
//! `SeeThroughTrees` keeps the walker in sight, with the shape an isometric game's own occlusion
//! fade has: a cylinder along the view (the rays are parallel), a third of each tree kept,
//! only what stands well before the walker cut, and nothing at ground level.
//!
//! ```text
//! cargo run --release --example isometric [-- --demo [--out <dir>]]
//! ```
//!
//! Controls: `W` `A` `S` `D` walk (up the screen, left, down, right), the mouse wheel or `+` and
//! `-` zoom, `T` turns the see-through off and on.
//!
//! `--demo` plays eight scripted seconds instead (the walker crosses the view along the path on a
//! still day, and the camera zooms out until the forest is impostors and back in) and writes them
//! as 192 frames to `target/capture/isometric` (`--out` for another directory), at a fixed 1/24 s
//! a frame, for `docs/media/isometric.gif` (16 fps and 64 colours, the palette made with a swatch of the
//! walker's red, which covers too few pixels to be kept otherwise):
//!
//! ```text
//! ffmpeg -f lavfi -i color=c=0xC81C12:s=560x315 -frames:v 1 red.png
//! ffmpeg -framerate 24 -i target/capture/isometric/frame_%04d.png -i red.png -filter_complex "[0]fps=16,scale=560:-1:flags=lanczos[a];[1]loop=loop=8:size=1[r];[a][r]concat=n=2:v=1,palettegen=max_colors=64:stats_mode=full" palette.png
//! ffmpeg -framerate 24 -i target/capture/isometric/frame_%04d.png -i palette.png -lavfi "fps=16,scale=560:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" docs/media/isometric.gif
//! ```

use std::f32::consts::FRAC_PI_4;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::UiTargetCamera;
use bevy_flora::{
    BackgroundPools, FloraPlugin, FloraWind, LodSettings, PooledTree, Scatter,
    SeeThroughTrees, TreePool, TreePreset,
};

/// The camera's bearing from the walker, how far it looks down (degrees) and how far back it
/// stands (metres). An orthographic camera's picture does not change with the distance, so it
/// stands far back: nothing in view is behind it at the widest zoom, and the trees at the top and
/// the bottom of the screen are at about the same distance from it, so they are drawn at about the
/// same level of detail (the plugin measures levels from where the camera stands).
const YAW: f32 = FRAC_PI_4;
const PITCH: f32 = 35.0;
const BACK: f32 = 400.0;
/// The view's height, metres of the world from the bottom of the screen to the top: the closest
/// and the widest zoom.
const NEAREST: f32 = 22.0;
const WIDEST: f32 = 260.0;
/// Walking pace, m/s, the height of the walker's chest, where the camera looks, and where along
/// the path (screen right of the forest's middle, metres) the walker starts.
const PACE: f32 = 2.6;
const CHEST: f32 = 1.2;
const START: f32 = -12.0;
/// The forest: half its width, the least distance between trunks and the half width of the path
/// kept clear through it, all metres.
const HALF: f32 = 140.0;
const SPACING: f32 = 6.5;
const PATH: f32 = 1.6;
/// The level-of-detail distances the pools are built with (the defaults): what the readout names
/// the level after.
const LEVELS: [(f32, &str); 3] = [(30.0, "drawn in full"), (80.0, "at level 1"), (200.0, "at level 2")];

/// What the walker and the camera are told to do this update: by the keyboard and the mouse, or
/// by the demo's script.
#[derive(Resource, Default, Clone, Copy)]
struct Controls {
    /// Screen right and screen up, -1..1 each.
    walk: Vec2,
    /// How much taller the view grows this update (a factor; 1 holds it).
    zoom: f32,
    /// Turn the see-through off, or back on.
    toggle: bool,
}

/// The walker the camera follows.
#[derive(Component)]
struct Walker;

/// The status line.
#[derive(Component)]
struct Hud;

/// How tall the camera's view is, metres, whether it sees through the trees, and where it looks
/// if not at the walker.
#[derive(Resource)]
struct View {
    height: f32,
    see_through: bool,
    anchor: Option<Vec3>,
}

/// The demo's recording: the clock steps 1/24 s for each frame written.
#[derive(Resource)]
struct Demo {
    out: PathBuf,
    target: Handle<Image>,
    frame: u32,
    wait: u32,
    in_flight: bool,
    /// The frame whose scripted controls are still to be played.
    due: Option<u32>,
}

const DEMO_FRAMES: u32 = 192;
const DEMO_WARMUP: u32 = 30;
const DEMO_SETTLE: u32 = 2;

fn main() {
    let mut demo = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => demo = Some(PathBuf::from("target/capture/isometric")),
            "--out" => demo = Some(PathBuf::from(args.next().expect("--out <dir>"))),
            _ => {
                eprintln!("usage: isometric [--demo [--out <dir>]]");
                std::process::exit(2);
            }
        }
    }
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.6, 0.6, 0.6), brightness: 300.0, ..default() })
        .insert_resource(FloraWind { strength: 0.3, ..default() })
        .insert_resource(View { height: NEAREST, see_through: true, anchor: None })
        .init_resource::<Controls>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (read_input.run_if(not(resource_exists::<Demo>)), (record, script).chain().run_if(resource_exists::<Demo>)),
                (walk, follow, hud).chain(),
            )
                .chain(),
        );
    if let Some(out) = demo {
        std::fs::create_dir_all(&out).expect("create the output directory");
        // The demo's camera holds still on the middle of the walk while the walker crosses the
        // view, on a still day: a GIF of a scrolling, swaying forest is twice as large.
        let (right, _) = screen_axes();
        let anchor = Some(right * (START + PACE * DEMO_FRAMES as f32 / 24.0 / 2.0) + Vec3::Y * CHEST);
        app.insert_resource(Demo { out, target: Handle::default(), frame: 0, wait: DEMO_WARMUP, in_flight: false, due: None })
            .insert_resource(View { height: NEAREST, see_through: true, anchor })
            .insert_resource(FloraWind { strength: 0.0, ..default() })
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    }
    app.run();
}

/// Screen right and screen up, on the ground.
fn screen_axes() -> (Vec3, Vec3) {
    let (sin, cos) = YAW.sin_cos();
    (Vec3::new(cos, 0.0, -sin), Vec3::new(-sin, 0.0, -cos))
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pools: BackgroundPools,
    mut images: ResMut<Assets<Image>>,
    demo: Option<ResMut<Demo>>,
) {
    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, illuminance: 10_000.0, ..default() },
        // The shadow maps cover the depths around the walker, not the empty way from the camera.
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: BACK - 60.0,
            first_cascade_far_bound: BACK + 40.0,
            maximum_distance: BACK + 500.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-10.0, 20.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0 * HALF, 12.0 * HALF))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.45, 0.30), perceptual_roughness: 1.0, ..default() })),
    ));

    // A pool per species, scattered in groves, and a path kept clear along the screen's x axis
    // through the forest's middle. The impostors are pictures taken from the camera's own pitch, so
    // the far trees show their crowns from above as the near ones do, and they cast shadows, so the
    // far forest stands on its shadows as the near one does. The pools are built in the background:
    // the first frames are drawn at once, and the forest appears when its pools are ready.
    let species = [TreePreset::Pine, TreePreset::Spruce, TreePreset::Oak, TreePreset::Maple];
    let lod = LodSettings { impostor_elevation: PITCH, impostor_shadows: true, ..default() };
    let pools: Vec<Handle<TreePool>> =
        species.iter().map(|preset| pools.build(&preset.params(), 4, f32::INFINITY, &lod)).collect();
    let forest = Scatter {
        seed: 3,
        species: vec![3.0, 2.0, 2.0, 1.0],
        grove_size: 40.0,
        ..Scatter::new(Rect::new(-HALF, -HALF, HALF, HALF), SPACING)
    };
    let (_, up) = screen_axes();
    let off_path = |at: Vec2| Vec3::new(at.x, 0.0, at.y).dot(up).abs() > PATH;
    for tree in forest.points_where(|at| if off_path(at) { 1.0 } else { 0.0 }) {
        commands.spawn((PooledTree { pool: pools[tree.species].clone(), variant: tree.variant }, tree.transform(0.0)));
    }

    let (right, _) = screen_axes();
    commands.spawn((
        Walker,
        Mesh3d(meshes.add(Capsule3d::new(0.3, 1.2))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.12, 0.08), ..default() })),
        Transform::from_translation(right * START + Vec3::Y * 0.9),
    ));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical { viewport_height: NEAREST },
                ..OrthographicProjection::default_3d()
            }),
            Transform::default(),
        ))
        .id();
    let hud = commands
        .spawn((
            Hud,
            Text::new(""),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            Node { position_type: PositionType::Absolute, top: px(10), left: px(12), ..default() },
        ))
        .id();
    // The demo draws into an image: its text must be told which camera it is drawn by, and is
    // larger, to read in a small GIF.
    if let Some(mut demo) = demo {
        demo.target = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb)));
        commands.entity(camera).insert(RenderTarget::Image(demo.target.clone().into()));
        commands.entity(hud).insert((UiTargetCamera(camera), TextFont { font_size: FontSize::Px(26.0), ..default() }));
    }
}

fn read_input(keys: Res<ButtonInput<KeyCode>>, scroll: Res<AccumulatedMouseScroll>, mut controls: ResMut<Controls>) {
    let axis = |plus: KeyCode, minus: KeyCode| keys.pressed(plus) as i32 as f32 - keys.pressed(minus) as i32 as f32;
    let keyed = axis(KeyCode::Minus, KeyCode::Equal);
    *controls = Controls {
        walk: Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS)),
        zoom: 1.1f32.powf(keyed - scroll.delta.y.clamp(-3.0, 3.0)),
        toggle: keys.just_pressed(KeyCode::KeyT),
    };
}

/// The view's height at `frame` of the demo: close, out to the widest over two seconds (even
/// steps in scale), held, back in.
fn demo_height(frame: u32) -> f32 {
    let ramp = |from: u32, to: u32| {
        let t = ((frame as f32 - from as f32) / (to - from) as f32).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let out = ramp(56, 104) - ramp(128, 176);
    NEAREST * (WIDEST / NEAREST).powf(out)
}

/// The demo: walk along the path the whole time, zooming out and back in on the way. Each frame's
/// controls are played once, the update the frame's clock step comes.
fn script(time: Res<Time>, mut demo: ResMut<Demo>, mut controls: ResMut<Controls>) {
    *controls = Controls { zoom: 1.0, ..default() };
    if time.delta_secs() <= 0.0 {
        return;
    }
    let Some(frame) = demo.due.take() else { return };
    controls.walk = Vec2::X;
    controls.zoom = demo_height(frame) / demo_height(frame - 1);
}

/// The demo's recording: once every pooled tree is drawn, hold the clock until each frame is
/// written, then step it 1/24 s.
fn record(
    mut commands: Commands,
    mut demo: ResMut<Demo>,
    waiting: Query<(), (With<PooledTree>, Without<Children>)>,
    screenshots: Query<(), With<Screenshot>>,
    mut strategy: ResMut<TimeUpdateStrategy>,
    mut exit: MessageWriter<AppExit>,
) {
    *strategy = TimeUpdateStrategy::ManualDuration(Duration::ZERO);
    if demo.in_flight {
        if !screenshots.is_empty() {
            return;
        }
        demo.in_flight = false;
        demo.frame += 1;
        if demo.frame == DEMO_FRAMES {
            println!("isometric: wrote {DEMO_FRAMES} frames to {}", demo.out.display());
            exit.write(AppExit::Success);
            return;
        }
        demo.wait = DEMO_SETTLE;
        demo.due = Some(demo.frame);
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 24.0));
    }
    // The trees appear once their pools' impostors are baked.
    if !waiting.is_empty() {
        return;
    }
    if demo.wait > 0 {
        demo.wait -= 1;
        return;
    }
    let path = demo.out.join(format!("frame_{:04}.png", demo.frame));
    commands.spawn(Screenshot::image(demo.target.clone())).observe(save_to_disk(path));
    demo.in_flight = true;
}

fn walk(time: Res<Time>, controls: Res<Controls>, mut walker: Single<&mut Transform, With<Walker>>) {
    let (right, up) = screen_axes();
    let way = right * controls.walk.x + up * controls.walk.y;
    walker.translation += way.normalize_or_zero() * PACE * time.delta_secs();
}

/// Keep the camera on the walker (or on the demo's anchor) at its zoom, and the clear view on the
/// walker's chest: about a tenth of the view's height wide, a third of each tree kept, only what
/// stands more than 1.5 m before the chest along the view, and nothing below 0.5 m.
fn follow(
    mut commands: Commands,
    controls: Res<Controls>,
    mut view: ResMut<View>,
    walker: Single<&Transform, (With<Walker>, Without<Camera3d>)>,
    camera: Single<(Entity, &mut Transform, &mut Projection), With<Camera3d>>,
) {
    let (entity, mut transform, mut projection) = camera.into_inner();
    let height = (view.height * controls.zoom).clamp(NEAREST, WIDEST);
    if height != view.height {
        view.height = height;
    }
    if controls.toggle {
        view.see_through = !view.see_through;
    }
    let chest = walker.translation.with_y(CHEST);
    let focus = view.anchor.unwrap_or(chest);
    let (sin, cos) = YAW.sin_cos();
    let pitch = PITCH.to_radians();
    let back = Vec3::new(sin * pitch.cos(), pitch.sin(), cos * pitch.cos());
    *transform = Transform::from_translation(focus + back * BACK).looking_at(focus, Vec3::Y);
    if let Projection::Orthographic(orthographic) = &mut *projection {
        orthographic.scaling_mode = ScalingMode::FixedVertical { viewport_height: height };
    }
    if view.see_through {
        commands.entity(entity).insert(SeeThroughTrees {
            target: chest,
            radius: (0.12 * height).clamp(2.5, 14.0),
            keep: 0.35,
            inner: 0.55,
            in_front: Some(1.5),
            floor: Some(0.5),
        });
    } else {
        commands.entity(entity).remove::<SeeThroughTrees>();
    }
}

fn hud(view: Res<View>, mut text: Single<&mut Text, With<Hud>>) {
    // The distance from which a 45° perspective camera sees as tall a view: where the plugin draws
    // the trees in the middle of the screen from.
    let seen_from = view.height / (2.0 * (FRAC_PI_4 / 2.0).tan());
    let level = LEVELS.iter().find(|(distance, _)| seen_from < *distance).map_or("as impostors", |(_, name)| name);
    let status = format!("orthographic view {:.0} m tall: trees {level}", view.height);
    if text.0 != status {
        text.0 = status;
    }
}
