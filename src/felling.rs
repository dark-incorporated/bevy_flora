//! Felling: a cut tree falls, lands with a bounce and leaves a stump.
//!
//! Insert [`FellTree`] on a tree ([`ProceduralTree`], or a [`PooledTree`], which first becomes
//! the `ProceduralTree` of its variant). Once the tree is built, the plugin cuts its wood at
//! `cut_height`: the part below becomes a [`TreeStump`] entity, the part above falls with the
//! tree, and both get a cut face (an end-grain disc). The tree then tips over toward `direction`
//! as a rod hinged at the front of the cut would, comes to rest where its limbs first touch the
//! ground (twigs and leaves give way), and bounces a little; [`TreeLanded`] is sent the moment
//! it hits, and [`Felled`] says how far it has tipped and when it lies still. The ground is flat
//! at the height the tree stands at, unless a [`FloraGround`] resource says otherwise: then the
//! cut is measured from the uphill side of the trunk and the tree lands on the terrain, down a
//! slope or against it.
//!
//! Once it lands a felled tree stops swaying in the wind (its leaves still flutter). It is no
//! longer rebuilt when its [`ProceduralTree`] or [`TreeAge`] changes, and keeps its entity:
//! despawn it, or insert [`BuckTree`] to cut its trunk into [`TreeLog`]s lying on the ground,
//! once it has landed. Its [`TreeShape`](crate::TreeShape) turns with it but is not cut.
//!
//! Before it falls, a tree can be notched: a [`TreeNotch`] carves an axe's wedge out of its
//! trunk, deeper with each blow, and a tree felled with one is cut at the notch's floor.

use bevy::asset::{Assets, Handle};
use bevy::camera::visibility::{Visibility, VisibilityRange};
use bevy::ecs::prelude::*;
use bevy::image::Image;
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::gpu::GpuGrowth;
use crate::ground::FloraGround;
use crate::mesh::MeshData;
use crate::mesh::cut::{Notch, carve_notch, cut, fell_notched};
use crate::mesh::branch::{bark_tile, prism_sides, skeleton_mesh};
use crate::mesh::trunk::{trunk_mesh_with, trunk_pieces};
use crate::pipeline::{aged_skeleton, build_aged};
use crate::plugin::{CoarseLevel, Growing, Paint, ProceduralTree, TreeAge, TreeRender, paint_child};
use crate::shape::WoodCapsule;
use crate::pool::{PooledShapeFilled, PooledTree, PooledTreeSpawned, TreePool};
use crate::shading::{FloraMaterial, FloraWindBuffer, SharedFlora, TextureLibrary, TreeShake, TreeShakes};
use crate::params::TreeParams;
use crate::skeleton::{NodeKind, Skeleton, trunk_wobble};

/// Fell this tree: insert it on a [`ProceduralTree`] or a [`PooledTree`]. See the
/// [module docs](self).
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FellTree {
    /// Which way the tree falls, world x and z; normalised when used. Zero falls along +x.
    pub direction: Vec2,
    /// Where the saw goes in, in metres above the tree's base (at least 5 cm, at most half the
    /// trunk); with a [`FloraGround`], above the ground on the uphill side of the trunk.
    pub cut_height: f32,
    /// How fast it falls: 1 as a real tree of its height would (a 15 m tree takes about four
    /// seconds from the first lean to the ground), 2 twice as fast.
    pub speed: f32,
    /// Lay it down at once where it would come to rest, with no fall and no landing (no
    /// [`TreeLanded`], no shake, no leaves shed): a tree felled before a save, restored. Its
    /// stump, its cut faces and the ground it lies on are as a fallen tree's; [`BuckTree`] cuts it
    /// into logs as soon as it lies.
    pub lying: bool,
}

impl Default for FellTree {
    fn default() -> Self {
        Self { direction: Vec2::X, cut_height: 0.5, speed: 1.0, lying: false }
    }
}

/// A tree that has been cut, added by the plugin in place of [`FellTree`]: falling, then lying
/// on the ground.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct Felled {
    /// The stump it left.
    pub stump: Entity,
    /// How far it has tipped from upright, in radians.
    pub angle: f32,
    /// True once it lies still.
    pub at_rest: bool,
    #[reflect(ignore)]
    fall: Fall,
}

/// The stump a felled tree left: the wood below the cut and its cut face, where the tree stood.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct TreeStump {
    /// The tree that was cut from it.
    pub tree: Entity,
}

/// Sent the frame a felled tree first hits the ground: the moment for the crash, the dust and
/// the camera shake.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TreeLanded {
    /// The tree.
    pub tree: Entity,
    /// Its stump.
    pub stump: Entity,
    /// Where it first touched the ground, in world space.
    pub at: Vec3,
}

/// Buck a felled tree: cut its trunk into logs. Insert it on a tree that is being felled or has
/// been ([`FellTree`], [`Felled`]); once it lies still the plugin replaces it with [`TreeLog`]
/// entities lying on the ground in a row from the stump, the way it fell (on the
/// [`FloraGround`] if there is one), sends [`TreeBucked`], and despawns the tree with its crown.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct BuckTree {
    /// The length of each log, metres. The trunk above the last whole log stays with the crown.
    pub log_length: f32,
    /// The thinnest a log's top end may be, metres: the trunk above it is too thin for logs.
    pub min_radius: f32,
}

impl Default for BuckTree {
    fn default() -> Self {
        Self { log_length: 2.0, min_radius: 0.08 }
    }
}

/// A log bucked from a felled tree's trunk: bark around it and end grain at both ends, lying on
/// the ground. Its children are the wood and the two end faces.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeLog {
    /// The stump of the tree it was cut from.
    pub stump: Entity,
    /// 0 for the butt log, counting up the trunk.
    pub index: u32,
    /// The log as a capsule in its own space, from the butt end to the top end, with the butt
    /// end's radius: a collider for it (`WoodCapsule::to_primitive`).
    #[reflect(ignore)]
    pub capsule: WoodCapsule,
}

/// Sent the frame a felled tree is bucked: its entity has been despawned and its logs spawned.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct TreeBucked {
    /// The tree, now despawned.
    pub tree: Entity,
    /// Its stump.
    pub stump: Entity,
    /// Its logs, butt first; empty when the trunk above the cut is shorter than a log.
    pub logs: Vec<Entity>,
}

/// The material of every cut face, stumps, felled trees and logs: the end-grain picture, made
/// the first time a tree is cut.
#[derive(Resource, Default)]
pub(crate) struct CutFace(Option<Handle<StandardMaterial>>);

impl CutFace {
    fn get(&mut self, textures: &mut TextureLibrary, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.0
            .get_or_insert_with(|| {
                let grain = textures.end_grain();
                materials.add(StandardMaterial {
                    base_color: if grain.is_some() { bevy::color::Color::WHITE } else { END_GRAIN_COLOR },
                    base_color_texture: grain,
                    perceptual_roughness: 0.85,
                    ..Default::default()
                })
            })
            .clone()
    }
}

/// An axe's notch in a standing tree: a wedge cut out of its trunk on the side `direction`
/// points to, its floor level `height` above the base and its face rising from the floor at 45°,
/// `depth` of the way through. Insert it on a [`ProceduralTree`] or a [`PooledTree`] (which first
/// becomes the `ProceduralTree` of its variant, as for [`FellTree`]), and deepen it blow by blow:
/// each change of `depth` carves the trunk again. Remove it and the trunk is whole again.
///
/// A notched tree stops growing ([`Growing`] is removed; a change of its [`TreeAge`] or
/// [`ProceduralTree`] rebuilds it, notch and all). With [`TreeLod`](crate::TreeLod) only its
/// nearest level is notched. Fell it ([`FellTree`]) and the saw goes in at the notch's floor,
/// whatever `cut_height` says: the stump keeps the floor, the falling tree the face. A real feller
/// cuts the notch on the side the tree should fall to, a quarter to a third of the way through.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeNotch {
    /// The side it is cut from, world x and z; normalised when used. Zero is +x.
    pub direction: Vec2,
    /// The height of its floor, metres above the tree's base (at least 5 cm, at most half the
    /// trunk); with a [`FloraGround`], above the ground on the uphill side of the trunk, as
    /// [`FellTree::cut_height`].
    pub height: f32,
    /// How far into the trunk it reaches at its floor, a share of the trunk's width there: 0
    /// cuts nothing, 0.25 is a felling notch, 0.9 the most.
    pub depth: f32,
}

