//! `bevy_flora`: parametric procedural trees that grow over time, for Bevy.
//!
//! Pipeline: [`TreeParams`] → [`generate_skeleton`] → [`Skeleton::aged`] →
//! wood mesh ([`mesh::branch::skeleton_mesh`]) + leaves
//! ([`foliage::collect_leaf_points`] / [`foliage::foliage_mesh`]) → [`MeshData`]
//! → Bevy `Mesh`. [`build_tree`] runs the whole thing; [`FloraPlugin`] drives it
//! from the [`ProceduralTree`] / [`TreeAge`] / [`Growing`] components.
//!
//! # Growing a tree in a Bevy app
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreePreset};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, FloraPlugin))
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn((
//!         ProceduralTree(TreePreset::Oak.params()),
//!         TreeAge(0.0),
//!         Growing::default(),
//!     ));
//! }
//! ```
//!
//! # Growing a forest: [`GpuGrowth`]
//!
//! Rebuilding a growing tree's meshes costs the CPU 0.1-0.4 ms per tree per
//! frame. Add [`GpuGrowth`] and the tree is built once, at maturity, and grown in
//! the vertex shader instead (a few microseconds of CPU per tree per frame, 0.004
//! ms in the 0.4.0 measurements):
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraPlugin, GpuGrowth, Growing, ProceduralTree, TreeAge, TreePreset};
//!
//! fn setup(mut commands: Commands) {
//!     for i in 0..200 {
//!         commands.spawn((
//!             ProceduralTree(TreePreset::Oak.params()),
//!             TreeAge(0.0),
//!             Growing::default(),
//!             GpuGrowth,
//!             Transform::from_xyz((i % 20) as f32 * 8.0, 0.0, (i / 20) as f32 * 8.0),
//!         ));
//!     }
//! }
//! ```
//!
//! It needs a device with storage buffers in the vertex stage (everything but
//! WebGL2; see [`GpuGrowthSupport`]); elsewhere the trees grow on the CPU.
//!
//! # Planting without a hitch: [`BuildInBackground`]
//!
//! A tree is built in the frame it is spawned, and that frame waits for it (200 oaks: about
//! 23 ms on an Apple M4 Pro). With [`BuildInBackground`] it is built on Bevy's async compute
//! pool instead and appears a few frames later, the frame its build is done; the frame it was
//! spawned in costs well under a millisecond.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{BuildInBackground, ProceduralTree, TreePreset};
//!
//! fn plant(mut commands: Commands) {
//!     for i in 0..200 {
//!         let at = Vec3::new((i % 20) as f32 * 8.0, 0.0, (i / 20) as f32 * 8.0);
//!         commands.spawn((ProceduralTree(TreePreset::Oak.params()), BuildInBackground, Transform::from_translation(at)));
//!     }
//! }
//! ```
//!
//! A forest's pools build off the main thread too: [`BackgroundPools`] hands back a pool's handle
//! at once and adds the pool a few frames later (an eight-variant pool takes 0.5 to 14 ms on all
//! of an Apple M4 Pro's cores, longer on fewer cores and about 0.3 s for every preset on wasm).
//!
//! # Distant trees for less: [`TreeLod`]
//!
//! A [`ProceduralTree`] is drawn in full at every distance. With [`TreeLod`] it is drawn as a
//! pool's trees are: in full up close, then at the two coarser levels of [`build_lods`] (from 30
//! and 80 m by default), cross-fading. 1 000 individual oaks draw at 6.5 ms a frame with it and
//! 25 ms without (1080p, Apple M4 Pro). They stay individual trees, which grow, change and fall;
//! a growing one rebuilds its coarse levels with every new age, which costs the CPU about 40%
//! more.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{ProceduralTree, TreeLod, TreePreset};
//!
//! fn plant(mut commands: Commands) {
//!     commands.spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default(), Transform::default()));
//! }
//! ```
//!
//! # Drawing a forest: [`TreePool`]
//!
//! Every [`ProceduralTree`] has meshes of its own, which a GPU draws a few thousand
//! of. For a forest build a [`TreePool`] (a few seed variants of one tree, each at
//! three levels of detail from [`build_lods`]) and place [`PooledTree`]s from it:
//! trees that share a mesh are drawn in one instanced call, the distance to the
//! camera picks the level, and beyond the last one a tree is a baked camera-facing
//! quad ([`ImpostorAtlas`]), shaded by the scene's own lights. 100 000 oaks draw at
//! 7.6 ms a frame (1080p, Apple M4 Pro).
//! Pooled trees are static; add [`FloraPlugin`] after `DefaultPlugins` to get the
//! impostors. See [`pool`] for an example.
//!
//! [`Scatter`] says where the trees of a forest stand: trunks at least a spacing apart and
//! otherwise irregular (Poisson-disc sampling), species in groves with bending borders, every
//! tree its own yaw, size and variant, and a density per spot for clearings and roads. It is
//! pure CPU and deterministic per seed; see [`scatter`].
//!
//! On terrain, [`FloraGround`] is the height of your ground: [`ScatterPoint::transform_on`] (or
//! [`FloraGround::footing`] with [`TreeParams::base_radius`]) stands a tree where its trunk meets
//! a slope all round instead of floating on its downhill side, and as a resource it is what
//! felled trees are cut above and land on; see [`ground`].
//!
//! # A world that is not in metres: [`FloraUnits`]
//!
//! Trees are built in metres and a game scales them to its world with a `Transform`. If the world's
//! unit is not the metre (a foot, say), insert [`FloraUnits`] with the number of world units to a
//! metre, and everything the crate measures in the world follows it: how fast a tree falls and a
//! leaf drifts, how wide a gust is, how far a fire reaches, the pusher's size, the see-through
//! radius and the distances the levels of detail change at. Fields documented as metres are
//! scaled by it; positions and sizes that address the world are in world units; see [`units`].
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraUnits;
//!
//! fn setup(mut commands: Commands) {
//!     commands.insert_resource(FloraUnits { units_per_metre: 3.2808 });
//! }
//! ```
//!
//! # Leaf shading and wind
//!
//! The plugin draws trees with [`FloraMaterial`]: a darker, glossier upper side of
//! every leaf, a paler lower side, veins and light through a leaf lit from behind
//! ([`LeafShading`]), and wind. The [`FloraWind`] resource sets the weather (a
//! [`GustFront`] sweeps a band of stronger wind across the world,
//! [`FloraWind::launch_gust`]) and [`TreeParams::wind`] ([`WindResponse`]) how each
//! species answers it.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraWind, GustFront};
//!
//! // Call this once, when the gust begins (a system run on an event or a timer), not
//! // every frame: launching restarts the front at its origin.
//! fn start_storm(mut wind: ResMut<FloraWind>, time: Res<Time>) {
//!     wind.strength = 0.6;
//!     wind.launch_gust(GustFront { speed: 20.0, ..default() }, &time);
//! }
//! ```
//!
//! # Bark
//!
//! Wood wears a bundled CC0 bark scan ([`BarkScan`], chosen per species by the presets in
//! [`TreeParams::bark`]). The scan is laid out in metres on the mature tree, so a growing tree's
//! bark never slides, and [`BarkParams`] fades it in as a branch thickens: smooth wood below
//! `smooth_below` metres of radius, the whole scan above `fissured_above`. The scan's average
//! colour is divided out of the base colour, so a textured tree keeps its preset colour. The
//! images are compiled in by the default `textures` feature (about 3.6 MB with the leaf scans);
//! without it every scan is absent and wood is plain colour.
//!
//! # Leaf scans and grown veins
//!
//! A [`FoliageParams::scan`] ([`LeafScan`]) cuts leaf cards to the real outline of a bundled leaf
//! scan (one of four leaves of an atlas, by a hash of the leaf's own random draws) and the leaf
//! shader wears the atlas's colour and normal map on them. Leaves without a scan keep the
//! procedural outline and get veins grown by space colonisation ([`venation`]), drawn from one
//! small image every such leaf shares. Both need the `textures` feature only for the scans: the
//! veins are grown at run time.
//!
//! # Low-poly conifers: hull crowns and prism trunks
//!
//! [`TreeParams::crown`] ([`CrownStyle`]) can draw the crown as one faceted hull instead of leaf
//! cards: `CrownStyle::Hull { tiers, sides, lift, jitter, trunk_only }` wraps a stack of cones round the leaf
//! cloud ([`crown_hull`]), `tiers * sides * 2` flat triangles that read as a few big planes.
//! [`TreeParams::trunk_style`] ([`TrunkStyle`]) meshes the wood as flat-shaded prisms. Both work
//! through every way a tree is drawn (pools, levels of detail, impostors, felling, fire, snow) but
//! `GpuGrowth`, which builds such a tree on the CPU ([`TreeParams::gpu_growable`]).
//!
//! # Looks: natural, painterly, toon
//!
//! [`TreeParams::style`] ([`RenderStyle`]) changes how a tree is lit, not its shape. `Painterly`
//! gives every leaf the normal that points out of the centre of its own clump (the leaves of a
//! primary branch, the leader or a split) rather than out of the crown's, so a crown reads as
//! rounded masses with a lit and a shaded side each. `Toon` keeps those normals and cuts the
//! light of every directional light into three bands, with an ink rim toward the silhouette. The
//! pools, the impostors and `GpuGrowth` draw them all.
//!
//! # Seasons
//!
//! The [`FloraSeason`] resource turns deciduous trees ([`FoliageParams::autumn`], set on the
//! oak, willow, birch, bonsai, maple, poplar, baobab, Japanese maple, cherry and apple presets) to
//! their autumn colour, sheds their leaves one leaf at a time (evergreens keep theirs), lays snow
//! on every tree, on what faces up, and brings flowering trees ([`FoliageParams::blossom`], the
//! cherry and the apple) into blossom, one flower at a time. A fruiting tree's fruit
//! ([`FoliageParams::fruit`], the apple's) hangs below some of its leaves all summer and drops in
//! the first part of the leaf fall. It shares the wind's buffer, so a season that changes every
//! frame costs no material change and nothing per tree. As the leaf fall rises, the leaves it
//! sheds from trees near a camera fall to the ground ([`FallingLeaves`]).
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraSeason;
//!
//! // A year every two minutes.
//! fn seasons(time: Res<Time>, mut season: ResMut<FloraSeason>) {
//!     *season = FloraSeason::of_year(time.elapsed_secs() / 120.0);
//! }
//! ```
//!
//! [`FloraWetness`] is how wet the rain has made the trees, 0 (dry) to 1 (soaked): wet bark
//! darkens and turns glossy and the leaves' green grows richer, on every way a tree is drawn, for
//! one value written to the same buffer.
//!
//! # Trees that react
//!
//! [`TreeShakes`] swings the trees near a point (an axe on a trunk, a character pushing through,
//! a blast): [`TreeShakes::shake`] a [`TreeShake`], and every tree within its radius swings away
//! and back, dying away, while its leaves rustle, on every way a tree is drawn. Like the wind it
//! lives in the shared buffer: no material changes, and nothing per tree while nothing shakes.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{TreeShake, TreeShakes};
//!
//! // An axe on the tree standing at the origin.
//! fn chop(mut shakes: ResMut<TreeShakes>, time: Res<Time>) {
//!     shakes.shake(TreeShake { at: Vec2::ZERO, radius: 1.0, strength: 2.0, ..default() }, &time);
//! }
//! ```
//!
//! A shake knocks leaves off the trees it reaches, and a felled tree sheds some as it lands: they
//! fall, tumbling and swinging, drift with the wind, lie flat on the ground for a while and shrink
//! away, all on the GPU, in the colours the tree's leaves wear this season. [`FallingLeaves`] sets
//! how many; [`DropLeaves`] drops some from any tree; see [`falling`].
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::DropLeaves;
//!
//! // A bird lands in the tree.
//! fn bird_lands(mut drops: MessageWriter<DropLeaves>, tree: Entity) {
//!     drops.write(DropLeaves { tree, count: 6 });
//! }
//! ```
//!
//! Insert [`FellTree`] on a tree and it is cut: a [`TreeStump`] stays, and the tree falls about
//! the hinge of the cut, lands on its limbs ([`TreeLanded`]) and lies still ([`Felled`]). Pooled
//! and GPU-grown trees can be felled too, and [`BuckTree`] then cuts a felled tree's trunk into
//! [`TreeLog`]s lying on the ground; the [`felling`] module has the details.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::FellTree;
//!
//! // Fell the tree toward +x.
//! fn fell(mut commands: Commands, tree: Entity) {
//!     commands.entity(tree).insert(FellTree { direction: Vec2::X, ..default() });
//! }
//! ```
//!
//! Before the saw, the axe: a [`TreeNotch`] cuts a wedge out of a standing trunk, deeper with each
//! change of its `depth`, and a notched tree is felled at the notch's floor.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FellTree, TreeNotch};
//!
//! // One blow of the axe on the tree's +x side; the fifth fells it that way.
//! fn chop(mut commands: Commands, mut notches: Query<&mut TreeNotch>, tree: Entity) {
//!     let Ok(mut notch) = notches.get_mut(tree) else {
//!         commands.entity(tree).insert(TreeNotch { direction: Vec2::X, depth: 0.06, ..default() });
//!         return;
//!     };
//!     notch.depth += 0.06;
//!     if notch.depth >= 0.29 {
//!         commands.entity(tree).insert(FellTree { direction: notch.direction, ..default() });
//!     }
//! }
//! ```
//!
//! # Picking fruit
//!
//! A fruiting tree ([`FoliageParams::fruit`], the apple) drawn as a [`ProceduralTree`] keeps a
//! [`TreeFruit`]: [`TreeFruit::nearest`] finds the fruit nearest a hand and [`TreeFruit::pick`]
//! takes it off the tree until the next blossom. A shake knocks some of a tree's fruit loose to
//! fall and lie on the ground as [`FallenFruit`] entities, for the game to pick up
//! ([`FallingFruit`], [`FruitFell`]). See [`harvest`].
//!
//! # Fire
//!
//! A [`FloraFire`] is a grid of cells over the game's world, each with how charred it is and how
//! hot it burns, that the plugin spreads a fire through from where the game lights it
//! ([`FloraFire::ignite`]): through the trees, faster downwind, stopped where nothing burns. The
//! trees it reaches scorch and lose their leaves, and their wood chars from the ground up with
//! glowing seams, on every way a tree is drawn; [`FloraFire::char_at`] and
//! [`FloraFire::heat_at`] read it back for the game, and each tree it burns through is reported
//! once ([`TreeBurnt`]) and marked [`Burnt`]. While no `FloraFire` exists no shader carries the
//! fire; see [`fire`].
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraFire;
//!
//! // A fire over a 256 m square, lit by a lightning strike.
//! fn setup(mut commands: Commands) {
//!     let mut fire = FloraFire::new(Vec2::ZERO, 256.0);
//!     fire.ignite(Vec3::new(20.0, 0.0, 5.0), 2.0);
//!     commands.insert_resource(fire);
//! }
//! ```
//!
//! # Trees from assets and files
//!
//! [`TreeParams`] is an asset: a [`ProceduralTreeAsset`] draws a tree from a handle and follows
//! every change to the asset, so trees that share one are edited together. With the `ron`
//! feature the plugin loads `.tree.ron` files (`TreeParams::to_ron` writes one), and with the
//! `file_watcher` feature (Bevy's) a saved file rebuilds its trees while the game runs. See
//! [`tree_asset`].
//!
//! # Gameplay shapes
//!
//! A [`TreeShape`] on a tree (opt-in: insert `TreeShape::default()`) is kept up to date by the
//! plugin: capsules up the trunk and along every limb thicker than its `limb_radius`, and a box
//! around the crown, in the tree's local space, for colliders, picking or line of sight.
//! [`TreeMature`] is sent once when a [`Growing`] tree reaches maturity. Everything a game
//! uses is re-exported flat at the crate root.
//!
//! # Finding trees
//!
//! The [`FloraIndex`] resource holds every standing tree, pooled ones included, as its trunk,
//! by where it stands, kept up to date by the plugin: [`FloraIndex::nearest`] a point,
//! [`FloraIndex::within`] a radius, and [`FloraIndex::cast_ray`] for the trunk a ray meets (an
//! axe's swing, an arrow, the player's look). A query costs the trees near it, not all of them.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraIndex, TreeShake, TreeShakes};
//!
//! // A blast at `at` swings the trees around it; the nearest one is the woodcutter's next.
//! fn blast(index: Res<FloraIndex>, mut shakes: ResMut<TreeShakes>, time: Res<Time>) {
//!     let at = Vec2::new(10.0, -4.0);
//!     if index.within(at, 8.0).next().is_some() {
//!         shakes.shake(TreeShake { at, radius: 8.0, ..default() }, &time);
//!     }
//!     let next = index.nearest(at, 30.0).map(|tree| tree.entity);
//!     info!("the nearest tree: {next:?}");
//! }
//! ```
//!
//! # Sick and dying trees
//!
//! A game marks a tree with [`TreeCondition`], a component on the tree's entity ([`ProceduralTree`]
//! or [`PooledTree`]), and reads it back through the index: every [`IndexedTree`] says how well it
//! is. A tree without the component is healthy. The condition is the tree's own, so a pooled tree
//! keeps it through a change of variant or a reload of its pool; a felled tree leaves the index and
//! keeps the component. The tree is drawn in its condition: a sick crown has lost about a third of
//! its leaves and what is left is half way to dry, a dying one four fifths of them
//! ([`TreeCondition::defoliation`], [`TreeCondition::withering`]); wood, fruit and flowers are as
//! they were. A change is drawn the frame after it is set; with [`ConditionGlide`] it is a decline of
//! that many seconds instead (the leaves go and dry continuously, a recovery brings them back, and a
//! tree that appears with its condition shows it at once), while the index answers the class at once.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraIndex, TreeCondition};
//!
//! // A blight takes the trees near `centre`; later the sick ones near it are counted.
//! fn blight(mut commands: Commands, index: Res<FloraIndex>) {
//!     let centre = Vec2::new(10.0, -4.0);
//!     for tree in index.within(centre, 12.0).filter(|tree| !tree.undergrowth) {
//!         commands.entity(tree.entity).insert(TreeCondition::Sick);
//!     }
//!     let sick = index.within(centre, 12.0).filter(|tree| tree.condition >= TreeCondition::Sick).count();
//!     info!("{sick} trees near the blight are sick or dying");
//! }
//! ```
//!
//! # Saving and loading
//!
//! A tree is its parameters and seed; a save holds what the game did to it. On load,
//! [`FellTree::lying`] lays a felled tree down at once where it came to rest (no fall, no
//! landing), [`TreeFruit::with_picked`] keeps its picked fruit off it ([`TreeFruit::picked`] gave
//! the keys), and [`FloraFire::restore`] puts a fire's grid back as [`FloraFire::cells`] gave it,
//! the cells still burning burning on.
//!
//! # Seeing the player through the trees
//!
//! [`SeeThroughTrees`] on a third-person or top-down camera dithers out the parts of trees between
//! it and its `target` (the player), in the camera's main pass and its prepasses alike, while
//! shadow maps and other cameras keep the whole trees. Nothing carries the fade while no camera
//! has it.
//!
//! # Deadfall on the forest floor
//!
//! A pool built with [`build_pool_with_deadfall`] also holds each variant cut as [`FellTree`] and
//! [`BuckTree`] would cut it, and a [`PooledDeadfall`] (a stump, a tree lying where it fell or
//! bucked logs) is placed from it for the cost of a pooled tree; see [`deadfall`].
//!
//! # Walking through undergrowth
//!
//! A [`FoliagePusher`] on the player, an NPC or a cart pushes the leaves and thin stems it walks
//! through out of its way, shadows included, and they close up behind it; trunks stay put. Nothing
//! carries the push while no entity has one; see [`push`].
//!
//! # Building meshes without Bevy's ECS
//!
//! ```rust
//! use bevy_flora::{TreeMeshes, TreePreset, build_tree};
//!
//! let params = TreePreset::Oak.params();
//! let meshes: TreeMeshes = build_tree(&params, f32::INFINITY);
//! assert!(!meshes.trunk.is_empty());
//! ```

pub mod background;
pub mod condition;
pub mod crown;
pub mod crown_hull;
pub mod deadfall;
pub mod falling;
pub mod felling;
pub mod fire;
pub mod foliage;
mod glide;
mod gpu;
pub mod ground;
pub mod growth;
pub mod harvest;
pub mod impostor;
pub mod index;
mod limits;
pub mod lod;
pub mod mesh;
pub mod params;
pub mod pipeline;
pub mod plugin;
pub mod pool;
pub mod presets;
pub mod push;
pub mod recolor;
pub mod rng;
pub mod scatter;
pub mod see_through;
pub mod shading;
pub mod shadows;
pub mod skeleton;
pub mod shape;
mod textures;
pub mod tree_asset;
pub mod units;
pub mod venation;

pub use background::BuildInBackground;
pub use crown::CrownShape;
pub use deadfall::{Deadfall, DeadfallKind, DeadfallSettings, IntoDeadfall, PoolLog, PooledDeadfall};
pub use falling::{DropLeaves, FallingLeaves, LeafBurst};
pub use felling::{BuckTree, FellTree, Felled, TreeBucked, TreeLanded, TreeLog, TreeNotch, TreeStump};
pub use fire::{BURNT, Burnt, FireCell, FireSnapshot, FireSpot, FireSpread, FloraFire, TreeBurnt};
pub use foliage::LeafPoint;
pub use gpu::{GpuGrowth, GpuGrowthSupport, GrowthExtension, GrowthMaterial};
pub use ground::FloraGround;
pub use growth::GrowthParams;
pub use harvest::{FallenFruit, FallingFruit, Fruit, FruitFell, TreeFruit};
pub use condition::{ConditionGlide, TreeCondition};
pub use index::{FloraIndex, IndexedTree, Obstacle, ObstacleKind, TreeHit};
pub use impostor::{IMPOSTOR_VIEWS, ImpostorAtlas, ImpostorMaterial, ImpostorTint};
pub use limits::{MAX_LEAVES, MAX_NODES};
pub use lod::{LodDistances, LodSettings, TreeLod, build_lods};
pub use mesh::MeshData;
pub use params::{
    BarkParams, BarkScan, BranchParams, CrownStyle, FoliageParams, FoliagePlacement, FruitParams, LeafOrientation,
    LeafScan, LeafShading, LeafStyle, MeshParams, RenderStyle, SplitParams, StrandParams, TreeParams, TrunkParams,
    TrunkStyle, TrunkTermination,
    WindResponse,
};
pub use pipeline::{TreeMeshes, build_tree};
pub use plugin::{FloraPlugin, FloraSystems, Growing, ProceduralTree, TreeAge, TreeGenerated, TreeMature, TreeRender};
pub use pool::{
    BackgroundPools, LodScale, PoolColors, PoolMaterials, PoolVariant, PooledLook, PooledTree, RecolorPool, TreePool,
    build_pool, build_pool_with_deadfall,
};
pub use recolor::{FromPool, RecolorTree};
pub use presets::{TreePreset, UnknownPreset};
pub use rng::SeededRng;
pub use scatter::{Scatter, ScatterPoint, poisson_disc};
pub use see_through::SeeThroughTrees;
pub use shadows::FloraShadows;
pub use push::{FoliagePusher, MAX_PUSHERS};
pub use shading::{
    FloraExtension, FloraKey, FloraMaterial, FloraMaterialSupport, FloraSeason, FloraUniform, FloraWetness,
    FloraWind, FloraWindBuffer, GustFront,
    SharedFlora, TreeShake, TreeShakes,
};
pub use shape::{TreeShape, WoodCapsule};
pub use tree_asset::ProceduralTreeAsset;
#[cfg(feature = "ron")]
pub use tree_asset::{TreeFileError, TreeParamsLoader};
pub use units::FloraUnits;
pub use skeleton::{Node, NodeKind, Skeleton, generate_skeleton};

// Compile the README's Rust examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
