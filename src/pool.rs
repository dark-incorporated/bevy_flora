//! Forests: many trees drawn from a few built ones.
//!
//! A [`TreePool`] is a handful of variants of one tree (the same parameters with
//! different seeds), each built once at three levels of detail
//! ([`build_lods`]) with its meshes and materials shared. A [`PooledTree`] is a
//! tree placed from a pool: it carries nothing but which variant it is, and
//! [`FloraPlugin`](crate::FloraPlugin) gives it a child entity per level of
//! detail and, far away, an impostor ([`crate::impostor`]). Entities that share
//! a mesh and a material are drawn in one instanced call by Bevy, so a few
//! thousand draw calls cover a hundred thousand trees; which child is drawn is
//! decided by the distance to the camera (a [`VisibilityRange`] on each, with a
//! short dithered cross-fade).
//!
//! Pooled trees are static: they have the age the pool was built at. Trees that
//! grow stay [`ProceduralTree`] (with
//! [`GpuGrowth`](crate::GpuGrowth)), which suits the few that are close.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraPlugin, LodSettings, PooledTree, TreePool, TreePreset, build_pool};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, FloraPlugin))
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(
//!     mut commands: Commands,
//!     mut pools: ResMut<Assets<TreePool>>,
//!     mut meshes: ResMut<Assets<Mesh>>,
//! ) {
//!     let pool = build_pool(
//!         &TreePreset::Oak.params(),
//!         8,
//!         f32::INFINITY,
//!         &LodSettings::default(),
//!         &mut meshes,
//!     );
//!     let pool = pools.add(pool);
//!     for i in 0..10_000u32 {
//!         commands.spawn((
//!             PooledTree { pool: pool.clone(), variant: i },
//!             Transform::from_xyz((i % 100) as f32 * 8.0, 0.0, (i / 100) as f32 * 8.0),
//!         ));
//!     }
//! }
//! ```

use bevy::asset::{Asset, AssetApp, AssetServer, Assets, Handle};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{Visibility, VisibilityRange};
use bevy::ecs::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::log::warn;
use bevy::math::{Vec3, Vec3A};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::TypePath;
use bevy::transform::components::Transform;

use crate::impostor::{ImpostorAtlas, ImpostorBakes, ImpostorMaterial, ImpostorSupport};
use crate::lod::{LodDistances, LodSettings, build_lods};
use crate::mesh::MeshData;
use crate::params::TreeParams;
use crate::pipeline::TreeMeshes;
use crate::shape::TreeShape;
use crate::skeleton::Skeleton;
use crate::plugin::{ProceduralTree, leaf_material, material};
use crate::shading::{FloraMaterial, FloraMaterialSupport, FloraWindBuffer, TextureLibrary, flora_material};
use crate::units::FloraUnits;

/// Trees whose children are created in one frame, at most: a hundred thousand
/// trees get their entities over a few dozen frames instead of one long one.
const MAX_TREES_PER_FRAME: usize = 4096;
/// Width of the cross-fade between two levels of detail, in metres (times [`FloraUnits`] in the
/// world).
const FADE: f32 = 2.0;

/// Variants of one tree, built once and drawn many times. See the [module
/// documentation](self).
#[derive(Asset, TypePath, Debug)]
pub struct TreePool {
    /// What the variants were built from; variant `k` has the seed `params.seed + k`.
    pub params: TreeParams,
    /// The age the variants were built at (`f32::INFINITY` for mature trees).
    pub age: f32,
    /// How the levels of detail were made and where they switch.
    pub lod: LodSettings,
    /// Shared by the wood and the leaves of every variant at every level. Made by the plugin
    /// the first time the pool is drawn (it needs to know whether [`FloraMaterial`] is
    /// registered); `None` before.
    pub materials: Option<PoolMaterials>,
    /// The variants, in seed order. Never empty.
    pub variants: Vec<PoolVariant>,
}

/// The materials a [`TreePool`] is drawn with, one for the wood and one for the leaves.
#[derive(Clone, Debug)]
pub enum PoolMaterials {
    /// Leaf shading and wind: [`FloraMaterial`].
    Flora {
        /// The wood.
        wood: Handle<FloraMaterial>,
        /// The leaves.
        leaves: Handle<FloraMaterial>,
    },
    /// Plain materials: [`FloraPlugin`](crate::FloraPlugin) was added before the renderer's
    /// plugins, or the device has no storage buffers in the vertex stage (WebGL2), so
    /// [`FloraMaterial`] cannot be drawn.
    Standard {
        /// The wood.
        wood: Handle<StandardMaterial>,
        /// The leaves.
        leaves: Handle<StandardMaterial>,
    },
}

/// One variant of a [`TreePool`].
#[derive(Clone, Debug)]
pub struct PoolVariant {
    /// The wood at levels 0 (finest), 1 and 2.
    pub lods: [Handle<Mesh>; 3],
    /// The leaves at levels 0, 1 and 2, with a flowering tree's flowers after them
    /// ([`TreeMeshes::flowers`]); `None` for a tree without leaves.
    pub leaves: [Option<Handle<Mesh>>; 3],
    /// Bounds of wood and leaves at each level.
    pub bounds: [Aabb; 3],
    /// The picture far trees are drawn from. `None` until the plugin has baked
    /// it, which takes a few frames once the renderer runs, and for good when it
    /// cannot (no renderer) or [`LodDistances::impostor`] is infinite.
    pub impostor: Option<ImpostorAtlas>,
    /// Height of the tree, in metres.
    pub height: f32,
    /// How far the crown reaches from the trunk axis, in metres.
    pub radius: f32,
    /// The variant's gameplay shape, with [`TreeShape::DEFAULT_LIMB_RADIUS`]: what a
    /// [`TreeShape`] on a [`PooledTree`] of this variant gets (with its own limb radius).
    pub shape: TreeShape,
    /// The variant's skeleton, at the pool's age: where its branches are, for gameplay (perches,
    /// fruit, nests) or a shape of your own.
    pub skeleton: Skeleton,
}

/// A tree placed from a [`TreePool`]. Static: it has the age the pool was built
/// at, and no components other than its children, which the plugin adds.
///
/// `variant` is taken modulo the pool's number of variants, so any number
/// (a hash of the position, the index of the tree) picks one. The tree appears
/// once its pool exists and, if the pool is baking impostors, once that is done:
/// a few frames after the pool was added.
///
/// The children are drawn by distance (see [`LodDistances`]). The impostor casts
/// no shadow; the three levels of detail do, unless the tree carries [`NotShadowCaster`]:
/// that reaches every level (taking it off gives the shadows back), so a game can let only
/// the trees near the player cast. (In the `forest` example, 100 000
/// trees with a shadow-casting sun cost 7.6 ms a frame on an Apple M4 Pro, leaf shading and
/// wind included (see the README). On an Apple M5 the shadow maps were about a third of the
/// pooled forest's time, and narrowing the cascades to 120 m in 2 did not change it.)
#[derive(Component, Clone, Debug)]
#[require(Transform, Visibility)]
pub struct PooledTree {
    /// The pool to take the tree from.
    pub pool: Handle<TreePool>,
    /// Which variant of the pool this tree is.
    pub variant: u32,
}

/// Marks a pooled tree whose [`TreeShape`] was filled from its variant.
#[derive(Component)]
pub(crate) struct PooledShapeFilled;

/// Fill the [`TreeShape`] of every pooled tree that has one, once its pool exists. A tree's
/// shape is its variant's skeleton with the shape's own limb radius; it is not refilled if the
/// tree's [`PooledTree`] changes later.
pub(crate) fn fill_pooled_shapes(
    mut commands: Commands,
    pools: Res<Assets<TreePool>>,
    mut trees: Query<(Entity, &PooledTree, &mut TreeShape), Without<PooledShapeFilled>>,
) {
    for (entity, pooled, mut shape) in &mut trees {
        let Some(pool) = pools.get(&pooled.pool) else { continue };
        let variant = &pool.variants[pooled.variant as usize % pool.variants.len()];
        *shape = if shape.limb_radius == TreeShape::DEFAULT_LIMB_RADIUS {
            variant.shape.clone()
        } else {
            TreeShape::from_skeleton(&variant.skeleton, &pool.params, shape.limb_radius)
        };
        commands.entity(entity).insert(PooledShapeFilled);
    }
}

/// Marks a [`PooledTree`] that has its children.
#[derive(Component)]
pub(crate) struct PooledTreeSpawned;

/// Build a pool of `variants` trees (at least 1) from `params`, as they look at
/// `age` (`f32::INFINITY` for mature trees). The meshes are added to `meshes`; the
/// materials are made by the plugin when the pool is first drawn.
///
/// The variants are built on all cores, which takes the time of one or two trees.
/// Impostors are not part of the result: the plugin bakes them once the pool is
/// an asset, see [`PoolVariant::impostor`].
///
/// # Panics
///
/// If `age` is so low that the tree has no wood yet (0).
pub fn build_pool(
    params: &TreeParams,
    variants: u32,
    age: f32,
    lod: &LodSettings,
    meshes: &mut Assets<Mesh>,
) -> TreePool {
    let built = build_variants(params, variants.max(1), age, lod);
    let mut add = |data: MeshData| meshes.add(data.into_bevy_mesh());
    let variants = built
        .into_iter()
        .map(|lods| {
            assert!(
                !lods[0].trunk.is_empty(),
                "build_pool: a tree of age {age} has no wood yet, so there is nothing to draw"
            );
            let (height, radius) = reach(&lods[0]);
            let bounds = lods.each_ref().map(bounds_of);
            let [fine, middle, coarse] = lods;
            let wood = [add(fine.trunk), add(middle.trunk), add(coarse.trunk)];
            // A flowering tree's flowers go in its leaf mesh: a pooled tree's flowers are hidden
            // by the shaders out of season, not left out (a pool's trees are mostly the coarser
            // levels and impostors, and one entity fewer per level keeps a big forest cheap).
            let mut leaves = |mut data: MeshData, flowers: MeshData| {
                data.extend(&flowers);
                (!data.is_empty()).then(|| add(data))
            };
            let leaves = [
                leaves(fine.foliage, fine.flowers),
                leaves(middle.foliage, middle.flowers),
                leaves(coarse.foliage, coarse.flowers),
            ];
            let shape = TreeShape::from_skeleton(&fine.skeleton, params, TreeShape::DEFAULT_LIMB_RADIUS);
            PoolVariant {
                lods: wood,
                leaves,
                bounds,
                impostor: None,
                height,
                radius,
                shape,
                skeleton: fine.skeleton,
            }
        })
        .collect();
    TreePool {
        params: params.clone(),
        age,
        lod: *lod,
        materials: None,
        variants,
    }
}

impl TreePool {
    /// The parameters of variant `variant` (taken modulo the number of variants, as a
    /// [`PooledTree`] does): a `ProceduralTree` with them, at [`age`](Self::age), is the tree
    /// the pool draws at level 0.
    pub fn tree_params(&self, variant: u32) -> TreeParams {
        let k = variant % self.variants.len() as u32;
        TreeParams { seed: self.params.seed.wrapping_add(k as i32), ..self.params.clone() }
    }
}

/// [`build_lods`] for the seeds `params.seed + k`, `k < count`, on all cores.
fn build_variants(
    params: &TreeParams,
    count: u32,
    age: f32,
    lod: &LodSettings,
) -> Vec<[TreeMeshes; 3]> {
    let build = |k: u32| {
        let mut variant = params.clone();
        variant.seed = params.seed.wrapping_add(k as i32);
        build_lods(&variant, age, lod)
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let workers = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(count as usize);
        let build = &build;
        let mut built: Vec<(u32, [TreeMeshes; 3])> = std::thread::scope(|scope| {
            let threads: Vec<_> = (0..workers)
                .map(|worker| {
                    scope.spawn(move || {
                        (worker as u32..count)
                            .step_by(workers)
                            .map(|k| (k, build(k)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            threads
                .into_iter()
                .flat_map(|thread| thread.join().expect("a tree variant failed to build"))
                .collect()
        });
        built.sort_by_key(|(k, _)| *k);
        built.into_iter().map(|(_, lods)| lods).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        (0..count).map(build).collect()
    }
}

/// Height of the tree and how far its crown reaches from the trunk axis.
fn reach(tree: &TreeMeshes) -> (f32, f32) {
    let points = tree.trunk.positions.iter().chain(&tree.foliage.positions).chain(&tree.flowers.positions);
    points.fold((0.0f32, 0.0f32), |(height, radius), p| {
        (height.max(p.y), radius.max(p.x.hypot(p.z)))
    })
}

/// Box around the wood and the leaves of `tree`.
fn bounds_of(tree: &TreeMeshes) -> Aabb {
    let mut points = tree
        .trunk
        .positions
        .iter()
        .chain(&tree.foliage.positions)
        .chain(&tree.flowers.positions)
        .copied();
    let first = points.next().unwrap_or(Vec3::ZERO);
    let (min, max) = points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    Aabb::from_min_max(min, max)
}

/// The cross-fading range of distances in which a child is drawn: from `start`
/// to `end` metres, each end fading over [`FADE`] metres around it, all times `units` (the world
/// units to a metre: [`FloraUnits`]). `end` may be infinite, and `start` zero.
pub(crate) fn band(start: f32, end: f32, units: f32) -> VisibilityRange {
    let half = FADE * units / 2.0;
    let edge = |at: f32| {
        if at <= 0.0 {
            0.0..0.0
        } else if at.is_finite() {
            let at = at * units;
            (at - half)..(at + half)
        } else {
            at..at
        }
    };
    VisibilityRange {
        start_margin: edge(start),
        end_margin: edge(end),
        use_aabb: false,
    }
}

/// Make the materials of every pool that has none yet: [`FloraMaterial`] where the renderer
/// registered it (`wind` is the shared wind buffer then), plain materials where it did not.
fn make_pool_materials(
    pools: &mut Assets<TreePool>,
    wind: Option<&FloraWindBuffer>,
    flora: &mut Assets<FloraMaterial>,
    standard: &mut Assets<StandardMaterial>,
    textures: &mut TextureLibrary,
) {
    let unpainted: Vec<_> = pools
        .iter()
        .filter(|(_, pool)| pool.materials.is_none())
        .map(|(id, _)| id)
        .collect();
    for id in unpainted {
        let Some(mut pool) = pools.get_mut(id) else { continue };
        let (wood, leaves) = (pool.params.trunk_color, pool.params.foliage.color);
        pool.materials = Some(if let Some(wind) = wind {
            PoolMaterials::Flora {
                wood: flora.add(flora_material(&textures.wood(&pool.params), wind)),
                leaves: flora.add(flora_material(&textures.leaves(&pool.params), wind)),
            }
        } else {
            PoolMaterials::Standard {
                wood: standard.add(material(wood)),
                leaves: standard.add(leaf_material(leaves)),
            }
        });
    }
}

/// Give every [`PooledTree`] whose pool is ready its children: wood and leaves at
/// each level of detail, each drawn in its own band of distance, and the
/// impostor beyond them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_pooled_trees(
    mut commands: Commands,
    mut pools: ResMut<Assets<TreePool>>,
    bakes: Res<ImpostorBakes>,
    support: Option<Res<ImpostorSupport>>,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut textures: TextureLibrary,
    trees: Query<(Entity, &PooledTree, Has<NotShadowCaster>), Without<PooledTreeSpawned>>,
    units: Res<FloraUnits>,
    mut warned: Local<bool>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let units = units.units_per_metre;
    make_pool_materials(&mut pools, wind, &mut flora, &mut standard, &mut textures);
    // A tree whose pool is not ready yet is skipped, not counted: it must not
    // keep trees of a ready pool from getting their children.
    let mut spawned = 0;
    for (tree, pooled, quiet) in &trees {
        let Some(pool) = pools.get(&pooled.pool) else {
            continue;
        };
        let index = pooled.variant as usize % pool.variants.len();
        let variant = &pool.variants[index];
        let impostors_wanted = pool.lod.distances.impostor.is_finite();
        if impostors_wanted && support.is_none() && !*warned {
            *warned = true;
            warn!(
                "bevy_flora: a tree pool has no impostors: FloraPlugin was added before the renderer's \
                 plugins, there is no renderer, or the device has no vertex-stage storage buffers \
                 (WebGL2). Add it after DefaultPlugins, or set \
                 LodDistances::impostor to infinity to draw the coarsest level of detail at any distance"
            );
        }
        if impostors_wanted
            && support.is_some()
            && variant.impostor.is_none()
            && !bakes.is_settled(pooled.pool.id(), index)
        {
            continue;
        }

        let LodDistances {
            lod1,
            lod2,
            impostor,
        } = pool.lod.distances;
        let has_impostor = variant.impostor.is_some();
        // Without an impostor the last level of detail is drawn at any distance.
        let bands = [
            band(0.0, lod1, units),
            band(lod1, lod2, units),
            band(
                lod2,
                if has_impostor {
                    impostor
                } else {
                    f32::INFINITY
                },
                units,
            ),
        ];
        let Some(materials) = &pool.materials else {
            continue;
        };
        let mut spawn = |mesh: &Handle<Mesh>, leaf: bool, range: &VisibilityRange, bounds: Aabb| {
            let child = match materials {
                PoolMaterials::Flora { wood, leaves } => {
                    let material = if leaf { leaves } else { wood };
                    commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            range.clone(),
                            bounds,
                            ChildOf(tree),
                        ))
                        .id()
                }
                PoolMaterials::Standard { wood, leaves } => {
                    let material = if leaf { leaves } else { wood };
                    commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            range.clone(),
                            bounds,
                            ChildOf(tree),
                        ))
                        .id()
                }
            };
            if quiet {
                commands.entity(child).insert(NotShadowCaster);
            }
        };
        for (level, range) in bands.into_iter().enumerate() {
            let bounds = variant.bounds[level];
            spawn(&variant.lods[level], false, &range, bounds);
            if let Some(leaves) = &variant.leaves[level] {
                spawn(leaves, true, &range, bounds);
            }
        }
        if let Some(atlas) = &variant.impostor {
            // The quad turns about the trunk, so any yaw of it must be inside the box.
            let half = Vec3A::new(atlas.size.x / 2.0, atlas.size.y / 2.0, atlas.size.x / 2.0);
            commands.spawn((
                Mesh3d(atlas.quad.clone()),
                MeshMaterial3d(atlas.material.clone()),
                band(impostor, f32::INFINITY, units),
                Aabb {
                    center: Vec3A::new(0.0, atlas.center_y, 0.0),
                    half_extents: half,
                },
                NotShadowCaster,
                ChildOf(tree),
            ));
        }
        commands.entity(tree).insert(PooledTreeSpawned);
        spawned += 1;
        if spawned >= MAX_TREES_PER_FRAME {
            break;
        }
    }
}

/// [`NotShadowCaster`] on a tree (pooled or procedural) reaches every mesh the plugin draws it
/// with: given to the children when it arrives or when they change (a rebuild, a new level of
/// detail), taken back off them when it leaves the tree. Impostors never cast, so they keep it.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_shadow_casting(
    mut commands: Commands,
    quieted: Query<
        &Children,
        (
            Or<(With<PooledTree>, With<ProceduralTree>)>,
            With<NotShadowCaster>,
            Or<(Added<NotShadowCaster>, Changed<Children>)>,
        ),
    >,
    mut removed: RemovedComponents<NotShadowCaster>,
    casting: Query<&Children, (Or<(With<PooledTree>, With<ProceduralTree>)>, Without<NotShadowCaster>)>,
    meshes: Query<(Has<NotShadowCaster>, Has<MeshMaterial3d<ImpostorMaterial>>), With<Mesh3d>>,
) {
    for children in &quieted {
        for child in children.iter() {
            if let Ok((false, _)) = meshes.get(child) {
                commands.entity(child).insert(NotShadowCaster);
            }
        }
    }
    for tree in removed.read() {
        let Ok(children) = casting.get(tree) else {
            continue;
        };
        for child in children.iter() {
            if let Ok((true, false)) = meshes.get(child) {
                commands.entity(child).remove::<NotShadowCaster>();
            }
        }
    }
}

/// Register the pool asset where the asset plugin is. Plugins may be added in
/// any order, so [`finish`] tries again.
pub(crate) fn build(app: &mut bevy::app::App) {
    add_asset_types(app);
}

/// See [`build`].
pub(crate) fn finish(app: &mut bevy::app::App) {
    add_asset_types(app);
}

fn add_asset_types(app: &mut bevy::app::App) {
    if app.world().contains_resource::<AssetServer>()
        && !app.world().contains_resource::<Assets<TreePool>>()
    {
        app.init_asset::<TreePool>();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::app::App;
    use bevy::asset::{AssetId, UntypedAssetId};
    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::plugin::FloraPlugin;
    use crate::presets::TreePreset;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        app
    }

    fn make_pool(app: &mut App, variants: u32) -> Handle<TreePool> {
        let pool = app
            .world_mut()
            .resource_scope(|_, mut meshes: Mut<Assets<Mesh>>| {
                build_pool(
                    &TreePreset::Oak.params(),
                    variants,
                    f32::INFINITY,
                    &LodSettings::default(),
                    &mut meshes,
                )
            });
        app.world_mut().resource_mut::<Assets<TreePool>>().add(pool)
    }

    /// The meshes and materials (of either kind) of every child of `trees`.
    fn drawn_by(
        app: &App,
        trees: &[Entity],
    ) -> (HashSet<AssetId<Mesh>>, HashSet<UntypedAssetId>) {
        let world = app.world();
        let mut meshes = HashSet::new();
        let mut materials = HashSet::new();
        for &tree in trees {
            for child in world
                .get::<Children>(tree)
                .expect("tree has children")
                .iter()
            {
                meshes.insert(world.get::<Mesh3d>(child).unwrap().0.id());
                let plain = world
                    .get::<MeshMaterial3d<StandardMaterial>>(child)
                    .map(|m| m.0.id().untyped());
                let flora = world
                    .get::<MeshMaterial3d<FloraMaterial>>(child)
                    .map(|m| m.0.id().untyped());
                materials.insert(plain.or(flora).expect("a child has a material"));
            }
        }
        (meshes, materials)
    }

    #[test]
    fn trees_of_a_variant_share_their_meshes_and_materials() {
        let mut app = app();
        let pool = make_pool(&mut app, 2);
        let trees: Vec<Entity> = (0..100u32)
            .map(|i| {
                app.world_mut()
                    .spawn((
                        PooledTree {
                            pool: pool.clone(),
                            variant: i,
                        },
                        Transform::from_xyz(i as f32 * 8.0, 0.0, 0.0),
                    ))
                    .id()
            })
            .collect();
        app.update();

        // Three levels of wood and of leaves for each of the two variants: 100
        // trees draw 12 meshes with 2 materials.
        let (meshes, materials) = drawn_by(&app, &trees);
        assert_eq!(meshes.len(), 12);
        assert_eq!(materials.len(), 2);
        for &tree in &trees {
            assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 6);
        }

        // Variants differ.
        let (even, _) = drawn_by(&app, &trees[0..1]);
        let (odd, _) = drawn_by(&app, &trees[1..2]);
        assert!(even.is_disjoint(&odd));
    }

    #[test]
    fn pools_use_flora_materials_where_supported() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        let pool = make_pool(&mut app, 2);
        let trees: Vec<Entity> = (0..100u32)
            .map(|i| {
                app.world_mut()
                    .spawn(PooledTree {
                        pool: pool.clone(),
                        variant: i,
                    })
                    .id()
            })
            .collect();
        app.update();

        let (_, materials) = drawn_by(&app, &trees);
        assert_eq!(materials.len(), 2, "one for the wood and one for the leaves, whatever the trees");
        let pools = app.world().resource::<Assets<TreePool>>();
        let Some(PoolMaterials::Flora { wood, leaves }) = &pools.get(&pool).unwrap().materials else {
            panic!("a pool is drawn with flora materials where they are supported");
        };
        let flora = app.world().resource::<Assets<FloraMaterial>>();
        assert!(!flora.get(wood).unwrap().extension.leaf);
        assert!(flora.get(leaves).unwrap().extension.leaf);
    }

    #[test]
    fn variant_wraps_around_the_pool() {
        let mut app = app();
        let pool = make_pool(&mut app, 3);
        let spawn = |app: &mut App, variant: u32| {
            app.world_mut()
                .spawn(PooledTree {
                    pool: pool.clone(),
                    variant,
                })
                .id()
        };
        let (a, b) = (spawn(&mut app, 1), spawn(&mut app, 4));
        app.update();
        assert_eq!(drawn_by(&app, &[a]).0, drawn_by(&app, &[b]).0);
    }

    #[test]
    fn a_pooled_trees_shape_is_its_variants_with_its_own_limb_radius() {
        let mut app = app();
        let pool = make_pool(&mut app, 2);
        let spawn = |app: &mut App, variant: u32, shape: TreeShape| {
            app.world_mut().spawn((PooledTree { pool: pool.clone(), variant }, shape)).id()
        };
        let a = spawn(&mut app, 1, TreeShape::default());
        let b = spawn(&mut app, 1, TreeShape::with_limb_radius(f32::INFINITY));
        app.update();
        let pools = app.world().resource::<Assets<TreePool>>();
        let built = pools.get(&pool).expect("the pool exists");
        let variant = &built.variants[1];
        assert_eq!(app.world().get::<TreeShape>(a), Some(&variant.shape));
        let trunk_only = app.world().get::<TreeShape>(b).unwrap();
        assert!(trunk_only.limbs.is_empty() && trunk_only.trunk == variant.shape.trunk);
        assert!(variant.shape.height() > 0.0 && (variant.shape.height() - variant.height).abs() < 1.0);
    }

    #[test]
    fn a_tree_waits_for_its_pool() {
        let mut app = app();
        let pool = app.world().resource::<Assets<TreePool>>().reserve_handle();
        let tree = app
            .world_mut()
            .spawn(PooledTree {
                pool: pool.clone(),
                variant: 0,
            })
            .id();
        app.update();
        assert!(app.world().get::<Children>(tree).is_none());

        let built = app
            .world_mut()
            .resource_scope(|_, mut meshes: Mut<Assets<Mesh>>| {
                build_pool(
                    &TreePreset::Pine.params(),
                    1,
                    f32::INFINITY,
                    &LodSettings::default(),
                    &mut meshes,
                )
            });
        app.world_mut()
            .resource_mut::<Assets<TreePool>>()
            .insert(pool.id(), built)
            .unwrap();
        app.update();
        assert!(!app.world().get::<Children>(tree).unwrap().is_empty());
    }

    #[test]
    fn levels_take_over_one_after_the_other_and_the_last_is_never_hidden() {
        let mut app = app();
        let pool = make_pool(&mut app, 1);
        let tree = app.world_mut().spawn(PooledTree { pool, variant: 0 }).id();
        app.update();

        let world = app.world();
        let LodDistances { lod1, lod2, .. } = LodSettings::default().distances;
        let mut ranges: Vec<VisibilityRange> = world
            .get::<Children>(tree)
            .unwrap()
            .iter()
            .map(|c| world.get::<VisibilityRange>(c).unwrap().clone())
            .collect();
        ranges.dedup();
        // Wood and leaves share a band: three bands in all.
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].start_margin, 0.0..0.0);
        // A level fades out over the very distances the next fades in over.
        assert_eq!(ranges[0].end_margin, ranges[1].start_margin);
        assert_eq!(ranges[1].end_margin, ranges[2].start_margin);
        assert!(ranges[1].start_margin.contains(&lod1) && ranges[2].start_margin.contains(&lod2));
        // With no impostor the last level is drawn at any distance.
        assert!(ranges[2].end_margin.start.is_infinite());
    }

    #[test]
    fn plugin_added_before_the_asset_plugin_still_spawns_pooled_trees() {
        let mut app = App::new();
        app.add_plugins(FloraPlugin);
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.finish();
        app.cleanup();

        let pool = make_pool(&mut app, 1);
        let tree = app.world_mut().spawn(PooledTree { pool, variant: 0 }).id();
        app.update();
        assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 6);
    }

    #[test]
    fn band_fades_around_its_ends() {
        let range = band(30.0, 80.0, 1.0);
        assert_eq!(range.start_margin, 29.0..31.0);
        assert_eq!(range.end_margin, 79.0..81.0);
        let first = band(0.0, f32::INFINITY, 1.0);
        assert_eq!(first.start_margin, 0.0..0.0);
        assert!(first.end_margin.start.is_infinite());
    }

    #[test]
    fn bands_and_their_fades_are_metres_scaled_to_the_world() {
        // 4 world units to the metre: 30 m is 120, its 2 m fade 8 wide, infinity stays infinite.
        let range = band(30.0, 80.0, 4.0);
        assert_eq!(range.start_margin, 116.0..124.0);
        assert_eq!(range.end_margin, 316.0..324.0);
        let first = band(0.0, f32::INFINITY, 4.0);
        assert_eq!(first.start_margin, 0.0..0.0);
        assert!(first.end_margin.start.is_infinite());
    }

    #[test]
    fn a_tree_without_shadows_keeps_every_child_out_of_them_until_it_casts_again() {
        let mut app = app();
        let pool = make_pool(&mut app, 1);
        let quiet = app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, NotShadowCaster)).id();
        let loud = app.world_mut().spawn(PooledTree { pool, variant: 0 }).id();
        let oak = crate::presets::TreePreset::Oak.params();
        let lods = crate::lod::TreeLod::default();
        let grown = app.world_mut().spawn((ProceduralTree(oak), lods, NotShadowCaster)).id();
        app.update();
        app.update();
        // (casting children, quiet children) of a tree.
        let count = |app: &App, tree: Entity| {
            let world = app.world();
            let children = world.get::<Children>(tree).expect("drawn");
            let quiet = children.iter().filter(|&c| world.get::<NotShadowCaster>(c).is_some()).count();
            (children.len() - quiet, quiet)
        };
        assert_eq!(count(&app, quiet), (0, 6), "every level of a quiet pooled tree");
        assert_eq!(count(&app, loud), (6, 0), "a tree without it casts as before");
        let (casting, quieted) = count(&app, grown);
        assert!(casting == 0 && quieted >= 6, "every mesh of a procedural tree, its levels included: {casting} cast");

        // Taken off, the shadows come back; put on, they go.
        app.world_mut().entity_mut(quiet).remove::<NotShadowCaster>();
        app.world_mut().entity_mut(loud).insert(NotShadowCaster);
        app.world_mut().entity_mut(grown).remove::<NotShadowCaster>();
        app.update();
        assert_eq!(count(&app, quiet), (6, 0));
        assert_eq!(count(&app, loud), (0, 6));
        assert_eq!(count(&app, grown), (quieted, 0));

        // A rebuild draws new meshes; they stay out of the shadows too.
        app.world_mut().entity_mut(grown).insert(NotShadowCaster);
        app.update();
        app.world_mut().get_mut::<ProceduralTree>(grown).unwrap().0.seed = 7;
        app.update();
        app.update();
        assert_eq!(count(&app, grown).0, 0, "a rebuilt tree's new meshes cast");
    }
}
