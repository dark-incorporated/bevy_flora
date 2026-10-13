//! Bevy integration: components, the `FloraPlugin`, and the systems that
//! grow trees and rebuild their meshes.
//!
//! A tree entity carries [`ProceduralTree`] (its parameters) and [`TreeAge`].
//! Whenever either changes, [`rebuild_trees`] regenerates the meshes and keeps
//! two child entities (wood and leaves) in sync. Adding [`Growing`] advances
//! `TreeAge` every frame until the tree is mature.
//!
//! Performance: a growing tree rebuilds both meshes every frame (0.1-0.4 ms of
//! CPU per tree); `Growing` is meant for a few hero trees, not a forest. The
//! full-grown skeleton is kept between frames and regenerated only when
//! [`ProceduralTree`] changes. Trees built or rebuilt in the same frame are built
//! on all cores, and that frame waits for them; a tree with [`BuildInBackground`] is built on
//! the async compute pool instead and appears the frame its build is done. Trees with equal
//! looks (equal colours, trunk height, wind response and leaf
//! shading; seeds do not matter) share their materials, which are never
//! written in place, so the renderer prepares each look once and does not
//! re-prepare anything as a tree grows.
//!
//! For a forest, add [`GpuGrowth`] to the trees: the mature meshes are built and
//! uploaded once, and each frame only a few kilobytes of per-node state are
//! written (a few microseconds of CPU per tree); the vertex shader does the
//! growing. See its docs for what differs from the CPU path.

use bevy::asset::{AssetEvent, Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::ecs::system::EntityCommands;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::render::render_resource::Face;
use bevy::camera::visibility::{Visibility, VisibilityRange};
use bevy::tasks::{ComputeTaskPool, TaskPool};
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::background::{BuildInBackground, PendingBuild};
use crate::falling::{
    DropLeaves, FallCaches, FallingLeaves, LeafBurst, SeasonShed, drop_leaves, expire_bursts, forget_changed_meshes,
    season_sheds, shake_leaves_loose, shed_landed_leaves, shed_with_season,
};
use crate::gpu::render::{GrowthPaint, detect_gpu_growth_support, rebuild_gpu_trees, sync_growth_mode};
use crate::gpu::{GpuGrowth, GpuGrowthSupport, grows_on_gpu};
use crate::impostor::{ImpostorSupport, drive_impostor_bakes, start_impostor_bakes};
use crate::params::TreeParams;
use crate::pipeline::{aged_skeleton, build_aged};
use crate::see_through::{SeeThroughTrees, arm_see_through, sync_see_through};
use crate::push::{FoliagePusher, PushSlots, arm_push, sync_pushers};
use crate::fire::{
    Burnt, FireRebind, FloraFire, TreeBurnt, arm_fire, burn, fire_burns, fire_changed, fire_rebinds, rebind_fire,
    report_burnt, sync_fire,
};
use crate::shape::TreeShape;
use crate::lod::{LodDistances, LodSettings, TreeLod, coarse_levels};
use crate::pool::{
    LodBand, LodScale, ZoomScale, banded, fill_pooled_shapes, follow_orthographic_zoom, scale_lod_bands,
    spawn_pooled_trees, sync_shadow_casting,
};
use crate::shading::{
    FloraMaterial, FloraMaterialSupport, FloraSeason, FloraWetness, FloraWind, FloraWindBuffer, SharedFlora,
    TextureLibrary, TreeShakes,
    detect_flora_support,
    expire_shakes, flora_decided, sync_wind,
};
use crate::skeleton::{Skeleton, generate_skeleton};
use crate::felling::{Felled, TreeLanded, buck_trees, carve_notches, drive_felling, start_felling};
use crate::tree_asset::sync_tree_assets;

/// Parameters of a procedural tree. Changing it rebuilds the meshes.
///
/// The plugin draws the tree with child entities (its wood, leaves, flowers, coarser levels,
/// cut faces). [`NotShadowCaster`](bevy::light::NotShadowCaster) on the tree reaches every one of
/// them, rebuilt or new, and taking it off gives the shadows back.
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
#[require(TreeAge, Transform, Visibility)]
pub struct ProceduralTree(pub TreeParams);

/// Age of the tree in growth units (see [`crate::GrowthParams`]).
/// Defaults to `f32::INFINITY`: fully grown.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeAge(pub f32);

impl Default for TreeAge {
    fn default() -> Self {
        Self(f32::INFINITY)
    }
}

/// Advances [`TreeAge`] by `rate` age units per second until mature.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Growing {
    /// Age units added to [`TreeAge`] per second.
    pub rate: f32,
}

impl Default for Growing {
    fn default() -> Self {
        Self { rate: 1.0 }
    }
}

/// Sent after a tree's meshes were (re)built.
#[derive(Message, Clone, Copy, Debug)]
pub struct TreeGenerated {
    /// The tree entity whose meshes were rebuilt.
    pub entity: Entity,
    /// Current trunk height (full height × growth progress).
    pub height: f32,
    /// Trunk radius parameter.
    pub radius: f32,
}

/// Sent once when a [`Growing`] tree reaches its mature age (the age after which it no longer
/// changes): the moment to swap a sapling's gameplay for a tree's, or to remove [`Growing`].
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeMature {
    /// The tree entity.
    pub entity: Entity,
}

/// Render state owned by the plugin for one tree entity.
#[derive(Component)]
pub struct TreeRender {
    pub(crate) trunk: Entity,
    /// `None` until the trunk first has geometry.
    pub(crate) trunk_mesh: Option<Handle<Mesh>>,
    pub(crate) paint: Paint,
    pub(crate) foliage: Option<(Entity, Handle<Mesh>)>,
    /// A flowering tree's flowers ([`TreeFlowerMesh`]), drawn with the leaf material.
    pub(crate) flowers: Option<(Entity, Handle<Mesh>)>,
    /// Full-grown skeleton generated from the current parameters; aging it
    /// is all a growing tree needs per frame.
    pub(crate) full: Skeleton,
    /// Age after which rebuilding changes nothing.
    pub mature_at: f32,
    /// The age the meshes show, for a tree rebuilt on the CPU (a [`BuildInBackground`] tree
    /// whose age moved while it was built is rebuilt until they agree).
    pub(crate) age: f32,
    /// Levels 1 and 2 of a tree drawn with [`TreeLod`], and the settings they were built with.
    pub(crate) lod: Option<(LodSettings, [CoarseLevel; 2])>,
}

/// How a tree's wood and leaves are drawn.
pub(crate) enum Paint {
    /// Meshes rebuilt on the CPU, drawn with plain materials (the renderer's plugins came after
    /// this one, or the device has no storage buffers in the vertex stage, so
    /// [`FloraMaterial`] cannot be drawn).
    Standard {
        trunk: Handle<StandardMaterial>,
        foliage: Handle<StandardMaterial>,
    },
    /// Meshes rebuilt on the CPU, drawn with leaf shading and wind.
    Flora {
        trunk: Handle<FloraMaterial>,
        foliage: Handle<FloraMaterial>,
    },
    /// The mature meshes, grown in the vertex shader.
    Growth(GrowthPaint),
}

impl Paint {
    /// This paint's handles, for a tree rebuilt on the CPU; `None` for one grown on the GPU.
    pub(crate) fn cloned_cpu(&self) -> Option<Paint> {
        match self {
            Paint::Standard { trunk, foliage } => Some(Paint::Standard {
                trunk: trunk.clone(),
                foliage: foliage.clone(),
            }),
            Paint::Flora { trunk, foliage } => Some(Paint::Flora {
                trunk: trunk.clone(),
                foliage: foliage.clone(),
            }),
            Paint::Growth(_) => None,
        }
    }
}

impl TreeRender {
    /// Child entity carrying the wood mesh.
    pub fn trunk_entity(&self) -> Entity {
        self.trunk
    }

    /// Child entity carrying the leaf mesh, if any leaves are currently visible.
    pub fn foliage_entity(&self) -> Option<Entity> {
        self.foliage.as_ref().map(|(e, _)| *e)
    }

    /// Child entity carrying a flowering tree's flowers ([`TreeMeshes::flowers`](crate::TreeMeshes::flowers)),
    /// if it has any leaves; hidden while [`FloraSeason::blossom`] is 0.
    pub fn flower_entity(&self) -> Option<Entity> {
        self.flowers.as_ref().map(|(e, _)| *e)
    }

    /// Every leaf mesh the tree draws: level 0's, then its coarser levels' ([`TreeLod`]).
    pub(crate) fn leaf_meshes(&self) -> impl Iterator<Item = &Handle<Mesh>> {
        let coarse = self.lod.iter().flat_map(|(_, levels)| levels.iter().filter_map(|level| level.leaves.as_ref()));
        self.foliage.iter().chain(coarse).map(|(_, handle)| handle)
    }
}

