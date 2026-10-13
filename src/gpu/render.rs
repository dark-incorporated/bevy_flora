//! The systems that put a [`GpuGrowth`] tree on screen.

use bevy::asset::{Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::MeshMaterial3d;
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;

use super::material::{GrowthMaterial, growth_material};
use super::mesh::build_growth_data;
use super::state::GrowthState;
use super::{GpuGrowth, GpuGrowthSupport, GrowthMaterialAdded, grows_on_gpu};
use crate::background::{BuildInBackground, PendingBuild};
use crate::felling::Felled;
use crate::params::TreeParams;
use crate::plugin::{
    CoarseLevel, FLOWERS_SPAWN, Paint, ProceduralTree, TreeAge, TreeFoliageMesh, TreeGenerated, TreeRender,
    TreeTrunkMesh, build_in_batches, replace_mesh, sync_leaf_child,
};
use crate::shading::{FloraWindBuffer, Surface, TextureLibrary};
use crate::skeleton::{Skeleton, generate_skeleton};

/// What a tree that grows on the GPU draws with.
pub(crate) struct GrowthPaint {
    pub trunk: Handle<GrowthMaterial>,
    pub foliage: Handle<GrowthMaterial>,
    /// The per-node state both materials bind.
    pub nodes: Handle<ShaderBuffer>,
    /// What was last written to `nodes`.
    pub state: GrowthState,
}

/// Decide once the renderer exists whether trees can grow on this device: it
/// must hold the per-node state (a storage buffer readable in the vertex stage)
/// and the plugin must have been added after the renderer's, which is what
/// registers [`GrowthMaterial`] with it.
pub(crate) fn detect_gpu_growth_support(
    device: Option<Res<RenderDevice>>,
    material: Option<Res<GrowthMaterialAdded>>,
    mut support: ResMut<GpuGrowthSupport>,
    mut checked: Local<bool>,
) {
    if *checked {
        return;
    }
    let Some(device) = device else { return };
    *checked = true;
    let storage = !bevy::render::storage_buffers_are_unsupported(&device.limits());
    if !storage {
        bevy::log::warn!(
            "this device has no storage buffers in the vertex stage (WebGL2?); trees with `GpuGrowth` grow on the CPU instead"
        );
    } else if material.is_none() {
        bevy::log::warn!(
            "`FloraPlugin` was added before the renderer's plugins, so `GpuGrowth` is off; add it after `DefaultPlugins`"
        );
    }
    let supported = storage && material.is_some();
    if support.0 != supported {
        support.0 = supported;
    }
}

/// Take a tree out of the mode it is no longer in: when its [`GpuGrowth`]
/// appears or goes, when GPU growth becomes (un)available, or when a change of its parameters
/// makes it (not) [`growable`](crate::TreeParams::gpu_growable), drop its render
/// state and children so the right system rebuilds it.
pub(crate) fn sync_growth_mode(
    mut commands: Commands,
    support: Res<GpuGrowthSupport>,
    added: Query<(), Added<GpuGrowth>>,
    mut removed: RemovedComponents<GpuGrowth>,
    changed: Query<(), (Changed<ProceduralTree>, With<TreeRender>)>,
    trees: Query<(Entity, &TreeRender, &ProceduralTree, Has<GpuGrowth>), Without<Felled>>,
) {
    let removed_any = removed.read().count() > 0;
    if !(support.is_changed() || removed_any || !added.is_empty() || !changed.is_empty()) {
        return;
    }
    for (entity, render, tree, gpu) in &trees {
        let wanted = grows_on_gpu(gpu, &support, &tree.0);
        if wanted != matches!(render.paint, Paint::Growth(_)) {
            commands.entity(render.trunk).despawn();
            let coarse = render.lod.iter().flat_map(|(_, levels)| levels.iter().flat_map(CoarseLevel::children));
            let level0 = render.foliage.iter().chain(&render.flowers).map(|child| child.0);
            for child in level0.chain(coarse) {
                commands.entity(child).despawn();
            }
            commands.entity(entity).remove::<TreeRender>();
        }
    }
}

/// Write `state` into the buffer behind `handle`, in place when the size is
/// unchanged (a plain write to the existing GPU buffer) and by replacing the
/// buffer when it is not.
fn write_state(buffers: &mut Assets<ShaderBuffer>, handle: &Handle<ShaderBuffer>, state: &GrowthState) {
    let Some(mut buffer) = buffers.get_mut(handle) else { return };
    if let Some(slice) = buffer.cast_slice_mut::<[f32; 4]>()
        && slice.len() == state.vec4_len()
    {
        state.encode(slice);
        return;
    }
    *buffer = state_buffer(state);
}

fn state_buffer(state: &GrowthState) -> ShaderBuffer {
    let mut data = vec![[0.0f32; 4]; state.vec4_len()];
    state.encode(&mut data);
    ShaderBuffer::from(data)
}

/// Bring a material's base colour, flora uniform and detail textures up to date, marking it
/// modified only when one differs.
fn sync_growth(materials: &mut Assets<GrowthMaterial>, handle: &Handle<GrowthMaterial>, surface: &Surface) {
    let (detail_color, detail_normal) = surface.detail();
    if let Some(mut material) = materials.get_mut(handle)
        && (material.base.base_color != surface.color
            || material.base.unlit != surface.unlit
            || material.extension.flora != surface.uniform
            || material.extension.detail_color != detail_color
            || material.extension.detail_normal != detail_normal
            || material.extension.venation != surface.venation
            || material.extension.style != surface.style)
    {
        material.base.base_color = surface.color;
        material.base.unlit = surface.unlit;
        material.extension.flora = surface.uniform;
        material.extension.detail_color = detail_color;
        material.extension.detail_normal = detail_normal;
        material.extension.venation = surface.venation.clone();
        material.extension.style = surface.style;
    }
}

/// Spawn a GPU-grown tree's leaf child (`marker` tells which: the leaves or the flowers) with
/// `mesh` and the leaf material.
fn spawn_growth_leaf(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: &Handle<GrowthMaterial>,
    marker: impl Bundle,
    tree: Entity,
) -> Entity {
    commands.spawn((Mesh3d(mesh), MeshMaterial3d(material.clone()), marker, ChildOf(tree))).id()
}

/// Build (or bring up to date) the render state of trees that grow on the GPU. A tree with
/// [`BuildInBackground`] has its mature meshes built on the async compute pool instead, and put
/// in the frame the build is done.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn rebuild_gpu_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrowthMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    support: Res<GpuGrowthSupport>,
    wind: Res<FloraWindBuffer>,
    mut textures: TextureLibrary,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (Entity, Ref<ProceduralTree>, Ref<TreeAge>, Option<&mut TreeRender>, Has<GpuGrowth>, Has<BuildInBackground>),
        Without<Felled>,
    >,
    mut pending: Query<(Entity, &mut PendingBuild<GrowthBuilt>)>,
) {
    // The background builds that are done.
    let mut finished = Vec::new();
    for (entity, mut build) in &mut pending {
        if let Some(out) = build.poll() {
            commands.entity(entity).remove::<PendingBuild<GrowthBuilt>>();
            finished.push(out);
        }
    }

    // Trees that only aged are updated here; the others are built on all cores below.
    let mut jobs = Vec::new();
    for (entity, tree, age, mut render, gpu, background) in &mut trees {
        if !grows_on_gpu(gpu, &support, &tree.0) {
            continue;
        }
        let building = pending.contains(entity);
        if !(tree.is_changed() || age.is_changed() || (render.is_none() && !building)) {
            continue;
        }
        // While a build is under way the tree keeps its state (the new meshes take the age they
        // find); only a change of parameters replaces the build.
        if building && !tree.is_changed() {
            continue;
        }
        match render.as_deref_mut() {
            // Only the age changed: age the kept skeleton and write the node state.
            Some(render) if !tree.is_changed() => {
                let TreeRender {
                    full,
                    paint: Paint::Growth(paint),
                    ..
                } = render
                else {
                    continue;
                };
                paint.state.update(full, &tree.0, age.0);
                write_state(&mut buffers, &paint.nodes, &paint.state);
                generated.write(TreeGenerated {
                    entity,
                    height: tree.0.trunk.height * paint.state.trunk_progress,
                    radius: tree.0.trunk.radius,
                });
                continue;
            }
            // The parameters changed: rebuild the mature meshes in place.
            Some(render) if !matches!(render.paint, Paint::Growth(_)) => continue,
            _ => {}
        }
        let job = GrowthJob { entity, params: tree.0.clone(), background };
        if background {
            commands.entity(entity).insert(PendingBuild::spawn(move || build_growth(job)));
        } else {
            jobs.push(job);
        }
    }

    let mut apply = |out: GrowthBuilt| {
        let GrowthBuilt { entity, background, full, state, wood, leaves, flowers } = out;
        let Ok((_, tree, age, mut render, gpu, _)) = trees.get_mut(entity) else { return };
        // A background build is out of date if the parameters changed again since it started (a
        // new one is under way) or the tree has left the GPU.
        if background && (tree.is_changed() || !grows_on_gpu(gpu, &support, &tree.0)) {
            return;
        }
        let params = &tree.0;
        let age = age.0;
        let progress;
        match render.as_deref_mut() {
            Some(render) => {
                let Paint::Growth(paint) = &mut render.paint else { return };
                paint.state.update(&full, params, age);
                write_state(&mut buffers, &paint.nodes, &paint.state);
                progress = paint.state.trunk_progress;
                render.mature_at = full.mature_at(&params.growth);
                render.age = age;
                render.full = full;
                if let Some(handle) = render.trunk_mesh.as_mut() {
                    replace_mesh(&mut commands, &mut meshes, render.trunk, handle, wood);
                }
                sync_growth(&mut materials, &paint.trunk, &textures.wood(params));
                sync_growth(&mut materials, &paint.foliage, &textures.leaves(params));
                let material = &paint.foliage;
                sync_leaf_child(&mut commands, &mut meshes, &mut render.foliage, leaves, |commands, mesh| {
                    spawn_growth_leaf(commands, mesh, material, TreeFoliageMesh, entity)
                });
                sync_leaf_child(&mut commands, &mut meshes, &mut render.flowers, flowers, |commands, mesh| {
                    spawn_growth_leaf(commands, mesh, material, FLOWERS_SPAWN, entity)
                });
            }
            None => {
                let mut state = state;
                state.update(&full, params, age);
                progress = state.trunk_progress;
                let nodes = buffers.add(state_buffer(&state));

                let trunk_material = materials.add(growth_material(&nodes, &textures.wood(params), &wind));
                let foliage_material = materials.add(growth_material(&nodes, &textures.leaves(params), &wind));
                let trunk_mesh = meshes.add(wood);
                let trunk = commands
                    .spawn((
                        Mesh3d(trunk_mesh.clone()),
                        MeshMaterial3d(trunk_material.clone()),
                        TreeTrunkMesh,
                        ChildOf(entity),
                    ))
                    .id();
                let mut foliage = None;
                sync_leaf_child(&mut commands, &mut meshes, &mut foliage, leaves, |commands, mesh| {
                    spawn_growth_leaf(commands, mesh, &foliage_material, TreeFoliageMesh, entity)
                });
                let mut flowers_child = None;
                sync_leaf_child(&mut commands, &mut meshes, &mut flowers_child, flowers, |commands, mesh| {
                    spawn_growth_leaf(commands, mesh, &foliage_material, FLOWERS_SPAWN, entity)
                });
                commands.entity(entity).insert(TreeRender {
                    trunk,
                    trunk_mesh: Some(trunk_mesh),
                    paint: Paint::Growth(GrowthPaint {
                        trunk: trunk_material,
                        foliage: foliage_material,
                        nodes,
                        state,
                    }),
                    foliage,
                    flowers: flowers_child,
                    mature_at: full.mature_at(&params.growth),
                    age,
                    full,
                    lod: None,
                });
            }
        }

        generated.write(TreeGenerated {
            entity,
            height: params.trunk.height * progress,
            radius: params.trunk.radius,
        });
    };
    for out in finished {
        apply(out);
    }
    build_in_batches(jobs, true, build_growth, &mut apply);
}

/// One tree for `rebuild_gpu_trees` to build: its parameters.
struct GrowthJob {
    entity: Entity,
    params: TreeParams,
    background: bool,
}

/// What a worker makes of one [`GrowthJob`]: the full skeleton, the mature state and the tagged
/// mature meshes.
pub(crate) struct GrowthBuilt {
    entity: Entity,
    /// Built by a [`PendingBuild`] task: it may be out of date by the time it is done.
    background: bool,
    full: Skeleton,
    state: GrowthState,
    wood: Mesh,
    leaves: Option<Mesh>,
    flowers: Option<Mesh>,
}

/// Build one [`GrowthJob`]: on any thread, so a pure function of the job.
fn build_growth(job: GrowthJob) -> GrowthBuilt {
    let full = generate_skeleton(&job.params);
    let mut data = build_growth_data(&full, &job.params);
    let state = std::mem::take(&mut data.state);
    let (wood, leaves, flowers) = data.into_meshes();
    GrowthBuilt { entity: job.entity, background: job.background, full, state, wood, leaves, flowers }
}
