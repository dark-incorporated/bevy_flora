//! Picking a tree's fruit, and fruit a shake knocks loose.
//!
//! A fruiting tree ([`FoliageParams::fruit`](crate::FoliageParams::fruit)) drawn as a
//! [`ProceduralTree`] gets a [`TreeFruit`] once it is built: the fruit it grew, each with a key,
//! where it hangs in the tree's own space and its radius. [`TreeFruit::pick`] takes one off the
//! tree: the shaders stop drawing it (its vertices in the tree's leaf meshes are marked, on every
//! level of detail), and it stays picked as the tree grows, until the next blossom
//! ([`FloraSeason::blossom`]) brings the fruit back. The season decides which fruit hang
//! ([`TreeFruit::on_tree`]): none while the blossom is out, and the fruit drop in the first two
//! thirds of the leaf fall, as the shaders draw them.
//!
//! A shake ([`TreeShakes`]) knocks some of a tree's fruit loose ([`FallingFruit::per_shake`]):
//! each is picked from the tree and falls from where it hung as a [`FallenFruit`] entity, bounces
//! and lies on the ground ([`FloraGround`], or the height the tree stands at) for
//! [`FallingFruit::rest`] seconds, for the game to pick up (despawn) meanwhile. A [`FruitFell`]
//! message tells of each.
//!
//! Pooled trees ([`PooledTree`](crate::PooledTree)) share their meshes, so their fruit cannot be
//! picked, and a tree grown on the GPU ([`GpuGrowth`](crate::GpuGrowth)) has no `TreeFruit`
//! either: their fruit hangs as the season says.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::prelude::*;
//!
//! // The player picks the fruit nearest their hand, within reach.
//! fn pick(
//!     hand: Single<&GlobalTransform, With<Camera3d>>,
//!     season: Res<FloraSeason>,
//!     mut trees: Query<(&GlobalTransform, &mut TreeFruit)>,
//! ) {
//!     let at = hand.translation();
//!     for (tree, mut fruit) in &mut trees {
//!         if let Some((apple, _)) = fruit.nearest(&season, tree, at, 1.5) {
//!             fruit.pick(apple.key);
//!         }
//!     }
//! }
//! ```

use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d, VertexAttributeValues};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::reflect::Reflect;
use bevy::shape::Sphere;
use bevy::time::Time;
use bevy::transform::components::{GlobalTransform, Transform};

use crate::falling::leaf_season_moment;
use crate::felling::Felled;
use crate::foliage::{FRUIT_ALPHA, is_fruit_alpha};
use crate::ground::FloraGround;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::plugin::{Paint, ProceduralTree, TreeGenerated, TreeRender};
use crate::rng::SeededRng;
use crate::shading::{FloraSeason, TreeShake, TreeShakes};
use crate::units::FloraUnits;

/// A picked fruit's vertex alpha: still in the fruit's band (`is_fruit`), which the shaders then
/// do not draw (`leaf_hidden` in `common.wesl`).
pub(crate) const PICKED_ALPHA: f32 = 0.375;

/// How much of the leaf fall has passed when the last fruit drops (`FRUIT_FALL` in `common.wesl`).
const FRUIT_FALL: f32 = 0.67;

/// Gravity, metres per second squared (times [`FloraUnits`] in the world).
const GRAVITY: f32 = 9.81;
/// What a bouncing fruit keeps of its speed down (up again) and along the ground.
const BOUNCE: f32 = 0.3;
const SKID: f32 = 0.5;
/// Slower than this down, metres per second (times [`FloraUnits`] in the world), a fruit landing
/// stops instead of bouncing.
const SETTLE: f32 = 0.8;

/// One fruit a tree grew.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Fruit {
    /// Which fruit: the same as the tree grows and on every level of detail.
    pub key: u32,
    /// Its centre, in the tree's own space.
    pub position: Vec3,
    /// Its radius, in the tree's own space.
    pub radius: f32,
    /// Its moment in the season, 0..1: it drops once the leaf fall passes two thirds of it.
    pub moment: f32,
}