/// Marker on the wood child entity.
#[derive(Component)]
pub struct TreeTrunkMesh;

/// Marker on the leaves child entity.
#[derive(Component)]
pub struct TreeFoliageMesh;

/// Marker on a flowering tree's flowers child entity ([`TreeMeshes::flowers`](crate::TreeMeshes::flowers)),
/// drawn with the leaves' material. The plugin hides it while
/// [`FloraSeason::blossom`] is 0, so flowers out of season cost nothing.
#[derive(Component)]
pub struct TreeFlowerMesh;

/// What a flowers child is spawned with: hidden until [`show_flowers`] has looked at the season,
/// so it never shows for a frame out of season (and has a `Visibility` without the renderer).
pub(crate) const FLOWERS_SPAWN: (TreeFlowerMesh, Visibility) = (TreeFlowerMesh, Visibility::Hidden);

/// Registers the tree components and the grow/rebuild systems.
pub struct FloraPlugin;

impl bevy::app::Plugin for FloraPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_message::<TreeGenerated>()
            .add_message::<TreeMature>()
            .add_message::<DropLeaves>()
            .add_message::<TreeBurnt>()
            .init_resource::<FallingLeaves>()
            .init_resource::<FallCaches>()
            .init_resource::<SeasonShed>()
            .init_resource::<FireRebind>()
            .init_resource::<PushSlots>()
            .register_type::<ProceduralTree>()
            .register_type::<TreeAge>()
            .register_type::<Growing>()
            .register_type::<BuildInBackground>()
            .register_type::<SeeThroughTrees>()
            .register_type::<FoliagePusher>()
            .register_type::<TreeLod>()
            .register_type::<FallingLeaves>()
            .register_type::<Burnt>()
            .init_resource::<LodScale>()
            .init_resource::<ZoomScale>()
            .register_type::<LodScale>()
            .add_systems(
                bevy::app::Update,
                (
                    detect_gpu_growth_support,
                    detect_flora_support,
                    expire_shakes,
                    sync_wind.run_if(
                        |wind: Res<FloraWind>,
                         season: Res<FloraSeason>,
                         wetness: Res<FloraWetness>,
                         shakes: Res<TreeShakes>| {
                            wind.is_changed() || season.is_changed() || wetness.is_changed() || shakes.is_changed()
                        },
                    ),
                    sync_tree_assets.run_if(resource_exists::<Assets<TreeParams>>),
                    grow_trees,
                    sync_growth_mode,
                    rebuild_trees.run_if(flora_decided),
                    rebuild_gpu_trees,
                    update_tree_shapes,
                    carve_notches,
                    start_felling,
                    drive_felling,
                    buck_trees,
                    show_flowers,
                )
                    .chain(),
            )
            .add_systems(
                bevy::app::Update,
                (
                    (start_impostor_bakes, drive_impostor_bakes)
                        .chain()
                        .run_if(resource_exists::<ImpostorSupport>),
                    spawn_pooled_trees.run_if(flora_decided),
                    fill_pooled_shapes,
                )
                    .chain(),
            )
            // After transform propagation: the shaders cut only in the view drawn from where the
            // fading camera is this frame, and push foliage from where the pushers are this frame.
            .add_systems(
                bevy::app::PostUpdate,
                (
                    arm_see_through,
                    sync_see_through,
                    arm_push,
                    sync_pushers,
                    crate::index::index_trees,
                    sync_shadow_casting,
                )
                    .after(bevy::transform::TransformSystems::Propagate),
            )
            // Levels of detail follow `LodScale` and an orthographic camera's zoom: after transform
            // propagation (the camera is where it is this frame), before the ranges are checked.
            .add_systems(
                bevy::app::PostUpdate,
                (follow_orthographic_zoom, scale_lod_bands)
                    .chain()
                    .after(bevy::transform::TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            )
            // The fire steps after transform propagation (its fuel is where the trees stand this
            // frame) and is written into the shared buffer when it changed; the materials are
            // re-prepared the frame after the buffer is resized, and carry the fire while one
            // exists.
            .add_systems(
                bevy::app::PostUpdate,
                (
                    rebind_fire.run_if(fire_rebinds),
                    burn.run_if(fire_burns),
                    report_burnt.run_if(resource_exists_and_changed::<FloraFire>),
                    sync_fire.run_if(fire_changed),
                    arm_fire,
                )
                    .chain()
                    .after(bevy::transform::TransformSystems::Propagate)
                    .before(bevy::asset::AssetEventSystems),
            )
            // Falling leaves start from where the trees are this frame, and from their leaf meshes
            // as they are this frame (a growing tree's change every frame). Nothing runs while
            // nothing shakes, lands, drops, sheds with the season or lies on the ground.
            .add_systems(
                bevy::app::PostUpdate,
                (
                    (
                        forget_changed_meshes.run_if(on_message::<AssetEvent<Mesh>>),
                        shake_leaves_loose.run_if(resource_changed::<TreeShakes>),
                        shed_landed_leaves.run_if(on_message::<TreeLanded>),
                        drop_leaves.run_if(on_message::<DropLeaves>),
                        shed_with_season.run_if(season_sheds),
                    )
                        .chain(),
                    expire_bursts.run_if(any_with_component::<LeafBurst>),
                )
                    .after(bevy::transform::TransformSystems::Propagate)
                    .after(bevy::asset::AssetEventSystems),
            );
        // Before `gpu`: its shaders import the flora library.
        crate::shading::build(app);
        crate::gpu::build(app);
        crate::impostor::build(app);
        crate::pool::build(app);
        crate::tree_asset::build(app);
        crate::felling::build(app);
        crate::index::build(app);
        crate::harvest::build(app);
    }

    fn finish(&self, app: &mut bevy::app::App) {
        crate::shading::finish(app);
        crate::gpu::finish(app);
        crate::impostor::finish(app);
        crate::pool::finish(app);
        crate::tree_asset::finish(app);
    }
}

/// Share of diffuse light a leaf lets through to its other side, so leaves
/// turned away from the sun are lit from behind instead of near-black.
const LEAF_TRANSLUCENCY: f32 = 0.5;

pub(crate) fn material(base_color: bevy::color::Color) -> StandardMaterial {
    StandardMaterial {
        base_color,
        perceptual_roughness: 1.0,
        cull_mode: Some(Face::Back),
        ..Default::default()
    }
}

pub(crate) fn leaf_material(base_color: bevy::color::Color) -> StandardMaterial {
    StandardMaterial {
        diffuse_transmission: LEAF_TRANSLUCENCY,
        ..material(base_color)
    }
}

/// The plain material of a tree's crown for devices without the flora shaders: a leaf's, which
/// lets light through, or a hull's ([`CrownStyle::Hull`](crate::CrownStyle)), which is matte
/// paper like its trunk.
pub(crate) fn crown_material(params: &crate::params::TreeParams) -> StandardMaterial {
    if matches!(params.crown, crate::params::CrownStyle::Hull { .. }) {
        material(params.foliage.color)
    } else {
        leaf_material(params.foliage.color)
    }
}

/// Advance the age of every [`Growing`] tree that already has render state, and send
/// [`TreeMature`] for each that reaches its mature age this frame.
pub fn grow_trees(
    time: Res<Time>,
    mut trees: Query<(Entity, &mut TreeAge, &Growing, &TreeRender), Without<Felled>>,
    mut mature: MessageWriter<TreeMature>,
) {
    let dt = time.delta_secs();
    for (entity, mut age, growing, render) in &mut trees {
        let next = (age.0 + growing.rate * dt).min(render.mature_at);
        if next != age.0 {
            if next >= render.mature_at && age.0 < render.mature_at {
                mature.write(TreeMature { entity });
            }
            age.0 = next;
        }
    }
}

/// Bring every [`TreeShape`] up to date: those of trees rebuilt this frame, and those just
/// added to a tree that already has its meshes. The aged skeleton is made from the kept full
/// skeleton, so both growth paths give the same shape. Written only when it changed.
pub fn update_tree_shapes(
    mut generated: MessageReader<TreeGenerated>,
    mut trees: Query<(&ProceduralTree, &TreeAge, &TreeRender, &mut TreeShape)>,
) {
    fn update(tree: &ProceduralTree, age: &TreeAge, render: &TreeRender, shape: &mut Mut<TreeShape>) {
        let skeleton = aged_skeleton(&render.full, &tree.0, age.0);
        let new = TreeShape::from_skeleton(&skeleton, &tree.0, shape.limb_radius);
        shape.set_if_neq(new);
    }
    for message in generated.read() {
        if let Ok((tree, age, render, mut shape)) = trees.get_mut(message.entity) {
            update(tree, age, render, &mut shape);
        }
    }
    for (tree, age, render, mut shape) in &mut trees {
        if shape.is_added() {
            update(tree, age, render, &mut shape);
        }
    }
}

