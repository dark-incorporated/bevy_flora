//! Baking the pictures of an impostor.
//!
//! For every [`TreePool`] variant without an [`ImpostorAtlas`], a scene is
//! spawned on two render layers of its own, far from the origin: eight copies of
//! the variant's level 1 tree, each turned a further eighth of a turn, twice (once
//! for the colour picture and once for the surface picture), and sixteen
//! orthographic cameras, each looking at its copy and drawing into its own tile of
//! a 2048x512 image: the top row of tiles holds the colours, the bottom row the
//! surface (normal, crown visibility, translucency). Nothing is lit, so there is
//! no light in the scene: the scene's own lights shade the pictures when an
//! impostor is drawn (`impostor.wesl`).
//!
//! The image is read back with a [`Screenshot`]. The first frames of a new
//! scene can come out empty (shaders and meshes are still being prepared), so a
//! capture with nothing in it is thrown away and tried again a few frames later.

use std::collections::HashSet;
use std::f32::consts::TAU;

use bevy::asset::{AssetId, Assets, Handle};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{
    Camera, Camera3d, CameraOutputMode, ClearColorConfig, OrthographicProjection, Projection,
    RenderTarget, ScalingMode, Viewport,
};
use bevy::color::{Color, LinearRgba};
use bevy::core_pipeline::tonemapping::DebandDither;
use bevy::ecs::prelude::*;
use bevy::image::Image;
use bevy::log::warn;
use bevy::math::{Quat, UVec2, Vec3};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::MeshMaterial3d;
use bevy::render::render_resource::{BlendState, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::view::{Msaa, Tonemapping};
use bevy::transform::components::Transform;

use super::material::ImpostorBakeMaterial;
use super::{
    IMPOSTOR_VIEWS, ImpostorAtlas, ImpostorFrame, ImpostorMaterial, ImpostorTint, TILE_PIXELS, atlas_image,
    linearize_surface, surface_image,
};
use crate::pool::{PoolRecolors, PoolVariant, TreePool};
use crate::shading::{FloraUniform, FloraWindBuffer};

/// The render layer the colour pictures are drawn on; nothing else should use it.
const BAKE_LAYER: usize = 31;
/// The render layer the surface pictures are drawn on; nothing else should use it.
const BAKE_SURFACE_LAYER: usize = 30;
/// Frames a new scene renders before it is captured.
const SETTLE_FRAMES: u32 = 6;
/// Captures of one variant before giving up on it.
const MAX_ATTEMPTS: u32 = 60;
/// Scenes baked at once. Every camera of a scene costs the renderer a few Metal command buffers
/// a frame, and Metal treats more than 4096 in flight as a lost device: a pool of eight variants
/// would put 128 cameras in one frame and hit that, so variants are baked a few at a time (four
/// is the 64 cameras the one-picture bake ran).
const MAX_CONCURRENT_BAKES: usize = 4;
/// Distance between the copies of a tree, and between the variants, in metres.
const SPACING: f32 = 400.0;
/// Where the baking scene starts: far from anything in a game's world, near
/// enough that `f32` keeps millimetres.
const BAKE_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, 30_000.0);
/// How far the cameras stand from what they look at, in metres.
const CAMERA_DISTANCE: f32 = 100.0;
/// A capture with fewer opaque pixels than this share of the atlas is empty.
const MIN_COVERAGE: f32 = 0.01;
/// The linear grey the pictures are baked in: half, so that a tint of twice a colour gives it back
/// (and a colour up to twice as bright as 1.0 still fits).
const NEUTRAL_GREY: f32 = 0.5;

/// Baking state: the scenes in flight and the variants that are settled.
#[derive(Resource, Default)]
pub(crate) struct ImpostorBakes {
    jobs: Vec<Job>,
    /// Variants that were baked, or that gave up.
    settled: HashSet<(AssetId<TreePool>, usize)>,
    /// Counts scenes, to give every camera its own order and every scene its own place.
    started: u32,
}

