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

use std::sync::Arc;

use bevy::asset::{AssetId, Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::ecs::system::EntityCommands;
use bevy::light::NotShadowCaster;
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::transform::components::Transform;

use crate::felling::{
    BuckTree, CutFace, LOG_GAP, TreeLog, TreeStump, calm_params, contact_points, fall_direction, hinge_point, lay_log, log_end,
    rest_angle, saw_height, trunk_alone,
};
use crate::ground::FloraGround;
use crate::lod::LodDistances;
use crate::mesh::MeshData;
use crate::mesh::cut::cut;
use crate::params::TreeParams;
use crate::pipeline::TreeMeshes;
use crate::plugin::{crown_material, material};
use crate::pool::{PoolColors, PoolRecolors, PoolVariant, PooledLook, RecolorPool, TreePool, banded};
use crate::recolor::CutWood;
use crate::units::FloraUnits;
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

/// What a mesh child of a [`PooledDeadfall`] is, so a change of look or colours can re-dress it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeadRole {
    Wood,
    Leaves,
    Face,
}

/// The materials one look of a pool's deadfall is drawn with: where they are chosen, for every
/// kind. A fallen tree does not sway (the wind response of its look is zero), so wood and leaves
/// are of the calm look, of the look's current colours (`RecolorPool` included).
#[derive(Clone)]
pub(crate) struct DeadMaterials {
    wood: Material,
    leaves: Material,
    face: Material,
    /// The colours they are made in.
    colors: PoolColors,
}

#[derive(Clone)]
pub(crate) enum Material {
    Flora(Handle<FloraMaterial>),
    Standard(Handle<StandardMaterial>),
}

impl DeadMaterials {
    fn of(&self, role: DeadRole) -> &Material {
        match role {
            DeadRole::Wood => &self.wood,
            DeadRole::Leaves => &self.leaves,
            DeadRole::Face => &self.face,
        }
    }
}

/// The deadfall materials made so far, by pool and look: one set for every instance of a look.
#[derive(Resource, Default)]
pub(crate) struct DeadMaterialCache(bevy::platform::collections::HashMap<(AssetId<TreePool>, u32), DeadMaterials>);

/// The materials of `look` (taken modulo the pool's looks) of the pool `id`, made on first use.
#[allow(clippy::too_many_arguments)]
fn dead_materials<'a>(
    cache: &'a mut DeadMaterialCache,
    id: AssetId<TreePool>,
    pool: &TreePool,
    look: u32,
    recolors: &PoolRecolors,
    wind: Option<&FloraWindBuffer>,
    flora: &mut Assets<FloraMaterial>,
    shared: &mut SharedFlora,
    standard: &mut Assets<StandardMaterial>,
    textures: &mut TextureLibrary,
    cut_face: &mut CutFace,
) -> &'a DeadMaterials {
    let look = look % pool.looks.len().max(1) as u32;
    cache.0.entry((id, look)).or_insert_with(|| {
        let colors = recolors.colors(id, look, pool);
        let face = Material::Standard(cut_face.get(textures, standard, colors.unlit));
        let calm = calm_params(&pool.params);
        match wind {
            Some(wind) => {
                let (wood, leaves) = colors.surfaces(&calm, textures);
                DeadMaterials {
                    wood: Material::Flora(shared.resolve(flora, &wood, wind)),
                    leaves: Material::Flora(shared.resolve(flora, &leaves, wind)),
                    face,
                    colors,
                }
            }
            None => {
                let unlit = |base: StandardMaterial| StandardMaterial { unlit: colors.unlit, ..base };
                DeadMaterials {
                    wood: Material::Standard(standard.add(unlit(material(colors.wood)))),
                    leaves: Material::Standard(standard.add(unlit(crown_material(&colors.apply(&pool.params))))),
                    face,
                    colors,
                }
            }
        }
    })
}

fn dress(child: &mut EntityCommands, material: &Material) {
    match material {
        Material::Flora(m) => child.insert(MeshMaterial3d(m.clone())),
        Material::Standard(m) => child.insert(MeshMaterial3d(m.clone())),
    };
}

fn spawn_mesh(
    commands: &mut Commands,
    parent: Entity,
    mesh: &Handle<Mesh>,
    role: DeadRole,
    materials: &DeadMaterials,
    at: Transform,
    quiet: bool,
) -> Entity {
    let mut child = commands.spawn((Mesh3d(mesh.clone()), at, role, ChildOf(parent)));
    dress(&mut child, materials.of(role));
    if quiet {
        child.insert(NotShadowCaster);
    }
    child.id()
}

