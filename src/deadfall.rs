//! Pooled deadfall: the stumps, fallen trees and bucked logs of a forest floor, drawn from a
//! [`TreePool`] as its trees are, so thousands of them cost a handful of draws.
//!
//! [`build_pool_with_deadfall`](crate::build_pool_with_deadfall) cuts every variant once, as [`FellTree`](crate::FellTree) would
//! (the stump and its cut face, the wood above the cut at each level of detail and its face, the
//! contact points a fallen tree rests on, the cut's outline for the hinge, and the logs
//! [`BuckTree`] would make), and a [`PooledDeadfall`] places one: a stump stands
//! where it was put, a lying tree is turned about the hinge of its cut until its limbs reach the
//! ground there ([`FloraGround`] included), and bucked logs lie in a row from the stump.
//!
//! Deadfall is static and never indexed ([`FloraIndex`](crate::FloraIndex)), shakes nothing and
//! sheds nothing; it is fire fuel, and takes [`NotShadowCaster`] like a pooled tree does. Insert
//! [`BuckTree`] on a lying one and it becomes bucked in place.

use bevy::asset::{Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::transform::components::Transform;

use crate::felling::{
    BuckTree, CutFace, LOG_GAP, TreeLog, TreeStump, contact_points, fall_direction, hinge_point, lay_log, log_end,
    rest_angle, saw_height, trunk_alone,
};
use crate::ground::FloraGround;
use crate::lod::LodDistances;
use crate::mesh::MeshData;
use crate::mesh::cut::cut;
use crate::params::TreeParams;
use crate::pipeline::TreeMeshes;
use crate::plugin::{crown_material, material};
use crate::pool::{PoolVariant, TreePool, banded};
use crate::shading::{FloraMaterial, FloraMaterialSupport, FloraWindBuffer, SharedFlora, TextureLibrary};
use crate::shape::{TreeShape, WoodCapsule};

/// How a pool's trees are cut into deadfall ([`build_pool_with_deadfall`](crate::build_pool_with_deadfall)). Lengths are in
/// metres, at the tree's own scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeadfallSettings {
    /// How high the saw goes in, as [`FellTree::cut_height`](crate::FellTree::cut_height).
    pub cut_height: f32,
    /// How long the logs of a bucked tree are, as [`BuckTree::log_length`](crate::BuckTree::log_length);
    /// `None` makes none ([`DeadfallKind::Bucked`] is then a stump).
    pub log_length: Option<f32>,
    /// The thinnest log's top end, as [`BuckTree::min_radius`](crate::BuckTree::min_radius).
    pub min_radius: f32,
}

impl Default for DeadfallSettings {
    fn default() -> Self {
        Self { cut_height: 0.5, log_length: Some(2.0), min_radius: 0.08 }
    }
}

/// One log of a bucked variant: its wood, its end faces and its ends, as [`TreeLog`] has them (in
/// the tree's own space: the butt end's centre and radius, the top end's).
#[derive(Clone, Debug)]
pub struct PoolLog {
    /// The wood.
    pub wood: Handle<Mesh>,
    /// The two end faces, if it has any.
    pub ends: Option<Handle<Mesh>>,
    /// The butt end's centre.
    pub c0: Vec3,
    /// The top end's centre.
    pub c1: Vec3,
    /// The butt end's radius.
    pub r0: f32,
    /// The top end's radius.
    pub r1: f32,
}

/// What a [`TreePool`] variant leaves when it is cut ([`build_pool_with_deadfall`](crate::build_pool_with_deadfall)).
#[derive(Clone, Debug)]
pub struct Deadfall {
    /// The saw's height, in the tree's own space.
    pub cut_y: f32,
    /// The stump: the wood below the cut, and its cut face.
    pub stump: Handle<Mesh>,
    /// The stump's cut face.
    pub stump_face: Option<Handle<Mesh>>,
    /// The wood above the cut at levels 0, 1 and 2: what lies on the ground.
    pub lying: [Handle<Mesh>; 3],
    /// The cut face of the part that lies.
    pub lying_face: Option<Handle<Mesh>>,
    /// The points of the lying part that can hold it up, few of them: what a fall's rest angle is
    /// searched over.
    pub contacts: Vec<Vec3>,
    /// The outline of the trunk's cut, for the hinge it turns about.
    pub outline: Vec<Vec3>,
    /// The logs of the bucked tree, butt log first.
    pub logs: Vec<PoolLog>,
}