impl Default for TreeNotch {
    fn default() -> Self {
        Self { direction: Vec2::X, height: 0.5, depth: 0.25 }
    }
}

/// The notch a tree's wood was carved with, and the child drawing its faces.
#[derive(Component)]
pub(crate) struct NotchCarved {
    notch: TreeNotch,
    faces: Option<Entity>,
}

/// How a felled tree moves, in its own (local) space.
#[derive(Clone, Debug, Default)]
struct Fall {
    /// Its transform before it was cut.
    base: Transform,
    /// The front edge of the cut, which it turns about.
    hinge: Vec3,
    /// The axis it turns about (through `hinge`).
    axis: Vec3,
    /// The angle at which it first touches the ground, and the point that touches.
    rest: f32,
    contact: Vec3,
    /// `3 g / 2 L` times the square of the speed: a rod of length `L` hinged at its foot.
    pull: f32,
    /// Angular speed, radians a second.
    spin: f32,
    landed: bool,
    /// Laid down at once ([`FellTree::lying`]).
    lying: bool,
    /// How far the crown reaches from the trunk, for the shake it gives its neighbours.
    crown_radius: f32,
    /// Its wood and leaves, and the materials they take once it lands (they do not sway).
    still: Vec<(Entity, Handle<FloraMaterial>)>,
}

/// How much of its speed a tree keeps when it bounces off the ground.
const RESTITUTION: f32 = 0.25;
/// A bounce that would lift the tree less than this, in radians, is over: it lies still.
const SETTLE_RISE: f32 = 0.004;
/// The push the last stroke of the saw gives a cut tree, radians a second: from upright it
/// leans slowly, then gravity takes it.
const START_SPIN: f32 = 0.08;
/// Integration step, seconds.
const STEP: f32 = 1.0 / 240.0;
const GRAVITY: f32 = 9.81;
/// Limbs at least this share of the trunk's radius at the cut, and at least [`CONTACT_RADIUS`]
/// (metres), stop a falling tree; thinner branches, twigs and the leaves give way and may end
/// below the ground, as a crown crushed into it. A conifer's whorls of thin limbs do not prop it
/// up at a slant.
const CONTACT_SHARE: f32 = 0.15;
const CONTACT_RADIUS: f32 = 0.04;
/// The furthest a felled tree turns, from upright: on flat ground its trunk meets the ground
/// before 105°; down a steep slope it can lie along it, well past 90°.
const REST_LIMIT: f32 = 150.0 * std::f32::consts::PI / 180.0;
/// The gap between bucked logs laid in a row, and between the first and the stump, metres:
/// enough that the row reads as separate logs, and their end grain shows between them.
const LOG_GAP: f32 = 0.15;

/// Start felling every tree with a [`FellTree`] that can be cut: turn a pooled tree into its
/// `ProceduralTree`, move a tree off the GPU growth path, then cut (at its [`TreeNotch`]'s
/// floor, if it has one), spawn the stump and set up the fall.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn start_felling(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut textures: TextureLibrary,
    wind: Option<Res<FloraWindBuffer>>,
    pools: Option<Res<Assets<TreePool>>>,
    ground: Option<Res<FloraGround>>,
    mut cut_face: ResMut<CutFace>,
    trees: Query<(
        Entity,
        &FellTree,
        &Transform,
        Option<&ProceduralTree>,
        Option<&TreeAge>,
        Option<&TreeRender>,
        Option<&PooledTree>,
        Has<GpuGrowth>,
        Has<Growing>,
        Option<&TreeNotch>,
        Option<&NotchCarved>,
    )>,
) {
    for (entity, fell, transform, tree, age, render, pooled, gpu, growing, notch, carved) in &trees {
        if let Some(pooled) = pooled {
            let Some(pool) = pools.as_deref().and_then(|pools| pools.get(&pooled.pool)) else { continue };
            let params = pool.tree_params(pooled.variant);
            commands
                .entity(entity)
                .despawn_related::<Children>()
                .remove::<(PooledTree, PooledTreeSpawned, PooledShapeFilled)>()
                .insert((ProceduralTree(params), TreeAge(pool.age)));
            continue;
        }
        let Some(tree) = tree else { continue };
        if growing {
            commands.entity(entity).remove::<Growing>();
        }
        if gpu {
            commands.entity(entity).remove::<GpuGrowth>();
            continue;
        }
        // Wait for the tree's meshes, rebuilt on the CPU.
        let Some(render) = render else { continue };
        let Some(paint) = render.paint.cloned_cpu() else { continue };
        let age = age.map_or(f32::INFINITY, |age| age.0);
        let built = build_aged(&render.full, &tree.0, age);
        if built.trunk.is_empty() {
            commands.entity(entity).remove::<FellTree>();
            continue;
        }
        // A tree drawn at levels of detail falls in full: its coarse levels go, and its own
        // children are drawn at every distance.
        if let Some((_, levels)) = &render.lod {
            for child in levels.iter().flat_map(CoarseLevel::children) {
                commands.entity(child).despawn();
            }
            let level0 = render.foliage.iter().chain(&render.flowers).map(|child| child.0);
            for child in std::iter::once(render.trunk).chain(level0) {
                commands.entity(child).remove::<(VisibilityRange, crate::pool::LodBand)>();
            }
        }

        let scale = transform.scale.y.max(1.0e-4);
        // A notched tree is cut at the notch's floor; on a slope the saw goes in above the uphill
        // side of the trunk.
        let notch = notch.and_then(|notch| notch_in(&built.trunk, notch, transform, &tree.0, ground.as_deref()));
        let cut_y = notch.map_or_else(|| saw_height(transform, &tree.0, ground.as_deref(), fell.cut_height), |n| n.floor);
        let above = cut(&built.trunk, cut_y, true);
        let below = cut(&built.trunk, cut_y, false);
        let face = cut_face.get(&mut textures, &mut materials);

        // The stump: the wood below the cut and its face, where the tree stands.
        let stump = commands.spawn((*transform, Visibility::Inherited, TreeStump { tree: entity })).id();
        let mut wood = commands.spawn((Mesh3d(meshes.add(below.kept.into_bevy_mesh())), ChildOf(stump)));
        paint_child(&mut wood, &paint, false);
        if !below.faces.is_empty() {
            commands.spawn((Mesh3d(meshes.add(below.faces.into_bevy_mesh())), MeshMaterial3d(face.clone()), ChildOf(stump)));
        }

        // How it will fall: about the front edge of the cut, until its limbs reach the ground.
        let local_dir = fall_direction(transform, fell.direction);
        let (hinge, trunk_reach) = hinge_point(&above.outlines, cut_y, local_dir);
        let axis = Vec3::Y.cross(local_dir).normalize();
        // How far a point of the tree, in its own space, stands above the ground.
        let clearance = |p: Vec3| match ground.as_deref() {
            None => p.y,
            Some(ground) => {
                let world = transform.transform_point(p);
                (world.y - ground.height(world.xz())) / scale
            }
        };
        // The tree keeps the wood above the cut and gains its faces: with a notch, less the
        // notch's wedge, with its face and the cut behind it.
        let (wood, faces) = match &notch {
            Some(notch) => fell_notched(&above, notch),
            None => (above.kept, above.faces),
        };
        let contact_set = contact_points(&wood, &built.skeleton, &tree.0, cut_y, trunk_reach);
        let (rest, contact) = rest_angle(&contact_set, hinge, axis, clearance);
        let length = wood.positions.iter().map(|p| (*p - hinge).length()).fold(trunk_reach, f32::max) * scale;
        let crown = built.foliage.positions.iter().chain(&built.flowers.positions);
        let crown_radius = crown.map(|p| Vec2::new(p.x, p.z).length()).fold(0.0, f32::max) * scale;
        commands.entity(render.trunk).insert((Mesh3d(meshes.add(wood.into_bevy_mesh())), Visibility::Inherited));
        if !faces.is_empty() {
            commands.spawn((Mesh3d(meshes.add(faces.into_bevy_mesh())), MeshMaterial3d(face), ChildOf(entity)));
        }
        if let Some(carved) = carved {
            if let Some(faces) = carved.faces {
                commands.entity(faces).despawn();
            }
            commands.entity(entity).remove::<NotchCarved>();
        }
        // Materials that do not sway, for when it lies on the ground: while it falls the wind
        // still moves it as it did (a change there would show).
        let mut still = Vec::new();
        if let (Paint::Flora { .. }, Some(wind)) = (&paint, wind.as_deref()) {
            let calm = TreeParams { wind: crate::WindResponse { sway: 0.0, ..tree.0.wind.clone() }, ..tree.0.clone() };
            still.push((render.trunk, shared.resolve(&mut flora, &textures.wood(&calm), wind)));
            if render.foliage.is_some() {
                let leaves = shared.resolve(&mut flora, &textures.leaves(&calm), wind);
                for (child, _) in render.foliage.iter().chain(&render.flowers) {
                    still.push((*child, leaves.clone()));
                }
            }
        }
        let speed = fell.speed.max(0.01);
        commands.entity(entity).remove::<FellTree>().insert(Felled {
            stump,
            angle: 0.0,
            at_rest: false,
            fall: Fall {
                base: *transform,
                hinge,
                axis,
                rest,
                contact,
                pull: 1.5 * GRAVITY / length.max(0.5) * speed * speed,
                spin: START_SPIN * speed,
                landed: false,
                lying: fell.lying,
                crown_radius,
                still,
            },
        });
    }
}

