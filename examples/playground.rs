//! A first-person sandbox where the crate's gameplay APIs meet: walk among trees and chop, shake,
//! pick, buck, burn, soak and save them. Read it as an example of wiring them into a game.
//!
//! ```text
//! cargo run --release --example playground [-- --demo [--out <dir>]]
//! ```
//!
//! Controls: `W` `A` `S` `D` walk, hold the right mouse button to look, the left one strikes the
//! trunk you look at with an axe (`FloraIndex::cast_ray`, `TreeNotch`; the fourth blow fells it
//! away from you, `FellTree`), `B` bucks the felled tree nearest you into logs (`BuckTree`), `F`
//! shakes the tree you look at (`TreeShakes`): leaves and apples fall (`FallenFruit`) and you
//! gather the apples you walk over, `E` picks the apple nearest your hand (`TreeFruit::nearest`,
//! `TreeFruit::pick`), `G` lights a fire where you look (`FloraFire`; a tree it burns through
//! sends `TreeBurnt`), `R` starts or stops the rain (`FloraWetness`), `Y` moves the year on a
//! month (`FloraSeason::of_year`), `F5` saves and `F9` loads what you did: felled and bucked trees
//! (`FellTree::lying`), notches, picked apples (`TreeFruit::with_picked`), burnt trees (`Burnt`)
//! and the fire (`FloraFire::restore`).
//!
//! `--demo` plays eight scripted seconds instead (an apple tree shaken and picked, an oak notched,
//! felled and bucked in the rain) and writes them as 192 frames to `target/capture/playground`
//! (`--out` for another directory), at a fixed 1/24 s a frame, for `docs/media/playground.gif` (16 fps
//! and 64 colours: a first-person camera changes every pixel of every frame):
//!
//! ```text
//! ffmpeg -framerate 24 -i target/capture/playground/frame_%04d.png -vf "fps=16,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" docs/media/playground.gif
//! ```

use std::f32::consts::{FRAC_PI_2, TAU};
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::UiTargetCamera;
use bevy_flora::{
    BuckTree, Burnt, FallenFruit, FellTree, Felled, FloraFire, FloraIndex, FloraPlugin, FloraSeason,
    FloraWetness, FloraWind, ProceduralTree, TreeBurnt, TreeFruit, TreeLog, TreeNotch, TreeParams,
    TreePreset, TreeShake, TreeShakes, TreeStump,
};
use bevy_flora::{FireCell, SeededRng};

/// Eye height, walking pace (m/s), how far the axe and the hand reach (m), and how far a fallen
/// apple may lie from your feet to be gathered.
const EYE: f32 = 1.7;
const PACE: f32 = 3.0;
const AXE_REACH: f32 = 3.0;
const HAND_REACH: f32 = 1.6;
const GATHER: f32 = 1.0;
/// How much deeper each blow cuts the notch, a share of the trunk's width, and the blows that
/// fell a tree.
const NOTCH_STEP: f32 = 0.1;
const BLOWS: u32 = 4;
/// How fast the trees get wet in the rain and dry after it, per second.
const SOAK: f32 = 0.5;
const DRY: f32 = 0.05;

/// What the player wants this frame: from the keyboard and mouse, or from the demo's script.
#[derive(Resource, Default, Clone, Copy)]
struct Controls {
    /// Walking: x across, y ahead, each -1..1.
    walk: Vec2,
    /// Turning, radians: yaw, pitch.
    look: Vec2,
    strike: bool,
    shake: bool,
    pick: bool,
    buck: bool,
    fire: bool,
    rain: bool,
    year: bool,
    save: bool,
    load: bool,
}

/// The player: the camera, at eye height.
#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
}

impl Player {
    fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }
}

/// A tree the playground planted, and what the player did to it: what a save holds.
#[derive(Clone)]
struct Record {
    params: TreeParams,
    at: Transform,
    notch: Option<TreeNotch>,
    felled: Option<Vec2>,
    bucked: bool,
    picked: Vec<u32>,
    burnt: bool,
}

/// Marks the playground's trees, by their record.
#[derive(Component)]
struct Planted;

/// What the player has done, the trees' records and the last save.
#[derive(Resource, Default)]
struct Sandbox {
    records: HashMap<Entity, Record>,
    blows: HashMap<Entity, u32>,
    apples: u32,
    raining: bool,
    year: f32,
    burnt: u32,
    said: String,
    save: Option<Save>,
}