/// Most contact points a variant keeps: the rest angle is searched over them for every lying tree.
const MAX_CONTACTS: usize = 600;

/// Cut a variant's three levels of detail as a felled and bucked tree: see [`Deadfall`].
pub(crate) fn build_deadfall(
    params: &TreeParams,
    levels: &[TreeMeshes; 3],
    settings: &DeadfallSettings,
    mut add: impl FnMut(MeshData) -> Handle<Mesh>,
) -> Deadfall {
    let fine = &levels[0];
    let cut_y = saw_height(&Transform::IDENTITY, params, None, settings.cut_height, 1.0);
    let above = levels.each_ref().map(|level| cut(&level.trunk, cut_y, true));
    let below = cut(&fine.trunk, cut_y, false);
    // The trunk's outline is the one that encloses the most.
    let area = |o: &Vec<Vec3>| {
        let twice: f32 = (0..o.len()).map(|i| o[i].x * o[(i + 1) % o.len()].z - o[(i + 1) % o.len()].x * o[i].z).sum();
        twice.abs()
    };
    let outline = above[0].outlines.iter().max_by(|a, b| area(a).total_cmp(&area(b))).cloned().unwrap_or_default();
    let (_, reach) = hinge_point(std::slice::from_ref(&outline), cut_y, Vec3::X);
    let mut contacts = contact_points(&above[0].kept, &fine.skeleton, params, cut_y, reach);
    let stride = contacts.len().div_ceil(MAX_CONTACTS).max(1);
    contacts = contacts.into_iter().step_by(stride).collect();

    let mut logs = Vec::new();
    if let (Some(length), Some(node)) = (settings.log_length, fine.skeleton.nodes.first()) {
        let length = length.max(0.1);
        let trunk = trunk_alone(params, node);
        let top = trunk.positions.iter().map(|p| p.y).fold(0.0, f32::max);
        let mut y0 = cut_y;
        while y0 + length <= top {
            let y1 = y0 + length;
            let lower = cut(&trunk, y0, true);
            let piece = cut(&lower.kept, y1, false);
            let (Some((c0, r0)), Some((c1, r1))) = (log_end(&lower.outlines), log_end(&piece.outlines)) else { break };
            if r1 < settings.min_radius {
                break;
            }
            let mut ends = lower.faces;
            ends.extend(&piece.faces);
            logs.push(PoolLog {
                wood: add(piece.kept),
                ends: (!ends.is_empty()).then(|| add(ends)),
                c0,
                c1,
                r0,
                r1,
            });
            y0 = y1;
        }
    }
    let [above0, above1, above2] = above;
    Deadfall {
        cut_y,
        stump: add(below.kept),
        stump_face: (!below.faces.is_empty()).then(|| add(below.faces)),
        lying_face: (!above0.faces.is_empty()).then(|| add(above0.faces)),
        lying: [add(above0.kept), add(above1.kept), add(above2.kept)],
        contacts,
        outline,
        logs,
    }
}

/// What a [`PooledDeadfall`] is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeadfallKind {
    /// A stump: the wood below the cut and its face ([`TreeStump`] with no tree).
    Stump,
    /// A tree that fell toward `direction` (world x, z) and lies on the ground, as a
    /// [`FellTree`](crate::FellTree) leaves one.
    Lying {
        /// The way it fell.
        direction: Vec2,
    },
    /// A tree that fell toward `direction` and was bucked: its stump, and its logs in a row from
    /// it ([`TreeLog`]).
    Bucked {
        /// The way it fell.
        direction: Vec2,
    },
}

/// A stump, a fallen tree or bucked logs placed from a [`TreePool`] built with
/// [`build_pool_with_deadfall`](crate::build_pool_with_deadfall). Static, never in the [`FloraIndex`](crate::FloraIndex), no
/// impostor. The plugin gives it its children once its pool is ready; `variant` is taken modulo
/// the pool's variants. Inserting [`BuckTree`] on a lying one makes it bucked, in place.
#[derive(Component, Clone, Debug)]
#[require(Transform, Visibility)]
pub struct PooledDeadfall {
    /// The pool to take it from, which must have been built with deadfall.
    pub pool: Handle<TreePool>,
    /// Which variant of the pool it is.
    pub variant: u32,
    /// What it is.
    pub kind: DeadfallKind,
}