/// Rebuild meshes for trees whose parameters or age changed. A tree with [`BuildInBackground`] is
/// built on the async compute pool instead, and its meshes are put in the frame the build is done.
// Public so that other systems can be ordered against it; the background builds it polls are the
// plugin's own.
#[allow(clippy::type_complexity, clippy::too_many_arguments, private_interfaces)]
pub fn rebuild_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut textures: TextureLibrary,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    support: Res<GpuGrowthSupport>,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (
            Entity,
            Ref<ProceduralTree>,
            Ref<TreeAge>,
            Option<&mut TreeRender>,
            Has<GpuGrowth>,
            Has<BuildInBackground>,
            Option<&TreeLod>,
        ),
        Without<Felled>,
    >,
    mut pending: Query<(Entity, &mut PendingBuild<RebuiltTree>)>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());

    // Phase 0: the background builds that are done.
    let mut finished = Vec::new();
    for (entity, mut build) in &mut pending {
        if let Some(out) = build.poll() {
            commands.entity(entity).remove::<PendingBuild<RebuiltTree>>();
            finished.push(out);
        }
    }

    // Phase 1: collect what to build.
    let mut jobs = Vec::new();
    for (entity, tree, age, mut render, gpu, background, lod) in &mut trees {
        // A tree that grows on the GPU is `rebuild_gpu_trees`'s.
        if grows_on_gpu(gpu, &support, &tree.0) {
            continue;
        }
        // Left over from before the tree stopped growing on the GPU;
        // `sync_growth_mode` removes it.
        if render.as_deref().is_some_and(|render| render.paint.cloned_cpu().is_none()) {
            continue;
        }
        // Its levels of detail are not those of its `TreeLod` (it came, went or changed).
        let lod = lod.map(|lod| lod.0);
        let levels_stale = render.as_deref().is_some_and(|render| render.lod.as_ref().map(|(s, _)| *s) != lod);
        if background {
            // A change of parameters replaces a build under way; otherwise the tree is built
            // whenever its meshes are not of its age or its levels (or it has none) and no build
            // is under way.
            let building = pending.contains(entity);
            let behind = render.as_deref().is_none_or(|render| render.age != age.0) || levels_stale;
            if !(tree.is_changed() || (behind && !building)) {
                continue;
            }
            // The kept skeleton is copied, not taken: the tree keeps it while the build runs.
            let kept = render.as_deref().filter(|_| !tree.is_changed()).map(|render| render.full.clone());
            let job = RebuildJob { entity, params: tree.0.clone(), age: age.0, kept, background: true, lod };
            commands.entity(entity).insert(PendingBuild::spawn(move || build_job(job)));
            continue;
        }
        if !(tree.is_changed() || age.is_changed() || render.is_none() || levels_stale) {
            continue;
        }
        // Only the age (or the levels) changed: age the skeleton generated last time.
        let kept = match render.as_mut() {
            Some(render) if !tree.is_changed() => Some(std::mem::take(&mut render.full)),
            _ => None,
        };
        jobs.push(RebuildJob { entity, params: tree.0.clone(), age: age.0, kept, background: false, lod });
    }

    // Phase 2 builds on all cores, meshes included; phase 3 applies each result here, in order,
    // after the background builds that are done.
    let mut apply = |out: RebuiltTree| {
        let (entity, full, trunk_progress) = (out.entity, out.full, out.trunk_progress);
        let Ok((_, tree, _, render, gpu, _, _)) = trees.get_mut(entity) else { return };
        // A background build is out of date if the parameters changed again since it started (a
        // new one is under way) or the tree has moved to the GPU.
        if out.background && (tree.is_changed() || grows_on_gpu(gpu, &support, &tree.0)) {
            return;
        }
        let params = &tree.0;
        let existing = render.as_deref().and_then(|render| render.paint.cloned_cpu());
        match (render, existing) {
            (None, _) => {
                let paint = if let Some(wind) = wind {
                    Paint::Flora {
                        trunk: shared.resolve(&mut flora, &textures.wood(params), wind),
                        foliage: shared.resolve(&mut flora, &textures.leaves(params), wind),
                    }
                } else {
                    Paint::Standard {
                        trunk: materials.add(material(params.trunk_color)),
                        foliage: materials.add(crown_material(params)),
                    }
                };
                let mut trunk_child = commands.spawn((TreeTrunkMesh, Visibility::Hidden, ChildOf(entity)));
                paint_child(&mut trunk_child, &paint, false);
                let trunk = trunk_child.id();
                let mut trunk_mesh = None;
                sync_trunk(&mut commands, &mut meshes, trunk, &mut trunk_mesh, out.trunk);
                let mut foliage = None;
                sync_leaf_child(&mut commands, &mut meshes, &mut foliage, out.foliage, |commands, mesh| {
                    spawn_mesh_child(commands, mesh, TreeFoliageMesh, entity, &paint, true)
                });
                let mut flowers = None;
                sync_leaf_child(&mut commands, &mut meshes, &mut flowers, out.flowers, |commands, mesh| {
                    spawn_mesh_child(commands, mesh, FLOWERS_SPAWN, entity, &paint, true)
                });
                let mut lod = None;
                let level0 = [Some(trunk), foliage.as_ref().map(|c| c.0), flowers.as_ref().map(|c| c.0)];
                sync_levels(&mut commands, &mut meshes, entity, &mut lod, out.lod, &paint, level0);
                commands.entity(entity).insert(TreeRender {
                    trunk,
                    trunk_mesh,
                    paint,
                    foliage,
                    flowers,
                    full,
                    mature_at: out.mature_at,
                    age: out.age,
                    lod,
                });
            }
            (Some(mut render), Some(paint)) => {
                let render = &mut *render;
                render.full = full;
                render.mature_at = out.mature_at;
                render.age = out.age;
                let trunk_entity = render.trunk;
                sync_trunk(
                    &mut commands,
                    &mut meshes,
                    trunk_entity,
                    &mut render.trunk_mesh,
                    out.trunk,
                );
                match &paint {
                    Paint::Standard { trunk, foliage } => {
                        set_color(&mut materials, trunk, params.trunk_color);
                        set_color(&mut materials, foliage, params.foliage.color);
                    }
                    // An age-only rebuild cannot change the looks, so it resolves nothing.
                    Paint::Flora { .. } if out.regenerated => {
                        let wind = wind.expect("flora paint implies the wind buffer");
                        let wood = shared.resolve(&mut flora, &textures.wood(params), wind);
                        let leaves = shared.resolve(&mut flora, &textures.leaves(params), wind);
                        if let Paint::Flora { trunk, foliage } = &mut render.paint {
                            let coarse = render.lod.iter().flat_map(|(_, levels)| levels);
                            if *trunk != wood {
                                let woods = coarse.clone().flat_map(|level| level.wood.iter().map(|c| c.0));
                                for child in std::iter::once(render.trunk).chain(woods) {
                                    commands.entity(child).insert(MeshMaterial3d(wood.clone()));
                                }
                                *trunk = wood;
                            }
                            if *foliage != leaves {
                                let coarse_leaves = coarse.flat_map(|level| level.leaves.iter().map(|c| c.0));
                                let level0 = render.foliage.iter().chain(&render.flowers).map(|c| c.0);
                                for child in level0.chain(coarse_leaves) {
                                    commands.entity(child).insert(MeshMaterial3d(leaves.clone()));
                                }
                                *foliage = leaves;
                            }
                        }
                    }
                    Paint::Flora { .. } => {}
                    Paint::Growth(_) => unreachable!("trees grown on the GPU were skipped above"),
                }

                let paint = &render.paint;
                sync_leaf_child(&mut commands, &mut meshes, &mut render.foliage, out.foliage, |commands, mesh| {
                    spawn_mesh_child(commands, mesh, TreeFoliageMesh, entity, paint, true)
                });
                sync_leaf_child(&mut commands, &mut meshes, &mut render.flowers, out.flowers, |commands, mesh| {
                    spawn_mesh_child(commands, mesh, FLOWERS_SPAWN, entity, paint, true)
                });
                let level0 = [Some(render.trunk), render.foliage.as_ref().map(|c| c.0), render.flowers.as_ref().map(|c| c.0)];
                sync_levels(&mut commands, &mut meshes, entity, &mut render.lod, out.lod, &render.paint, level0);
            }
            // A tree painted for the GPU was skipped above.
            (Some(_), None) => {}
        }

        generated.write(TreeGenerated {
            entity,
            height: params.trunk.height * trunk_progress,
            radius: params.trunk.radius,
        });
    };
    for out in finished {
        apply(out);
    }
    // Ageing a kept skeleton costs about 20 us a tree, less than waking the pool for a few
    // hundred of them (the growing-tree case); generating a skeleton is worth spreading.
    let parallel = jobs.len() > 1000 || jobs.iter().filter(|job| job.kept.is_none()).count() >= 2;
    build_in_batches(jobs, parallel, build_job, &mut apply);
}

