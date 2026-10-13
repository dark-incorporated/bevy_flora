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
//! A pool can be drawn in more than one colouring ([`PoolColors`], [`TreePool::add_look`],
//! [`PooledLook`]) without a second set of meshes, and a look's colours can change while the game
//! runs ([`RecolorPool`]): see [`PoolColors`].
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

use std::collections::HashMap;

use bevy::asset::{Asset, AssetApp, AssetId, AssetServer, Assets, Handle};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{Visibility, VisibilityRange};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::log::warn;
use bevy::math::{Vec3, Vec3A, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::{Reflect, TypePath};
use bevy::camera::{Camera, Camera3d, Projection};
use bevy::transform::components::{GlobalTransform, Transform};

use crate::ground::FloraGround;
use crate::impostor::{ImpostorAtlas, ImpostorBakes, ImpostorMaterial, ImpostorSupport, ImpostorTint};
use crate::lod::{LodDistances, LodSettings, build_lods};
use crate::mesh::MeshData;
use crate::params::TreeParams;
use crate::pipeline::TreeMeshes;
use crate::shape::TreeShape;
use crate::skeleton::Skeleton;
use crate::plugin::{ProceduralTree, crown_material, material};
use crate::shading::{
    FloraMaterial, FloraMaterialSupport, FloraUniform, FloraWindBuffer, Surface, TextureLibrary, flora_material,
};
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
    /// The looks the pool can be drawn in: the colours of the wood and the leaves of each, and
    /// whether it is lit. Look 0 is the colours of [`params`](Self::params), lit;
    /// [`add_look`](Self::add_look) adds more. A [`PooledTree`] picks one with [`PooledLook`]; all of them
    /// share the pool's meshes. Never empty.
    pub looks: Vec<PoolColors>,
    /// One pair of materials per look, shared by the wood and the leaves of every variant at every
    /// level. Made by the plugin the first time the pool is drawn (it needs to know whether
    /// [`FloraMaterial`] is registered), for the looks the pool has then; empty before.
    pub materials: Vec<PoolMaterials>,
    /// The variants, in seed order. Never empty.
    pub variants: Vec<PoolVariant>,
}

/// The colours of one look of a [`TreePool`]: what its trees' wood and leaves are, and whether
/// they are lit at all. The pool's meshes are the same in every look.
///
/// A look is made like the pool's own: the colours replace the parameters' `trunk_color` and
/// `foliage.color`, so a bark or leaf scan keeps its pattern and has its average colour divided
/// out, and an autumn or blossom colour is still the same ratio of the leaf colour away. Impostors
/// bake neutral grey and take the colours as a tint ([`ImpostorTint`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolColors {
    /// The wood.
    pub wood: Color,
    /// The leaves.
    pub leaves: Color,
    /// Draw the surface as its colour alone (with the weather's wet, snow and fire still on it): no
    /// lights, no shadows received, no light through the leaves. For a game whose palette is the
    /// picture, such as a map or a diagram, or a dream.
    pub unlit: bool,
}

impl PoolColors {
    /// The colours `params` give its trees, lit.
    pub fn of(params: &TreeParams) -> Self {
        Self { wood: params.trunk_color, leaves: params.foliage.color, unlit: false }
    }

    /// `params` painted in these colours.
    pub(crate) fn apply(&self, params: &TreeParams) -> TreeParams {
        let mut painted = params.clone();
        painted.trunk_color = self.wood;
        painted.foliage.color = self.leaves;
        painted
    }

    /// The wood's and the leaves' surfaces in these colours, made from `params`.
    pub(crate) fn surfaces(&self, params: &TreeParams, textures: &mut TextureLibrary) -> (Surface, Surface) {
        let painted = self.apply(params);
        (textures.wood(&painted).with_unlit(self.unlit), textures.leaves(&painted).with_unlit(self.unlit))
    }
}

/// Which look of its pool a [`PooledTree`] is drawn in: an index into [`TreePool::looks`], taken
/// modulo their number, so any number (a hash of the position) picks one. A tree without it has
/// look 0. Changing it re-dresses the tree's children without rebuilding anything; a tree felled
/// or notched keeps its look.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PooledLook(pub u32);

/// Change the colours of one look of a pool, for every tree drawn in it, at once: the pool's
/// materials and its impostors' are rewritten in place (only where a value changed), and the pool
/// itself is not touched, so no tree is re-indexed or rebuilt. The pool must have the look
/// ([`TreePool::add_look`]); a pool that was never drawn takes the colours when it first is.
/// `look` and `colors` are as for [`TreePool::looks`]; felled trees follow with [`RecolorTree`](crate::RecolorTree).
///
/// [`TreePool::looks`] keeps the colours the look was added with: the current ones are in the
/// materials.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RecolorPool {
    /// The pool.
    pub pool: AssetId<TreePool>,
    /// Which look of it: an index into [`TreePool::looks`].
    pub look: u32,
    /// The new colours.
    pub colors: PoolColors,
}

/// The colours [`RecolorPool`] gave looks, over those in [`TreePool::looks`]: what a pool that
/// has not been drawn yet, a new impostor and a tree about to be felled take.
#[derive(Resource, Default)]
pub(crate) struct PoolRecolors(HashMap<(AssetId<TreePool>, u32), PoolColors>);

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
        looks: vec![PoolColors::of(params)],
        materials: Vec::new(),
        variants,
    }
}