/// Marks a [`PooledDeadfall`] that has its children.
#[derive(Component)]
pub(crate) struct DeadfallSpawned;

/// Deadfall given its children in one frame, at most.
const MAX_PER_FRAME: usize = 4096;

/// The materials deadfall is drawn with: where they are chosen, for every kind. A fallen tree does
/// not sway (the wind response of its look is zero), so wood and leaves are of the calm look.
#[derive(Clone)]
pub(crate) struct DeadMaterials {
    pub wood: Material,
    pub leaves: Material,
}

#[derive(Clone)]
pub(crate) enum Material {
    Flora(Handle<FloraMaterial>),
    Standard(Handle<StandardMaterial>),
}

fn dead_materials(
    pool: &TreePool,
    wind: Option<&FloraWindBuffer>,
    flora: &mut Assets<FloraMaterial>,
    shared: &mut SharedFlora,
    standard: &mut Assets<StandardMaterial>,
    textures: &mut TextureLibrary,
) -> DeadMaterials {
    let calm = TreeParams { wind: crate::WindResponse { sway: 0.0, ..pool.params.wind.clone() }, ..pool.params.clone() };
    match wind {
        Some(wind) => DeadMaterials {
            wood: Material::Flora(shared.resolve(flora, &textures.wood(&calm), wind)),
            leaves: Material::Flora(shared.resolve(flora, &textures.leaves(&calm), wind)),
        },
        None => DeadMaterials {
            wood: Material::Standard(standard.add(material(pool.params.trunk_color))),
            leaves: Material::Standard(standard.add(crown_material(&pool.params))),
        },
    }
}

fn spawn_mesh(commands: &mut Commands, parent: Entity, mesh: &Handle<Mesh>, material: &Material, at: Transform, quiet: bool) -> Entity {
    let mut child = commands.spawn((Mesh3d(mesh.clone()), at, ChildOf(parent)));
    match material {
        Material::Flora(m) => child.insert(MeshMaterial3d(m.clone())),
        Material::Standard(m) => child.insert(MeshMaterial3d(m.clone())),
    };
    if quiet {
        child.insert(NotShadowCaster);
    }
    child.id()
}

/// The rotation about `hinge` that turns a tree `angle` about `axis`, as a transform in its space.
fn pivot_about(hinge: Vec3, axis: Vec3, angle: f32) -> Transform {
    let turn = Quat::from_axis_angle(axis, angle);
    Transform::from_translation(hinge - turn * hinge).with_rotation(turn)
}