/// Carve every [`TreeNotch`] into its tree's wood when it changes or the tree is rebuilt (turning
/// a pooled tree into its `ProceduralTree` and moving a tree off the GPU growth path first, as
/// felling does), and make the wood of a tree whose notch was removed whole again.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn carve_notches(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut textures: TextureLibrary,
    pools: Option<Res<Assets<TreePool>>>,
    ground: Option<Res<FloraGround>>,
    mut cut_face: ResMut<CutFace>,
    trees: Query<
        (
            Entity,
            &TreeNotch,
            &Transform,
            Option<&ProceduralTree>,
            Option<Ref<TreeRender>>,
            Option<&PooledTree>,
            Has<GpuGrowth>,
            Has<Growing>,
            Option<&NotchCarved>,
        ),
        (Without<Felled>, Without<FellTree>),
    >,
    healed: Query<(Entity, &NotchCarved, &ProceduralTree, &TreeRender), (Without<TreeNotch>, Without<Felled>)>,
) {
    for (entity, notch, transform, tree, render, pooled, gpu, growing, carved) in &trees {
        if let Some(pooled) = pooled {
            let Some(pool) = pools.as_deref().and_then(|pools| pools.get(&pooled.pool)) else { continue };
            let params = pool.tree_params(pooled.variant);
            commands
                .entity(entity)
                .despawn_related::<Children>()
                .remove::<(PooledTree, PooledTreeSpawned, PooledShapeFilled)>()
                .insert((ProceduralTree(params), TreeAge(pool.age)));
            continue;
        }
        let Some(tree) = tree else { continue };
        if growing {
            commands.entity(entity).remove::<Growing>();
        }
        if gpu {
            commands.entity(entity).remove::<GpuGrowth>();
            continue;
        }
        // Wait for the tree's meshes, rebuilt on the CPU; carve again when they are rebuilt.
        let Some(render) = render else { continue };
        if render.paint.cloned_cpu().is_none() || (carved.is_some_and(|c| c.notch == *notch) && !render.is_changed()) {
            continue;
        }
        let Some(handle) = &render.trunk_mesh else { continue };
        let wood = skeleton_mesh(&aged_skeleton(&render.full, &tree.0, render.age), &tree.0);
        let (wood, faces) = match notch_in(&wood, notch, transform, &tree.0, ground.as_deref()) {
            Some(cut) => carve_notch(&wood, &cut),
            None => (wood, MeshData::default()),
        };
        if let Some(mut mesh) = meshes.get_mut(handle) {
            *mesh = wood.into_bevy_mesh();
        }
        let mut drawn = carved.and_then(|c| c.faces);
        match (drawn, faces.is_empty()) {
            (Some(child), true) => {
                commands.entity(child).despawn();
                drawn = None;
            }
            (Some(child), false) => {
                commands.entity(child).insert(Mesh3d(meshes.add(faces.into_bevy_mesh())));
            }
            (None, false) => {
                let face = cut_face.get(&mut textures, &mut materials);
                let mesh = Mesh3d(meshes.add(faces.into_bevy_mesh()));
                drawn = Some(commands.spawn((mesh, MeshMaterial3d(face), ChildOf(entity))).id());
            }
            (None, true) => {}
        }
        commands.entity(entity).insert(NotchCarved { notch: *notch, faces: drawn });
    }
    for (entity, carved, tree, render) in &healed {
        if let Some(mut mesh) = render.trunk_mesh.as_ref().and_then(|handle| meshes.get_mut(handle)) {
            *mesh = skeleton_mesh(&aged_skeleton(&render.full, &tree.0, render.age), &tree.0).into_bevy_mesh();
        }
        if let Some(faces) = carved.faces {
            commands.entity(faces).despawn();
        }
        commands.entity(entity).remove::<NotchCarved>();
    }
}

/// How high the saw or the axe goes in, in a tree's own space: `height` metres above its base
/// (at least 5 cm, at most half its trunk), or with `ground` above the ground on the uphill side
/// of its trunk.
fn saw_height(transform: &Transform, params: &TreeParams, ground: Option<&FloraGround>, height: f32) -> f32 {
    let scale = transform.scale.y.max(1.0e-4);
    let uphill = ground.map_or(0.0, |ground| {
        let top = ground.top(transform.translation.xz(), params.base_radius() * scale);
        (top - transform.translation.y).max(0.0) / scale
    });
    (uphill + height / scale).clamp(0.05 / scale, params.trunk.height * 0.5)
}

/// Where `notch` cuts into `wood`, the wood of a tree of `params` standing at `transform`, in its
/// own space; `None` where it cuts nothing (no depth, or no trunk at its floor).
fn notch_in(wood: &MeshData, notch: &TreeNotch, transform: &Transform, params: &TreeParams, ground: Option<&FloraGround>) -> Option<Notch> {
    let depth = notch.depth.min(0.9);
    if depth.is_nan() || depth <= 0.0 {
        return None;
    }
    let floor = saw_height(transform, params, ground, notch.height);
    let level = cut(wood, floor, false);
    // The trunk's outline: the one round the tree's base, not a hanging limb's.
    let centre_of = |o: &[Vec3]| o.iter().copied().sum::<Vec3>() / o.len() as f32;
    let trunk = level.outlines.iter().min_by(|a, b| centre_of(a).xz().length().total_cmp(&centre_of(b).xz().length()))?;
    let centre = centre_of(trunk);
    let toward = fall_direction(transform, notch.direction);
    let front = trunk.iter().map(|p| (*p - centre).dot(toward)).fold(0.0, f32::max);
    let back = trunk.iter().map(|p| (centre - *p).dot(toward)).fold(0.0, f32::max);
    let inner = front - depth * (front + back);
    // The face rises at 45° and leaves the trunk `front - inner` above the floor; the carving
    // stops well clear of that, above and beyond.
    let rise = front - inner;
    let margin = 0.05 / transform.scale.y.max(1.0e-4);
    Some(Notch {
        floor,
        mouth: floor + 1.5 * rise + margin,
        axis: centre.xz(),
        toward: toward.xz(),
        inner,
        outer: front + 0.5 * rise + margin,
        radius: trunk.iter().map(|p| p.distance(centre)).fold(0.0, f32::max),
        slope: 1.0,
    })
}