struct Save {
    trees: Vec<Record>,
    fire: Vec<FireCell>,
    wetness: f32,
    year: f32,
    apples: u32,
}

#[derive(Component)]
struct Hud;

/// The demo's recording: like `capture_growth`, the clock steps 1/24 s for each frame written.
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
            "--demo" => demo = Some(PathBuf::from("target/capture/playground")),
            "--out" => demo = Some(PathBuf::from(args.next().expect("--out <dir>"))),
            _ => {
                eprintln!("usage: playground [--demo [--out <dir>]]");
                std::process::exit(2);
            }
        }
    }
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.6, 0.6, 0.6), brightness: 300.0, ..default() })
        .insert_resource(FloraWind { strength: 0.3, ..default() })
        .insert_resource(FloraSeason::of_year(0.4))
        .insert_resource(FloraFire::with_cells(Vec2::ZERO, 96.0, 96))
        .init_resource::<Controls>()
        .insert_resource(Sandbox { year: 0.4, ..default() })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (read_input.run_if(not(resource_exists::<Demo>)), (record, script).chain().run_if(resource_exists::<Demo>)),
                (walk, act, weather, save_and_load, gather, count_burnt, hud).chain(),
            )
                .chain(),
        );
    if let Some(out) = demo {
        std::fs::create_dir_all(&out).expect("create the output directory");
        app.insert_resource(Demo { out, target: Handle::default(), frame: 0, wait: DEMO_WARMUP, in_flight: false, due: None })
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut sandbox: ResMut<Sandbox>,
    demo: Option<ResMut<Demo>>,
) {
    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, illuminance: 10_000.0, ..default() },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.45, 0.30), perceptual_roughness: 1.0, ..default() })),
    ));
    for (params, at) in grove() {
        plant(&mut commands, &mut sandbox, Record { params, at, notch: None, felled: None, bucked: false, picked: Vec::new(), burnt: false });
    }

    let player = Player { yaw: 0.0, pitch: -0.12 };
    let camera = commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, EYE, 7.5).looking_to(player.forward(), Vec3::Y), player)).id();
    let hud = commands
        .spawn((
            Hud,
            Text::new(""),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            Node { position_type: PositionType::Absolute, top: px(10), left: px(12), ..default() },
        ))
        .id();
    let cross = commands
        .spawn((
            Text::new("+"),
            TextFont { font_size: FontSize::Px(22.0), ..default() },
            Node { position_type: PositionType::Absolute, top: percent(50), left: percent(50), ..default() },
        ))
        .id();
    // The demo draws into an image: its text must be told which camera it is drawn by, and is
    // larger, to read in a small GIF.
    if let Some(mut demo) = demo {
        demo.target = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb)));
        commands.entity(camera).insert(RenderTarget::Image(demo.target.clone().into()));
        commands.entity(hud).insert((UiTargetCamera(camera), TextFont { font_size: FontSize::Px(26.0), ..default() }));
        commands.entity(cross).insert(UiTargetCamera(camera));
    }
}

/// The trees: an apple tree ahead, an oak to the right, and a ring of mixed trees around them.
fn grove() -> Vec<(TreeParams, Transform)> {
    let mut trees = vec![
        (TreePreset::Apple.params(), Transform::default()),
        (TreeParams { seed: 7, ..TreePreset::Oak.params() }, Transform::from_xyz(2.7, 0.0, 1.9)),
    ];
    let species = [TreePreset::Apple, TreePreset::Birch, TreePreset::Maple, TreePreset::Pine, TreePreset::Cherry, TreePreset::Oak];
    let mut rng = SeededRng::new(11);
    for i in 0..36 {
        let angle = i as f32 * 2.4;
        let distance = 9.0 + 22.0 * (i as f32 / 36.0).sqrt() + rng.range(-1.5, 1.5);
        let at = Vec3::new(angle.cos() * distance, 0.0, angle.sin() * distance - 6.0);
        // Keep the view ahead open.
        if at.x.abs() < 4.0 && at.z > -2.0 {
            continue;
        }
        let params = TreeParams { seed: i, ..species[i as usize % species.len()].params() };
        trees.push((params, Transform::from_translation(at).with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))));
    }
    trees
}