impl Fruit {
    /// Whether the season has this fruit on the tree: not while the blossom is out, nor once the
    /// leaf fall has passed its moment, as the shaders draw it.
    pub fn in_season(&self, season: &FloraSeason) -> bool {
        season.blossom.clamp(0.0, 1.0) <= 0.0 && self.moment * FRUIT_FALL >= season.leaf_fall.clamp(0.0, 1.0)
    }

    /// Where it hangs in the world, on the tree standing at `tree`.
    pub fn world(&self, tree: &GlobalTransform) -> Vec3 {
        tree.transform_point(self.position)
    }
}

/// The fruit of a fruiting [`ProceduralTree`], kept by the plugin: see the
/// [module documentation](self).
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct TreeFruit {
    fruit: Vec<Fruit>,
    #[reflect(ignore)]
    picked: HashSet<u32>,
    /// The picks marked in the tree's leaf meshes as they are now.
    #[reflect(ignore)]
    marked: HashSet<u32>,
}

impl TreeFruit {
    /// Every fruit the tree grew at the age it shows, picked or not, in season or not.
    pub fn all(&self) -> &[Fruit] {
        &self.fruit
    }

    /// The fruit hanging on the tree: in season ([`Fruit::in_season`]) and not picked.
    pub fn on_tree<'a>(&'a self, season: &'a FloraSeason) -> impl Iterator<Item = &'a Fruit> + 'a {
        self.fruit.iter().filter(|fruit| fruit.in_season(season) && !self.picked.contains(&fruit.key))
    }

    /// The fruit on the tree ([`Self::on_tree`]) nearest `at` (world space) on the tree standing at
    /// `tree`, if one hangs within `max` world units of it, and where it hangs in the world.
    pub fn nearest(&self, season: &FloraSeason, tree: &GlobalTransform, at: Vec3, max: f32) -> Option<(Fruit, Vec3)> {
        self.on_tree(season)
            .map(|fruit| (*fruit, fruit.world(tree)))
            .filter(|(_, world)| world.distance(at) <= max)
            .min_by(|a, b| a.1.distance_squared(at).total_cmp(&b.1.distance_squared(at)))
    }

    /// Take the fruit `key` off the tree; `false` if the tree grew no such fruit or it was
    /// already picked. The season is not asked: a fruit picked out of season stays picked.
    pub fn pick(&mut self, key: u32) -> bool {
        self.fruit.iter().any(|fruit| fruit.key == key) && self.picked.insert(key)
    }

    /// Whether the fruit `key` was picked.
    pub fn is_picked(&self, key: u32) -> bool {
        self.picked.contains(&key)
    }

    /// Every picked fruit back on the tree, as the next blossom does.
    pub fn regrow(&mut self) {
        self.picked.clear();
    }

    /// The keys of the picked fruit, to save.
    pub fn picked(&self) -> impl Iterator<Item = u32> + '_ {
        self.picked.iter().copied()
    }

    /// A tree's fruit with the fruit `keys` picked, from a save: insert it on the tree before or
    /// after it is built; the plugin fills in the fruit it grew and keeps the picks.
    pub fn with_picked(keys: impl IntoIterator<Item = u32>) -> Self {
        Self { picked: keys.into_iter().collect(), ..Default::default() }
    }
}

/// How much fruit a shake knocks loose and how long it lies. A resource; the plugin adds the
/// default.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FallingFruit {
    /// The share of a tree's fruit a shake of strength 1 knocks loose where it happens; less the
    /// further the tree stands from it, none at its radius. 0 knocks none loose.
    pub per_shake: f32,
    /// Seconds a fallen fruit lies on the ground before it is despawned; `f32::INFINITY` leaves
    /// it to the game.
    pub rest: f32,
}

impl Default for FallingFruit {
    fn default() -> Self {
        Self { per_shake: 0.3, rest: 60.0 }
    }
}

/// A fruit knocked off a tree: it falls from where it hung, bounces and lies on the ground, and
/// is despawned [`FallingFruit::rest`] seconds after it comes to rest. Its [`Transform`] is where
/// it is, its scale its radius.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct FallenFruit {
    /// The tree it fell from.
    pub tree: Entity,
    /// Which of the tree's fruit it was ([`Fruit::key`]).
    pub key: u32,
    /// Its colour.
    pub color: Color,
    /// How fast it moves, metres per second.
    pub velocity: Vec3,
    /// When it came to rest ([`Time::elapsed_secs`]), or `None` while it falls.
    pub rested_at: Option<f32>,
    /// The ground's height where no [`FloraGround`] says: the height the tree stands at.
    base: f32,
}

