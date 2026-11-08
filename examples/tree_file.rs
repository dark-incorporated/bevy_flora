//! A tree from a `.tree.ron` file that rebuilds the moment the file is saved.
//!
//! ```text
//! cargo run --release --example tree_file --features ron,file_watcher [-- --tree trees/maple.tree.ron]
//! cargo run --release --example tree_file --features ron -- --export oak > examples/assets/trees/oak.tree.ron
//! cargo run --release --example tree_file --features ron -- --screenshot tree.png
//! ```
//!
//! The tree is a `ProceduralTreeAsset` loaded from `examples/assets/<--tree>` (default
//! `trees/maple.tree.ron`). With the `file_watcher` feature (Bevy's), edit the file while the
//! example runs and the tree rebuilds when you save. A file that does not parse keeps the last
//! good tree, and the error (line, column, and for a misspelt field the fields that exist) shows
//! on screen and in the log. `--export <preset>` prints a preset as a tree file to start from;
//! `--screenshot <path>` renders offscreen, saves the tree once it is drawn, and exits.

use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_flora::{FloraPlugin, ProceduralTree, ProceduralTreeAsset, TreeParams, TreePreset};
use bevy_flora::build_tree;

/// Where the example's tree files live, relative to the crate.
const ASSETS: &str = "examples/assets";

#[derive(Resource)]
struct Example {
    path: String,
    tree: Handle<TreeParams>,
    /// `--screenshot`: the offscreen picture, where to save it, and frames left before it.
    shot: Option<(Handle<Image>, PathBuf, u32)>,
    rebuilds: u32,
}

#[derive(Component)]
struct Hud;

fn usage() -> ! {
    eprintln!("usage: tree_file [--tree <path under examples/assets>] [--export <preset>] [--screenshot <path>]");
    std::process::exit(2);
}

fn main() {
    let mut path = "trees/maple.tree.ron".to_string();
    let mut screenshot = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--tree" => path = value(),
            "--screenshot" => screenshot = Some(PathBuf::from(value())),
            "--export" => {
                let name = value();
                let preset = match name.to_ascii_lowercase().parse::<TreePreset>() {
                    Ok(preset) => preset,
                    Err(error) => {
                        eprintln!("{error}");
                        usage();
                    }
                };
                print!("{}", preset.params().to_ron());
                return;
            }
            _ => usage(),
        }
    }

    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: ASSETS.to_string(),
                // Reload a file when it is saved: Bevy's `file_watcher`, this crate's feature
                // of the same name.
                watch_for_changes_override: Some(cfg!(feature = "file_watcher")),
                ..default()
            }),
            FloraPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.6, 0.6, 0.6), brightness: 300.0, ..default() })
        .insert_resource(Example {
            path,
            tree: Handle::default(),
            shot: screenshot.map(|path| (Handle::default(), path, 30)),
            rebuilds: 0,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (frame_tree, update_hud, take_screenshot))
        .run();
}

fn setup(
    mut commands: Commands,
    mut example: ResMut<Example>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    example.tree = assets.load(example.path.clone());
    commands.spawn(ProceduralTreeAsset(example.tree.clone()));
    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, illuminance: 10_000.0, ..default() },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.5, 0.25),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));
    let camera = commands.spawn((Camera3d::default(), Transform::from_xyz(14.0, 7.0, 14.0))).id();
    if let Some((target, ..)) = &mut example.shot {
        *target = images.add(Image::new_target_texture(
            1280,
            720,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        commands.entity(camera).insert(RenderTarget::Image(target.clone().into()));
    }
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        // The camera may draw into an offscreen image (`--screenshot`): put the text there too.
        UiTargetCamera(camera),
        // A load error is one long line: wrap it.
        Node { position_type: PositionType::Absolute, top: px(12), left: px(12), max_width: percent(94), ..default() },
    ));
}

/// Point the camera at the whole tree whenever the file changes it.
fn frame_tree(
    mut example: ResMut<Example>,
    trees: Query<&ProceduralTree, Changed<ProceduralTree>>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    let Some(tree) = trees.iter().next() else { return };
    example.rebuilds += 1;
    let meshes = build_tree(&tree.0, f32::INFINITY);
    let (lo, hi) = meshes
        .trunk
        .positions
        .iter()
        .chain(&meshes.foliage.positions)
        .chain(&meshes.flowers.positions)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let centre = (lo + hi) / 2.0;
    let size = (hi - lo).max_element().max(1.0);
    let toward = Vec3::new(1.0, 0.35, 1.0).normalize();
    **camera = Transform::from_translation(centre + toward * size * 1.9).looking_at(centre, Vec3::Y);
}

fn update_hud(example: Res<Example>, assets: Res<AssetServer>, mut hud: Query<&mut Text, With<Hud>>) {
    let Ok(mut text) = hud.single_mut() else { return };
    let state = match assets.get_load_state(&example.tree) {
        Some(LoadState::Loaded) => format!("loaded, built {} time(s)", example.rebuilds),
        Some(LoadState::Failed(error)) => format!("not loaded: {error}"),
        Some(_) | None => "loading".to_string(),
    };
    let how = if cfg!(feature = "file_watcher") {
        "Edit the file and save it: the tree rebuilds. A file with an error keeps the last good tree."
    } else {
        "Run with `--features ron,file_watcher` and the tree rebuilds when the file is saved."
    };
    text.0 = format!("{ASSETS}/{}: {state}\n{how}", example.path);
}

/// `--screenshot`: once the tree has been drawn (or the file failed to load) for a few frames,
/// save the picture and exit.
fn take_screenshot(
    mut commands: Commands,
    mut example: ResMut<Example>,
    assets: Res<AssetServer>,
    screenshots: Query<(), With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
    mut taken: Local<bool>,
) {
    let settled =
        example.rebuilds > 0 || matches!(assets.get_load_state(&example.tree), Some(LoadState::Failed(_)));
    let Some((target, path, wait)) = &mut example.shot else { return };
    if *taken {
        if screenshots.is_empty() {
            println!("tree_file: wrote {}", path.display());
            exit.write(AppExit::Success);
        }
        return;
    }
    if !settled {
        return;
    }
    if *wait > 0 {
        *wait -= 1;
        return;
    }
    commands.spawn(Screenshot::image(target.clone())).observe(save_to_disk(path.clone()));
    *taken = true;
}