/// Plant the tree `record` describes, with what was done to it: felled trees lie where they fell,
/// bucked ones are cut into logs, picked apples stay picked.
fn plant(commands: &mut Commands, sandbox: &mut Sandbox, record: Record) {
    let mut tree = commands.spawn((Planted, ProceduralTree(record.params.clone()), record.at));
    if !record.picked.is_empty() {
        tree.insert(TreeFruit::with_picked(record.picked.iter().copied()));
    }
    if let Some(notch) = record.notch {
        tree.insert(notch);
    }
    if record.burnt {
        tree.insert(Burnt);
    }
    if let Some(direction) = record.felled {
        tree.insert(FellTree { direction, lying: true, ..default() });
        if record.bucked {
            tree.insert(BuckTree::default());
        }
    }
    sandbox.records.insert(tree.id(), record);
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut controls: ResMut<Controls>,
) {
    let axis = |plus: KeyCode, minus: KeyCode| keys.pressed(plus) as i32 as f32 - keys.pressed(minus) as i32 as f32;
    *controls = Controls {
        walk: Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS)),
        look: if buttons.pressed(MouseButton::Right) { -motion.delta * 0.003 } else { Vec2::ZERO },
        strike: buttons.just_pressed(MouseButton::Left),
        shake: keys.just_pressed(KeyCode::KeyF),
        pick: keys.just_pressed(KeyCode::KeyE),
        buck: keys.just_pressed(KeyCode::KeyB),
        fire: keys.just_pressed(KeyCode::KeyG),
        rain: keys.just_pressed(KeyCode::KeyR),
        year: keys.just_pressed(KeyCode::KeyY),
        save: keys.just_pressed(KeyCode::F5),
        load: keys.just_pressed(KeyCode::F9),
    };
}

/// The demo: walk to the apple tree, shake it, gather and pick apples, turn to the oak, notch and
/// fell it, call the rain, buck the oak. Each frame's controls are played once, the update the
/// frame's clock step comes.
fn script(time: Res<Time>, mut demo: ResMut<Demo>, mut controls: ResMut<Controls>) {
    *controls = Controls::default();
    // The step `record` asked for comes an update later: play the frame then, or walking (by
    // the time that passed) would go nowhere.
    if time.delta_secs() <= 0.0 {
        return;
    }
    let Some(frame) = demo.due.take() else { return };
    let turn = |frames: u32, radians: f32| radians / frames as f32;
    match frame {
        4..=43 => controls.walk.y = 1.0,
        48 => controls.shake = true,
        70..=80 => controls.walk.y = 0.5,
        86 | 90 | 94 | 98 | 102 | 106 => controls.pick = true,
        112..=119 => controls.look.x = turn(8, -FRAC_PI_2),
        124 | 130 | 136 | 142 => controls.strike = true,
        // Step back and aside from the falling oak, in the rain, and look along it as it lies and
        // is bucked into logs.
        146..=149 | 151..=165 => controls.walk = Vec2::new(1.0, -1.0),
        150 => controls.rain = true,
        166 => controls.buck = true,
        167..=171 => controls.look = Vec2::new(turn(5, 0.38), turn(5, -0.15)),
        _ => {}
    }
}

/// The demo's recording: hold the clock until each frame is written, then step it 1/24 s.
fn record(
    mut commands: Commands,
    mut demo: ResMut<Demo>,
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
            println!("playground: wrote {DEMO_FRAMES} frames to {}", demo.out.display());
            exit.write(AppExit::Success);
            return;
        }
        demo.wait = DEMO_SETTLE;
        demo.due = Some(demo.frame);
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 24.0));
    }
    if demo.wait > 0 {
        demo.wait -= 1;
        return;
    }
    let path = demo.out.join(format!("frame_{:04}.png", demo.frame));
    commands.spawn(Screenshot::image(demo.target.clone())).observe(save_to_disk(path));
    demo.in_flight = true;
}