/// A fruit fell from a tree ([`FallenFruit`]).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FruitFell {
    /// The tree.
    pub tree: Entity,
    /// The falling fruit.
    pub fruit: Entity,
}

/// The fruit of a leaf mesh: each run of fruit vertices (alpha in the fruit's band) with one
/// phase, its centre and radius from its bounds.
pub(crate) fn fruit_in(mesh: &Mesh) -> Vec<Fruit> {
    let (
        Some(VertexAttributeValues::Float32x3(positions)),
        Some(VertexAttributeValues::Float32x4(colors)),
        Some(VertexAttributeValues::Float32x4(sway)),
    ) = (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.attribute(Mesh::ATTRIBUTE_COLOR), mesh.attribute(ATTRIBUTE_LEAF_SWAY))
    else {
        return Vec::new();
    };
    let mut fruit: Vec<Fruit> = Vec::new();
    let mut bounds: Vec<(Vec3, Vec3)> = Vec::new();
    for ((position, colour), sway) in positions.iter().zip(colors).zip(sway) {
        if !is_fruit_alpha(colour[3]) {
            continue;
        }
        let (key, p) = (sway[3].to_bits(), Vec3::from_array(*position));
        match (fruit.last(), bounds.last_mut()) {
            (Some(last), Some((lo, hi))) if last.key == key => {
                *lo = lo.min(p);
                *hi = hi.max(p);
            }
            _ => {
                fruit.push(Fruit { key, position: p, radius: 0.0, moment: leaf_season_moment(sway[3]) });
                bounds.push((p, p));
            }
        }
    }
    for (fruit, (lo, hi)) in fruit.iter_mut().zip(bounds) {
        fruit.position = (lo + hi) / 2.0;
        fruit.radius = 0.5 * (hi.x - lo.x).max(hi.z - lo.z);
    }
    fruit
}