/// Run `build` on every job and hand each result to `apply` in the order of
/// `jobs`. With `parallel` the jobs are built on the compute task pool, a few per
/// thread at a time, so a large input never holds all its results at once;
/// without it (or with fewer than two jobs) they run inline, without a scope.
/// `build` must be a pure function of its job: the results do not depend on the
/// number of threads.
pub(crate) fn build_in_batches<J: Send, R: Send + 'static>(
    jobs: Vec<J>,
    parallel: bool,
    build: impl Fn(J) -> R + Sync,
    mut apply: impl FnMut(R),
) {
    if !parallel || jobs.len() < 2 {
        jobs.into_iter().for_each(|job| apply(build(job)));
        return;
    }
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    let batch = pool.thread_num() * 4;
    let build = &build;
    let mut jobs = jobs.into_iter();
    loop {
        let results = pool.scope(|scope| {
            for job in jobs.by_ref().take(batch) {
                scope.spawn(async move { build(job) });
            }
        });
        if results.is_empty() {
            return;
        }
        results.into_iter().for_each(&mut apply);
    }
}

/// Give a wood or leaf child the material `paint` holds for it.
pub(crate) fn paint_child(child: &mut EntityCommands, paint: &Paint, leaf: bool) {
    match (paint, leaf) {
        (Paint::Standard { trunk, .. }, false) => child.insert(MeshMaterial3d(trunk.clone())),
        (Paint::Standard { foliage, .. }, true) => child.insert(MeshMaterial3d(foliage.clone())),
        (Paint::Flora { trunk, .. }, false) => child.insert(MeshMaterial3d(trunk.clone())),
        (Paint::Flora { foliage, .. }, true) => child.insert(MeshMaterial3d(foliage.clone())),
        (Paint::Growth(_), _) => unreachable!("trees grown on the GPU are spawned by rebuild_gpu_trees"),
    };
}

/// Set the material's base colour, marking it modified only when the colour
/// differs: a modified material is re-extracted and re-prepared by the
/// renderer, which a growing tree would otherwise trigger every frame.
fn set_color(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, color: bevy::color::Color) {
    if let Some(mut material) = materials.get_mut(handle)
        && material.base_color != color
    {
        material.base_color = color;
    }
}

/// Bring the trunk child in sync with `data`. An empty trunk (age 0) hides the
/// child and uploads nothing; the mesh asset is created the first time the
/// trunk has geometry.
fn sync_trunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    child: Entity,
    handle: &mut Option<Handle<Mesh>>,
    mesh: Option<Mesh>,
) {
    let Some(mesh) = mesh else {
        commands.entity(child).insert(Visibility::Hidden);
        return;
    };
    commands.entity(child).insert(Visibility::Inherited);
    match handle {
        Some(handle) => replace_mesh(commands, meshes, child, handle, mesh),
        None => {
            let new = meshes.add(mesh);
            commands.entity(child).insert(Mesh3d(new.clone()));
            *handle = Some(new);
        }
    }
}

/// What a worker makes of one [`RebuildJob`]: the skeleton and the Bevy meshes
/// (`None` for an empty one).
pub(crate) struct RebuiltTree {
    entity: Entity,
    /// The age the meshes show.
    age: f32,
    /// The skeleton was generated, not kept: the parameters changed (or the tree is new), so its
    /// looks may have too.
    regenerated: bool,
    /// Built by a [`PendingBuild`] task: it may be out of date by the time it is done.
    background: bool,
    full: Skeleton,
    trunk: Option<Mesh>,
    foliage: Option<Mesh>,
    flowers: Option<Mesh>,
    mature_at: f32,
    trunk_progress: f32,
    /// A tree drawn with [`TreeLod`]: its settings and levels 1 and 2.
    lod: Option<(LodSettings, [CoarseMeshes; 2])>,
}

/// One tree for `rebuild_trees` to build: its parameters, its age and, when only the age
/// changed, the full-grown skeleton kept from last time.
struct RebuildJob {
    entity: Entity,
    params: TreeParams,
    age: f32,
    kept: Option<Skeleton>,
    background: bool,
    /// [`TreeLod`]'s settings, when the tree has one.
    lod: Option<LodSettings>,
}

/// Build one [`RebuildJob`]: on any thread, so a pure function of the job.
fn build_job(job: RebuildJob) -> RebuiltTree {
    let regenerated = job.kept.is_none();
    let full = job.kept.unwrap_or_else(|| generate_skeleton(&job.params));
    let out = build_aged(&full, &job.params, job.age);
    let lod = job.lod.map(|settings| {
        let levels = coarse_levels(&out, &job.params, &settings).map(|level| {
            let mut leaves = level.foliage;
            leaves.extend(&level.flowers);
            CoarseMeshes {
                wood: (!level.trunk.is_empty()).then(|| level.trunk.into_bevy_mesh()),
                leaves: (!leaves.is_empty()).then(|| leaves.into_bevy_mesh()),
            }
        });
        (settings, levels)
    });
    RebuiltTree {
        entity: job.entity,
        age: job.age,
        regenerated,
        background: job.background,
        full,
        trunk: (!out.trunk.is_empty()).then(|| out.trunk.into_bevy_mesh()),
        foliage: (!out.foliage.is_empty()).then(|| out.foliage.into_bevy_mesh()),
        flowers: (!out.flowers.is_empty()).then(|| out.flowers.into_bevy_mesh()),
        mature_at: out.mature_at,
        trunk_progress: out.skeleton.nodes[0].progress,
        lod,
    }
}

/// A coarser level of detail of a tree drawn with [`TreeLod`]: its wood and leaf children.
#[derive(Default)]
pub(crate) struct CoarseLevel {
    wood: Option<(Entity, Handle<Mesh>)>,
    leaves: Option<(Entity, Handle<Mesh>)>,
}

impl CoarseLevel {
    /// Its children, the wood's first.
    pub(crate) fn children(&self) -> impl Iterator<Item = Entity> + '_ {
        self.wood.iter().chain(&self.leaves).map(|(child, _)| *child)
    }
}

/// What a worker makes of a coarse level: its wood and its leaves, with a flowering tree's
/// flowers after them (the shaders hide them out of season, as on a pool's coarse levels).
pub(crate) struct CoarseMeshes {
    wood: Option<Mesh>,
    leaves: Option<Mesh>,
}

/// Marker on a child of a tree drawn with [`TreeLod`] that carries one of its coarse levels
/// (1 or 2), its wood or its leaves.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeLodMesh(pub u8);

/// The distances level `level` of a tree drawn with [`TreeLod`] is drawn at: a pool's bands,
/// with level 2 at every distance beyond `lod2` (an individual tree has no impostor).
fn level_band(level: usize, distances: LodDistances) -> (LodBand, VisibilityRange) {
    match level {
        0 => banded(0.0, distances.lod1),
        1 => banded(distances.lod1, distances.lod2),
        _ => banded(distances.lod2, f32::INFINITY),
    }
}

/// Bring a tree's levels of detail in line with `want` (its [`TreeLod`] settings and coarse
/// meshes, or `None` without one): the coarse children, and the band of distances the full
/// tree's own children (`level0`) are drawn at, which a tree without levels does not limit.
fn sync_levels(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    tree: Entity,
    have: &mut Option<(LodSettings, [CoarseLevel; 2])>,
    want: Option<(LodSettings, [CoarseMeshes; 2])>,
    paint: &Paint,
    level0: [Option<Entity>; 3],
) {
    let old = have.as_ref().map(|(settings, _)| settings.distances);
    let Some((settings, built)) = want else {
        if let Some((_, levels)) = have.take() {
            for child in levels.iter().flat_map(CoarseLevel::children) {
                commands.entity(child).despawn();
            }
            for child in level0.into_iter().flatten() {
                commands.entity(child).remove::<(VisibilityRange, LodBand)>();
            }
        }
        return;
    };
    let distances = settings.distances;
    let moved = old != Some(distances);
    // A child of the full tree spawned since (the leaves of a tree that had none) gets its band;
    // the others keep theirs unless the distances moved.
    for child in level0.into_iter().flatten() {
        if moved {
            commands.entity(child).insert(level_band(0, distances));
        } else {
            commands.entity(child).insert_if_new(level_band(0, distances));
        }
    }
    let (kept, levels) = have.get_or_insert_with(|| (settings, Default::default()));
    *kept = settings;
    for (index, (level, built)) in levels.iter_mut().zip(built).enumerate() {
        let range = level_band(index + 1, distances);
        if moved {
            for child in level.children() {
                commands.entity(child).insert(range.clone());
            }
        }
        let marker = TreeLodMesh(index as u8 + 1);
        sync_leaf_child(commands, meshes, &mut level.wood, built.wood, |commands, mesh| {
            spawn_mesh_child(commands, mesh, (marker, range.clone()), tree, paint, false)
        });
        sync_leaf_child(commands, meshes, &mut level.leaves, built.leaves, |commands, mesh| {
            spawn_mesh_child(commands, mesh, (marker, range.clone()), tree, paint, true)
        });
    }
}