impl ImpostorBakes {
    /// Whether baking `variant` of `pool` is over, one way or the other.
    pub(crate) fn is_settled(&self, pool: AssetId<TreePool>, variant: usize) -> bool {
        self.settled.contains(&(pool, variant))
    }
}

/// Counts finished bakes, so systems that depend on them can tell when to look.
#[derive(Resource, Default)]
pub(crate) struct ImpostorsBaked(pub(crate) u32);

struct Job {
    pool: AssetId<TreePool>,
    variant: usize,
    frame: ImpostorFrame,
    /// What the cameras draw into.
    target: Handle<Image>,
    /// The scene's entities.
    scene: Vec<Entity>,
    /// Frames since the scene was spawned or the last capture came back empty.
    frames: u32,
    /// The capture in flight, if any.
    capture: Option<Entity>,
    /// Captures that came back empty.
    attempts: u32,
}

/// Identifies what a [`Screenshot`] captures.
#[derive(Component)]
struct BakeShot {
    pool: AssetId<TreePool>,
    variant: usize,
}

/// The four materials a scene draws a tree with: wood and leaves, for each picture.
struct BakeMaterials {
    wood_colour: Handle<ImpostorBakeMaterial>,
    leaf_colour: Handle<ImpostorBakeMaterial>,
    wood_surface: Handle<ImpostorBakeMaterial>,
    leaf_surface: Handle<ImpostorBakeMaterial>,
}

/// Start a scene for every variant that has no impostor yet and is not being
/// baked.
pub(crate) fn start_impostor_bakes(
    mut commands: Commands,
    pools: Res<Assets<TreePool>>,
    mut bakes: ResMut<ImpostorBakes>,
    mut images: ResMut<Assets<Image>>,
    mut bake_materials: ResMut<Assets<ImpostorBakeMaterial>>,
) {
    for (pool_id, pool) in pools.iter() {
        if !pool.lod.distances.impostor.is_finite() {
            continue;
        }
        for (index, variant) in pool.variants.iter().enumerate() {
            let busy = bakes
                .jobs
                .iter()
                .any(|j| j.pool == pool_id && j.variant == index);
            if variant.impostor.is_some() || busy || bakes.is_settled(pool_id, index) {
                continue;
            }
            if bakes.jobs.len() >= MAX_CONCURRENT_BAKES {
                return;
            }
            let target = images.add(Image::new_target_texture(
                IMPOSTOR_VIEWS * TILE_PIXELS,
                2 * TILE_PIXELS,
                TextureFormat::Rgba8Unorm,
                Some(TextureFormat::Rgba8UnormSrgb),
            ));
            let frame = ImpostorFrame::new(variant.height, variant.radius);
            let slot = bakes.started;
            bakes.started += 1;
            // Neutral grey: the colours of a look are a tint on the finished picture
            // (`ImpostorMaterial::tint`), so one picture serves every look and a recolouring
            // needs no new bake.
            let grey = LinearRgba::rgb(NEUTRAL_GREY, NEUTRAL_GREY, NEUTRAL_GREY);
            let materials = BakeMaterials {
                wood_colour: bake_materials.add(ImpostorBakeMaterial::colour(grey)),
                leaf_colour: bake_materials.add(ImpostorBakeMaterial::colour(grey)),
                wood_surface: bake_materials.add(ImpostorBakeMaterial::surface(grey, 0.0)),
                leaf_surface: bake_materials.add(ImpostorBakeMaterial::surface(grey, 1.0)),
            };
            let scene = spawn_scene(&mut commands, variant, &frame, &target, &materials, slot);
            bakes.jobs.push(Job {
                pool: pool_id,
                variant: index,
                frame,
                target,
                scene,
                frames: 0,
                capture: None,
                attempts: 0,
            });
        }
    }
}