/// Give every [`PooledDeadfall`] whose pool is ready its children.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn spawn_pooled_deadfall(
    mut commands: Commands,
    pools: Res<Assets<TreePool>>,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut textures: TextureLibrary,
    mut cut_face: ResMut<CutFace>,
    ground: Option<Res<FloraGround>>,
    deadfall: Query<(Entity, &PooledDeadfall, &Transform, Has<NotShadowCaster>), Without<DeadfallSpawned>>,
    mut made: Local<bevy::platform::collections::HashMap<bevy::asset::AssetId<TreePool>, DeadMaterials>>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let mut spawned = 0;
    for (entity, dead, base, quiet) in &deadfall {
        let Some(pool) = pools.get(&dead.pool) else { continue };
        let variant = &pool.variants[dead.variant as usize % pool.variants.len()];
        let Some(cut) = &variant.deadfall else {
            bevy::log::warn_once!("bevy_flora: a PooledDeadfall's pool was built without deadfall (`build_pool_with_deadfall`)");
            continue;
        };
        let face = cut_face.get(&mut textures, &mut standard, false);
        if !made.contains_key(&dead.pool.id()) {
            let materials = dead_materials(pool, wind, &mut flora, &mut shared, &mut standard, &mut textures);
            made.insert(dead.pool.id(), materials);
        }
        let materials = &made[&dead.pool.id()];
        let face = Material::Standard(face);
        commands.entity(entity).insert(DeadfallSpawned);
        spawned += 1;
        match dead.kind {
            DeadfallKind::Stump => spawn_stump(&mut commands, entity, cut, materials, &face, quiet),
            DeadfallKind::Lying { direction } => {
                let lie = lie(base, cut, direction, ground.as_deref());
                spawn_lying(&mut commands, entity, variant, cut, materials, &face, lie.pivot, quiet, pool.lod.distances);
            }
            DeadfallKind::Bucked { direction } => {
                spawn_stump(&mut commands, entity, cut, materials, &face, quiet);
                let lie = lie(base, cut, direction, ground.as_deref());
                let up = lie.pivot.rotation * Vec3::Y;
                let along = Vec2::new(up.x, up.z).normalize_or(Vec2::X);
                let scale = base.scale.y.max(1.0e-4);
                let rest = |p: Vec2| match ground.as_deref() {
                    None => (base.translation.y, 1.0),
                    Some(g) => (g.height(p), 1.0 / g.normal(p).y.max(0.2)),
                };
                let local_dir = fall_direction(base, direction);
                let (hinge, _) = hinge_point(std::slice::from_ref(&cut.outline), cut.cut_y, local_dir);
                let mut at = base.transform_point(hinge).xz() + along * LOG_GAP;
                for (index, log) in cut.logs.iter().enumerate() {
                    let length = (log.c1 - log.c0).length() * scale;
                    let place = lay_log(base, at, along, length, (log.c0, log.r0), (log.c1, log.r1), rest);
                    let capsule = WoodCapsule { a: log.c0, b: log.c1, radius: log.r0 };
                    // Placed in the world, as bucking a tree does; a child of the stump, so it goes
                    // with it, in the stump's space.
                    let local = Transform::from_matrix(base.to_matrix().inverse() * place.to_matrix());
                    let entity_log = commands
                        .spawn((local, Visibility::Inherited, TreeLog { stump: entity, index: index as u32, capsule }, ChildOf(entity)))
                        .id();
                    spawn_mesh(&mut commands, entity_log, &log.wood, &materials.wood, Transform::IDENTITY, quiet);
                    if let Some(ends) = &log.ends {
                        spawn_mesh(&mut commands, entity_log, ends, &face, Transform::IDENTITY, quiet);
                    }
                    at = place.transform_point(log.c1).xz() + along * LOG_GAP;
                }
            }
        }
        if spawned >= MAX_PER_FRAME {
            break;
        }
    }
}

struct Lie {
    pivot: Transform,
}

/// How a tree of `cut` standing as `base` lies after falling toward `direction` (world x, z): the
/// turn about the hinge of its cut, until its limbs reach the ground.
fn lie(base: &Transform, cut: &Deadfall, direction: Vec2, ground: Option<&FloraGround>) -> Lie {
    let scale = base.scale.y.max(1.0e-4);
    let local_dir = fall_direction(base, direction);
    let (hinge, _) = hinge_point(std::slice::from_ref(&cut.outline), cut.cut_y, local_dir);
    let axis = Vec3::Y.cross(local_dir).normalize_or(Vec3::Z);
    let clearance = |p: Vec3| match ground {
        None => p.y,
        Some(ground) => {
            let world = base.transform_point(p);
            (world.y - ground.height(world.xz())) / scale
        }
    };
    let (rest, _) = rest_angle(&cut.contacts, hinge, axis, clearance);
    Lie { pivot: pivot_about(hinge, axis, rest) }
}

fn spawn_stump(commands: &mut Commands, entity: Entity, cut: &Deadfall, materials: &DeadMaterials, face: &Material, quiet: bool) {
    commands.entity(entity).insert(TreeStump { tree: None });
    spawn_mesh(commands, entity, &cut.stump, &materials.wood, Transform::IDENTITY, quiet);
    if let Some(stump_face) = &cut.stump_face {
        spawn_mesh(commands, entity, stump_face, face, Transform::IDENTITY, quiet);
    }
}

/// The part that lies: its wood and its leaves at every level of detail, each child turned by
/// `pivot`, and its face; no impostor, and the variant's shape.
#[allow(clippy::too_many_arguments)]
fn spawn_lying(
    commands: &mut Commands,
    entity: Entity,
    variant: &PoolVariant,
    cut: &Deadfall,
    materials: &DeadMaterials,
    face: &Material,
    pivot: Transform,
    quiet: bool,
    distances: LodDistances,
) {
    let bands = [banded(0.0, distances.lod1), banded(distances.lod1, distances.lod2), banded(distances.lod2, f32::INFINITY)];
    for (level, (lod_band, range)) in bands.into_iter().enumerate() {
        let wood = spawn_mesh(commands, entity, &cut.lying[level], &materials.wood, pivot, quiet);
        commands.entity(wood).insert((lod_band, range.clone(), variant.bounds[level]));
        if let Some(leaves) = &variant.leaves[level] {
            let leaf = spawn_mesh(commands, entity, leaves, &materials.leaves, pivot, quiet);
            commands.entity(leaf).insert((lod_band, range, variant.bounds[level]));
        }
    }
    if let Some(lying_face) = &cut.lying_face {
        spawn_mesh(commands, entity, lying_face, face, pivot, quiet);
    }
    // The variant's shape, in the space of the lying tree.
    commands.spawn((pivot, TreeShape::clone(&variant.shape), ChildOf(entity)));
}