/// Overwrite the asset behind `handle` in place; if it was removed externally,
/// allocate a new one and re-point `child`'s `Mesh3d` at it.
pub(crate) fn replace_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    child: Entity,
    handle: &mut Handle<Mesh>,
    mesh: Mesh,
) {
    if let Some(mut slot) = meshes.get_mut(&*handle) {
        *slot = mesh;
        return;
    }
    *handle = meshes.add(mesh);
    commands.entity(child).insert(Mesh3d(handle.clone()));
}

/// Bring a leaf child (the leaves, or a flowering tree's flowers) in sync with `mesh`: replace its
/// mesh, spawn it with `spawn` when the tree first has one, despawn it when the tree has none.
pub(crate) fn sync_leaf_child<'w, 's>(
    commands: &mut Commands<'w, 's>,
    meshes: &mut Assets<Mesh>,
    slot: &mut Option<(Entity, Handle<Mesh>)>,
    mesh: Option<Mesh>,
    spawn: impl FnOnce(&mut Commands<'w, 's>, Handle<Mesh>) -> Entity,
) {
    match (slot.as_mut(), mesh) {
        (Some((child, handle)), Some(mesh)) => replace_mesh(commands, meshes, *child, handle, mesh),
        (None, Some(mesh)) => {
            let handle = meshes.add(mesh);
            *slot = Some((spawn(commands, handle.clone()), handle));
        }
        (Some((child, _)), None) => {
            commands.entity(*child).despawn();
            *slot = None;
        }
        (None, None) => {}
    }
}

/// Spawn a CPU-built tree's child (`bundle` tells which, with what else it starts with) with
/// `mesh` and the leaf paint (`leaf`) or the wood's.
fn spawn_mesh_child(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    bundle: impl Bundle,
    tree: Entity,
    paint: &Paint,
    leaf: bool,
) -> Entity {
    let mut child = commands.spawn((Mesh3d(mesh), bundle, ChildOf(tree)));
    paint_child(&mut child, paint, leaf);
    child.id()
}