/// Spawn the copies and the cameras of one variant. Returns every entity.
fn spawn_scene(
    commands: &mut Commands,
    variant: &PoolVariant,
    frame: &ImpostorFrame,
    target: &Handle<Image>,
    materials: &BakeMaterials,
    slot: u32,
) -> Vec<Entity> {
    let mut scene = Vec::new();
    let origin = BAKE_ORIGIN + Vec3::Z * (slot as f32 * SPACING);

    for view in 0..IMPOSTOR_VIEWS {
        let yaw = view as f32 / IMPOSTOR_VIEWS as f32 * TAU;
        let spot = origin + Vec3::X * (view as f32 * SPACING);
        // The camera looks along the same direction at every copy, so the copy
        // is what turns: the camera stands where view `view` says in the tree's
        // own frame once the tree is turned back by `yaw`.
        let turned = Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(-yaw));
        let look_at = spot + Vec3::Y * frame.center_y;
        let transparent = ClearColorConfig::Custom(Color::NONE);

        // Row 0 of the image is the colours, on `BAKE_LAYER`; row 1 the surface, on `BAKE_SURFACE_LAYER`.
        for (row, layer, wood, leaf) in [
            (0u32, BAKE_LAYER, &materials.wood_colour, &materials.leaf_colour),
            (1u32, BAKE_SURFACE_LAYER, &materials.wood_surface, &materials.leaf_surface),
        ] {
            let layers = RenderLayers::layer(layer);
            scene.push(
                commands
                    .spawn((
                        Mesh3d(variant.lods[1].clone()),
                        MeshMaterial3d(wood.clone()),
                        turned,
                        layers.clone(),
                    ))
                    .id(),
            );
            if let Some(leaves) = &variant.leaves[1] {
                scene.push(
                    commands
                        .spawn((
                            Mesh3d(leaves.clone()),
                            MeshMaterial3d(leaf.clone()),
                            turned,
                            layers.clone(),
                        ))
                        .id(),
                );
            }

            scene.push(
                commands
                    .spawn((
                        Camera3d::default(),
                        Camera {
                            // Before every other camera; each camera of a target has its own order.
                            order: -10_000
                                + (slot * 2 * IMPOSTOR_VIEWS + row * IMPOSTOR_VIEWS + view) as isize,
                            viewport: Some(Viewport {
                                physical_position: UVec2::new(view * TILE_PIXELS, row * TILE_PIXELS),
                                physical_size: UVec2::splat(TILE_PIXELS),
                                ..Default::default()
                            }),
                            clear_color: transparent,
                            // `None` would blend every camera after the first of the target
                            // by alpha, which drops the wood from the surface row (alpha 0).
                            output_mode: CameraOutputMode::Write {
                                blend_state: Some(BlendState::REPLACE),
                                clear_color: transparent,
                            },
                            ..Default::default()
                        },
                        RenderTarget::Image(target.clone().into()),
                        Projection::Orthographic(OrthographicProjection {
                            scaling_mode: ScalingMode::Fixed {
                                width: frame.view.x,
                                height: frame.view.y,
                            },
                            ..OrthographicProjection::default_3d()
                        }),
                        Transform::from_translation(
                            look_at + ImpostorFrame::toward_camera() * CAMERA_DISTANCE,
                        )
                        .looking_at(look_at, Vec3::Y),
                        Msaa::Off,
                        Tonemapping::None,
                        DebandDither::Disabled,
                        layers,
                    ))
                    .id(),
            );
        }
    }
    scene
}

/// Capture the scenes that have rendered for a few frames.
pub(crate) fn drive_impostor_bakes(
    mut commands: Commands,
    pools: Res<Assets<TreePool>>,
    mut bakes: ResMut<ImpostorBakes>,
) {
    let bakes = &mut *bakes;
    // A pool that is gone takes its jobs with it.
    let mut gone = Vec::new();
    for (index, job) in bakes.jobs.iter_mut().enumerate() {
        if pools.get(job.pool).is_none() {
            gone.push(index);
            continue;
        }
        if job.capture.is_none() {
            job.frames += 1;
            if job.frames >= SETTLE_FRAMES {
                let shot = BakeShot {
                    pool: job.pool,
                    variant: job.variant,
                };
                let entity = commands
                    .spawn((Screenshot::image(job.target.clone()), shot))
                    .observe(finish_bake)
                    .id();
                job.capture = Some(entity);
            }
        }
    }
    for index in gone.into_iter().rev() {
        let job = bakes.jobs.remove(index);
        for entity in job.scene {
            commands.entity(entity).despawn();
        }
    }
}