fn walk(time: Res<Time>, controls: Res<Controls>, player: Single<(&mut Player, &mut Transform)>) {
    let (mut player, mut transform) = player.into_inner();
    player.yaw += controls.look.x;
    player.pitch = (player.pitch + controls.look.y).clamp(-1.4, 1.4);
    let ahead = Vec3::new(-player.yaw.sin(), 0.0, -player.yaw.cos());
    let right = Vec3::new(-ahead.z, 0.0, ahead.x);
    transform.translation += (ahead * controls.walk.y + right * controls.walk.x) * PACE * time.delta_secs();
    transform.translation.y = EYE;
    *transform = transform.looking_to(player.forward(), Vec3::Y);
}

/// The axe, the shake, the hand, the saw and the match: each on what the player looks at.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn act(
    mut commands: Commands,
    time: Res<Time>,
    controls: Res<Controls>,
    player: Single<(&Player, &Transform)>,
    index: Res<FloraIndex>,
    season: Res<FloraSeason>,
    mut shakes: ResMut<TreeShakes>,
    mut fire: ResMut<FloraFire>,
    mut fruit: Query<(&GlobalTransform, &mut TreeFruit)>,
    felled: Query<(Entity, &GlobalTransform), (With<Felled>, Without<BuckTree>)>,
    mut sandbox: ResMut<Sandbox>,
) {
    let (player, at) = *player;
    let eye = at.translation;
    let look = Ray3d::new(eye, Dir3::new(player.forward()).unwrap_or(Dir3::NEG_Z));
    let away = Vec2::new(-player.yaw.sin(), -player.yaw.cos());
    if (controls.strike || controls.shake)
        && let Some(hit) = index.cast_ray(look, AXE_REACH)
    {
        let tree = index.get(hit.tree).copied().expect("an indexed tree");
        if controls.shake {
            shakes.shake(TreeShake { at: tree.base.xz(), radius: 1.0, strength: 1.6, duration: 2.5, ..default() }, &time);
            sandbox.said = "shaken".into();
        } else {
            // An axe's blow: the notch on the far side deepens and the crown shivers; the last
            // blow fells the tree away from the player, at its notch.
            let blows = sandbox.blows.entry(hit.tree).or_default();
            *blows += 1;
            let blows = *blows;
            let notch = TreeNotch { direction: away, depth: NOTCH_STEP * blows as f32, ..default() };
            commands.entity(hit.tree).insert(notch);
            shakes.shake(TreeShake { at: tree.base.xz(), radius: 0.8, strength: 0.8, direction: away, duration: 1.2, ..default() }, &time);
            if let Some(record) = sandbox.records.get_mut(&hit.tree) {
                record.notch = Some(notch);
            }
            if blows >= BLOWS {
                commands.entity(hit.tree).insert(FellTree { direction: away, speed: 2.0, ..default() });
                if let Some(record) = sandbox.records.get_mut(&hit.tree) {
                    record.felled = Some(away);
                }
                sandbox.said = "timber!".into();
            } else {
                sandbox.said = format!("blow {blows} of {BLOWS}");
            }
        }
    }
    if controls.pick {
        // The apple nearest the hand, on any tree.
        let hand = eye + player.forward() * 0.6;
        let nearest = fruit
            .iter()
            .enumerate()
            .filter_map(|(i, (tree, fruit))| fruit.nearest(&season, tree, hand, HAND_REACH).map(|(apple, at)| (i, apple.key, at)))
            .min_by(|a, b| a.2.distance_squared(hand).total_cmp(&b.2.distance_squared(hand)));
        if let Some((i, key, _)) = nearest
            && let Some((_, mut fruit)) = fruit.iter_mut().nth(i)
            && fruit.pick(key)
        {
            sandbox.apples += 1;
            sandbox.said = "picked an apple".into();
        }
    }
    if controls.buck
        && let Some((tree, _)) = felled
            .iter()
            .map(|(tree, at)| (tree, at.translation().distance(eye)))
            .filter(|(_, d)| *d < 9.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    {
        commands.entity(tree).insert(BuckTree::default());
        if let Some(record) = sandbox.records.get_mut(&tree) {
            record.bucked = true;
        }
        sandbox.said = "bucked into logs".into();
    }
    if controls.fire && look.direction.y < -0.05 {
        // Where the look meets the ground.
        let ground = eye + *look.direction * (eye.y / -look.direction.y);
        if ground.distance(eye) < 30.0 {
            fire.ignite(ground, 1.5);
            sandbox.said = "fire!".into();
        }
    }
}

/// The rain soaks the trees and they dry after it; `Y` moves the year on a month.
fn weather(time: Res<Time>, controls: Res<Controls>, mut sandbox: ResMut<Sandbox>, mut wetness: ResMut<FloraWetness>, mut season: ResMut<FloraSeason>) {
    if controls.rain {
        sandbox.raining = !sandbox.raining;
        sandbox.said = if sandbox.raining { "rain".into() } else { "the rain stops".into() };
    }
    let target = if sandbox.raining { 1.0 } else { 0.0 };
    let rate = if sandbox.raining { SOAK } else { DRY };
    let next = wetness.0 + (target - wetness.0).clamp(-rate * time.delta_secs(), rate * time.delta_secs());
    if next != wetness.0 {
        wetness.0 = next;
    }
    if controls.year {
        sandbox.year = (sandbox.year + 1.0 / 12.0).rem_euclid(1.0);
        *season = FloraSeason::of_year(sandbox.year);
    }
}

/// `F5` keeps what the player did; `F9` puts the world back as it was then.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn save_and_load(
    mut commands: Commands,
    controls: Res<Controls>,
    mut sandbox: ResMut<Sandbox>,
    mut fire: ResMut<FloraFire>,
    mut wetness: ResMut<FloraWetness>,
    mut season: ResMut<FloraSeason>,
    fruit: Query<&TreeFruit>,
    burnt: Query<(), With<Burnt>>,
    gone: Query<Entity, Or<(With<Planted>, With<TreeStump>, With<TreeLog>, With<FallenFruit>)>>,
) {
    if controls.save {
        let trees = sandbox
            .records
            .iter()
            .map(|(&tree, record)| Record {
                picked: fruit.get(tree).map_or(record.picked.clone(), |fruit| fruit.picked().collect()),
                burnt: record.burnt || burnt.contains(tree),
                ..record.clone()
            })
            .collect();
        sandbox.save = Some(Save { trees, fire: fire.cells().to_vec(), wetness: wetness.0, year: sandbox.year, apples: sandbox.apples });
        sandbox.said = "saved".into();
    }
    if controls.load
        && let Some(save) = sandbox.save.take()
    {
        for entity in &gone {
            commands.entity(entity).despawn();
        }
        sandbox.records.clear();
        sandbox.blows.clear();
        for record in &save.trees {
            plant(&mut commands, &mut sandbox, record.clone());
        }
        fire.restore(&save.fire);
        wetness.0 = save.wetness;
        sandbox.year = save.year;
        *season = FloraSeason::of_year(save.year);
        sandbox.apples = save.apples;
        sandbox.said = "loaded".into();
        sandbox.save = Some(save);
    }
}