/// Mark the fruit `hide` picked in `mesh` (the shaders stop drawing them) and the fruit `show`
/// back on the tree.
fn mark(mesh: &mut Mesh, hide: &HashSet<u32>, show: &HashSet<u32>) {
    let Some(VertexAttributeValues::Float32x4(mut colors)) = mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR) else {
        return;
    };
    if let Some(VertexAttributeValues::Float32x4(sway)) = mesh.attribute(ATTRIBUTE_LEAF_SWAY) {
        for (colour, sway) in colors.iter_mut().zip(sway) {
            if is_fruit_alpha(colour[3]) {
                let key = sway[3].to_bits();
                if hide.contains(&key) {
                    colour[3] = PICKED_ALPHA;
                } else if show.contains(&key) {
                    colour[3] = FRUIT_ALPHA;
                }
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

/// A rebuilt tree's fruit, from its level-0 leaf mesh: a fruiting tree gets or updates its
/// [`TreeFruit`] (its picks stay, and are marked again in the new meshes); a tree that no longer
/// bears fruit loses it.
pub(crate) fn track_fruit(
    mut commands: Commands,
    mut generated: MessageReader<TreeGenerated>,
    meshes: Res<Assets<Mesh>>,
    mut trees: Query<(&ProceduralTree, &TreeRender, Option<&mut TreeFruit>)>,
) {
    for message in generated.read() {
        let Ok((tree, render, fruit)) = trees.get_mut(message.entity) else { continue };
        // A tree grown on the GPU draws its mature mesh, not where its fruit hangs at its age.
        if tree.0.foliage.fruit.is_none() || matches!(render.paint, Paint::Growth(_)) {
            if fruit.is_some() {
                commands.entity(message.entity).remove::<TreeFruit>();
            }
            continue;
        }
        let grown = render.leaf_meshes().next().and_then(|handle| meshes.get(handle)).map(fruit_in).unwrap_or_default();
        match fruit {
            Some(mut fruit) => {
                fruit.fruit = grown;
                fruit.marked.clear();
            }
            None => {
                commands.entity(message.entity).insert(TreeFruit { fruit: grown, ..Default::default() });
            }
        }
    }
}

/// The next blossom brings every picked fruit back.
pub(crate) fn regrow_in_spring(season: Res<FloraSeason>, mut trees: Query<&mut TreeFruit>) {
    if season.blossom > 0.0 {
        for mut fruit in &mut trees {
            if !fruit.picked.is_empty() {
                fruit.regrow();
            }
        }
    }
}

/// The picks of every [`TreeFruit`] that changed, marked in its tree's leaf meshes, every level's.
pub(crate) fn mark_picks(mut meshes: ResMut<Assets<Mesh>>, mut trees: Query<(&mut TreeFruit, &TreeRender), Changed<TreeFruit>>) {
    for (mut fruit, render) in &mut trees {
        let hide: HashSet<u32> = fruit.picked.difference(&fruit.marked).copied().collect();
        let show: HashSet<u32> = fruit.marked.difference(&fruit.picked).copied().collect();
        if hide.is_empty() && show.is_empty() {
            continue;
        }
        for handle in render.leaf_meshes() {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                mark(&mut mesh, &hide, &show);
            }
        }
        let picked = fruit.picked.clone();
        fruit.bypass_change_detection().marked = picked;
    }
}

/// The fallen fruit's sphere and a material for each colour, made as they are first needed.
#[derive(Default)]
pub(crate) struct FruitLooks {
    mesh: Option<Handle<Mesh>>,
    materials: HashMap<[u32; 4], Handle<StandardMaterial>>,
}

/// Each new shake knocks loose a share of the fruit of the fruiting trees it reaches
/// ([`FallingFruit::per_shake`]), each by its own draw: picked from the tree, falling as a
/// [`FallenFruit`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn shake_fruit_loose(
    mut commands: Commands,
    shakes: Res<TreeShakes>,
    settings: Res<FallingFruit>,
    season: Res<FloraSeason>,
    ground: Option<Res<FloraGround>>,
    mut seen: Local<Vec<TreeShake>>,
    mut looks: Local<FruitLooks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut trees: Query<(Entity, &GlobalTransform, &ProceduralTree, &mut TreeFruit), Without<Felled>>,
    mut fell: MessageWriter<FruitFell>,
    units: Res<FloraUnits>,
) {
    let new: Vec<TreeShake> = shakes.iter().filter(|shake| !seen.contains(shake)).copied().collect();
    seen.clear();
    seen.extend(shakes.iter().copied());
    if settings.per_shake <= 0.0 {
        return;
    }
    for shake in new {
        // The shake's radius is metres, the distance to a tree world units.
        let radius = shake.radius.max(0.01) * units.units_per_metre;
        // One draw per fruit and shake: the shake's start (render time) tells one shake from the next.
        let stream = shake.start.to_bits();
        for (tree, at, params, mut fruit) in &mut trees {
            let distance = at.translation().xz().distance(shake.at);
            let t = (distance / radius).clamp(0.0, 1.0);
            let share = (settings.per_shake * shake.strength.max(0.0) * (1.0 - t * t * (3.0 - 2.0 * t))).min(1.0);
            if share <= 0.0 {
                continue;
            }
            let loose: Vec<Fruit> = fruit
                .on_tree(&season)
                .filter(|f| SeededRng::from_parts(f.key as i32, stream).next_f32() < share)
                .copied()
                .collect();
            if loose.is_empty() {
                continue;
            }
            let colour = params.0.foliage.fruit.as_ref().map_or(Color::WHITE, |f| f.color);
            let away = match shake.direction.try_normalize() {
                Some(direction) => direction,
                None => (at.translation().xz() - shake.at).try_normalize().unwrap_or(Vec2::X),
            };
            let scale = at.scale().max_element();
            for apple in loose {
                fruit.pick(apple.key);
                let mut rng = SeededRng::from_parts(apple.key as i32, stream ^ 0x5EED);
                // A push of a few tenths of a metre a second, in the world's units.
                let push = (away * rng.range(0.2, 0.8) + Vec2::new(rng.range(-0.4, 0.4), rng.range(-0.4, 0.4)))
                    * units.units_per_metre;
                let transform = Transform::from_translation(apple.world(at)).with_scale(Vec3::splat(apple.radius * scale));
                let fallen = FallenFruit {
                    tree,
                    key: apple.key,
                    color: colour,
                    velocity: Vec3::new(push.x, 0.0, push.y),
                    rested_at: None,
                    base: ground.as_ref().map_or(at.translation().y, |g| g.height(at.translation().xz())),
                };
                let mut entity = commands.spawn((fallen, transform, GlobalTransform::from(transform)));
                if let Some(materials) = materials.as_mut() {
                    let mesh = looks.mesh.get_or_insert_with(|| meshes.add(Sphere::new(1.0))).clone();
                    let linear = colour.to_linear();
                    let material = looks
                        .materials
                        .entry([linear.red, linear.green, linear.blue, linear.alpha].map(f32::to_bits))
                        .or_insert_with(|| {
                            materials.add(StandardMaterial { base_color: colour, perceptual_roughness: 0.35, ..Default::default() })
                        })
                        .clone();
                    entity.insert((Mesh3d(mesh), MeshMaterial3d(material)));
                }
                let id = entity.id();
                fell.write(FruitFell { tree, fruit: id });
            }
        }
    }
}

/// Fallen fruit fall, bounce, come to rest on the ground and are despawned after
/// [`FallingFruit::rest`] seconds there.
pub(crate) fn drop_fallen_fruit(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<FallingFruit>,
    ground: Option<Res<FloraGround>>,
    units: Res<FloraUnits>,
    mut fruit: Query<(Entity, &mut FallenFruit, &mut Transform)>,
) {
    let (dt, now) = (time.delta_secs(), time.elapsed_secs());
    let (gravity, settle) = (GRAVITY * units.units_per_metre, SETTLE * units.units_per_metre);
    for (entity, mut fallen, mut transform) in &mut fruit {
        if let Some(rested_at) = fallen.rested_at {
            if now - rested_at >= settings.rest {
                commands.entity(entity).despawn();
            }
            continue;
        }
        fallen.velocity.y -= gravity * dt;
        transform.translation += fallen.velocity * dt;
        let radius = transform.scale.y;
        let floor = ground.as_ref().map_or(fallen.base, |g| g.height(transform.translation.xz())) + radius;
        if transform.translation.y <= floor {
            transform.translation.y = floor;
            if fallen.velocity.y < -settle {
                fallen.velocity = Vec3::new(fallen.velocity.x * SKID, -fallen.velocity.y * BOUNCE, fallen.velocity.z * SKID);
            } else {
                fallen.velocity = Vec3::ZERO;
                fallen.rested_at = Some(now);
            }
        }
    }
}

pub(crate) fn build(app: &mut bevy::app::App) {
    app.init_resource::<FallingFruit>()
        .add_message::<FruitFell>()
        .register_type::<FallingFruit>()
        .register_type::<TreeFruit>()
        .register_type::<FallenFruit>()
        .add_systems(bevy::app::Update, drop_fallen_fruit.run_if(any_with_component::<FallenFruit>))
        // After transform propagation: fruit falls from where the trees stand this frame. Before
        // the meshes go to the renderer, the picks are marked in them.
        .add_systems(
            bevy::app::PostUpdate,
            (
                track_fruit.run_if(on_message::<TreeGenerated>),
                regrow_in_spring.run_if(resource_changed::<FloraSeason>),
                shake_fruit_loose.run_if(resource_changed::<TreeShakes>),
                mark_picks,
            )
                .chain()
                .after(bevy::transform::TransformSystems::Propagate),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lod::TreeLod;
    use crate::params::TreeParams;
    use crate::presets::TreePreset;

    fn app() -> bevy::app::App {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::time::TimeUpdateStrategy;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(crate::FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app
    }

    /// Each fruit vertex of each leaf mesh of `tree`: its fruit's key and its alpha.
    fn fruit_vertices(app: &bevy::app::App, tree: Entity) -> Vec<Vec<(u32, f32)>> {
        let render = app.world().get::<TreeRender>(tree).expect("a built tree");
        let meshes = app.world().resource::<Assets<Mesh>>();
        render
            .leaf_meshes()
            .map(|handle| {
                let mesh = meshes.get(handle).expect("a leaf mesh");
                let (Some(VertexAttributeValues::Float32x4(colors)), Some(VertexAttributeValues::Float32x4(sway))) =
                    (mesh.attribute(Mesh::ATTRIBUTE_COLOR), mesh.attribute(ATTRIBUTE_LEAF_SWAY))
                else {
                    panic!("a leaf mesh without colours or sway")
                };
                colors.iter().zip(sway).filter(|(c, _)| is_fruit_alpha(c[3])).map(|(c, s)| (s[3].to_bits(), c[3])).collect()
            })
            .collect()
    }

    #[test]
    fn a_picked_fruit_leaves_every_level_of_its_tree_and_stays_picked_until_the_next_blossom() {
        let mut app = app();
        let apple = TreePreset::Apple.params();
        let tree = app.world_mut().spawn((ProceduralTree(apple.clone()), TreeLod::default(), Transform::default())).id();
        app.update();
        app.update();
        let fruit = app.world().get::<TreeFruit>(tree).expect("a fruiting tree keeps its fruit").clone();
        let summer = FloraSeason::SUMMER;
        assert!(fruit.all().len() > 20, "{} fruit", fruit.all().len());
        assert_eq!(fruit.on_tree(&summer).count(), fruit.all().len());
        // The levels of detail carry some of the same fruit.
        let levels = fruit_vertices(&app, tree);
        assert_eq!(levels.len(), 3);
        let key = levels[2].first().expect("fruit on the coarsest level").0;

        assert!(app.world_mut().get_mut::<TreeFruit>(tree).unwrap().pick(key));
        app.update();
        let picked = |app: &bevy::app::App| {
            fruit_vertices(app, tree).iter().all(|level| level.iter().all(|&(k, alpha)| (alpha == PICKED_ALPHA) == (k == key)))
        };
        assert!(picked(&app), "the picked fruit, and only it, is marked on every level");
        let fruit = app.world().get::<TreeFruit>(tree).unwrap();
        assert!(fruit.is_picked(key) && fruit.on_tree(&summer).all(|f| f.key != key));
        assert!(!app.world_mut().get_mut::<TreeFruit>(tree).unwrap().pick(key), "picked twice");

        // Rebuilt (here for its bark's colour), the tree keeps its picks.
        let recoloured = TreeParams { trunk_color: Color::srgb(0.3, 0.3, 0.3), ..apple };
        app.world_mut().entity_mut(tree).insert(ProceduralTree(recoloured));
        app.update();
        app.update();
        assert!(picked(&app), "a rebuilt tree lost its picks");

        // The next blossom brings the fruit back.
        app.world_mut().resource_mut::<FloraSeason>().blossom = 0.5;
        app.update();
        assert!(fruit_vertices(&app, tree).iter().flatten().all(|&(_, alpha)| alpha == FRUIT_ALPHA));
        assert!(!app.world().get::<TreeFruit>(tree).unwrap().is_picked(key));
    }

    #[test]
    fn a_shake_knocks_fruit_loose_to_fall_and_lie_on_the_ground() {
        let mut app = app();
        app.insert_resource(FloraGround::new(|_| 0.25));
        let apple = TreePreset::Apple.params();
        let near = app.world_mut().spawn((ProceduralTree(apple.clone()), Transform::from_xyz(0.0, 0.25, 0.0))).id();
        let far = app.world_mut().spawn((ProceduralTree(apple), Transform::from_xyz(30.0, 0.25, 0.0))).id();
        app.update();
        app.update();
        let summer = FloraSeason::SUMMER;
        let hanging = |app: &bevy::app::App, tree| app.world().get::<TreeFruit>(tree).unwrap().on_tree(&summer).count();
        let (before, far_before) = (hanging(&app, near), hanging(&app, far));
        let mut cursor = app.world().resource::<Messages<FruitFell>>().get_cursor_current();
        let time = *app.world().resource::<Time>();
        let shake = TreeShake { at: Vec2::ZERO, radius: 3.0, strength: 1.0, ..Default::default() };
        app.world_mut().resource_mut::<TreeShakes>().shake(shake, &time);
        app.update();

        let fell: Vec<FruitFell> = cursor.read(app.world().resource::<Messages<FruitFell>>()).copied().collect();
        let share = fell.len() as f32 / before as f32;
        assert!((0.15..0.45).contains(&share), "{} of {before} fell", fell.len());
        assert!(fell.iter().all(|f| f.tree == near));
        assert_eq!(hanging(&app, near), before - fell.len(), "the fallen fruit is off the tree");
        assert_eq!(hanging(&app, far), far_before, "a tree out of the shake's reach lost fruit");

        // Within four seconds every fruit lies still on the ground, near the tree.
        for _ in 0..40 {
            app.update();
        }
        let lying: Vec<(FallenFruit, Transform)> =
            app.world_mut().query::<(&FallenFruit, &Transform)>().iter(app.world()).map(|(f, t)| (*f, *t)).collect();
        assert_eq!(lying.len(), fell.len());
        for (fruit, transform) in &lying {
            assert!(fruit.rested_at.is_some(), "still falling: {fruit:?}");
            assert!((transform.translation.y - (0.25 + transform.scale.y)).abs() < 1e-4, "not on the ground: {transform:?}");
            assert!(transform.translation.xz().length() < 3.0, "rolled away: {transform:?}");
            assert!(app.world().get::<TreeFruit>(near).unwrap().is_picked(fruit.key));
        }

        // It lies there for `FallingFruit::rest` seconds.
        app.world_mut().resource_mut::<FallingFruit>().rest = 1.0;
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(app.world_mut().query::<&FallenFruit>().iter(app.world()).count(), 0);
    }

    #[test]
    fn picks_saved_from_one_tree_hide_the_same_fruit_on_its_restored_twin() {
        let apple = TreePreset::Apple.params();
        let mut before = app();
        let tree = before.world_mut().spawn((ProceduralTree(apple.clone()), Transform::default())).id();
        before.update();
        before.update();
        let keys: Vec<u32> = before.world().get::<TreeFruit>(tree).unwrap().all().iter().step_by(5).map(|f| f.key).collect();
        let mut fruit = before.world_mut().get_mut::<TreeFruit>(tree).unwrap();
        for &key in &keys {
            assert!(fruit.pick(key));
        }
        let saved: Vec<u32> = fruit.picked().collect();

        // A new world: the tree comes back with its picks before it is first built.
        let mut loaded = app();
        let twin = loaded.world_mut().spawn((ProceduralTree(apple), Transform::default(), TreeFruit::with_picked(saved))).id();
        loaded.update();
        loaded.update();
        let fruit = loaded.world().get::<TreeFruit>(twin).unwrap();
        assert_eq!(fruit.all().len(), before.world().get::<TreeFruit>(tree).unwrap().all().len());
        assert!(keys.iter().all(|&key| fruit.is_picked(key)));
        assert_eq!(fruit.on_tree(&FloraSeason::SUMMER).count(), fruit.all().len() - keys.len());
        let hidden: Vec<u32> = fruit_vertices(&loaded, twin)[0].iter().filter(|(_, a)| *a == PICKED_ALPHA).map(|&(k, _)| k).collect();
        assert!(!hidden.is_empty() && hidden.iter().all(|k| keys.contains(k)), "the restored picks are not the drawn ones");
    }

    #[test]
    fn only_a_fruiting_tree_keeps_its_fruit() {
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::default())).id();
        let mut bare = TreePreset::Apple.params();
        let apple = app.world_mut().spawn((ProceduralTree(bare.clone()), Transform::default())).id();
        app.update();
        app.update();
        assert!(app.world().get::<TreeFruit>(oak).is_none());
        assert!(app.world().get::<TreeFruit>(apple).is_some());
        bare.foliage.fruit = None;
        app.world_mut().entity_mut(apple).insert(ProceduralTree(bare));
        app.update();
        app.update();
        assert!(app.world().get::<TreeFruit>(apple).is_none(), "a tree without fruit kept its TreeFruit");
    }
}