/// Tip every falling tree over, a few small steps a frame, and settle it where it lands.
pub(crate) fn drive_felling(
    mut commands: Commands,
    time: Res<Time>,
    mut shakes: ResMut<TreeShakes>,
    mut landed: MessageWriter<TreeLanded>,
    mut trees: Query<(Entity, &mut Felled, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut felled, mut transform) in &mut trees {
        if felled.at_rest {
            continue;
        }
        let felled = &mut *felled;
        let fall = &mut felled.fall;
        // A tree restored lying: at once where it would come to rest, on its still materials,
        // with no landing.
        if fall.lying {
            felled.angle = fall.rest;
            fall.landed = true;
            fall.spin = 0.0;
            felled.at_rest = true;
            for (child, material) in fall.still.drain(..) {
                commands.entity(child).insert(MeshMaterial3d(material));
            }
        }
        let mut left = dt;
        while left > 0.0 && !felled.at_rest {
            let h = left.min(STEP);
            left -= h;
            fall.spin += fall.pull * felled.angle.sin() * h;
            felled.angle += fall.spin * h;
            if felled.angle >= fall.rest && fall.spin > 0.0 {
                felled.angle = fall.rest;
                if !fall.landed {
                    fall.landed = true;
                    let resting = fall.base.mul_transform(pivot(fall, fall.rest));
                    let at = resting.transform_point(fall.contact);
                    landed.write(TreeLanded { tree: entity, stump: felled.stump, at });
                    // On the ground it stops swaying with the wind; the crash hides the change.
                    for (child, material) in fall.still.drain(..) {
                        commands.entity(child).insert(MeshMaterial3d(material));
                    }
                    // Its own leaves shake on the ground (it no longer sways), and the crowns it
                    // crashed through swing.
                    let own = TreeShake { at: resting.translation.xz(), radius: 0.5, strength: 2.0, duration: 2.0, ..Default::default() };
                    shakes.shake(own, &time);
                    let crash = TreeShake {
                        at: at.xz(),
                        radius: fall.crown_radius.max(2.0),
                        strength: 1.2,
                        duration: 2.5,
                        ..Default::default()
                    };
                    shakes.shake(crash, &time);
                }
                fall.spin *= -RESTITUTION;
                // How far the bounce would lift it back against gravity; a small or quick tree
                // falls back within a step, so its speed alone never gets small.
                let rise = fall.spin * fall.spin / (2.0 * fall.pull * fall.rest.sin().max(0.05));
                if rise < SETTLE_RISE {
                    fall.spin = 0.0;
                    felled.at_rest = true;
                }
            }
        }
        *transform = fall.base.mul_transform(pivot(fall, felled.angle));
    }
}

/// Turning by `angle` about the fall's hinge, in the tree's own space.
fn pivot(fall: &Fall, angle: f32) -> Transform {
    let turn = Quat::from_axis_angle(fall.axis, angle);
    Transform::from_translation(fall.hinge - turn * fall.hinge).with_rotation(turn)
}

/// Buck every felled tree with a [`BuckTree`] once it lies still: cut its trunk into logs, lay
/// them on the ground in a row from the stump, the way it fell, and clear the tree.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn buck_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut textures: TextureLibrary,
    mut cut_face: ResMut<CutFace>,
    ground: Option<Res<FloraGround>>,
    mut bucked: MessageWriter<TreeBucked>,
    trees: Query<(Entity, &BuckTree, &Felled, &Transform, &ProceduralTree, Option<&TreeAge>, &TreeRender)>,
    flora_wood: Query<&MeshMaterial3d<FloraMaterial>>,
    plain_wood: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    for (entity, buck, felled, lying, tree, age, render) in &trees {
        if !felled.at_rest {
            continue;
        }
        let fall = &felled.fall;
        let params = &tree.0;
        // The trunk alone, as the tree had it: the same rings and bark.
        let skeleton = aged_skeleton(&render.full, params, age.map_or(f32::INFINITY, |age| age.0));
        let node = &skeleton.nodes[0];
        let mut trunk_params = params.trunk.clone();
        let prism = prism_sides(params);
        if let Some(sides) = prism {
            trunk_params.radial_segments = sides;
        }
        let pieces = trunk_pieces(&trunk_params, node);
        let mut trunk = trunk_mesh_with(&trunk_params, node, params.seed, pieces, bark_tile(params));
        if prism.is_some() {
            trunk = trunk.flat_shaded();
        }
        let top = trunk.positions.iter().map(|p| p.y).fold(0.0, f32::max);
        let face = cut_face.get(&mut textures, &mut materials);
        let scale = fall.base.scale.y.max(1.0e-4);
        // The ground's height at a point, and how much higher than its radius a log's axis lies
        // there (1 on flat ground, more on a slope).
        let rest = |p: Vec2| match ground.as_deref() {
            None => (fall.base.translation.y, 1.0),
            Some(ground) => (ground.height(p), 1.0 / ground.normal(p).y.max(0.2)),
        };
        // In a row from the stump's front edge, the way the tree fell.
        let up = lying.rotation * Vec3::Y;
        let along = Vec2::new(up.x, up.z).normalize_or(Vec2::X);
        let mut at = fall.base.transform_point(fall.hinge).xz() + along * LOG_GAP;
        let length = buck.log_length.max(0.1);
        let mut logs = Vec::new();
        let mut y0 = fall.hinge.y;
        while y0 + length / scale <= top {
            let y1 = y0 + length / scale;
            let lower = cut(&trunk, y0, true);
            let piece = cut(&lower.kept, y1, false);
            let (Some((c0, r0)), Some((c1, r1))) = (log_end(&lower.outlines), log_end(&piece.outlines)) else { break };
            if r1 * scale < buck.min_radius {
                break;
            }
            // Its ends on the ground, a log's length apart along it.
            let (g0, lift0) = rest(at);
            let rise = rest(at + along * length).0 - g0;
            let far = at + along * (length * length / (length * length + rise * rise).sqrt());
            let (g1, lift1) = rest(far);
            let e0 = Vec3::new(at.x, g0 + r0 * scale * lift0, at.y);
            let e1 = Vec3::new(far.x, g1 + r1 * scale * lift1, far.y);
            let axis = (fall.base.rotation * (c1 - c0)).normalize_or(Vec3::Y);
            let rotation = Quat::from_rotation_arc(axis, (e1 - e0).normalize_or(axis)) * fall.base.rotation;
            let place = Transform { translation: e0 - rotation * (c0 * scale), rotation, scale: fall.base.scale };

            let capsule = WoodCapsule { a: c0, b: c1, radius: r0 };
            let log = commands
                .spawn((place, Visibility::Inherited, TreeLog { stump: felled.stump, index: logs.len() as u32, capsule }))
                .id();
            let mut wood = commands.spawn((Mesh3d(meshes.add(piece.kept.into_bevy_mesh())), ChildOf(log)));
            if let Ok(material) = flora_wood.get(render.trunk) {
                wood.insert(material.clone());
            } else if let Ok(material) = plain_wood.get(render.trunk) {
                wood.insert(material.clone());
            }
            let mut ends = lower.faces;
            ends.extend(&piece.faces);
            if !ends.is_empty() {
                commands.spawn((Mesh3d(meshes.add(ends.into_bevy_mesh())), MeshMaterial3d(face.clone()), ChildOf(log)));
            }
            logs.push(log);
            // The next starts a gap past where this one ends (its taper turns it a little, so not
            // quite at `far`).
            at = place.transform_point(c1).xz() + along * LOG_GAP;
            y0 = y1;
        }
        commands.entity(entity).despawn();
        bucked.write(TreeBucked { tree: entity, stump: felled.stump, logs });
    }
}