/// Draw a flowering tree's flowers ([`TreeFlowerMesh`]) only while [`FloraSeason::blossom`] is
/// above 0. Out of season they are not drawn at all, so they cost nothing; in season the shaders
/// open them one by one.
#[allow(clippy::type_complexity)]
pub(crate) fn show_flowers(
    season: Res<FloraSeason>,
    mut flowers: ParamSet<(
        Query<&mut Visibility, With<TreeFlowerMesh>>,
        Query<&mut Visibility, Added<TreeFlowerMesh>>,
    )>,
) {
    let want = if season.blossom > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    if season.is_changed() {
        for mut visibility in &mut flowers.p0() {
            visibility.set_if_neq(want);
        }
    } else {
        for mut visibility in &mut flowers.p1() {
            visibility.set_if_neq(want);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;
    use crate::shading::{GustFront, TreeShake, wind_vectors};
    use bevy::math::Vec2;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::render::storage::ShaderBuffer;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        app
    }

    #[test]
    fn a_tree_shape_follows_its_tree_and_maturity_is_announced_once() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        let params = TreePreset::Oak.params();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(params.clone()), TreeAge(2.0), TreeShape::with_limb_radius(0.04)))
            .id();
        let without = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(2.0))).id();
        app.update();
        let young = app.world().get::<TreeShape>(tree).expect("kept").clone();
        assert!(app.world().get::<TreeShape>(without).is_none(), "the shape is opt-in");
        // It is the shape of the aged tree with the component's limb radius.
        let expected = crate::pipeline::build_tree(&params, 2.0);
        assert_eq!(young, TreeShape::from_skeleton(&expected.skeleton, &params, 0.04));

        // Growing to maturity grows the shape and sends `TreeMature` once.
        app.world_mut().entity_mut(tree).insert(Growing { rate: 40.0 });
        let mut cursor = app.world().resource::<Messages<TreeMature>>().get_cursor_current();
        let mut announced = 0;
        for _ in 0..30 {
            app.update();
            announced += cursor.read(app.world().resource::<Messages<TreeMature>>()).count();
        }
        assert_eq!(announced, 1);
        let mature = app.world().get::<TreeShape>(tree).unwrap();
        assert!(mature.height() > young.height() && mature.limbs.len() > young.limbs.len());
        let full = crate::pipeline::build_tree(&params, f32::INFINITY);
        assert_eq!(*mature, TreeShape::from_skeleton(&full.skeleton, &params, 0.04));
    }

    #[test]
    fn spawns_children_and_drops_foliage_at_age_zero() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY)))
            .id();
        app.update();

        {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).expect("TreeRender inserted");
            assert!(render.foliage.is_some());
            assert_eq!(world.get::<Children>(tree).unwrap().len(), 2);
            let meshes = world.resource::<Assets<Mesh>>();
            let trunk = meshes.get(render.trunk_mesh.as_ref().unwrap()).unwrap();
            assert!(trunk.count_vertices() > 0);
            assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Inherited));
            assert!(render.mature_at.is_finite());
        }

        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 0.0;
        app.update();
        {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).unwrap();
            assert!(render.foliage.is_none());
            assert_eq!(world.get::<Children>(tree).unwrap().len(), 1);
            assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Hidden));
        }

        // Back to mature: foliage child returns.
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = f32::INFINITY;
        app.update();
        assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Inherited));
    }

    #[test]
    fn flowers_are_a_child_of_their_own_drawn_only_in_blossom() {
        let mut app = app();
        let cherry = app.world_mut().spawn((ProceduralTree(TreePreset::Cherry.params()), TreeAge(f32::INFINITY))).id();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY))).id();
        app.update();
        assert_eq!(app.world().get::<TreeRender>(oak).unwrap().flower_entity(), None, "an oak has no flowers");
        let flowers = app.world().get::<TreeRender>(cherry).unwrap().flower_entity().expect("a cherry flowers");
        assert!(app.world().get::<TreeFlowerMesh>(flowers).is_some());
        assert_eq!(app.world().get::<Children>(cherry).unwrap().len(), 3);
        let visibility = |app: &App| *app.world().get::<Visibility>(flowers).unwrap();
        // Summer: not drawn at all.
        assert_eq!(visibility(&app), Visibility::Hidden);

        for (season, shown) in [
            (FloraSeason::BLOSSOM, Visibility::Inherited),
            (FloraSeason { blossom: 0.01, ..FloraSeason::BLOSSOM }, Visibility::Inherited),
            (FloraSeason::SUMMER, Visibility::Hidden),
            (FloraSeason::BLOSSOM, Visibility::Inherited),
        ] {
            *app.world_mut().resource_mut::<FloraSeason>() = season;
            app.update();
            assert_eq!(visibility(&app), shown, "{season:?}");
        }

        // A rebuild keeps the flowers child and its visibility; one that has no leaves drops it,
        // and when the leaves come back out of season their new flowers are hidden from their
        // first frame, though the season did not change.
        app.world_mut().get_mut::<TreeAge>(cherry).unwrap().0 = 30.0;
        app.update();
        assert_eq!(app.world().get::<TreeRender>(cherry).unwrap().flower_entity(), Some(flowers));
        assert_eq!(visibility(&app), Visibility::Inherited);
        *app.world_mut().resource_mut::<FloraSeason>() = FloraSeason::SUMMER;
        app.world_mut().get_mut::<TreeAge>(cherry).unwrap().0 = 0.0;
        app.update();
        assert_eq!(app.world().get::<TreeRender>(cherry).unwrap().flower_entity(), None);
        app.world_mut().get_mut::<TreeAge>(cherry).unwrap().0 = f32::INFINITY;
        app.update();
        let back = app.world().get::<TreeRender>(cherry).unwrap().flower_entity().expect("flowers again");
        assert_eq!(app.world().get::<Visibility>(back), Some(&Visibility::Hidden));
    }

    #[test]
    fn trunk_born_empty_gets_its_mesh_once_it_grows() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(0.0)))
            .id();
        app.update();
        let trunk = {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).unwrap();
            assert!(render.trunk_mesh.is_none());
            let trunk = render.trunk_entity();
            assert_eq!(world.get::<Visibility>(trunk), Some(&Visibility::Hidden));
            assert!(world.get::<Mesh3d>(trunk).is_none());
            trunk
        };

        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 2.0;
        app.update();
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let handle = render.trunk_mesh.as_ref().expect("mesh created once the trunk exists");
        assert!(world.resource::<Assets<Mesh>>().get(handle).unwrap().count_vertices() > 0);
        assert_eq!(world.get::<Mesh3d>(trunk).map(|m| &m.0), Some(handle));
        assert_eq!(world.get::<Visibility>(trunk), Some(&Visibility::Inherited));
    }

    #[test]
    fn growing_stops_at_maturity() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((
                ProceduralTree(TreePreset::Oak.params()),
                TreeAge(0.0),
                Growing { rate: 1.0e7 },
            ))
            .id();
        app.update(); // first build at age 0 (no TreeRender yet when grow runs)
        app.update(); // grows to mature_at
        let age = app.world().get::<TreeAge>(tree).unwrap().0;
        let mature_at = app.world().get::<TreeRender>(tree).unwrap().mature_at;
        assert_eq!(age, mature_at);
        app.update();
        assert_eq!(app.world().get::<TreeAge>(tree).unwrap().0, mature_at);
    }

    /// Wood vertex positions the plugin uploaded for `tree`.
    fn uploaded_trunk(app: &App, tree: Entity) -> Vec<[f32; 3]> {
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let mesh = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec()
    }

    fn expected_trunk(params: &TreeParams, age: f32) -> Vec<[f32; 3]> {
        crate::pipeline::build_tree(params, age).trunk.positions.iter().map(|p| p.to_array()).collect()
    }

    #[test]
    fn growing_trees_reuse_their_skeleton_until_the_parameters_change() {
        // The full skeleton is kept between age-only rebuilds; the meshes must
        // still be exactly what `build_tree` makes, and a parameter change
        // must regenerate it rather than age the old tree.
        let mut app = app();
        let mut params = TreePreset::Oak.params();
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(2.0))).id();
        app.update();
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 5.0;
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 5.0));

        params.seed = 7;
        params.trunk.height *= 1.2;
        app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0 = params.clone();
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 5.0));
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 6.0;
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 6.0));
    }

    #[test]
    fn a_tree_built_in_the_background_shows_only_its_latest_parameters_and_reaches_its_age() {
        let mut app = app();
        let mut params = TreePreset::Oak.params();
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(3.0), BuildInBackground)).id();
        // The frame it is spawned only starts the build: nothing is drawn yet.
        app.update();
        assert!(app.world().get::<TreeRender>(tree).is_none());
        // A new seed while it is built replaces the build, and an age set while the next one runs
        // is caught up once it is done.
        params.seed = 7;
        app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0 = params.clone();
        app.update();
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 6.0;
        let mut cursor = app.world().resource::<Messages<TreeGenerated>>().get_cursor_current();
        let mut generated = 0;
        for _ in 0..3000 {
            app.update();
            generated += cursor.read(app.world().resource::<Messages<TreeGenerated>>()).count();
            // Whatever it shows is the new seed, at the age it says, exactly as built in the frame.
            if let Some(age) = app.world().get::<TreeRender>(tree).map(|render| render.age) {
                assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, age), "age {age}");
                if age == 6.0 {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(app.world().get::<TreeRender>(tree).map(|render| render.age), Some(6.0));
        assert!((1..=2).contains(&generated), "{generated} builds applied");

        // Despawned while it is built: the task is dropped with it.
        let gone = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(3.0), BuildInBackground)).id();
        app.update();
        app.world_mut().despawn(gone);
        for _ in 0..20 {
            app.update();
        }
    }

    #[test]
    fn many_trees_built_in_one_frame_match_the_serial_build() {
        // More trees than one batch, so the scope runs more than once; a changed tree and an
        // aged tree go through the regenerate and kept-skeleton paths.
        let mut app = app();
        let mut specs = Vec::new();
        for i in 0..40 {
            let mut params = TreePreset::ALL[i % TreePreset::ALL.len()].params();
            params.seed = i as i32;
            let age = 1.0 + i as f32 * 0.4;
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(age))).id();
            specs.push((tree, params, age));
        }
        app.update();
        let check = |app: &App, specs: &[(Entity, TreeParams, f32)]| {
            for (tree, params, age) in specs {
                let want = crate::pipeline::build_tree(params, *age);
                let world = app.world();
                let render = world.get::<TreeRender>(*tree).unwrap();
                let meshes = world.resource::<Assets<Mesh>>();
                // Every attribute and the indices, not just the positions.
                let same = |handle: &Handle<Mesh>, data: crate::mesh::MeshData| {
                    let got = meshes.get(handle).unwrap();
                    let want = data.into_bevy_mesh();
                    assert_eq!(got.attributes().count(), want.attributes().count());
                    for (attribute, values) in want.attributes() {
                        assert_eq!(got.attribute(attribute.id), Some(values), "{}", attribute.name);
                    }
                    assert_eq!(got.indices(), want.indices());
                };
                match &render.trunk_mesh {
                    Some(handle) => same(handle, want.trunk),
                    None => assert!(want.trunk.is_empty()),
                }
                match &render.foliage {
                    Some((_, handle)) => same(handle, want.foliage),
                    None => assert!(want.foliage.is_empty()),
                }
                assert_eq!(render.mature_at, want.mature_at);
            }
        };
        check(&app, &specs);

        specs[3].1.trunk.height *= 1.3;
        specs[3].1.seed = 99;
        app.world_mut().get_mut::<ProceduralTree>(specs[3].0).unwrap().0 = specs[3].1.clone();
        specs[5].2 += 2.0;
        app.world_mut().get_mut::<TreeAge>(specs[5].0).unwrap().0 = specs[5].2;
        app.update();
        check(&app, &specs);
    }

    #[test]
    fn growing_leaves_materials_alone_until_a_colour_changes() {
        // A modified material is re-extracted and re-prepared by the renderer;
        // a growing tree used to mark both of its materials modified every frame.
        let modified = |app: &App| {
            app.world()
                .resource::<Messages<bevy::asset::AssetEvent<StandardMaterial>>>()
                .iter_current_update_messages()
                .filter(|e| matches!(e, bevy::asset::AssetEvent::Modified { .. }))
                .count()
        };
        let mut app = app();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(2.0))).id();
        app.update();
        for age in [3.0, 4.0] {
            app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
            app.update();
            assert_eq!(modified(&app), 0, "age {age}");
        }
        let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
        app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0.trunk_color = red;
        app.update();
        assert_eq!(modified(&app), 1);
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let Paint::Standard { trunk, .. } = &render.paint else { panic!("a plain tree has plain materials") };
        assert_eq!(world.resource::<Assets<StandardMaterial>>().get(trunk).unwrap().base_color, red);
    }

    /// The flora materials of one tree drawn with `FloraMaterial`: `(wood, leaves)`.
    fn flora_pair(app: &App, tree: Entity) -> (Handle<FloraMaterial>, Handle<FloraMaterial>) {
        let render = app.world().get::<TreeRender>(tree).unwrap();
        let Paint::Flora { trunk, foliage } = &render.paint else { panic!("expected flora materials") };
        (trunk.clone(), foliage.clone())
    }

    /// Run one update and return the ids of the flora materials it marked modified.
    fn update_and_modified(app: &mut App) -> Vec<bevy::asset::AssetId<FloraMaterial>> {
        let mut cursor = app
            .world()
            .resource::<Messages<bevy::asset::AssetEvent<FloraMaterial>>>()
            .get_cursor_current();
        app.update();
        let messages = app.world().resource::<Messages<bevy::asset::AssetEvent<FloraMaterial>>>();
        cursor
            .read(messages)
            .filter_map(|e| match e {
                bevy::asset::AssetEvent::Modified { id } => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn flora_app() -> App {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app
    }

    fn spawn_oak(app: &mut App, edit: impl FnOnce(&mut TreeParams)) -> Entity {
        let mut params = TreePreset::Oak.params();
        edit(&mut params);
        app.world_mut().spawn((ProceduralTree(params), TreeAge(f32::INFINITY))).id()
    }

    fn material_of(app: &App, child: Entity) -> Handle<FloraMaterial> {
        app.world().get::<MeshMaterial3d<FloraMaterial>>(child).unwrap().0.clone()
    }

    #[test]
    fn trees_use_flora_materials_where_the_renderer_registered_them() {
        let mut app = flora_app();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(2.0))).id();
        app.update();
        let (trunk, foliage) = flora_pair(&app, tree);
        {
            let assets = app.world().resource::<Assets<FloraMaterial>>();
            assert!(assets.get(&foliage).unwrap().extension.leaf, "the leaves are drawn as leaves");
            assert!(!assets.get(&trunk).unwrap().extension.leaf, "the wood is not");
        }
        for age in [3.0, 4.0, 5.0] {
            app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
            assert!(update_and_modified(&mut app).is_empty(), "age {age} rewrote a material");
        }
        let render = app.world().get::<TreeRender>(tree).unwrap();
        assert_eq!(material_of(&app, render.trunk), trunk);
    }

    #[test]
    fn equal_looks_share_materials() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        let b = spawn_oak(&mut app, |p| p.seed = 99);
        let c = spawn_oak(&mut app, |p| p.foliage.color = bevy::color::Color::srgb(1.0, 0.0, 0.0));
        let d = spawn_oak(&mut app, |p| p.foliage.shading.translucency = 0.2);
        let e = spawn_oak(&mut app, |p| p.wind.sway = 2.0);
        app.update();
        let (aw, al) = flora_pair(&app, a);
        let (bw, bl) = flora_pair(&app, b);
        assert_eq!((&aw, &al), (&bw, &bl));
        let (cw, cl) = flora_pair(&app, c);
        assert_eq!(aw, cw);
        assert_ne!(al, cl);
        let (dw, dl) = flora_pair(&app, d);
        assert_eq!(aw, dw);
        assert_ne!(al, dl);
        let (ew, el) = flora_pair(&app, e);
        assert_ne!(aw, ew);
        assert_ne!(al, el);
        let render = app.world().get::<TreeRender>(b).unwrap();
        assert_eq!(material_of(&app, render.trunk), aw);
        assert_eq!(material_of(&app, render.foliage.as_ref().unwrap().0), al);
    }

    #[test]
    fn changing_one_trees_look_leaves_the_sharing_tree_alone() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        let b = spawn_oak(&mut app, |p| p.seed = 5);
        app.update();
        let (aw, al) = flora_pair(&app, a);
        app.world_mut().get_mut::<ProceduralTree>(b).unwrap().0.foliage.shading.translucency = 0.2;
        assert!(update_and_modified(&mut app).is_empty(), "the shared asset is not modified");
        assert_eq!(flora_pair(&app, a), (aw.clone(), al.clone()));
        let (bw, bl) = flora_pair(&app, b);
        assert_eq!(bw, aw);
        assert_ne!(bl, al);
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert_eq!(assets.get(&bl).unwrap().extension.flora.leaf.x, 0.2);
        assert_ne!(assets.get(&al).unwrap().extension.flora.leaf.x, 0.2);
        let render = app.world().get::<TreeRender>(b).unwrap();
        assert_eq!(material_of(&app, render.foliage.as_ref().unwrap().0), bl);
    }

    #[test]
    fn a_look_nobody_uses_any_more_is_made_again() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        app.update();
        app.world_mut().entity_mut(a).despawn();
        // Dropped handles are freed over a couple of updates.
        for _ in 0..3 {
            app.update();
        }
        let b = spawn_oak(&mut app, |p| p.seed = 3);
        app.update();
        let (bw, bl) = flora_pair(&app, b);
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert!(assets.get(&bw).is_some() && assets.get(&bl).is_some());
    }

    /// The weather's slots of the shared buffer (the fire's follow them).
    fn wind_buffer(app: &App) -> Vec<[f32; 4]> {
        let handle = &app.world().resource::<FloraWindBuffer>().0;
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        buffers.get(handle).unwrap().cast_slice::<[f32; 4]>().unwrap()[..crate::shading::WIND_VECTORS].to_vec()
    }

    /// Run one update and count the `Modified` events of shader buffers it produced.
    fn modified_buffers(app: &mut App) -> usize {
        let mut cursor = app
            .world()
            .resource::<Messages<bevy::asset::AssetEvent<ShaderBuffer>>>()
            .get_cursor_current();
        app.update();
        let messages = app.world().resource::<Messages<bevy::asset::AssetEvent<ShaderBuffer>>>();
        cursor.read(messages).filter(|e| matches!(e, bevy::asset::AssetEvent::Modified { .. })).count()
    }

    #[test]
    fn changing_the_wind_modifies_no_material() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY)));
        for seed in 1..4 {
            spawn_oak(&mut app, |p| p.seed = seed);
        }
        app.update();
        assert!(update_and_modified(&mut app).is_empty());
        let (season, still) = (FloraSeason::default(), TreeShakes::default());
        assert_eq!(wind_buffer(&app), wind_vectors(&FloraWind::default(), &season, &still).map(|v| v.to_array()));

        app.world_mut().resource_mut::<FloraWind>().strength = 0.9;
        assert!(update_and_modified(&mut app).is_empty(), "a new strength modified a material");
        let wind = *app.world().resource::<FloraWind>();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind, &season, &still).map(|v| v.to_array()));
        assert_eq!(wind_buffer(&app)[0][2], 0.9);

        // A gust front is wind too.
        app.world_mut().resource_mut::<FloraWind>().launch_gust(GustFront::default(), &Time::default());
        assert!(update_and_modified(&mut app).is_empty(), "a launched gust modified a material");
        let wind = *app.world().resource::<FloraWind>();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind, &season, &still).map(|v| v.to_array()));
        assert!(wind_buffer(&app)[4][2] > 0.0);

        // Touching the resource without changing the wind writes nothing.
        app.world_mut().resource_mut::<FloraWind>().set_changed();
        assert_eq!(modified_buffers(&mut app), 0, "an unchanged wind wrote the buffer");
        app.world_mut().resource_mut::<FloraWind>().strength = 0.2;
        assert_eq!(modified_buffers(&mut app), 1, "a changed wind writes the buffer once");

        // The season shares the buffer: changing it rewrites the buffer and no material.
        *app.world_mut().resource_mut::<FloraSeason>() = FloraSeason::AUTUMN;
        assert!(update_and_modified(&mut app).is_empty(), "the season modified a material");
        let wind = *app.world().resource::<FloraWind>();
        let still = TreeShakes::default();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind, &FloraSeason::AUTUMN, &still).map(|v| v.to_array()));
        assert_eq!(wind_buffer(&app)[5][0], 1.0);

        // So do the shakes.
        // A long one, on the app's own clock: a debug build may spend seconds in a frame here.
        let shake = TreeShake { at: Vec2::new(2.0, 1.0), duration: 1.0e4, ..TreeShake::default() };
        let time = *app.world().resource::<Time>();
        app.world_mut().resource_mut::<TreeShakes>().shake(shake, &time);
        assert!(update_and_modified(&mut app).is_empty(), "a shake modified a material");
        let shakes = app.world().resource::<TreeShakes>().clone();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind, &FloraSeason::AUTUMN, &shakes).map(|v| v.to_array()));
        assert_eq!(wind_buffer(&app)[7][0], 2.0);

        // And the rain: one value of the buffer, as wet as it gets at 1, and no material.
        app.world_mut().insert_resource(FloraWetness(0.7));
        assert!(update_and_modified(&mut app).is_empty(), "the rain modified a material");
        assert_eq!(wind_buffer(&app)[2][2], 0.7);
        app.world_mut().insert_resource(FloraWetness(3.0));
        app.update();
        assert_eq!(wind_buffer(&app)[2][2], 1.0);
    }

    #[test]
    fn a_shake_that_has_died_away_is_dropped_and_the_shaders_skip_shakes_again() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        let time = *app.world().resource::<Time>();
        let knock = TreeShake { duration: 0.25, ..TreeShake::default() };
        app.world_mut().resource_mut::<TreeShakes>().shake(knock, &time);
        app.update();
        assert_eq!(app.world().resource::<TreeShakes>().iter().count(), 1, "a shake under way was dropped");
        assert!(wind_buffer(&app)[6][0] >= time.elapsed_secs_wrapped(), "the header says no shake is under way");
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<TreeShakes>().iter().count(), 0, "a finished shake was kept");
        assert_eq!(wind_buffer(&app)[6][0], -1.0, "the shaders still look at the shakes");
    }

    #[test]
    fn a_material_made_after_a_wind_change_sees_the_current_wind() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app.world_mut().resource_mut::<FloraWind>().strength = 0.77;
        app.update();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY))).id();
        app.update();
        let (_, foliage) = flora_pair(&app, tree);
        // The material holds the shared buffer, not a copy of the wind.
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert_eq!(assets.get(&foliage).unwrap().extension.wind, app.world().resource::<FloraWindBuffer>().0);
        assert_eq!(wind_buffer(&app)[0][2], 0.77);
    }

    #[test]
    fn a_fire_grows_the_shared_buffer_and_arms_the_materials_until_it_goes() {
        use crate::fire::{FIRE_SLOT, FloraFire};
        use bevy::math::Vec3;
        use crate::shading::WIND_VECTORS;
        let shared = |app: &App| {
            let handle = &app.world().resource::<FloraWindBuffer>().0;
            let buffers = app.world().resource::<Assets<ShaderBuffer>>();
            buffers.get(handle).unwrap().cast_slice::<[f32; 4]>().unwrap().to_vec()
        };
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY))).id();
        app.update();
        let (wood, foliage) = flora_pair(&app, tree);
        let armed = |app: &App| {
            let assets = app.world().resource::<Assets<FloraMaterial>>();
            (assets.get(&wood).unwrap().extension.fire, assets.get(&foliage).unwrap().extension.fire)
        };
        let both = |modified: &[bevy::asset::AssetId<FloraMaterial>]| {
            modified.contains(&wood.id()) && modified.contains(&foliage.id())
        };
        assert_eq!(shared(&app).len(), FIRE_SLOT + 1);
        assert_eq!(armed(&app), (false, false));

        // A fire: its grid follows the weather, which stays; nothing burns yet, so the shaders stop
        // at the header. The materials carry the fire, and are prepared again the frame after the
        // buffer grew, to bind the new one; then nothing more.
        let weather = wind_buffer(&app);
        app.insert_resource(FloraFire::with_cells(Vec2::ZERO, 16.0, 8));
        assert!(both(&update_and_modified(&mut app)), "arming did not re-prepare the materials");
        assert_eq!(armed(&app), (true, true));
        let buffer = shared(&app);
        assert_eq!(buffer.len(), FIRE_SLOT + 2 + 32);
        assert_eq!(&buffer[..WIND_VECTORS], &weather[..]);
        assert_eq!(buffer[FIRE_SLOT][3], 0.0, "nothing burns, yet the shaders read the cells");
        assert!(both(&update_and_modified(&mut app)), "the materials were not bound to the grown buffer");
        assert!(update_and_modified(&mut app).is_empty(), "the materials are prepared again and again");

        // A burnt patch is written in place: the grid, the rectangle around the touched cell (grown
        // by a cell), the cell's char; no material is touched.
        app.world_mut().resource_mut::<FloraFire>().scorch(Vec3::new(1.0, 0.0, 1.0), 0.1, 0.5);
        assert!(update_and_modified(&mut app).is_empty(), "the fire's cells modified a material");
        let buffer = shared(&app);
        assert_eq!(buffer[FIRE_SLOT], [-8.0, -8.0, 0.5, 8.0]);
        assert_eq!(buffer[FIRE_SLOT + 1], [-2.0, -2.0, 4.0, 4.0]);
        // Cell (4, 4) is the 36th, the first of the 18th pair.
        assert_eq!(buffer[FIRE_SLOT + 2 + 18], [0.5, 0.0, 0.0, 0.0]);

        // Gone: the buffer shrinks back and the materials carry no fire.
        app.world_mut().remove_resource::<FloraFire>();
        app.update();
        assert_eq!(shared(&app).len(), FIRE_SLOT + 1);
        assert_eq!(armed(&app), (false, false));
    }

    /// Check that `tree` draws `build_lods`' levels for `params` at `age` in `lod`'s bands: the
    /// full tree's children up close, the coarse ones' wood and leaves (flowers included) beyond.
    fn assert_levels(app: &App, tree: Entity, params: &TreeParams, age: f32, lod: &LodSettings) {
        use crate::lod::build_lods;
        let world = app.world();
        let margins = |child: Entity| {
            let range = world.get::<VisibilityRange>(child).unwrap_or_else(|| panic!("{child} has no band"));
            (range.start_margin.clone(), range.end_margin.clone())
        };
        let band_of = |range: VisibilityRange| (range.start_margin, range.end_margin);
        let mesh = |child: Entity| {
            let handle = &world.get::<Mesh3d>(child).unwrap().0;
            world.resource::<Assets<Mesh>>().get(handle).unwrap().attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec()
        };
        let render = world.get::<TreeRender>(tree).unwrap();
        assert_eq!(render.age, age);
        let level0 = [Some(render.trunk), render.foliage.as_ref().map(|c| c.0), render.flowers.as_ref().map(|c| c.0)];
        for child in level0.into_iter().flatten() {
            assert_eq!(margins(child), band_of(level_band(0, lod.distances).1));
        }
        let expected = build_lods(params, age, lod);
        let (settings, levels) = render.lod.as_ref().expect("no coarse levels");
        assert_eq!(settings, lod);
        for (index, level) in levels.iter().enumerate() {
            let want = &expected[index + 1];
            let (wood, _) = level.wood.as_ref().expect("no coarse wood");
            let (leaves, _) = level.leaves.as_ref().expect("no coarse leaves");
            for child in [*wood, *leaves] {
                assert_eq!(world.get::<TreeLodMesh>(child), Some(&TreeLodMesh(index as u8 + 1)));
                assert_eq!(margins(child), band_of(level_band(index + 1, lod.distances).1));
            }
            let wood_positions: Vec<[f32; 3]> = want.trunk.positions.iter().map(|p| p.to_array()).collect();
            assert_eq!(mesh(*wood), wood_positions, "level {} wood", index + 1);
            assert_eq!(mesh(*leaves).len(), want.foliage.positions.len() + want.flowers.positions.len());
        }
        // Trunk, leaves and flowers, then a wood and a leaf child per coarse level.
        assert_eq!(world.get::<Children>(tree).unwrap().len(), 7);
    }

    #[test]
    fn a_tree_with_levels_of_detail_draws_its_coarse_levels_beyond_the_full_tree() {
        use crate::felling::FellTree;
        let mut app = app();
        let params = TreePreset::Cherry.params();
        let mut lod = LodSettings::default();
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(9.0), TreeLod(lod), Transform::default())).id();
        app.update();
        assert_levels(&app, tree, &params, 9.0, &lod);
        // A new age rebuilds every level from the kept skeleton.
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = f32::INFINITY;
        app.update();
        assert_levels(&app, tree, &params, f32::INFINITY, &lod);
        // New distances move every child's band.
        lod.distances.lod1 = 12.0;
        lod.distances.lod2 = 40.0;
        app.world_mut().get_mut::<TreeLod>(tree).unwrap().0 = lod;
        app.update();
        assert_levels(&app, tree, &params, f32::INFINITY, &lod);

        // Without its `TreeLod` the tree is drawn in full at every distance; with it back, the
        // levels return.
        let coarse: Vec<Entity> = {
            let render = app.world().get::<TreeRender>(tree).unwrap();
            render.lod.as_ref().unwrap().1.iter().flat_map(CoarseLevel::children).collect()
        };
        app.world_mut().entity_mut(tree).remove::<TreeLod>();
        app.update();
        let full_only = |app: &App| {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).unwrap();
            assert!(render.lod.is_none());
            assert!(coarse.iter().all(|child| world.get_entity(*child).is_err()), "a coarse level is left");
            let level0 = [Some(render.trunk), render.foliage.as_ref().map(|c| c.0), render.flowers.as_ref().map(|c| c.0)];
            for child in level0.into_iter().flatten() {
                assert!(world.get::<VisibilityRange>(child).is_none(), "{child} is still banded");
            }
        };
        full_only(&app);
        assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 3);
        app.world_mut().entity_mut(tree).insert(TreeLod(lod));
        app.update();
        assert_levels(&app, tree, &params, f32::INFINITY, &lod);

        // Felled, it falls in full: a coarse level standing in its place far away would not.
        let coarse: Vec<Entity> = {
            let render = app.world().get::<TreeRender>(tree).unwrap();
            render.lod.as_ref().unwrap().1.iter().flat_map(CoarseLevel::children).collect()
        };
        app.world_mut().entity_mut(tree).insert(FellTree::default());
        app.update();
        let world = app.world();
        assert!(coarse.iter().all(|child| world.get_entity(*child).is_err()), "a coarse level is left");
        let render = world.get::<TreeRender>(tree).unwrap();
        for child in std::iter::once(render.trunk).chain(render.foliage.as_ref().map(|c| c.0)) {
            assert!(world.get::<VisibilityRange>(child).is_none(), "{child} is still banded");
        }

        // A tree built in the background gets its levels with its meshes.
        let background = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(9.0), TreeLod(lod), BuildInBackground)).id();
        for _ in 0..3000 {
            app.update();
            if app.world().get::<TreeRender>(background).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_levels(&app, background, &params, 9.0, &lod);
    }
}