/// Fallen apples the player walks over are gathered.
fn gather(mut commands: Commands, player: Single<&Transform, With<Player>>, fallen: Query<(Entity, &Transform), With<FallenFruit>>, mut sandbox: ResMut<Sandbox>) {
    let feet = player.translation.xz();
    for (apple, at) in &fallen {
        if at.translation.xz().distance(feet) < GATHER {
            commands.entity(apple).despawn();
            sandbox.apples += 1;
            sandbox.said = "gathered an apple".into();
        }
    }
}

fn count_burnt(mut burnt: MessageReader<TreeBurnt>, mut sandbox: ResMut<Sandbox>) {
    for message in burnt.read() {
        sandbox.burnt += 1;
        if let Some(record) = sandbox.records.get_mut(&message.tree) {
            record.burnt = true;
        }
    }
}

fn hud(
    sandbox: Res<Sandbox>,
    wetness: Res<FloraWetness>,
    demo: Option<Res<Demo>>,
    logs: Query<(), With<TreeLog>>,
    mut text: Single<&mut Text, With<Hud>>,
) {
    let mut line = format!(
        "apples {}   logs {}   burnt {}   rain {}   {}",
        sandbox.apples,
        logs.iter().count(),
        sandbox.burnt,
        if sandbox.raining { format!("on ({:.0}% wet)", wetness.0 * 100.0) } else { "off".to_string() },
        sandbox.said,
    );
    if demo.is_none() {
        line.push_str(
            "\nWASD walk, right mouse look, left click axe, F shake, E pick, B buck, G fire, R rain, Y month, F5 save, F9 load",
        );
    }
    if text.0 != line {
        text.0 = line;
    }
}