impl PoolRecolors {
    /// The colours of `look` (taken modulo the pool's looks) of `pool`, now.
    pub(crate) fn colors(&self, id: AssetId<TreePool>, look: u32, pool: &TreePool) -> PoolColors {
        let look = look % pool.looks.len().max(1) as u32;
        self.0.get(&(id, look)).copied().unwrap_or_else(|| pool.looks[look as usize])
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

    /// [`tree_params`](Self::tree_params) painted in the colours of `look` (taken modulo the
    /// number of looks): what a pooled tree in that look becomes when it is felled.
    pub fn tree_params_for(&self, variant: u32, look: u32) -> TreeParams {
        let params = self.tree_params(variant);
        match self.looks.get(look as usize % self.looks.len().max(1)) {
            Some(colors) => colors.apply(&params),
            None => params,
        }
    }

    /// Add a look and return its index, for [`PooledLook`] and [`RecolorPool`]. Call it before the
    /// pool is added to `Assets<TreePool>`: the materials and impostors are made for the looks it
    /// has when it is first drawn.
    pub fn add_look(&mut self, colors: PoolColors) -> u32 {
        self.looks.push(colors);
        (self.looks.len() - 1) as u32
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

/// A level-of-detail child's band before [`LodScale`]: drawn from `start` to `end` metres, each
/// end fading over [`FADE`] metres around it (`end` may be infinite, and `start` zero). The
/// plugin keeps the child's `VisibilityRange` at this band times the scale and the world units to
/// a metre ([`FloraUnits`]; [`scale_lod_bands`]).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct LodBand {
    start: f32,
    end: f32,
}

impl LodBand {
    /// The band with its distances, and its fade, times `scale`.
    fn range(self, scale: f32) -> VisibilityRange {
        let half = FADE / 2.0 * scale;
        let edge = |at: f32| {
            let at = at * scale;
            if at <= 0.0 {
                0.0..0.0
            } else if at.is_finite() {
                (at - half)..(at + half)
            } else {
                at..at
            }
        };
        VisibilityRange {
            start_margin: edge(self.start),
            end_margin: edge(self.end),
            use_aabb: false,
        }
    }
}

/// A level-of-detail child's band (metres) and the range it is drawn with at `units` world units
/// to a metre ([`FloraUnits`]) and no [`LodScale`] or zoom, to spawn or insert together;
/// [`scale_lod_bands`] rescales the range.
pub(crate) fn banded(start: f32, end: f32, units: f32) -> (LodBand, VisibilityRange) {
    let band = LodBand { start, end };
    (band, band.range(units))
}

/// Multiplies every level-of-detail distance: where pooled trees ([`PooledTree`]) and trees
/// drawn with [`TreeLod`](crate::TreeLod) hand over to a coarser level, and where pooled trees
/// turn into impostors. 1 (the default) keeps the distances they were built with; 0.5 hands every
/// tree over at half the distance (a lower quality setting), 2 at twice.
///
/// Under an orthographic camera a tree's size on screen follows the projection's zoom, not its
/// distance, so while the game's main camera (the active `Camera3d` with the lowest `order`) is
/// orthographic the plugin multiplies this by the camera's zoom as well: a tree where the
/// camera's view meets the ground ([`FloraGround`] if there is one, else the plane y = 0) gets
/// the level a 45° perspective camera gives it from where that camera would see as tall a view.
/// Zoomed in, the trees in view are drawn in full; zoomed out, they turn into impostors. The
/// bands are rewritten when the zoom moves by more than 2%, so a smooth zoom rewrites them every
/// few frames, and as trees at different distances switch at slightly different zooms, the
/// forest cross-fades as the zoom changes.
///
/// The distances are still measured from where the camera stands, as for any camera. An
/// orthographic camera's picture does not change with its distance, so stand it well back (a few
/// times as far as its widest view is tall, with `near` at 0): the trees at the top and the bottom
/// of the screen are then at about the same distance from it and drawn at about the same level,
/// and nothing in view is behind it. Close to its focus, the trees low on the screen stand much
/// nearer the camera than those at the top and are drawn finer. The `isometric` example stands its
/// camera 400 m back for views up to 260 m tall.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct LodScale(pub f32);

impl Default for LodScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The main camera's zoom as a factor on the level-of-detail distances: 1 unless it is
/// orthographic (see [`LodScale`]).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ZoomScale(pub(crate) f32);

impl Default for ZoomScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far, as a share, the zoom moves before the bands are rewritten.
const ZOOM_STEP: f32 = 0.02;

/// The field of view of the perspective camera an orthographic camera's zoom is matched to:
/// Bevy's default, 45°.
const REFERENCE_FOV: f32 = std::f32::consts::FRAC_PI_4;

/// Follow the main camera's zoom ([`ZoomScale`]) while it is orthographic; 1 otherwise.
pub(crate) fn follow_orthographic_zoom(
    cameras: Query<(&Camera, &Projection, &GlobalTransform), With<Camera3d>>,
    ground: Option<Res<FloraGround>>,
    units: Res<FloraUnits>,
    mut zoom: ResMut<ZoomScale>,
) {
    let main = cameras.iter().filter(|(camera, ..)| camera.is_active).min_by_key(|(camera, ..)| camera.order);
    let wanted = match main {
        Some((_, Projection::Orthographic(orthographic), at)) => {
            orthographic_zoom(orthographic.area.height(), at, ground.as_deref(), &units)
        }
        _ => 1.0,
    };
    // Read before borrowing mutably: a mutable borrow marks it changed.
    if (wanted / zoom.0 - 1.0).abs() > ZOOM_STEP || (wanted == 1.0 && zoom.0 != 1.0) {
        zoom.0 = wanted;
    }
}

/// The zoom of an orthographic camera at `at` whose view is `height` world units tall: its
/// distance to where its view meets the ground, over the distance from which a perspective camera
/// of [`REFERENCE_FOV`] sees as tall a view. 1 when it does not look down. The view height and the
/// distance are read in metres (`units`), so a world in feet picks the levels a world in metres
/// does at the same view.
fn orthographic_zoom(height: f32, at: &GlobalTransform, ground: Option<&FloraGround>, units: &FloraUnits) -> f32 {
    let height = units.metres(height);
    let forward = at.forward().as_vec3();
    let eye = at.translation();
    if forward.y > -1.0e-3 || height.is_nan() || height <= 0.0 {
        return 1.0;
    }
    let mut distance = eye.y / -forward.y;
    if let Some(ground) = ground {
        // Down to the ground under where the view meets the last guess, a few times over.
        for _ in 0..4 {
            let floor = ground.height((eye + forward * distance).xz());
            distance = (eye.y - floor) / -forward.y;
        }
    }
    let seen_from = height / (2.0 * (REFERENCE_FOV / 2.0).tan());
    (units.metres(distance.max(0.0)) / seen_from).clamp(1.0e-3, 1.0e3)
}

/// Keep every level-of-detail child's `VisibilityRange` at its band times [`LodScale`] and the
/// zoom and the world units to a metre: all of them when the factor changed, else only the
/// children whose band is new or changed (none needs it while `LodScale` and the zoom are 1: they
/// are spawned at the unit).
#[allow(clippy::type_complexity)]
pub(crate) fn scale_lod_bands(
    lod: Res<LodScale>,
    zoom: Res<ZoomScale>,
    units: Res<FloraUnits>,
    mut applied: Local<Option<f32>>,
    mut bands: ParamSet<(
        Query<(&LodBand, &mut VisibilityRange)>,
        Query<(&LodBand, &mut VisibilityRange), Changed<LodBand>>,
    )>,
) {
    let unit = units.units_per_metre;
    let scale = lod.0 * zoom.0 * unit;
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { unit };
    let was = applied.replace(scale).unwrap_or(unit);
    if scale != was {
        bands.p0().par_iter_mut().for_each(|(band, mut range)| *range = band.range(scale));
    } else if scale != unit {
        for (band, mut range) in &mut bands.p1() {
            *range = band.range(scale);
        }
    }
}

/// Make the materials of every pool that has none yet, a pair per look: [`FloraMaterial`] where
/// the renderer registered it (`wind` is the shared wind buffer then), plain materials where it
/// did not. A look [`RecolorPool`] has changed is made in its new colours.
fn make_pool_materials(
    pools: &mut Assets<TreePool>,
    recolors: &PoolRecolors,
    wind: Option<&FloraWindBuffer>,
    flora: &mut Assets<FloraMaterial>,
    standard: &mut Assets<StandardMaterial>,
    textures: &mut TextureLibrary,
) {
    let unpainted: Vec<_> = pools
        .iter()
        .filter(|(_, pool)| pool.materials.is_empty())
        .map(|(id, _)| id)
        .collect();
    for id in unpainted {
        let Some(mut pool) = pools.get_mut(id) else { continue };
        let materials = (0..pool.looks.len() as u32)
            .map(|look| {
                let colors = recolors.colors(id, look, &pool);
                if let Some(wind) = wind {
                    let (wood, leaves) = colors.surfaces(&pool.params, textures);
                    PoolMaterials::Flora {
                        wood: flora.add(flora_material(&wood, wind)),
                        leaves: flora.add(flora_material(&leaves, wind)),
                    }
                } else {
                    let unlit = |base: StandardMaterial| StandardMaterial { unlit: colors.unlit, ..base };
                    PoolMaterials::Standard {
                        wood: standard.add(unlit(material(colors.wood))),
                        leaves: standard.add(unlit(crown_material(&colors.apply(&pool.params)))),
                    }
                }
            })
            .collect();
        pool.materials = materials;
    }
}

/// Give every [`PooledTree`] whose pool is ready its children: wood and leaves at
/// each level of detail, each drawn in its own band of distance, and the
/// impostor beyond them.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
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
    recolors: Res<PoolRecolors>,
    trees: Query<(Entity, &PooledTree, Option<&PooledLook>, Has<NotShadowCaster>), Without<PooledTreeSpawned>>,
    units: Res<FloraUnits>,
    mut warned: Local<bool>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let units = units.units_per_metre;
    make_pool_materials(&mut pools, &recolors, wind, &mut flora, &mut standard, &mut textures);
    // A tree whose pool is not ready yet is skipped, not counted: it must not
    // keep trees of a ready pool from getting their children.
    let mut spawned = 0;
    for (tree, pooled, look, quiet) in &trees {
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
            banded(0.0, lod1, units),
            banded(lod1, lod2, units),
            banded(
                lod2,
                if has_impostor {
                    impostor
                } else {
                    f32::INFINITY
                },
                units,
            ),
        ];
        let look = look.map_or(0, |look| look.0 as usize);
        let Some(materials) = pool.materials.get(look % pool.materials.len().max(1)) else {
            continue;
        };
        let mut spawn = |mesh: &Handle<Mesh>, leaf: bool, range: &(LodBand, VisibilityRange), bounds: Aabb| {
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
        if let Some(atlas) = &variant.impostor
            && let Some(material) = atlas.materials.get(look % atlas.materials.len().max(1))
        {
            // The quad turns about the trunk, so any yaw of it must be inside the box.
            let half = Vec3A::new(atlas.size.x / 2.0, atlas.size.y / 2.0, atlas.size.x / 2.0);
            commands.spawn((
                Mesh3d(atlas.quad.clone()),
                MeshMaterial3d(material.clone()),
                banded(impostor, f32::INFINITY, units),
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

/// Write `surface` into the pool's flora material `handle`, marking the asset modified only if
/// the colour, whether it is lit or the uniform differs: a modified material is re-prepared by
/// the renderer.
fn write_flora(assets: &mut Assets<FloraMaterial>, handle: &Handle<FloraMaterial>, surface: &Surface) {
    let Some(material) = assets.get(handle) else { return };
    if material.base.base_color == surface.color
        && material.base.unlit == surface.unlit
        && material.extension.flora == surface.uniform
    {
        return;
    }
    if let Some(mut material) = assets.get_mut(handle) {
        material.base.base_color = surface.color;
        material.base.unlit = surface.unlit;
        material.extension.flora = surface.uniform;
    }
}

/// [`write_flora`] for a plain material.
fn write_standard(assets: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, color: Color, unlit: bool) {
    let Some(material) = assets.get(handle) else { return };
    if material.base_color == color && material.unlit == unlit {
        return;
    }
    if let Some(mut material) = assets.get_mut(handle) {
        material.base_color = color;
        material.unlit = unlit;
    }
}

/// [`write_flora`] for an impostor's material: its tint, flora uniform and unlit flag.
pub(crate) fn write_impostor(
    assets: &mut Assets<ImpostorMaterial>,
    handle: &Handle<ImpostorMaterial>,
    params: &TreeParams,
    colors: &PoolColors,
) {
    let (flora, tint, unlit) = (
        FloraUniform::new(&colors.apply(params)),
        ImpostorTint::new(colors.wood, colors.leaves),
        f32::from(colors.unlit),
    );
    let Some(material) = assets.get(handle) else { return };
    if material.flora == flora && material.tint == tint && material.params.w == unlit {
        return;
    }
    if let Some(mut material) = assets.get_mut(handle) {
        material.flora = flora;
        material.tint = tint;
        material.params.w = unlit;
    }
}

/// Apply every [`RecolorPool`]: rebuild the look's surfaces from the pool's parameters in the new
/// colours and write them into the materials the trees already draw with, and into the look's
/// impostor material of every variant. Never takes the pool mutably: that would mark the asset
/// changed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn recolor_pools(
    mut messages: MessageReader<RecolorPool>,
    pools: Res<Assets<TreePool>>,
    mut recolors: ResMut<PoolRecolors>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut impostors: Option<ResMut<Assets<ImpostorMaterial>>>,
    mut textures: TextureLibrary,
    mut warned: Local<bool>,
) {
    for message in messages.read() {
        let Some(pool) = pools.get(message.pool) else { continue };
        let look = message.look as usize;
        if look >= pool.looks.len() {
            if !*warned {
                *warned = true;
                warn!(
                    "bevy_flora: RecolorPool for look {look} of a pool that has {} (TreePool::add_look adds one)",
                    pool.looks.len()
                );
            }
            continue;
        }
        recolors.0.insert((message.pool, message.look), message.colors);
        // A pool not drawn yet makes its materials in these colours when it first is.
        let Some(materials) = pool.materials.get(look) else { continue };
        match materials {
            PoolMaterials::Flora { wood, leaves } => {
                let (wood_surface, leaf_surface) = message.colors.surfaces(&pool.params, &mut textures);
                write_flora(&mut flora, wood, &wood_surface);
                write_flora(&mut flora, leaves, &leaf_surface);
            }
            PoolMaterials::Standard { wood, leaves } => {
                write_standard(&mut standard, wood, message.colors.wood, message.colors.unlit);
                write_standard(&mut standard, leaves, message.colors.leaves, message.colors.unlit);
            }
        }
        if let Some(assets) = impostors.as_deref_mut() {
            for variant in &pool.variants {
                if let Some(handle) = variant.impostor.as_ref().and_then(|atlas| atlas.materials.get(look)) {
                    write_impostor(assets, handle, &pool.params, &message.colors);
                }
            }
        }
    }
}

/// Dress a [`PooledTree`] whose [`PooledLook`] changed (or was taken off: look 0) in its new
/// look: its children take the matching material of the look, wood for wood, leaves for leaves,
/// the impostor's for the impostor.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync_pooled_looks(
    mut commands: Commands,
    pools: Res<Assets<TreePool>>,
    changed: Query<Entity, (With<PooledTreeSpawned>, Changed<PooledLook>)>,
    mut removed: RemovedComponents<PooledLook>,
    trees: Query<(&PooledTree, Option<&PooledLook>, &Children), With<PooledTreeSpawned>>,
    flora: Query<&MeshMaterial3d<FloraMaterial>>,
    standard: Query<&MeshMaterial3d<StandardMaterial>>,
    impostors: Query<&MeshMaterial3d<ImpostorMaterial>>,
) {
    let touched: Vec<Entity> = changed.iter().chain(removed.read()).collect();
    for tree in touched {
        let Ok((pooled, look, children)) = trees.get(tree) else { continue };
        let Some(pool) = pools.get(&pooled.pool) else { continue };
        let look = look.map_or(0, |look| look.0 as usize);
        let Some(now) = pool.materials.get(look % pool.materials.len().max(1)) else { continue };
        let variant = &pool.variants[pooled.variant as usize % pool.variants.len()];
        for child in children.iter() {
            if let Ok(current) = impostors.get(child) {
                let wanted = variant
                    .impostor
                    .as_ref()
                    .and_then(|atlas| atlas.materials.get(look % atlas.materials.len().max(1)));
                if let Some(wanted) = wanted
                    && current.0 != *wanted
                {
                    commands.entity(child).insert(MeshMaterial3d(wanted.clone()));
                }
            } else if let Ok(current) = flora.get(child) {
                // Which role the child's material plays in any look; the same role in this one.
                let leaf = pool.materials.iter().find_map(|m| match m {
                    PoolMaterials::Flora { wood, leaves } => (current.0 == *wood)
                        .then_some(false)
                        .or_else(|| (current.0 == *leaves).then_some(true)),
                    PoolMaterials::Standard { .. } => None,
                });
                if let (Some(leaf), PoolMaterials::Flora { wood, leaves }) = (leaf, now) {
                    let wanted = if leaf { leaves } else { wood };
                    if current.0 != *wanted {
                        commands.entity(child).insert(MeshMaterial3d(wanted.clone()));
                    }
                }
            } else if let Ok(current) = standard.get(child) {
                let leaf = pool.materials.iter().find_map(|m| match m {
                    PoolMaterials::Standard { wood, leaves } => (current.0 == *wood)
                        .then_some(false)
                        .or_else(|| (current.0 == *leaves).then_some(true)),
                    PoolMaterials::Flora { .. } => None,
                });
                if let (Some(leaf), PoolMaterials::Standard { wood, leaves }) = (leaf, now) {
                    let wanted = if leaf { leaves } else { wood };
                    if current.0 != *wanted {
                        commands.entity(child).insert(MeshMaterial3d(wanted.clone()));
                    }
                }
            }
        }
    }
}

/// The pool's messages and resources, and the asset type where the asset plugin is. Plugins may
/// be added in any order, so [`finish`] tries again.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.add_message::<RecolorPool>().init_resource::<PoolRecolors>();
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
    use crate::felling::FellTree;
    use crate::presets::TreePreset;
    use bevy::asset::AssetEvent;

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
        let Some(PoolMaterials::Flora { wood, leaves }) = pools.get(&pool).unwrap().materials.first() else {
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
        let (_, range) = banded(30.0, 80.0, 1.0);
        assert_eq!(range.start_margin, 29.0..31.0);
        assert_eq!(range.end_margin, 79.0..81.0);
        let (_, first) = banded(0.0, f32::INFINITY, 1.0);
        assert_eq!(first.start_margin, 0.0..0.0);
        assert!(first.end_margin.start.is_infinite());
    }

    #[test]
    fn bands_and_their_fades_are_metres_scaled_to_the_world() {
        // 4 world units to the metre: 30 m is 120, its 2 m fade 8 wide, infinity stays infinite.
        let (_, range) = banded(30.0, 80.0, 4.0);
        assert_eq!(range.start_margin, 116.0..124.0);
        assert_eq!(range.end_margin, 316.0..324.0);
        let (_, first) = banded(0.0, f32::INFINITY, 4.0);
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

    #[test]
    fn levels_follow_the_lod_scale_and_an_orthographic_cameras_zoom() {
        use bevy::camera::OrthographicProjection;
        use bevy::math::Rect;
        let mut app = app();
        let pool = make_pool(&mut app, 1);
        let pooled = app.world_mut().spawn(PooledTree { pool, variant: 0 }).id();
        let oak = crate::presets::TreePreset::Oak.params();
        let grown = app.world_mut().spawn((ProceduralTree(oak), crate::lod::TreeLod::default())).id();
        app.update();
        app.update();
        // Every banded child of both trees is drawn where its band says, times `scale`: the
        // middles of its fades at the band's ends times the scale.
        let drawn_at = |app: &App, scale: f32| {
            let world = app.world();
            let mut banded = 0;
            for tree in [pooled, grown] {
                for child in world.get::<Children>(tree).unwrap().iter() {
                    let (Some(band), Some(range)) = (world.get::<LodBand>(child), world.get::<VisibilityRange>(child))
                    else {
                        continue;
                    };
                    let middle = |r: &std::ops::Range<f32>| (r.start + r.end) / 2.0;
                    for (end, fade) in [(band.start, &range.start_margin), (band.end, &range.end_margin)] {
                        if end > 0.0 && end.is_finite() {
                            assert!((middle(fade) - end * scale).abs() < 1.0e-3 * end, "{end} m at {scale}: {fade:?}");
                            assert!(((fade.end - fade.start) - FADE * scale).abs() < 1.0e-4, "the fade at {scale}");
                        }
                    }
                    banded += 1;
                }
            }
            assert!(banded >= 12, "{banded} banded children");
        };
        drawn_at(&app, 1.0);

        // A lower quality setting: every tree hands over at half the distance, and so does a
        // tree rebuilt meanwhile.
        app.world_mut().resource_mut::<LodScale>().0 = 0.5;
        app.update();
        drawn_at(&app, 0.5);
        app.world_mut().get_mut::<ProceduralTree>(grown).unwrap().0.seed = 7;
        app.update();
        app.update();
        drawn_at(&app, 0.5);
        app.world_mut().resource_mut::<LodScale>().0 = 1.0;

        // An orthographic camera 50 m up and 50 m back, looking 45° down at the origin: 70.7 m
        // from where its view meets the ground. A 20 m tall view is what a 45° perspective camera
        // sees from 24.1 m: the trees are drawn as if 2.93 times nearer.
        let view = |height: f32| {
            let area = Rect::new(-height, -height / 2.0, height, height / 2.0);
            Projection::Orthographic(OrthographicProjection { area, ..OrthographicProjection::default_3d() })
        };
        let eye = Transform::from_xyz(0.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y);
        let camera = app.world_mut().spawn((Camera3d::default(), view(20.0), GlobalTransform::from(eye))).id();
        app.update();
        let near = 50.0 * 2f32.sqrt() / (20.0 / (2.0 * (std::f32::consts::FRAC_PI_8).tan()));
        assert!((app.world().resource::<ZoomScale>().0 - near).abs() < 1.0e-3);
        drawn_at(&app, near);
        // Zoomed out to a 200 m view: ten times as far.
        app.world_mut().entity_mut(camera).insert(view(200.0));
        app.update();
        drawn_at(&app, near / 10.0);
        // A perspective camera draws them at their own distances again.
        app.world_mut().entity_mut(camera).insert(Projection::default());
        app.update();
        drawn_at(&app, 1.0);
    }

    const CRIMSON: PoolColors =
        PoolColors { wood: Color::srgb(0.30, 0.12, 0.05), leaves: Color::srgb(0.85, 0.25, 0.10), unlit: false };
    const FLAT_BLUE: PoolColors =
        PoolColors { wood: Color::srgb(0.10, 0.20, 0.60), leaves: Color::srgb(0.30, 0.55, 0.95), unlit: true };

    /// A pool of `params` with `looks` after look 0, added to the assets.
    fn pool_of(app: &mut App, params: &TreeParams, variants: u32, looks: &[PoolColors]) -> Handle<TreePool> {
        let mut pool = app.world_mut().resource_scope(|_, mut meshes: Mut<Assets<Mesh>>| {
            build_pool(params, variants, f32::INFINITY, &LodSettings::default(), &mut meshes)
        });
        for (k, look) in looks.iter().enumerate() {
            assert_eq!(pool.add_look(*look), k as u32 + 1);
        }
        app.world_mut().resource_mut::<Assets<TreePool>>().add(pool)
    }

    /// The wood and the leaf material of look `look` of `pool`, as they are now.
    fn dressed(app: &App, pool: &Handle<TreePool>, look: usize) -> (FloraMaterial, FloraMaterial) {
        let pools = app.world().resource::<Assets<TreePool>>();
        let Some(PoolMaterials::Flora { wood, leaves }) = pools.get(pool).unwrap().materials.get(look) else {
            panic!("look {look} has no flora materials");
        };
        let flora = app.world().resource::<Assets<FloraMaterial>>();
        (flora.get(wood).unwrap().clone(), flora.get(leaves).unwrap().clone())
    }

    fn send<M: Message>(app: &mut App, message: M) {
        app.world_mut().resource_mut::<bevy::ecs::message::Messages<M>>().write(message);
    }

    #[test]
    fn a_recoloured_pools_materials_are_those_of_a_pool_built_with_the_new_colours() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        // Images, so a bark scan loads and its average colour is divided out of the colour.
        app.init_asset::<bevy::image::Image>();
        let cherry = TreePreset::Cherry.params();
        let pool = pool_of(&mut app, &cherry, 1, &[]);
        let tree = app.world_mut().spawn(PooledTree { pool: pool.clone(), variant: 0 }).id();
        app.update();
        app.update();
        let before = dressed(&app, &pool, 0);
        let drawn_before = drawn_by(&app, &[tree]);

        let mut events = app.world().resource::<bevy::ecs::message::Messages<AssetEvent<TreePool>>>().get_cursor_current();
        send(&mut app, RecolorPool { pool: pool.id(), look: 0, colors: CRIMSON });
        app.update();
        let after = dressed(&app, &pool, 0);

        let mut painted = cherry.clone();
        painted.trunk_color = CRIMSON.wood;
        painted.foliage.color = CRIMSON.leaves;
        let built = pool_of(&mut app, &painted, 1, &[]);
        app.world_mut().spawn(PooledTree { pool: built.clone(), variant: 0 });
        app.update();
        let reference = dressed(&app, &built, 0);

        for (recoloured, built) in [(&after.0, &reference.0), (&after.1, &reference.1)] {
            assert_eq!(recoloured.base.base_color, built.base.base_color);
            assert_eq!(recoloured.base.unlit, built.base.unlit);
            assert_eq!(recoloured.extension.flora, built.extension.flora);
            assert_eq!(recoloured.extension.detail_color, built.extension.detail_color, "the scan stays");
        }
        assert!(after.0.extension.detail_color.is_some(), "the wood wears a bark scan, whose average the colour was divided by");
        assert_ne!(before.0.base.base_color, after.0.base.base_color);
        assert_ne!(before.1.extension.flora.autumn, after.1.extension.flora.autumn, "the autumn ratio follows the leaf colour");
        assert_ne!(before.1.extension.flora.blossom, after.1.extension.flora.blossom, "the blossom ratio does too");
        // The same materials and meshes are drawn, and the pool asset itself was not modified.
        assert_eq!(drawn_by(&app, &[tree]), drawn_before);
        let modified = events
            .read(app.world().resource::<bevy::ecs::message::Messages<AssetEvent<TreePool>>>())
            .filter(|event| matches!(event, AssetEvent::Modified { id } if *id == pool.id()))
            .count();
        assert_eq!(modified, 0, "a recolouring must not touch the pool: every tree would be re-indexed");
    }

    #[test]
    fn a_pool_recoloured_before_it_is_drawn_is_drawn_in_the_new_colours() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        let oak = TreePreset::Oak.params();
        let pool = pool_of(&mut app, &oak, 1, &[CRIMSON]);
        send(&mut app, RecolorPool { pool: pool.id(), look: 1, colors: FLAT_BLUE });
        app.update();
        app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, PooledLook(1)));
        app.update();
        let (wood, leaves) = dressed(&app, &pool, 1);
        assert_eq!((wood.base.base_color, leaves.base.base_color), (FLAT_BLUE.wood, FLAT_BLUE.leaves));
        assert!(wood.base.unlit && leaves.base.unlit);
        let (wood, _) = dressed(&app, &pool, 0);
        assert_ne!(wood.base.base_color, FLAT_BLUE.wood, "the other looks are as they were");
    }