/// What [`RecolorTree`](crate::RecolorTree) needs to dress a stump or a log: the pool's
/// parameters in the colours of the look, calm.
fn cut_wood(pool: &TreePool, materials: &DeadMaterials) -> CutWood {
    CutWood { params: Arc::new(materials.colors.apply(&pool.params)), unlit: materials.colors.unlit, calm: true }
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
    recolors: Res<PoolRecolors>,
    mut cache: ResMut<DeadMaterialCache>,
    ground: Option<Res<FloraGround>>,
    units: Res<FloraUnits>,
    deadfall: Query<(Entity, &PooledDeadfall, Option<&PooledLook>, &Transform, Has<NotShadowCaster>), Without<DeadfallSpawned>>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let mut spawned = 0;
    for (entity, dead, look, base, quiet) in &deadfall {
        let Some(pool) = pools.get(&dead.pool) else { continue };
        let variant = &pool.variants[dead.variant as usize % pool.variants.len()];
        let Some(cut) = &variant.deadfall else {
            bevy::log::warn_once!("bevy_flora: a PooledDeadfall's pool was built without deadfall (`build_pool_with_deadfall`)");
            continue;
        };
        let materials = dead_materials(
            &mut cache,
            dead.pool.id(),
            pool,
            look.map_or(0, |look| look.0),
            &recolors,
            wind,
            &mut flora,
            &mut shared,
            &mut standard,
            &mut textures,
            &mut cut_face,
        )
        .clone();
        commands.entity(entity).insert(DeadfallSpawned);
        spawned += 1;
        match dead.kind {
            DeadfallKind::Stump => spawn_stump(&mut commands, entity, pool, cut, &materials, quiet),
            DeadfallKind::Lying { direction } => {
                let lie = lie(base, cut, direction, ground.as_deref());
                spawn_lying(&mut commands, entity, variant, cut, &materials, lie.pivot, quiet, pool.lod.distances, units.units_per_metre);
            }
            DeadfallKind::Bucked { direction } => {
                spawn_stump(&mut commands, entity, pool, cut, &materials, quiet);
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
                let gap = LOG_GAP * units.units_per_metre;
                let mut at = base.transform_point(hinge).xz() + along * gap;
                for (index, log) in cut.logs.iter().enumerate() {
                    let length = (log.c1 - log.c0).length() * scale;
                    let place = lay_log(base, at, along, length, (log.c0, log.r0), (log.c1, log.r1), rest);
                    let capsule = WoodCapsule { a: log.c0, b: log.c1, radius: log.r0 };
                    // Placed in the world, as bucking a tree does; a child of the stump, so it goes
                    // with it, in the stump's space.
                    let local = Transform::from_matrix(base.to_matrix().inverse() * place.to_matrix());
                    let entity_log = commands
                        .spawn((
                            local,
                            Visibility::Inherited,
                            TreeLog { stump: entity, index: index as u32, capsule },
                            cut_wood(pool, &materials),
                            ChildOf(entity),
                        ))
                        .id();
                    spawn_mesh(&mut commands, entity_log, &log.wood, DeadRole::Wood, &materials, Transform::IDENTITY, quiet);
                    if let Some(ends) = &log.ends {
                        spawn_mesh(&mut commands, entity_log, ends, DeadRole::Face, &materials, Transform::IDENTITY, quiet);
                    }
                    at = place.transform_point(log.c1).xz() + along * gap;
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
/// turn about the hinge of its cut, until the points of its wood that hold it up reach the
/// ground. `base`'s scale and the ground are the world's; the tree's own space is metres.
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

fn spawn_stump(commands: &mut Commands, entity: Entity, pool: &TreePool, cut: &Deadfall, materials: &DeadMaterials, quiet: bool) {
    commands.entity(entity).insert((TreeStump { tree: None }, cut_wood(pool, materials)));
    spawn_mesh(commands, entity, &cut.stump, DeadRole::Wood, materials, Transform::IDENTITY, quiet);
    if let Some(stump_face) = &cut.stump_face {
        spawn_mesh(commands, entity, stump_face, DeadRole::Face, materials, Transform::IDENTITY, quiet);
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
    pivot: Transform,
    quiet: bool,
    distances: LodDistances,
    units: f32,
) {
    let bands = [
        banded(0.0, distances.lod1, units),
        banded(distances.lod1, distances.lod2, units),
        banded(distances.lod2, f32::INFINITY, units),
    ];
    for (level, (lod_band, range)) in bands.into_iter().enumerate() {
        let wood = spawn_mesh(commands, entity, &cut.lying[level], DeadRole::Wood, materials, pivot, quiet);
        commands.entity(wood).insert((lod_band, range.clone(), variant.bounds[level]));
        if let Some(leaves) = &variant.leaves[level] {
            let leaf = spawn_mesh(commands, entity, leaves, DeadRole::Leaves, materials, pivot, quiet);
            commands.entity(leaf).insert((lod_band, range, variant.bounds[level]));
        }
    }
    if let Some(lying_face) = &cut.lying_face {
        spawn_mesh(commands, entity, lying_face, DeadRole::Face, materials, pivot, quiet);
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
            commands.entity(entity).despawn_related::<Children>().remove::<(DeadfallSpawned, TreeStump, CutWood)>();
        }
    }
}

/// What choosing a look's materials needs, for the systems that re-dress deadfall.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct LookParams<'w> {
    flora: ResMut<'w, Assets<FloraMaterial>>,
    shared: ResMut<'w, SharedFlora>,
    standard: ResMut<'w, Assets<StandardMaterial>>,
    cut_face: ResMut<'w, CutFace>,
    recolors: Res<'w, PoolRecolors>,
    cache: ResMut<'w, DeadMaterialCache>,
}

/// Re-dress deadfall whose look changed ([`PooledLook`] changed or taken off) or whose pool's look
/// was recoloured ([`RecolorPool`]): every mesh child takes the material of its role in the
/// look's calm materials, and the stump's and logs' [`CutWood`] follow, so a later
/// [`RecolorTree`](crate::RecolorTree) starts from these colours.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn restyle_deadfall(
    mut commands: Commands,
    mut recolored: MessageReader<RecolorPool>,
    changed: Query<Entity, (With<DeadfallSpawned>, Changed<PooledLook>)>,
    mut removed: RemovedComponents<PooledLook>,
    pools: Res<Assets<TreePool>>,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    mut look: LookParams,
    mut textures: TextureLibrary,
    deadfall: Query<(Entity, &PooledDeadfall, Option<&PooledLook>, &Children), With<DeadfallSpawned>>,
    roles: Query<&DeadRole>,
    all_children: Query<&Children>,
    mut cuts: Query<&mut CutWood>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let mut stale: Vec<(AssetId<TreePool>, u32)> = Vec::new();
    for message in recolored.read() {
        stale.push((message.pool, message.look));
    }
    let moved: Vec<Entity> = changed.iter().chain(removed.read()).collect();
    if stale.is_empty() && moved.is_empty() {
        return;
    }
    // The recoloured looks are made again in their new colours.
    for key in &stale {
        look.cache.0.remove(key);
    }
    for (entity, dead, picked, children) in &deadfall {
        let Some(pool) = pools.get(&dead.pool) else { continue };
        let this_look = picked.map_or(0, |look| look.0) % pool.looks.len().max(1) as u32;
        if !moved.contains(&entity) && !stale.contains(&(dead.pool.id(), this_look)) {
            continue;
        }
        let materials = dead_materials(
            &mut look.cache,
            dead.pool.id(),
            pool,
            this_look,
            &look.recolors,
            wind,
            &mut look.flora,
            &mut look.shared,
            &mut look.standard,
            &mut textures,
            &mut look.cut_face,
        )
        .clone();
        let wood = CutWood { params: Arc::new(materials.colors.apply(&pool.params)), unlit: materials.colors.unlit, calm: true };
        if let Ok(mut cut) = cuts.get_mut(entity) {
            *cut = wood.clone();
        }
        let mut stack: Vec<Entity> = children.iter().collect();
        while let Some(child) = stack.pop() {
            if let Ok(role) = roles.get(child) {
                dress(&mut commands.entity(child), materials.of(*role));
            }
            if let Ok(mut cut) = cuts.get_mut(child) {
                // A log: its meshes are its children.
                *cut = wood.clone();
            }
            if let Ok(below) = all_children.get(child) {
                stack.extend(below.iter());
            }
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
    use bevy::color::Color;

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

    fn looked(app: &mut App) -> Handle<TreePool> {
        let params = TreePreset::Oak.params();
        let lod = LodSettings { distances: LodDistances { impostor: f32::INFINITY, ..Default::default() }, ..Default::default() };
        let mut pool = {
            let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
            crate::pool::build_pool_with_deadfall(&params, 1, f32::INFINITY, &lod, &DeadfallSettings::default(), &mut meshes)
        };
        pool.add_look(PoolColors { wood: Color::srgb(0.9, 0.1, 0.1), leaves: Color::srgb(0.1, 0.9, 0.1), unlit: false });
        app.world_mut().resource_mut::<Assets<TreePool>>().add(pool)
    }

    /// The base colours of the wood and leaf materials (plain ones, in a headless app) of `entity`'s
    /// mesh children, one of each, and how many materials there were.
    fn colours(app: &App, entity: Entity) -> (Vec<Color>, Vec<Color>, usize) {
        let (mut wood, mut leaves) = (Vec::new(), Vec::new());
        let mut all = std::collections::HashSet::new();
        let mut stack = descendants(app, entity);
        while let Some(child) = stack.pop() {
            stack.extend(descendants(app, child));
            let (Some(role), Some(m)) = (app.world().get::<DeadRole>(child), app.world().get::<MeshMaterial3d<StandardMaterial>>(child)) else {
                continue;
            };
            all.insert(m.0.id());
            let colour = app.world().resource::<Assets<StandardMaterial>>().get(&m.0).unwrap().base_color;
            match role {
                DeadRole::Wood => wood.push(colour),
                DeadRole::Leaves => leaves.push(colour),
                DeadRole::Face => {}
            }
        }
        (wood, leaves, all.len())
    }

    #[test]
    fn a_look_picks_the_materials_of_deadfall_and_a_recolour_and_a_new_look_reach_it() {
        use crate::pool::RecolorPool;
        let mut app = app();
        let pool = looked(&mut app);
        let red = Color::srgb(0.9, 0.1, 0.1);
        let lying = |app: &mut App, look: Option<u32>, x: f32| {
            let mut dead = app.world_mut().spawn((
                PooledDeadfall { pool: pool.clone(), variant: 0, kind: DeadfallKind::Lying { direction: Vec2::X } },
                Transform::from_xyz(x, 0.0, 0.0),
            ));
            if let Some(look) = look {
                dead.insert(PooledLook(look));
            }
            dead.id()
        };
        let (plain, dressed, other) = (lying(&mut app, None, 0.0), lying(&mut app, Some(1), 20.0), lying(&mut app, Some(1), 40.0));
        app.update();
        app.update();
        let oak = TreePreset::Oak.params();
        let (wood, leaves, _) = colours(&app, plain);
        assert!(wood.iter().all(|c| *c == oak.trunk_color) && leaves.iter().all(|c| *c == oak.foliage.color), "look 0 is the parameters'");
        let (wood, leaves, count) = colours(&app, dressed);
        assert!(wood.iter().all(|c| *c == red) && !wood.is_empty(), "the look's wood");
        assert!(leaves.iter().all(|c| *c == Color::srgb(0.1, 0.9, 0.1)) && !leaves.is_empty(), "the look's leaves");
        assert_eq!(count, 3, "wood, leaves and the end grain");
        assert_eq!(colours(&app, other).2, count);
        let all = |app: &App| -> std::collections::HashSet<_> {
            [dressed, other].into_iter().flat_map(|e| descendants(app, e)).filter_map(|c| app.world().get::<MeshMaterial3d<StandardMaterial>>(c).map(|m| m.0.id())).collect()
        };
        assert_eq!(all(&app).len(), 3, "two deadfall of a look share one set of materials");

        // A recolour of the pool's look reaches the deadfall in it, and not the other looks'.
        let blue = PoolColors { wood: Color::srgb(0.0, 0.0, 1.0), leaves: Color::srgb(1.0, 1.0, 0.0), unlit: false };
        let id = pool.id();
        app.world_mut().write_message(RecolorPool { pool: id, look: 1, colors: blue });
        app.update();
        app.update();
        for e in [dressed, other] {
            let (wood, leaves, _) = colours(&app, e);
            assert!(wood.iter().all(|c| *c == blue.wood) && leaves.iter().all(|c| *c == blue.leaves), "recoloured");
        }
        let (wood, _, _) = colours(&app, plain);
        assert!(wood.iter().all(|c| *c == oak.trunk_color), "look 0 is as it was");

        // A change of look re-dresses it, and taking the look off puts look 0 back.
        app.world_mut().entity_mut(dressed).remove::<PooledLook>();
        app.update();
        assert!(colours(&app, dressed).0.iter().all(|c| *c == oak.trunk_color));
        app.world_mut().entity_mut(dressed).insert(PooledLook(1));
        app.update();
        assert!(colours(&app, dressed).0.iter().all(|c| *c == blue.wood));
    }

    #[test]
    fn recolor_tree_dresses_a_pooled_stump_and_a_pooled_log() {
        use crate::RecolorTree;
        let mut app = app();
        let pool = looked(&mut app);
        let dead = app.world_mut().spawn((PooledDeadfall { pool, variant: 0, kind: DeadfallKind::Bucked { direction: Vec2::X } }, Transform::IDENTITY)).id();
        app.update();
        app.update();
        let log = descendants(&app, dead).into_iter().find(|c| app.world().get::<TreeLog>(*c).is_some()).expect("a log");
        let green = PoolColors { wood: Color::srgb(0.0, 1.0, 0.0), leaves: Color::WHITE, unlit: false };
        for tree in [dead, log] {
            app.world_mut().write_message(RecolorTree { tree, colors: green });
        }
        app.update();
        let wood_of = |app: &App, entity: Entity| -> Vec<Color> {
            descendants(app, entity)
                .into_iter()
                .filter(|c| app.world().get::<DeadRole>(*c) == Some(&DeadRole::Wood))
                .filter_map(|c| app.world().get::<MeshMaterial3d<StandardMaterial>>(c))
                .filter_map(|m| app.world().resource::<Assets<StandardMaterial>>().get(&m.0).map(|m| m.base_color))
                .collect()
        };
        assert!(!wood_of(&app, dead).is_empty() && wood_of(&app, dead).iter().all(|c| *c == green.wood), "the stump");
        assert!(!wood_of(&app, log).is_empty() && wood_of(&app, log).iter().all(|c| *c == green.wood), "the log");
    }

    /// A deadfall of `kind` in a world of `units` to the metre, its tree scaled by `units`, on the
    /// slope of 0.25 (the same in every unit): the turn of what lies and the places of its logs.
    fn in_a_world_of(units: f32, kind: DeadfallKind) -> (f32, Vec<Vec3>) {
        let mut app = app();
        app.insert_resource(crate::FloraUnits { units_per_metre: units });
        app.insert_resource(FloraGround::new(|at: Vec2| 0.25 * at.x));
        let pool = pool(&mut app, TreePreset::Oak);
        let at = Vec3::new(2.0 * units, 0.25 * 2.0 * units, 1.0 * units);
        let dead = app.world_mut().spawn((PooledDeadfall { pool, variant: 0, kind }, Transform::from_translation(at).with_scale(Vec3::splat(units)))).id();
        app.update();
        app.update();
        let turn = descendants(&app, dead)
            .into_iter()
            .filter(|c| app.world().get::<DeadRole>(*c).is_some())
            .filter_map(|c| app.world().get::<Transform>(c))
            .map(|t| t.rotation.to_axis_angle().1)
            .fold(0.0, f32::max);
        // The logs' ends, in the world (the dead's own transform applied by hand: there is no propagation here).
        let root = *app.world().get::<Transform>(dead).unwrap();
        let mut ends = Vec::new();
        for c in descendants(&app, dead) {
            if let (Some(log), Some(t)) = (app.world().get::<TreeLog>(c), app.world().get::<Transform>(c)) {
                ends.push(root.transform_point(t.transform_point(log.capsule.a)));
                ends.push(root.transform_point(t.transform_point(log.capsule.b)));
            }
        }
        (turn, ends)
    }

    #[test]
    fn deadfall_in_a_world_of_feet_lies_as_the_metre_world_scaled() {
        const FEET: f32 = 3.2808;
        let lying = DeadfallKind::Lying { direction: Vec2::new(0.8, 0.6) };
        let (metres, feet) = (in_a_world_of(1.0, lying).0, in_a_world_of(FEET, lying).0);
        assert!(metres > 0.3, "it lies at {metres}");
        assert!((metres - feet).abs() < 0.5_f32.to_radians(), "{} degrees in metres, {} in feet", metres.to_degrees(), feet.to_degrees());
        let bucked = DeadfallKind::Bucked { direction: Vec2::new(0.8, 0.6) };
        let ((_, in_metres), (_, in_feet)) = (in_a_world_of(1.0, bucked), in_a_world_of(FEET, bucked));
        assert!(in_metres.len() >= 4 && in_metres.len() == in_feet.len(), "{} and {} log ends", in_metres.len(), in_feet.len());
        for (m, f) in in_metres.iter().zip(&in_feet) {
            assert!(m.distance(*f / FEET) < 0.01 * (1.0 + m.length()), "a log end at {m} metres is at {f} feet");
        }
    }
}