/// A capture came back: keep it if it has a tree in it, otherwise ask again.
#[allow(clippy::too_many_arguments)]
fn finish_bake(
    captured: On<ScreenshotCaptured>,
    shots: Query<&BakeShot>,
    mut commands: Commands,
    mut bakes: ResMut<ImpostorBakes>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ImpostorMaterial>>,
    mut pools: ResMut<Assets<TreePool>>,
    mut baked: ResMut<ImpostorsBaked>,
    wind: Res<FloraWindBuffer>,
    recolors: Res<PoolRecolors>,
) {
    let Ok(shot) = shots.get(captured.entity) else {
        return;
    };
    let bakes = &mut *bakes;
    let Some(index) = bakes
        .jobs
        .iter()
        .position(|j| j.pool == shot.pool && j.variant == shot.variant)
    else {
        return;
    };

    let image = &captured.image;
    let (width, height) = (IMPOSTOR_VIEWS * TILE_PIXELS, 2 * TILE_PIXELS);
    // Bytes in the colour row; the surface row follows it.
    let row = (width * TILE_PIXELS * 4) as usize;
    let pixels = image
        .data
        .as_deref()
        .filter(|data| data.len() == (width * height * 4) as usize);
    // A tree is there if the colour row has one: the surface row's alpha is translucency.
    let covered = pixels.map_or(0, |data| {
        data[..row]
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] >= 128)
            .count()
    });
    let enough = covered as f32 >= MIN_COVERAGE * (width * TILE_PIXELS) as f32;

    let job = &mut bakes.jobs[index];
    if !enough {
        job.attempts += 1;
        if job.attempts < MAX_ATTEMPTS {
            job.capture = None;
            job.frames = SETTLE_FRAMES.saturating_sub(2);
            return;
        }
        warn!(
            "bevy_flora: no impostor for variant {} of a tree pool: {MAX_ATTEMPTS} captures of its baking scene came back empty",
            job.variant
        );
    }

    let job = bakes.jobs.remove(index);
    for entity in &job.scene {
        commands.entity(*entity).despawn();
    }
    bakes.settled.insert((job.pool, job.variant));
    if let (true, Some(pixels)) = (enough, pixels)
        && let Some(mut pool) = pools.get_mut(job.pool)
    {
        let albedo = images.add(atlas_image(&pixels[..row], width, TILE_PIXELS));
        let surface = images.add(surface_image(
            &linearize_surface(&pixels[row..]),
            &pixels[..row],
            width,
            TILE_PIXELS,
        ));
        // The pictures are neutral grey: one material per look tints them to its colours.
        let looks = (0..pool.looks.len() as u32)
            .map(|look| {
                let colors = recolors.colors(job.pool, look, &pool);
                materials.add(ImpostorMaterial::new(
                    albedo.clone(),
                    surface.clone(),
                    IMPOSTOR_VIEWS,
                    pool.params.style,
                    FloraUniform::new(&colors.apply(&pool.params)),
                    &wind,
                    ImpostorTint::new(colors.wood, colors.leaves),
                    colors.unlit,
                ))
            })
            .collect();
        pool.variants[job.variant].impostor = Some(ImpostorAtlas {
            views: IMPOSTOR_VIEWS,
            size: job.frame.size,
            center_y: job.frame.center_y,
            materials: looks,
            quad: meshes.add(job.frame.quad()),
            albedo,
            surface,
        });
    }
    baked.0 += 1;
}