    #[test]
    fn three_looks_draw_one_set_of_meshes_in_a_pair_of_materials_each() {
        let mut app = app();
        let pool = pool_of(&mut app, &TreePreset::Oak.params(), 2, &[CRIMSON, FLAT_BLUE]);
        let looks: Vec<Vec<Entity>> = (0..3u32)
            .map(|look| {
                (0..20u32)
                    .map(|i| {
                        let tree = PooledTree { pool: pool.clone(), variant: i };
                        app.world_mut().spawn((tree, PooledLook(look), Transform::from_xyz(i as f32 * 8.0, 0.0, 0.0))).id()
                    })
                    .collect()
            })
            .collect();
        app.update();

        let every: Vec<Entity> = looks.concat();
        let (meshes, materials) = drawn_by(&app, &every);
        assert_eq!(meshes.len(), 12, "three levels of wood and leaves of two variants, whatever the look");
        assert_eq!(materials.len(), 6, "a pair of materials a look");
        let by_look: Vec<_> = looks.iter().map(|trees| drawn_by(&app, trees)).collect();
        for (meshes_of_look, materials_of_look) in &by_look {
            assert_eq!(meshes_of_look, &meshes);
            assert_eq!(materials_of_look.len(), 2);
        }
        assert!(by_look[0].1.is_disjoint(&by_look[1].1) && by_look[1].1.is_disjoint(&by_look[2].1));

        // The look is taken modulo the looks, and a tree that changes look changes materials only.
        let odd = app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, PooledLook(4))).id();
        let loose = looks[0][0];
        app.update();
        assert_eq!(drawn_by(&app, &[odd]).1, by_look[1].1, "4 is 1 of 3 looks");
        let children_before: Vec<Entity> = app.world().get::<Children>(loose).unwrap().iter().collect();
        app.world_mut().entity_mut(loose).insert(PooledLook(2));
        app.update();
        assert_eq!(drawn_by(&app, &[loose]).1, by_look[2].1);
        assert_eq!(app.world().get::<Children>(loose).unwrap().iter().collect::<Vec<_>>(), children_before);
        app.world_mut().entity_mut(loose).remove::<PooledLook>();
        app.update();
        assert_eq!(drawn_by(&app, &[loose]).1, by_look[0].1, "no look is look 0");

        // The plain materials wear the colours of their look.
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let wood_of = |look: usize| {
            let Some(PoolMaterials::Standard { wood, .. }) = app.world().resource::<Assets<TreePool>>().get(&pool).unwrap().materials.get(look).cloned()
            else {
                panic!("plain materials")
            };
            materials.get(&wood).unwrap().base_color
        };
        assert_eq!((wood_of(1), wood_of(2)), (CRIMSON.wood, FLAT_BLUE.wood));
    }

    #[test]
    fn an_unlit_look_keeps_its_exact_colour_and_the_others_stay_lit() {
        let mut oak = TreePreset::Oak.params();
        oak.bark.scan = None;
        oak.foliage.scan = None;
        // Flora materials (the shaders skip the lights on the base material's flag), then plain ones.
        for flora_support in [true, false] {
            let mut app = app();
            if flora_support {
                app.insert_resource(FloraMaterialSupport);
            }
            let pool = pool_of(&mut app, &oak, 1, &[FLAT_BLUE]);
            app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, PooledLook(1)));
            app.update();
            let pools = app.world().resource::<Assets<TreePool>>();
            let looks = &pools.get(&pool).unwrap().materials;
            assert_eq!(looks.len(), 2);
            let (lit, unlit): (Vec<_>, Vec<_>) = if flora_support {
                let flora = app.world().resource::<Assets<FloraMaterial>>();
                looks
                    .iter()
                    .map(|look| {
                        let PoolMaterials::Flora { wood, leaves } = look else { panic!("flora materials") };
                        let (wood, leaves) = (flora.get(wood).unwrap(), flora.get(leaves).unwrap());
                        ((wood.base.unlit, leaves.base.unlit), (wood.base.base_color, leaves.base.base_color))
                    })
                    .unzip()
            } else {
                let plain = app.world().resource::<Assets<StandardMaterial>>();
                looks
                    .iter()
                    .map(|look| {
                        let PoolMaterials::Standard { wood, leaves } = look else { panic!("plain materials") };
                        let (wood, leaves) = (plain.get(wood).unwrap(), plain.get(leaves).unwrap());
                        ((wood.unlit, leaves.unlit), (wood.base_color, leaves.base_color))
                    })
                    .unzip()
            };
            assert_eq!(lit, [(false, false), (true, true)], "flora support: {flora_support}");
            assert_eq!(unlit[1], (FLAT_BLUE.wood, FLAT_BLUE.leaves), "exactly the colours it was given");
        }
    }

    #[test]
    fn a_felled_pooled_tree_keeps_its_look_and_a_recoloured_look() {
        let mut app = app();
        let pool = pool_of(&mut app, &TreePreset::Oak.params(), 3, &[CRIMSON, FLAT_BLUE]);
        let spawn = |app: &mut App, look: u32| {
            app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 2 }, PooledLook(look), Transform::default())).id()
        };
        let (plain, red, blue) = (spawn(&mut app, 0), spawn(&mut app, 1), spawn(&mut app, 2));
        app.update();
        let expected = app.world().resource::<Assets<TreePool>>().get(&pool).unwrap().tree_params_for(2, 1);
        assert_eq!((expected.trunk_color, expected.foliage.color), (CRIMSON.wood, CRIMSON.leaves));
        for tree in [plain, red, blue] {
            app.world_mut().entity_mut(tree).insert(FellTree::default());
        }
        app.update();
        app.update();
        let colours = |app: &App, tree: Entity| {
            let params = &app.world().get::<ProceduralTree>(tree).expect("the pooled tree became a tree").0;
            (params.trunk_color, params.foliage.color)
        };
        assert_eq!(colours(&app, plain), (TreePreset::Oak.params().trunk_color, TreePreset::Oak.params().foliage.color));
        assert_eq!(colours(&app, red), (CRIMSON.wood, CRIMSON.leaves));
        assert_eq!(colours(&app, blue), (FLAT_BLUE.wood, FLAT_BLUE.leaves));
        assert_eq!(app.world().get::<ProceduralTree>(red).unwrap().0, expected);
        assert!(app.world().get::<PooledLook>(red).is_none() && app.world().get::<PooledTree>(red).is_none());
        // Drawn in them (the trunk's plain material), the unlit look unlit.
        let trunk_material = |app: &App, tree: Entity| {
            let trunk = app.world().get::<crate::TreeRender>(tree).unwrap().trunk_entity();
            let handle = &app.world().get::<MeshMaterial3d<StandardMaterial>>(trunk).unwrap().0;
            let material = app.world().resource::<Assets<StandardMaterial>>().get(handle).unwrap();
            (material.base_color, material.unlit)
        };
        assert_eq!(trunk_material(&app, red), (CRIMSON.wood, false));
        assert_eq!(trunk_material(&app, blue), (FLAT_BLUE.wood, true));

        // A look recoloured since the pool was built is the look a tree is felled in.
        let violet = PoolColors { wood: Color::srgb(0.4, 0.1, 0.5), leaves: Color::srgb(0.7, 0.2, 0.9), unlit: false };
        send(&mut app, RecolorPool { pool: pool.id(), look: 1, colors: violet });
        app.update();
        let late = spawn(&mut app, 1);
        app.update();
        app.world_mut().entity_mut(late).insert(FellTree::default());
        app.update();
        app.update();
        assert_eq!(colours(&app, late), (violet.wood, violet.leaves));
    }

    #[test]
    fn tree_params_for_paints_a_look_and_takes_it_modulo_the_looks() {
        let mut app = app();
        let pool = pool_of(&mut app, &TreePreset::Pine.params(), 2, &[CRIMSON]);
        let pools = app.world().resource::<Assets<TreePool>>();
        let pool = pools.get(&pool).unwrap();
        assert_eq!(pool.tree_params_for(1, 0), pool.tree_params(1));
        assert_eq!(pool.tree_params_for(1, 2), pool.tree_params_for(1, 0), "look 2 of 2 is look 0");
        let painted = pool.tree_params_for(3, 1);
        assert_eq!((painted.trunk_color, painted.foliage.color), (CRIMSON.wood, CRIMSON.leaves));
        assert_eq!(painted.seed, pool.tree_params(3).seed, "the variant's seed stays");
        assert_eq!(PoolColors::of(&pool.params), pool.looks[0]);
    }

    #[test]
    fn an_impostor_material_takes_twice_the_colour_as_its_tint_and_is_written_only_when_it_changes() {
        use bevy::math::Vec4;
        let tint = ImpostorTint::new(Color::srgb(0.2, 0.4, 0.6), Color::srgb(1.0, 0.5, 0.0));
        let (wood, leaf) = (Color::srgb(0.2, 0.4, 0.6).to_linear(), Color::srgb(1.0, 0.5, 0.0).to_linear());
        assert_eq!(tint.wood, Vec4::new(2.0 * wood.red, 2.0 * wood.green, 2.0 * wood.blue, 1.0));
        assert_eq!(tint.leaf, Vec4::new(2.0 * leaf.red, 2.0 * leaf.green, 2.0 * leaf.blue, 1.0));

        let params = TreePreset::Maple.params();
        let mut materials = Assets::<ImpostorMaterial>::default();
        let handle = materials.add(ImpostorMaterial::new(
            Handle::default(),
            Handle::default(),
            8,
            params.style,
            FloraUniform::new(&params),
            &FloraWindBuffer(Handle::default()),
            ImpostorTint::new(params.trunk_color, params.foliage.color),
            false,
        ));
        write_impostor(&mut materials, &handle, &params, &FLAT_BLUE);
        let written = materials.get(&handle).unwrap();
        assert_eq!(written.tint, ImpostorTint::new(FLAT_BLUE.wood, FLAT_BLUE.leaves));
        assert_eq!(written.params.w, 1.0, "unlit");
        let painted = FLAT_BLUE.apply(&params);
        assert_eq!(written.flora, FloraUniform::new(&painted), "the autumn ratio is of the new leaf colour");
        assert_eq!((written.params.x, written.params.y), (8.0, params.style as u32 as f32), "views and style stay");
    }

    #[test]
    fn a_hull_crown_pool_recolours_and_keeps_its_hull_flag_in_every_look() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        let hull = crate::crown_hull::hull_spruce();
        let pool = pool_of(&mut app, &hull, 2, &[CRIMSON, FLAT_BLUE]);
        for look in 0..3 {
            app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, PooledLook(look)));
        }
        app.update();
        app.update();
        for look in 0..3 {
            let (_, leaves) = dressed(&app, &pool, look);
            assert!(leaves.extension.flora.tree.y > 0.5, "look {look} lost the hull flag");
        }
        let violet = PoolColors { wood: Color::srgb(0.4, 0.1, 0.5), leaves: Color::srgb(0.7, 0.2, 0.9), unlit: false };
        send(&mut app, RecolorPool { pool: pool.id(), look: 1, colors: violet });
        app.update();
        let (wood, leaves) = dressed(&app, &pool, 1);
        assert_eq!((wood.base.base_color, leaves.base.base_color), (violet.wood, violet.leaves));
        assert!(leaves.extension.flora.tree.y > 0.5, "a recoloured hull stays a hull");
        assert_eq!(leaves.extension.flora, FloraUniform::new(&violet.apply(&hull)));
        // The other looks are untouched.
        let (_, flat) = dressed(&app, &pool, 2);
        assert_eq!(flat.base.base_color, FLAT_BLUE.leaves);
        assert!(flat.base.unlit);
    }

    #[test]
    fn a_world_in_feet_picks_the_levels_a_world_in_metres_does() {
        use bevy::camera::OrthographicProjection;
        use bevy::math::Rect;
        const FEET: f32 = 3.2808;
        let mut app = app();
        app.insert_resource(FloraUnits { units_per_metre: FEET });
        let pool = make_pool(&mut app, 1);
        let pooled = app.world_mut().spawn(PooledTree { pool, variant: 0 }).id();
        let oak = crate::presets::TreePreset::Oak.params();
        let grown = app.world_mut().spawn((ProceduralTree(oak), crate::lod::TreeLod::default())).id();
        app.update();
        app.update();
        // Every banded child is drawn where its band, in metres, says, times `scale` metres'
        // worth of world units; the fade is 2 m wide.
        let drawn_at = |app: &App, scale: f32| {
            let world = app.world();
            let mut banded = 0;
            for tree in [pooled, grown] {
                for child in world.get::<Children>(tree).unwrap().iter() {
                    let (Some(band), Some(range)) = (world.get::<LodBand>(child), world.get::<VisibilityRange>(child))
                    else {
                        continue;
                    };
                    let middle = |r: &std::ops::Range<f32>| (r.start + r.end) / 2.0;
                    for (end, fade) in [(band.start, &range.start_margin), (band.end, &range.end_margin)] {
                        if end > 0.0 && end.is_finite() {
                            let want = end * scale * FEET;
                            assert!((middle(fade) - want).abs() < 1.0e-3 * want, "{end} m at {scale}: {fade:?}");
                            assert!(((fade.end - fade.start) - FADE * scale * FEET).abs() < 1.0e-3, "the fade at {scale}");
                        }
                    }
                    banded += 1;
                }
            }
            assert!(banded >= 12, "{banded} banded children");
        };
        drawn_at(&app, 1.0);

        // The camera of the metre test, in feet: 50 m up and back, a 20 m view.
        let view = |height: f32| {
            let height = height * FEET;
            let area = Rect::new(-height, -height / 2.0, height, height / 2.0);
            Projection::Orthographic(OrthographicProjection { area, ..OrthographicProjection::default_3d() })
        };
        let eye = Transform::from_xyz(0.0, 50.0 * FEET, 50.0 * FEET).looking_at(Vec3::ZERO, Vec3::Y);
        let camera = app.world_mut().spawn((Camera3d::default(), view(20.0), GlobalTransform::from(eye))).id();
        app.update();
        let near = 50.0 * 2f32.sqrt() / (20.0 / (2.0 * (std::f32::consts::FRAC_PI_8).tan()));
        assert!((app.world().resource::<ZoomScale>().0 - near).abs() < 1.0e-3, "the zoom is the metre world's");
        drawn_at(&app, near);
        app.world_mut().entity_mut(camera).insert(view(200.0));
        app.update();
        drawn_at(&app, near / 10.0);
        app.world_mut().entity_mut(camera).insert(Projection::default());
        app.update();
        drawn_at(&app, 1.0);
    }
}