/// The centre of a log's end and the radius it rests on (the trunk's outline at a cut: its
/// centre, and its mean radius to the middle of its sides, so a polygonal log neither floats
/// on its corners nor sinks by more than them).
fn log_end(outlines: &[Vec<Vec3>]) -> Option<(Vec3, f32)> {
    let outline = outlines.iter().max_by_key(|o| o.len()).filter(|o| o.len() >= 3)?;
    // Only its corners: a flat-shaded trunk's cut has a point in the middle of each side too (where
    // the cut crosses the diagonal of the side's two triangles), which would pull the mean in.
    let n = outline.len();
    let corners: Vec<Vec3> = (0..n)
        .filter(|&i| {
            let (before, here, after) = (outline[(i + n - 1) % n], outline[i], outline[(i + 1) % n]);
            let (u, v) = (here - before, after - here);
            u.cross(v).length() > 0.035 * u.length() * v.length()
        })
        .map(|i| outline[i])
        .collect();
    let trunk = if corners.len() >= 3 { &corners } else { outline };
    let centre = trunk.iter().copied().sum::<Vec3>() / trunk.len() as f32;
    let radius = trunk.iter().map(|p| p.distance(centre)).sum::<f32>() / trunk.len() as f32;
    Some((centre, radius * (std::f32::consts::PI / trunk.len() as f32).cos()))
}

/// `direction` (world x, z) in the tree's own horizontal plane, unit length.
fn fall_direction(transform: &Transform, direction: Vec2) -> Vec3 {
    let world = Vec3::new(direction.x, 0.0, direction.y).normalize_or(Vec3::X);
    let local = transform.rotation.inverse() * world;
    Vec3::new(local.x, 0.0, local.z).normalize_or(Vec3::X)
}

/// The front edge of the cut toward `direction`, which a falling tree turns about (the hinge
/// wood of a real felling cut), and how far it is from the trunk's centre.
fn hinge_point(outlines: &[Vec<Vec3>], cut_y: f32, direction: Vec3) -> (Vec3, f32) {
    let trunk = outlines.iter().max_by_key(|o| o.len()).map(Vec::as_slice).unwrap_or(&[]);
    if trunk.is_empty() {
        return (Vec3::new(0.0, cut_y, 0.0), 0.0);
    }
    let centre = trunk.iter().copied().sum::<Vec3>() / trunk.len() as f32;
    let reach = trunk.iter().map(|p| (*p - centre).dot(direction)).fold(0.0, f32::max);
    (Vec3::new(centre.x, cut_y, centre.z) + direction * reach, reach)
}

/// The points of the wood above the cut that can hold a falling tree up: the trunk and leader
/// all along (sampled from `skeleton`), and the vertices of limbs at least [`CONTACT_SHARE`] as
/// thick as the trunk at the cut and [`CONTACT_RADIUS`] (`uv1.x` is a wood vertex's radius,
/// `trunk_radius` the trunk's at the cut, both in the tree's own units).
fn contact_points(wood: &MeshData, skeleton: &Skeleton, params: &TreeParams, cut_y: f32, trunk_radius: f32) -> Vec<Vec3> {
    let least = (trunk_radius * CONTACT_SHARE).max(CONTACT_RADIUS);
    let mut points: Vec<Vec3> =
        wood.positions.iter().zip(&wood.uv1).filter(|(_, uv1)| uv1.x >= least).map(|(p, _)| *p).collect();
    for node in skeleton.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Trunk | NodeKind::Leader)) {
        let (side, up) = node.direction.any_orthonormal_pair();
        for i in 0..=48 {
            let t = i as f32 / 48.0;
            let mut centre = node.start + node.direction * (node.length * t);
            if node.kind == NodeKind::Trunk {
                // The trunk's mesh follows its wobble; the node's axis does not.
                let (x, z) = trunk_wobble(&params.trunk, params.seed, centre.y / params.trunk.height.max(1.0e-4));
                centre += Vec3::new(x, 0.0, z);
            }
            let radius = node.base_radius + (node.tip_radius - node.base_radius) * t;
            for k in 0..12 {
                let (sin, cos) = (k as f32 * std::f32::consts::TAU / 12.0).sin_cos();
                let p = centre + (side * cos + up * sin) * radius;
                if p.y >= cut_y {
                    points.push(p);
                }
            }
        }
    }
    if points.is_empty() { wood.positions.clone() } else { points }
}

/// The smallest angle, turning about `axis` through `hinge`, at which a point of `points` first
/// reaches the ground, and that point; [`REST_LIMIT`] if none does. `clearance(p)` is how far
/// `p` (in the tree's space) stands above the ground; points already below it before the fall
/// (a limb low on the uphill side of a slope) are left out, as they would plough through.
fn rest_angle(points: &[Vec3], hinge: Vec3, axis: Vec3, clearance: impl Fn(Vec3) -> f32) -> (f32, Vec3) {
    let points: Vec<Vec3> = points.iter().copied().filter(|p| clearance(*p) >= 0.0).collect();
    let lowest = |angle: f32, stride: usize| {
        let turn = Quat::from_axis_angle(axis, angle);
        points
            .iter()
            .step_by(stride)
            .map(|p| (*p, clearance(turn * (*p - hinge) + hinge)))
            .fold((hinge, f32::INFINITY), |best, (p, y)| if y < best.1 { (p, y) } else { best })
    };
    let stride = (points.len() / 6000).max(1);
    let step = 0.5f32.to_radians();
    let mut below = None;
    let mut angle = 0.0;
    while angle < REST_LIMIT {
        angle += step;
        if lowest(angle, stride).1 < 0.0 {
            below = Some(angle);
            break;
        }
    }
    let Some(mut high) = below else { return (REST_LIMIT, lowest(REST_LIMIT, 1).0) };
    let mut low = (high - step).max(0.0);
    for _ in 0..14 {
        let mid = 0.5 * (low + high);
        if lowest(mid, 1).1 < 0.0 {
            high = mid;
        } else {
            low = mid;
        }
    }
    (low, lowest(low, 1).0)
}

/// The colour of fresh-cut wood, for a cut face without the end-grain picture.
pub(crate) const END_GRAIN_COLOR: bevy::color::Color = bevy::color::Color::srgb(0.80, 0.66, 0.46);

/// The end-grain picture a cut face wears: pale sapwood, warmer heartwood, growth rings that
/// wander a little, a dark pith and a few checks, RGBA8 sRGB, `size` square.
fn end_grain_pixels(size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let half = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5 - half) / half;
            let dz = (y as f32 + 0.5 - half) / half;
            let r = (dx * dx + dz * dz).sqrt();
            let theta = dz.atan2(dx);
            let wander = 0.012 * (3.0 * theta + 1.3).sin() + 0.008 * (7.0 * theta + 0.4).sin();
            let ring = ((r + wander) * 17.0).fract();
            let late = smoothstep(0.78, 0.95, ring) * (1.0 - smoothstep(0.95, 1.0, ring));
            let sapwood = Vec3::new(225.0, 199.0, 152.0);
            let heartwood = Vec3::new(201.0, 156.0, 104.0);
            let mut colour = heartwood.lerp(sapwood, smoothstep(0.45, 0.7, r));
            colour *= 1.0 - 0.18 * late;
            // The pith, and a few checks running out from it.
            colour *= 0.55 + 0.45 * smoothstep(0.02, 0.06, r);
            let check = [0.4f32, 2.3, 4.4]
                .iter()
                .map(|&a| {
                    let off = (theta - a).sin().abs() * r;
                    let facing = if (theta - a).cos() > 0.0 { 1.0 } else { 0.0 };
                    facing * (1.0 - smoothstep(0.0, 0.012, off)) * (1.0 - smoothstep(0.25, 0.55, r))
                })
                .fold(0.0, f32::max);
            colour *= 1.0 - 0.55 * check;
            // The cambium, darker, where the bark begins.
            colour *= 1.0 - 0.35 * smoothstep(0.93, 1.0, r);
            data.extend([colour.x, colour.y, colour.z].map(|c| c.clamp(0.0, 255.0) as u8));
            data.push(255);
        }
    }
    data
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The end-grain image with its mip chain, for [`TextureLibrary::end_grain`].
pub(crate) fn end_grain_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIZE: u32 = 256;
    let base = end_grain_pixels(SIZE);
    // `Image::new` takes the base level alone (debug builds check its length); the chain
    // replaces the data afterwards.
    let mut image = Image::new(
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        base.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(crate::impostor::with_mip_chain(&base, SIZE, SIZE));
    image.texture_descriptor.mip_level_count = crate::impostor::MIP_LEVELS;
    image
}