/// A [`PooledDeadfall`] given [`BuckTree`] that lies is bucked in place: its children go and it is
/// spawned again as [`DeadfallKind::Bucked`].
pub(crate) fn buck_pooled_deadfall(
    mut commands: Commands,
    mut deadfall: Query<(Entity, &mut PooledDeadfall), With<BuckTree>>,
) {
    for (entity, mut dead) in &mut deadfall {
        commands.entity(entity).remove::<BuckTree>();
        if let DeadfallKind::Lying { direction } = dead.kind {
            dead.kind = DeadfallKind::Bucked { direction };
            commands.entity(entity).despawn_related::<Children>().remove::<(DeadfallSpawned, TreeStump)>();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::felling::{FellTree, Felled};
    use crate::lod::LodSettings;
    use crate::plugin::{FloraPlugin, ProceduralTree};
    use crate::presets::TreePreset;
    use crate::{FloraIndex, PooledTree};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app
    }

    fn pool(app: &mut App, preset: TreePreset) -> Handle<TreePool> {
        let params = preset.params();
        let lod = LodSettings { distances: LodDistances { impostor: f32::INFINITY, ..Default::default() }, ..Default::default() };
        let pool = {
            let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
            crate::pool::build_pool_with_deadfall(&params, 2, f32::INFINITY, &lod, &DeadfallSettings::default(), &mut meshes)
        };
        app.world_mut().resource_mut::<Assets<TreePool>>().add(pool)
    }

    fn descendants(app: &App, entity: Entity) -> Vec<Entity> {
        app.world().get::<Children>(entity).map_or_else(Vec::new, |c| c.iter().collect())
    }

    #[test]
    fn a_pooled_lying_tree_lies_as_a_felled_one_of_its_variant_does_on_flat_ground() {
        let mut app = app();
        let pool = pool(&mut app, TreePreset::Oak);
        for direction in [Vec2::X, Vec2::new(-0.6, 0.8)] {
            let at = Transform::from_xyz(3.0, 0.0, -2.0);
            let dead = app.world_mut().spawn((PooledDeadfall { pool: pool.clone(), variant: 1, kind: DeadfallKind::Lying { direction } }, at)).id();
            let params = app.world().resource::<Assets<TreePool>>().get(&pool).unwrap().tree_params(1);
            let felled = app
                .world_mut()
                .spawn((ProceduralTree(params), at, FellTree { direction, speed: 4.0, lying: true, ..Default::default() }))
                .id();
            for _ in 0..30 {
                app.update();
            }
            let rest = app.world().get::<Felled>(felled).expect("the felled tree").angle;
            let turned = descendants(&app, dead)
                .into_iter()
                .filter_map(|c| app.world().get::<Transform>(c))
                .map(|t| t.rotation.to_axis_angle().1)
                .fold(0.0_f32, f32::max);
            assert!(rest > 0.3, "{direction}: the felled tree rests at {rest}");
            assert!((turned - rest).abs() < 3.0_f32.to_radians(), "{direction}: pooled {} degrees, felled {}", turned.to_degrees(), rest.to_degrees());
            // Its place on the ground is the felled tree's: the lying trunk's base is at the hinge.
            let felled_at = app.world().get::<Transform>(felled).unwrap().translation;
            let pivot = descendants(&app, dead).into_iter().filter_map(|c| app.world().get::<Transform>(c)).next().unwrap();
            let placed = at.translation + at.rotation * pivot.translation;
            assert!(placed.distance(felled_at) < 0.2, "{direction}: pooled at {placed}, felled at {felled_at}");
        }
    }

    #[test]
    fn a_hundred_lying_trees_draw_a_variants_meshes_with_one_material_pair_and_are_never_indexed() {
        let mut app = app();
        let pool = pool(&mut app, TreePreset::Oak);
        let mut roots = Vec::new();
        for i in 0..100 {
            let kind = DeadfallKind::Lying { direction: Vec2::from_angle(i as f32 * 0.7) };
            roots.push(app.world_mut().spawn((
                PooledDeadfall { pool: pool.clone(), variant: 0, kind },
                Transform::from_xyz((i % 10) as f32 * 30.0, 0.0, (i / 10) as f32 * 30.0),
            )).id());
        }
        app.update();
        app.update();
        let mut meshes = std::collections::HashSet::new();
        let mut materials = std::collections::HashSet::new();
        for &root in &roots {
            let kids = descendants(&app, root);
            assert!(!kids.is_empty(), "a lying tree has no children");
            for kid in kids {
                if let Some(m) = app.world().get::<Mesh3d>(kid) {
                    meshes.insert(m.0.id());
                }
                if let Some(m) = app.world().get::<MeshMaterial3d<StandardMaterial>>(kid) {
                    materials.insert(m.0.id());
                }
            }
        }
        // Three levels of wood, up to three of leaves, the cut face; wood and leaves, and the face.
        assert!(meshes.len() <= 7, "{} meshes", meshes.len());
        assert_eq!(materials.len(), 3, "wood, leaves and the end grain");
        assert_eq!(app.world().resource::<FloraIndex>().len(), 0, "deadfall is never indexed");
        let _ = PooledTree { pool: pool.clone(), variant: 0 };
    }

    #[test]
    fn bucked_logs_lie_in_a_row_on_the_ground_from_the_stump_and_a_stump_is_a_stump() {
        let mut app = app();
        let pool = pool(&mut app, TreePreset::Oak);
        let direction = Vec2::X;
        let dead = app.world_mut().spawn((PooledDeadfall { pool: pool.clone(), variant: 0, kind: DeadfallKind::Bucked { direction } }, Transform::IDENTITY)).id();
        let stump = app.world_mut().spawn((PooledDeadfall { pool, variant: 0, kind: DeadfallKind::Stump }, Transform::from_xyz(40.0, 0.0, 0.0))).id();
        app.update();
        app.update();
        assert!(app.world().get::<TreeStump>(dead).is_some_and(|s| s.tree.is_none()), "the bucked tree's stump");
        assert!(app.world().get::<TreeStump>(stump).is_some_and(|s| s.tree.is_none()), "a stump");
        // The root stands at the origin and there is no transform propagation here: a log's own
        // transform is its place in the world.
        let mut logs: Vec<(TreeLog, Transform)> = descendants(&app, dead)
            .into_iter()
            .filter_map(|c| Some((*app.world().get::<TreeLog>(c)?, *app.world().get::<Transform>(c)?)))
            .collect();
        logs.sort_by_key(|(log, _)| log.index);
        assert!(logs.len() >= 2, "{} logs", logs.len());
        let mut reached = 0.0;
        for (log, global) in &logs {
            let (a, b) = (global.transform_point(log.capsule.a), global.transform_point(log.capsule.b));
            assert!((a.distance(b) - 2.0).abs() < 0.05, "log {} is {} long", log.index, a.distance(b));
            assert!((a.y - log.capsule.radius).abs() < 0.03, "log {}'s butt end is {} off the ground", log.index, a.y - log.capsule.radius);
            let (from, to) = (a.xz().dot(direction), b.xz().dot(direction));
            assert!(from > reached + 0.02 && to > from, "log {} from {from} to {to}, after {reached}", log.index);
            reached = to;
        }
    }

    #[test]
    fn inserting_buck_tree_on_a_lying_tree_makes_it_bucked_in_place() {
        let mut app = app();
        let pool = pool(&mut app, TreePreset::Oak);
        let dead = app.world_mut().spawn((PooledDeadfall { pool, variant: 0, kind: DeadfallKind::Lying { direction: Vec2::X } }, Transform::IDENTITY)).id();
        app.update();
        app.world_mut().entity_mut(dead).insert(BuckTree::default());
        for _ in 0..3 {
            app.update();
        }
        assert!(matches!(app.world().get::<PooledDeadfall>(dead).unwrap().kind, DeadfallKind::Bucked { .. }));
        assert!(descendants(&app, dead).into_iter().any(|c| app.world().get::<TreeLog>(c).is_some()), "no logs");
    }
}