/// Messages, reflection and the cut faces' material; the systems are in the plugin's chain.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.add_message::<TreeLanded>()
        .add_message::<TreeBucked>()
        .init_resource::<CutFace>()
        .register_type::<FellTree>()
        .register_type::<Felled>()
        .register_type::<TreeStump>()
        .register_type::<BuckTree>()
        .register_type::<TreeLog>()
        .register_type::<TreeNotch>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TreePreset, build_tree};

    #[test]
    fn a_felled_tree_rests_on_its_limbs_without_sinking_into_the_ground() {
        for preset in [
            TreePreset::Oak,
            TreePreset::Pine,
            TreePreset::Willow,
            TreePreset::Birch,
            TreePreset::Maple,
            TreePreset::Spruce,
            TreePreset::Redwood,
        ] {
            let params = preset.params();
            let tree = build_tree(&params, f32::INFINITY);
            let above = cut(&tree.trunk, 0.5, true);
            let direction = Vec3::X;
            let (hinge, reach) = hinge_point(&above.outlines, 0.5, direction);
            let points = contact_points(&above.kept, &tree.skeleton, &params, 0.5, reach);
            let axis = Vec3::Y.cross(direction).normalize();
            let (rest, contact) = rest_angle(&points, hinge, axis, |p| p.y);
            assert!(
                rest > 45f32.to_radians() && rest < 105f32.to_radians(),
                "{preset:?}: rests at {}°",
                rest.to_degrees()
            );
            let turn = Quat::from_axis_angle(axis, rest);
            let lowest = points.iter().map(|p| (turn * (*p - hinge) + hinge).y).fold(f32::INFINITY, f32::min);
            assert!(lowest > -0.005, "{preset:?}: its limbs sink {lowest} m into the ground at rest");
            assert!(lowest < 0.05, "{preset:?}: floats {lowest} m above the ground at rest");
            assert!((turn * (contact - hinge) + hinge).y < 0.05, "{preset:?}: the contact point is not on the ground");
            // It falls toward +x: the top of the trunk ends on that side.
            let top = points.iter().copied().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
            assert!((turn * (top - hinge) + hinge).x > hinge.x, "{preset:?}: fell the wrong way");
        }
    }

    fn app() -> bevy::app::App {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::time::TimeUpdateStrategy;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app
    }

    /// Run updates until every felled tree lies still (at most `frames`), counting landings.
    fn settle(app: &mut bevy::app::App, frames: u32) -> usize {
        let mut cursor = app.world().resource::<bevy::ecs::message::Messages<TreeLanded>>().get_cursor_current();
        let mut landings = 0;
        for _ in 0..frames {
            app.update();
            landings += cursor.read(app.world().resource::<bevy::ecs::message::Messages<TreeLanded>>()).count();
            let mut query = app.world_mut().query::<&Felled>();
            let felled: Vec<bool> = query.iter(app.world()).map(|f| f.at_rest).collect();
            if !felled.is_empty() && felled.iter().all(|&rest| rest) {
                break;
            }
        }
        landings
    }

    #[test]
    fn a_tree_restored_lying_lies_at_once_where_a_felled_one_comes_to_rest() {
        let params = TreePreset::Oak.params();
        let lay = |lying: bool| {
            let mut app = app();
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), Transform::from_xyz(2.0, 0.0, -1.0))).id();
            app.update();
            app.world_mut().entity_mut(tree).insert(FellTree { direction: Vec2::new(1.0, 1.0), lying, ..Default::default() });
            app.update();
            let at_once = app.world().get::<Felled>(tree).map(|felled| felled.at_rest);
            let landings = settle(&mut app, 900);
            let lying_at = *app.world().get::<Transform>(tree).unwrap();
            (app, tree, at_once, landings, lying_at)
        };
        let (_, _, fell_at_once, fell_landings, fell_at) = lay(false);
        let (mut app, tree, at_once, landings, at) = lay(true);
        assert_eq!((fell_at_once, fell_landings), (Some(false), 1));
        assert_eq!(at_once, Some(true), "a tree restored lying fell first");
        assert_eq!(landings, 0, "a tree restored lying landed");
        assert!(at.translation.distance(fell_at.translation) < 1e-3, "{at:?} against {fell_at:?}");
        assert!(at.rotation.angle_between(fell_at.rotation) < 1e-3, "{at:?} against {fell_at:?}");
        // Bucked as soon as it lies.
        app.world_mut().entity_mut(tree).insert(BuckTree::default());
        app.update();
        app.update();
        assert!(app.world_mut().query::<&TreeLog>().iter(app.world()).count() > 0, "no logs");
    }

    #[test]
    fn a_felled_tree_falls_the_way_it_was_sent_lands_once_and_leaves_a_stump() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(3.0, 0.0, -2.0)))
            .id();
        app.update();
        let fell = FellTree { direction: Vec2::new(0.0, 1.0), speed: 3.0, ..Default::default() };
        app.world_mut().entity_mut(tree).insert(fell);
        assert_eq!(settle(&mut app, 600), 1, "the tree did not land exactly once");

        let felled = app.world().get::<Felled>(tree).expect("the tree was not felled").clone();
        assert!(felled.at_rest, "the tree is still moving");
        let up = app.world().get::<Transform>(tree).unwrap().rotation * Vec3::Y;
        assert!(up.z > 0.6 && up.y < 0.6, "it fell along {up}, not toward +z");
        let stump = app.world().get::<TreeStump>(felled.stump).expect("no stump");
        assert_eq!(stump.tree, tree);
        assert_eq!(app.world().get::<Transform>(felled.stump).unwrap().translation, Vec3::new(3.0, 0.0, -2.0));
        let pieces = app.world().get::<Children>(felled.stump).map_or(0, |c| c.len());
        assert_eq!(pieces, 2, "the stump should be its wood and its cut face");

        // It is cut for good: ageing it rebuilds nothing.
        let mut generated =
            app.world().resource::<bevy::ecs::message::Messages<crate::TreeGenerated>>().get_cursor_current();
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 3.0;
        app.update();
        app.update();
        let rebuilt = generated.read(app.world().resource::<bevy::ecs::message::Messages<crate::TreeGenerated>>());
        assert_eq!(rebuilt.filter(|g| g.entity == tree).count(), 0, "a felled tree was rebuilt");
    }

    #[test]
    fn a_pooled_tree_is_felled_as_the_tree_of_its_variant() {
        let mut app = app();
        let pool = crate::build_pool(
            &TreePreset::Birch.params(),
            3,
            f32::INFINITY,
            &crate::LodSettings::default(),
            &mut app.world_mut().resource_mut::<Assets<Mesh>>(),
        );
        let expected = pool.tree_params(4);
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        let tree = app.world_mut().spawn((PooledTree { pool, variant: 4 }, Transform::default())).id();
        app.update();
        app.world_mut().entity_mut(tree).insert(FellTree { speed: 4.0, ..Default::default() });
        assert_eq!(settle(&mut app, 600), 1);
        assert!(app.world().get::<PooledTree>(tree).is_none(), "it is still pooled");
        assert_eq!(app.world().get::<ProceduralTree>(tree), Some(&ProceduralTree(expected)));
        assert!(app.world().get::<Felled>(tree).is_some_and(|f| f.at_rest));
    }

    #[test]
    fn on_a_slope_a_tree_is_cut_above_its_uphill_side_and_lands_on_the_terrain() {
        // The ground falls 0.4 m a metre toward +x, 22°.
        let slope = FloraGround::new(|at: Vec2| 3.0 - 0.4 * at.x);
        let params = TreePreset::Oak.params();
        let at = Vec2::new(2.0, 1.0);
        let fell = |ground: Option<&FloraGround>, direction: Vec2| {
            let mut app = app();
            let y = ground.map_or(0.0, |ground| ground.footing(at, params.base_radius()));
            if let Some(ground) = ground {
                app.insert_resource(ground.clone());
            }
            let base = Transform::from_xyz(at.x, y, at.y);
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), base)).id();
            app.update();
            app.world_mut().entity_mut(tree).insert(FellTree { direction, speed: 4.0, ..Default::default() });
            assert_eq!(settle(&mut app, 900), 1, "it did not land exactly once");
            let felled = app.world().get::<Felled>(tree).unwrap().clone();
            assert!(felled.at_rest, "it is still moving");
            (base, *app.world().get::<Transform>(tree).unwrap(), felled)
        };
        let (_, _, flat) = fell(None, Vec2::X);
        for (direction, downhill) in [(Vec2::X, true), (Vec2::NEG_X, false)] {
            let (base, lying, felled) = fell(Some(&slope), direction);
            // The saw went in half a metre above the ground on the trunk's uphill side.
            let cut = base.translation.y + felled.fall.hinge.y;
            let uphill = slope.top(at, params.base_radius());
            assert!((cut - uphill - 0.5).abs() < 1.0e-3, "cut at {cut}, the uphill ground at {uphill}");
            // Down the slope it turns further than on flat ground, up it less.
            let (angle, on_flat) = (felled.angle.to_degrees(), flat.angle.to_degrees());
            let turned = if downhill { angle > on_flat + 8.0 } else { angle < on_flat - 8.0 };
            assert!(turned, "downhill {downhill}: rests at {angle}°, {on_flat}° on flat ground");
            // And it lies on the terrain.
            let contact = lying.transform_point(felled.fall.contact);
            let gap = contact.y - slope.height(contact.xz());
            assert!(gap.abs() < 0.03, "downhill {downhill}: it touches the ground {gap} m off it");
        }
    }

    #[test]
    fn every_preset_felled_on_a_slope_lands_once_and_lies_on_it() {
        // 27°, falling toward +x.
        let slope = FloraGround::new(|at: Vec2| -0.5 * at.x);
        for preset in TreePreset::ALL {
            let params = preset.params();
            for direction in [Vec2::X, Vec2::NEG_X, Vec2::Y] {
                let mut app = app();
                app.insert_resource(slope.clone());
                let base = Transform::from_xyz(0.0, slope.footing(Vec2::ZERO, params.base_radius()), 0.0);
                let tree = app.world_mut().spawn((ProceduralTree(params.clone()), base)).id();
                app.update();
                app.world_mut().entity_mut(tree).insert(FellTree { direction, speed: 4.0, ..Default::default() });
                let landings = settle(&mut app, 900);
                let felled = app.world().get::<Felled>(tree).unwrap().clone();
                let lying = *app.world().get::<Transform>(tree).unwrap();
                let contact = lying.transform_point(felled.fall.contact);
                let gap = contact.y - slope.height(contact.xz());
                assert!(
                    felled.at_rest && landings == 1 && gap.abs() < 0.05,
                    "{preset:?} toward {direction}: landed {landings} times, at rest {}, at {}°, {gap} m off the ground",
                    felled.at_rest,
                    felled.angle.to_degrees()
                );
            }
        }
    }

    /// Fell a tree of `params` standing at `at` toward `direction` and buck it into logs of the
    /// default length; the `TreeBucked` message.
    fn fell_and_buck(app: &mut bevy::app::App, params: &TreeParams, at: Vec3, direction: Vec2) -> TreeBucked {
        use bevy::ecs::message::Messages;
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), Transform::from_translation(at))).id();
        app.update();
        let fell = FellTree { direction, speed: 4.0, ..Default::default() };
        app.world_mut().entity_mut(tree).insert((fell, BuckTree::default()));
        let mut cursor = app.world().resource::<Messages<TreeBucked>>().get_cursor_current();
        for _ in 0..900 {
            app.update();
            let bucked: Vec<TreeBucked> = cursor.read(app.world().resource::<Messages<TreeBucked>>()).cloned().collect();
            if let Some(bucked) = bucked.into_iter().next() {
                assert_eq!(bucked.tree, tree);
                assert!(app.world().get_entity(tree).is_err(), "the bucked tree is still there");
                return bucked;
            }
        }
        panic!("the tree was never bucked");
    }

    #[test]
    fn a_bucked_tree_becomes_logs_lying_on_the_ground_in_a_row_from_its_stump() {
        let params = TreePreset::Oak.params();
        // Flat, and falling 0.3 m a metre toward +x (17°).
        for slope in [None, Some(FloraGround::new(|at: Vec2| 1.0 - 0.3 * at.x))] {
            let ground = slope.clone().unwrap_or_default();
            for direction in [Vec2::X, Vec2::NEG_X] {
                let mut app = app();
                if let Some(slope) = &slope {
                    app.insert_resource(slope.clone());
                }
                let at = Vec3::new(0.0, ground.footing(Vec2::ZERO, params.base_radius()), 0.0);
                let bucked = fell_and_buck(&mut app, &params, at, direction);
                let what = format!("slope {}, toward {direction}", slope.is_some());
                assert!(bucked.logs.len() >= 2, "{what}: {} logs", bucked.logs.len());
                let lift = |p: Vec3| 1.0 / ground.normal(p.xz()).y;
                let mut reached = 0.0;
                for (index, &entity) in bucked.logs.iter().enumerate() {
                    let log = *app.world().get::<TreeLog>(entity).unwrap();
                    assert_eq!((log.index, log.stump), (index as u32, bucked.stump));
                    let place = app.world().get::<Transform>(entity).unwrap();
                    let (a, b) = (place.transform_point(log.capsule.a), place.transform_point(log.capsule.b));
                    assert!((a.distance(b) - 2.0).abs() < 0.04, "{what}: log {index} is {} m long", a.distance(b));
                    // Its butt end lies on the ground; its thinner top end too, a little lower.
                    let butt = a.y - ground.height(a.xz()) - log.capsule.radius * lift(a);
                    let top = b.y - ground.height(b.xz());
                    assert!(butt.abs() < 0.03, "{what}: log {index}'s butt end is {butt} m off the ground");
                    assert!(top > 0.0 && top < log.capsule.radius * lift(b) + 0.03, "{what}: its top end {top} m up");
                    // In a row from the stump, the way the tree fell, apart.
                    let (from, to) = (a.xz().dot(direction), b.xz().dot(direction));
                    assert!(from > reached + 0.02 && to > from, "{what}: log {index} from {from} to {to}, after {reached}");
                    reached = to;
                    let pieces = app.world().get::<Children>(entity).map_or(0, |c| c.len());
                    assert_eq!(pieces, 2, "{what}: a log is its wood and its end faces");
                }
            }
        }
    }

    #[test]
    fn every_preset_can_be_bucked() {
        for preset in TreePreset::ALL {
            let mut app = app();
            let bucked = fell_and_buck(&mut app, &preset.params(), Vec3::ZERO, Vec2::Y);
            for &entity in &bucked.logs {
                let log = app.world().get::<TreeLog>(entity).unwrap();
                let length = log.capsule.a.distance(log.capsule.b);
                assert!((length - 2.0).abs() < 0.04 && log.capsule.radius >= 0.08, "{preset:?}: {log:?}");
            }
        }
    }

    #[test]
    fn every_preset_falls_and_lies_still() {
        for preset in TreePreset::ALL {
            for seed in [42, 1, 2] {
                let mut app = app();
                let tree = app.world_mut().spawn(ProceduralTree(TreeParams { seed, ..preset.params() })).id();
                app.update();
                app.world_mut().entity_mut(tree).insert(FellTree { speed: 4.0, ..Default::default() });
                let landings = settle(&mut app, 900);
                let felled = app.world().get::<Felled>(tree).unwrap_or_else(|| panic!("{preset:?} was not felled"));
                assert!(
                    felled.at_rest && landings == 1,
                    "{preset:?} seed {seed}: landed {landings} times, at rest: {}, angle {}° of {}°, spin {}",
                    felled.at_rest,
                    felled.angle.to_degrees(),
                    felled.fall.rest.to_degrees(),
                    felled.fall.spin
                );
            }
        }
    }

    /// The positions of a tree's wood as drawn.
    fn drawn_wood(app: &bevy::app::App, tree: Entity) -> Vec<Vec3> {
        let handle = app.world().get::<TreeRender>(tree).unwrap().trunk_mesh.clone().unwrap();
        let mesh = app.world().resource::<Assets<Mesh>>().get(&handle).unwrap();
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        positions.iter().map(|p| Vec3::from_array(*p)).collect()
    }

    /// The positions and normals of the mesh `entity` draws.
    fn drawn(app: &bevy::app::App, entity: Entity) -> (Vec<Vec3>, Vec<Vec3>) {
        let handle = &app.world().get::<Mesh3d>(entity).unwrap().0;
        let mesh = app.world().resource::<Assets<Mesh>>().get(handle).unwrap();
        let read = |attribute| -> Vec<Vec3> {
            let values = mesh.attribute(attribute).unwrap().as_float3().unwrap();
            values.iter().map(|p| Vec3::from_array(*p)).collect()
        };
        (read(Mesh::ATTRIBUTE_POSITION), read(Mesh::ATTRIBUTE_NORMAL))
    }

    #[test]
    fn a_notch_is_carved_into_a_standing_tree_deepens_with_each_blow_and_heals() {
        let mut app = app();
        let params = TreePreset::Oak.params();
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), Transform::default())).id();
        app.update();
        let whole = drawn_wood(&app, tree);
        let wood = build_tree(&params, f32::INFINITY).trunk;
        // The trunk's width along -z (the side cut from) at the notch's floor, from the wood itself.
        let level = cut(&wood, 0.6, false);
        let ring = level.outlines.iter().max_by_key(|o| o.len()).unwrap();
        let front = ring.iter().map(|p| -p.z).fold(f32::NEG_INFINITY, f32::max);
        let width = front - ring.iter().map(|p| -p.z).fold(f32::INFINITY, f32::min);
        for depth in [0.1, 0.2, 0.3] {
            let notch = TreeNotch { direction: Vec2::new(0.0, -1.0), height: 0.6, depth };
            app.world_mut().entity_mut(tree).insert(notch);
            app.update();
            let wedge = notch_in(&wood, &notch, &Transform::default(), &params, None).unwrap();
            let face = wedge.face();
            let inside = drawn_wood(&app, tree)
                .into_iter()
                .filter(|p| p.y > wedge.floor + 1e-3 && p.y < wedge.mouth - 1e-3 && face.distance(*p) < -1e-3);
            assert_eq!(inside.count(), 0, "depth {depth}: wood left in the notch");
            // Its faces reach `depth` of the way through the trunk, from the front.
            let faces = app.world().get::<NotchCarved>(tree).unwrap().faces.expect("no faces drawn");
            let (positions, _) = drawn(&app, faces);
            let reached = front - positions.iter().map(|p| -p.z).fold(f32::INFINITY, f32::min);
            assert!((reached / width - depth).abs() < 0.03, "depth {depth}: the notch reaches {} of the way", reached / width);
        }
        let faces = app.world().get::<NotchCarved>(tree).unwrap().faces.unwrap();
        app.world_mut().entity_mut(tree).remove::<TreeNotch>();
        app.update();
        assert_eq!(drawn_wood(&app, tree), whole, "the trunk is not whole again");
        assert!(app.world().get_entity(faces).is_err(), "the notch's faces are still drawn");
    }

    #[test]
    fn a_notched_tree_is_felled_at_its_notch_and_falls_with_its_face() {
        let mut app = app();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::default())).id();
        app.update();
        app.world_mut().entity_mut(tree).insert(TreeNotch { direction: Vec2::X, height: 0.8, depth: 0.3 });
        app.update();
        let standing = app.world().get::<NotchCarved>(tree).unwrap().faces.unwrap();
        app.world_mut().entity_mut(tree).insert(FellTree { direction: Vec2::X, cut_height: 0.3, speed: 4.0, ..Default::default() });
        assert_eq!(settle(&mut app, 900), 1, "it did not land exactly once");
        let felled = app.world().get::<Felled>(tree).unwrap().clone();
        assert!((felled.fall.hinge.y - 0.8).abs() < 1e-4, "cut at {}, not at the notch's floor", felled.fall.hinge.y);
        assert!(app.world().get_entity(standing).is_err(), "the standing notch's faces are still drawn");
        assert!(app.world().get::<NotchCarved>(tree).is_none());
        // The fallen tree's faces: the notch's face (out of the trunk and down) and the cut behind
        // it (down); the floor stays on the stump.
        let cut_face = app.world().resource::<CutFace>().0.clone().unwrap();
        let children: Vec<Entity> = app.world().get::<Children>(tree).unwrap().iter().collect();
        let faces = children
            .into_iter()
            .find(|&c| app.world().get::<MeshMaterial3d<StandardMaterial>>(c).is_some_and(|m| m.0 == cut_face))
            .expect("the fallen tree has no cut faces");
        let (_, normals) = drawn(&app, faces);
        assert!(normals.iter().any(|n| n.x > 0.5 && n.y < -0.5), "the fallen tree has no notch face");
        assert!(normals.iter().any(|n| n.y < -0.99), "the fallen tree has no cut behind its notch");
        assert!(!normals.iter().any(|n| n.y > 0.0), "the fallen tree carries the notch's floor");
    }

    #[test]
    fn a_prism_trunk_can_be_notched_felled_and_bucked() {
        use crate::params::TrunkStyle;
        let params = crate::crown_hull::hull_spruce();

        // Notched: its faces are drawn, and the notch is felled at its floor.
        for (floor, cut_height) in [(0.8, 0.3), (0.6, 0.5)] {
            let mut app = app();
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), Transform::default())).id();
            app.update();
            app.world_mut().entity_mut(tree).insert(TreeNotch { direction: Vec2::X, height: floor, depth: 0.3 });
            app.update();
            let faces = app.world().get::<NotchCarved>(tree).and_then(|carved| carved.faces);
            assert!(faces.is_some(), "a prism trunk's notch has no faces");
            app.world_mut().entity_mut(tree).insert(FellTree { direction: Vec2::X, cut_height, speed: 4.0, ..Default::default() });
            assert_eq!(settle(&mut app, 900), 1, "the notched prism (floor {floor}) did not land exactly once");
            let felled = app.world().get::<Felled>(tree).unwrap().clone();
            assert!(felled.at_rest && (felled.fall.hinge.y - floor).abs() < 1e-4, "cut at {}, floor {floor}", felled.fall.hinge.y);
        }

        // Bucked: logs a length long, as thick as the round trunk's (a flat trunk's cut has a point
        // in the middle of each side, which must not pull the log's radius in).
        let (mut app, mut round_app) = (self::app(), self::app());
        let bucked = fell_and_buck(&mut app, &params, Vec3::ZERO, Vec2::Y);
        let round = TreeParams { trunk_style: TrunkStyle::Round, ..params.clone() };
        let round_logs = fell_and_buck(&mut round_app, &round, Vec3::ZERO, Vec2::Y);
        assert!(!bucked.logs.is_empty(), "the prism trunk made no logs");
        let radius = |app: &bevy::app::App, bucked: &TreeBucked| app.world().get::<TreeLog>(bucked.logs[0]).unwrap().capsule.radius;
        for &entity in &bucked.logs {
            let log = app.world().get::<TreeLog>(entity).unwrap();
            let length = log.capsule.a.distance(log.capsule.b);
            assert!((length - 2.0).abs() < 0.04 && log.capsule.radius >= 0.08, "{log:?}");
        }
        let (flat, smooth) = (radius(&app, &bucked), radius(&round_app, &round_logs));
        assert!((0.7..=1.1).contains(&(flat / smooth)), "the prism log is {flat} thick, the round one {smooth}");
    }
}
