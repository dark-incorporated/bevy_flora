//! Finding trees: [`FloraIndex`] holds every standing tree by where it stands, for a game to ask
//! which trees are near a point (the player's axe, a blast, a fire) or which trunk a ray from the
//! camera or a weapon hits.
//!
//! The plugin keeps it up to date: every [`ProceduralTree`] (a [`ProceduralTreeAsset`] tree once
//! its asset has loaded) and every [`PooledTree`] (once its [`TreePool`] exists), at the age it
//! shows, from where it stands after transform propagation. A tree moved, rebuilt or grown is
//! updated that frame; a tree despawned or felled leaves it ([`Felled`] trees, their stumps and
//! logs are not indexed). Each tree is its trunk, a cone from its foot to its top along the trunk's
//! axis: what an axe or a bullet hits, not the crown, which a ray passes through.
//!
//! It is a grid of [`FloraIndex::CELL`] square cells, so a query costs the trees near it, not all
//! of them: a hundred thousand pooled trees answer [`FloraIndex::nearest`] in microseconds.
//!
//! [`ProceduralTreeAsset`]: crate::ProceduralTreeAsset

use std::collections::HashMap;

use bevy::asset::{AssetEvent, Assets};
use bevy::ecs::prelude::*;
use bevy::math::{IVec2, Vec2, Vec3, Vec3Swizzles};
use bevy::shape::Ray3d;
use bevy::transform::components::GlobalTransform;

use crate::felling::Felled;
use crate::growth::fraction;
use crate::plugin::{ProceduralTree, TreeAge, TreeRender};
use crate::pool::{PooledTree, TreePool};
use crate::skeleton::Node;

/// Every standing tree by where it stands; see the [module docs](self). The plugin inserts and
/// maintains it; read it with `Res<FloraIndex>`.
#[derive(Resource, Default, Debug)]
pub struct FloraIndex {
    /// The trees in each cell their trunk reaches into, by cell.
    cells: HashMap<IVec2, Vec<Entity>>,
    /// Each tree, with the cells it is in (inclusive corners).
    trees: HashMap<Entity, (IndexedTree, IVec2, IVec2)>,
}

/// A tree as [`FloraIndex`] knows it: its trunk, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexedTree {
    /// The tree's entity.
    pub entity: Entity,
    /// The middle of the trunk's foot.
    pub base: Vec3,
    /// Up the trunk, unit length.
    pub axis: Vec3,
    /// How long the trunk is, in world units (a growing tree's grows).
    pub height: f32,
    /// The trunk's radius at its foot, in world units.
    pub base_radius: f32,
    /// The trunk's radius at its top, in world units.
    pub top_radius: f32,
}

/// Where a ray met a trunk: [`FloraIndex::cast_ray`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeHit {
    /// The tree.
    pub tree: Entity,
    /// The point on the bark.
    pub point: Vec3,
    /// The bark's outward normal there, unit length.
    pub normal: Vec3,
    /// How far along the ray, in world units.
    pub distance: f32,
}

impl IndexedTree {
    /// The middle of the trunk's top.
    pub fn top(&self) -> Vec3 {
        self.base + self.axis * self.height
    }

    /// The trunk's radius `h` world units up its axis (clamped to the trunk).
    pub fn radius_at(&self, h: f32) -> f32 {
        let t = if self.height > 0.0 { (h / self.height).clamp(0.0, 1.0) } else { 0.0 };
        self.base_radius + (self.top_radius - self.base_radius) * t
    }

    /// How far `at` (world x and z) is from the bark at the trunk's foot, in world units; 0 within it.
    pub fn distance_to(&self, at: Vec2) -> f32 {
        (self.base.xz().distance(at) - self.base_radius).max(0.0)
    }

    /// Where `ray` first meets the trunk's side within `max` world units, if it does.
    pub fn cast(&self, ray: Ray3d, max: f32) -> Option<TreeHit> {
        if self.height <= 0.0 {
            return None;
        }
        let (o, d, a) = (ray.origin, *ray.direction, self.axis);
        // The radius grows by `k` a metre up the axis: r(h) = base_radius + k h.
        let k = (self.top_radius - self.base_radius) / self.height;
        let w = o - self.base;
        let (wa, da) = (w.dot(a), d.dot(a));
        let (wp, dp) = (w - a * wa, d - a * da);
        let r0 = self.base_radius + k * wa;
        // |wp + t dp|² = (r0 + k da t)²
        let qa = dp.dot(dp) - k * k * da * da;
        let qb = 2.0 * (wp.dot(dp) - k * da * r0);
        let qc = wp.dot(wp) - r0 * r0;
        let mut roots = [f32::NAN; 2];
        if qa.abs() < 1.0e-9 {
            if qb.abs() > 1.0e-9 {
                roots[0] = -qc / qb;
            }
        } else {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                return None;
            }
            let s = disc.sqrt();
            roots = [(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)];
            if roots[0] > roots[1] {
                roots.swap(0, 1);
            }
        }
        roots.into_iter().filter(|t| t.is_finite() && *t >= 0.0 && *t <= max).find_map(|t| {
            let h = wa + t * da;
            let radius = r0 + k * da * t;
            if !(0.0..=self.height).contains(&h) || radius <= 0.0 {
                return None;
            }
            let point = o + d * t;
            let out = (point - self.base - a * h).normalize_or(Vec3::X);
            Some(TreeHit { tree: self.entity, point, normal: (out - a * k).normalize(), distance: t })
        })
    }

    /// The cells the trunk reaches into, inclusive corners.
    fn cells(&self) -> (IVec2, IVec2) {
        let (top, reach) = (self.top(), self.base_radius.max(self.top_radius));
        let low = self.base.xz().min(top.xz()) - Vec2::splat(reach);
        let high = self.base.xz().max(top.xz()) + Vec2::splat(reach);
        (cell_of(low), cell_of(high))
    }
}

fn cell_of(at: Vec2) -> IVec2 {
    (at / FloraIndex::CELL).floor().as_ivec2()
}

impl FloraIndex {
    /// The side of a grid cell, in world units (queries are in world units too: [`FloraUnits`](crate::FloraUnits)
    /// only scales what the crate measures in metres).
    pub const CELL: f32 = 8.0;

    /// How many trees it holds.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    /// Whether it holds no tree.
    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// The tree `entity`, if it is indexed.
    pub fn get(&self, entity: Entity) -> Option<&IndexedTree> {
        self.trees.get(&entity).map(|(tree, ..)| tree)
    }

    /// Every indexed tree, in no order.
    pub fn iter(&self) -> impl Iterator<Item = &IndexedTree> {
        self.trees.values().map(|(tree, ..)| tree)
    }

    /// The trees whose bark at the foot is within `radius` world units of `at` (world x and z), in no
    /// order.
    pub fn within(&self, at: Vec2, radius: f32) -> impl Iterator<Item = &IndexedTree> {
        let (low, high) = (cell_of(at - Vec2::splat(radius)), cell_of(at + Vec2::splat(radius)));
        (low.y..=high.y)
            .flat_map(move |y| (low.x..=high.x).map(move |x| IVec2::new(x, y)))
            .flat_map(move |cell| {
                self.cells.get(&cell).into_iter().flatten().filter_map(move |entity| {
                    let (tree, first, _) = self.trees.get(entity)?;
                    // A tree in several cells is met once: in the first of them the query covers.
                    (first.max(low) == cell && tree.distance_to(at) <= radius).then_some(tree)
                })
            })
    }

    /// The tree whose bark at the foot is nearest `at` (world x and z), within `max` world units.
    pub fn nearest(&self, at: Vec2, max: f32) -> Option<&IndexedTree> {
        self.within(at, max).min_by(|a, b| a.distance_to(at).total_cmp(&b.distance_to(at)))
    }

    /// The first trunk `ray` meets within `max` world units: what an axe swung, a bullet fired or the
    /// player's look along it hits. Crowns and leaves do not stop it.
    pub fn cast_ray(&self, ray: Ray3d, max: f32) -> Option<TreeHit> {
        let mut best: Option<TreeHit> = None;
        let consider = |cell: IVec2, best: &mut Option<TreeHit>| {
            for entity in self.cells.get(&cell).into_iter().flatten() {
                let Some((tree, ..)) = self.trees.get(entity) else { continue };
                let limit = best.map_or(max, |hit| hit.distance);
                if let Some(hit) = tree.cast(ray, limit) {
                    *best = Some(hit);
                }
            }
        };
        // Walk the cells the ray crosses over the ground (x, z), nearest first, until a hit is
        // nearer than the next cell.
        let flat = Vec2::new(ray.direction.x, ray.direction.z);
        let start = ray.origin.xz();
        let mut cell = cell_of(start);
        let speed = flat.length();
        if speed < 1.0e-6 {
            consider(cell, &mut best);
            return best;
        }
        let step = IVec2::new(flat.x.signum() as i32, flat.y.signum() as i32);
        let next_edge = |c: i32, s: i32| (c + i32::from(s > 0)) as f32 * Self::CELL;
        // Ray distance (3D) to the next x and y cell edges, and per cell.
        let along = |edge: f32, from: f32, rate: f32| if rate.abs() < 1.0e-9 { f32::INFINITY } else { (edge - from) / rate };
        let mut t_next = Vec2::new(
            along(next_edge(cell.x, step.x), start.x, flat.x),
            along(next_edge(cell.y, step.y), start.y, flat.y),
        );
        let t_step = Vec2::new(
            if flat.x.abs() < 1.0e-9 { f32::INFINITY } else { Self::CELL / flat.x.abs() },
            if flat.y.abs() < 1.0e-9 { f32::INFINITY } else { Self::CELL / flat.y.abs() },
        );
        loop {
            consider(cell, &mut best);
            let leave = t_next.x.min(t_next.y);
            if leave > max || best.is_some_and(|hit| hit.distance <= leave) {
                return best;
            }
            if t_next.x < t_next.y {
                cell.x += step.x;
                t_next.x += t_step.x;
            } else {
                cell.y += step.y;
                t_next.y += t_step.y;
            }
        }
    }

    /// Put `tree` in (again, if it was).
    fn insert(&mut self, tree: IndexedTree) {
        self.remove(tree.entity);
        let (low, high) = tree.cells();
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                self.cells.entry(IVec2::new(x, y)).or_default().push(tree.entity);
            }
        }
        self.trees.insert(tree.entity, (tree, low, high));
    }

    /// Take `entity` out, if it is in.
    fn remove(&mut self, entity: Entity) {
        let Some((_, low, high)) = self.trees.remove(&entity) else { return };
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let cell = IVec2::new(x, y);
                if let Some(list) = self.cells.get_mut(&cell) {
                    list.retain(|e| *e != entity);
                    if list.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }
        }
    }
}

/// A trunk in its tree's own space.
struct Trunk {
    start: Vec3,
    direction: Vec3,
    length: f32,
    base_radius: f32,
    tip_radius: f32,
}

impl Trunk {
    /// The trunk node `node`, `progress` of the way grown in length and `radius_progress` in
    /// girth.
    fn grown(node: &Node, progress: f32, radius_progress: f32) -> Self {
        Trunk {
            start: node.start,
            direction: node.direction,
            length: node.length * progress,
            base_radius: node.base_radius * radius_progress,
            tip_radius: node.tip_radius * radius_progress,
        }
    }

    /// The trunk where `place` stands it, as the index holds it.
    fn placed(&self, entity: Entity, place: &GlobalTransform) -> IndexedTree {
        let base = place.transform_point(self.start);
        let top = place.transform_point(self.start + self.direction * self.length);
        let height = base.distance(top);
        let scale = place.affine().matrix3.x_axis.length();
        IndexedTree {
            entity,
            base,
            axis: if height > 0.0 { (top - base) / height } else { place.up().as_vec3() },
            height,
            base_radius: self.base_radius * scale,
            top_radius: self.tip_radius * scale,
        }
    }
}

/// Keep [`FloraIndex`] up to date: trees gone or felled out, trees new, moved, rebuilt or grown
/// in, and every tree of a pool that has just loaded or changed.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn index_trees(
    mut index: ResMut<FloraIndex>,
    pools: Option<Res<Assets<TreePool>>>,
    mut pool_events: MessageReader<AssetEvent<TreePool>>,
    mut gone_procedural: RemovedComponents<ProceduralTree>,
    mut gone_pooled: RemovedComponents<PooledTree>,
    felled: Query<Entity, Added<Felled>>,
    procedural: Query<
        (Entity, &ProceduralTree, &TreeAge, Option<&TreeRender>, &GlobalTransform),
        (
            Without<Felled>,
            Or<(Changed<GlobalTransform>, Changed<ProceduralTree>, Changed<TreeAge>, Changed<TreeRender>)>,
        ),
    >,
    moved_pooled: Query<(Entity, &PooledTree, &GlobalTransform), (Without<Felled>, Or<(Changed<GlobalTransform>, Changed<PooledTree>)>)>,
    all_pooled: Query<(Entity, &PooledTree, &GlobalTransform), Without<Felled>>,
) {
    for entity in gone_procedural.read().chain(gone_pooled.read()).chain(&felled) {
        index.remove(entity);
    }
    for (entity, tree, age, render, place) in &procedural {
        let shape = match render {
            // Its trunk at the age it shows.
            Some(render) => {
                let node = &render.full.nodes[0];
                let progress = fraction(age.0, node.birth, node.duration);
                let thickening = fraction(age.0, node.birth, node.duration * tree.0.growth.thickening_lag);
                Trunk::grown(node, progress, thickening)
            }
            // Not built yet: its trunk as the parameters give it, full grown.
            None => {
                let params = &tree.0.trunk;
                Trunk {
                    start: Vec3::ZERO,
                    direction: Vec3::Y,
                    length: params.height,
                    base_radius: params.radius * params.flare,
                    tip_radius: params.radius * params.taper,
                }
            }
        };
        index.insert(shape.placed(entity, place));
    }
    let Some(pools) = pools else { return };
    let mut reloaded = Vec::new();
    for event in pool_events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } = event {
            reloaded.push(*id);
        }
    }
    let mut add = |entity: Entity, pooled: &PooledTree, place: &GlobalTransform| {
        let Some(pool) = pools.get(&pooled.pool) else { return };
        let variant = &pool.variants[pooled.variant as usize % pool.variants.len()];
        index.insert(Trunk::grown(&variant.skeleton.nodes[0], 1.0, 1.0).placed(entity, place));
    };
    for (entity, pooled, place) in &moved_pooled {
        add(entity, pooled, place);
    }
    if !reloaded.is_empty() {
        for (entity, pooled, place) in &all_pooled {
            if reloaded.contains(&pooled.pool.id()) {
                add(entity, pooled, place);
            }
        }
    }
}

/// The index resource; its system is in the plugin's `PostUpdate`.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.init_resource::<FloraIndex>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Dir3;

    /// A 6 m trunk, 0.5 m round at its foot and 0.25 m at its top, standing at (x, 0, z).
    fn tree(entity: Entity, x: f32, z: f32) -> IndexedTree {
        IndexedTree { entity, base: Vec3::new(x, 0.0, z), axis: Vec3::Y, height: 6.0, base_radius: 0.5, top_radius: 0.25 }
    }

    #[test]
    fn a_ray_meets_the_trunk_on_its_bark_and_misses_above_and_beside_it() {
        let trunk = tree(World::new().spawn_empty().id(), 0.0, 0.0);
        // Level, 2 m up, where the radius is 0.5 - 0.25 * 2/6.
        let hit = trunk.cast(Ray3d::new(Vec3::new(-10.0, 2.0, 0.0), Dir3::X), 50.0).expect("a hit");
        let radius = 0.5 - 0.25 * 2.0 / 6.0;
        assert!((hit.point.x + radius).abs() < 1e-4, "hit at {}", hit.point);
        assert!((hit.distance - (10.0 - radius)).abs() < 1e-4);
        assert!(hit.normal.x < -0.99 && hit.normal.y > 0.0, "normal {}", hit.normal);
        // From inside the trunk the ray meets the far side.
        let inside = trunk.cast(Ray3d::new(Vec3::new(0.0, 1.0, 0.0), Dir3::Z), 5.0).expect("from inside");
        assert!(inside.point.z > 0.0);
        // Above the top, beside it, too short, or pointing away: nothing.
        for (origin, dir, max) in [
            (Vec3::new(-10.0, 6.5, 0.0), Dir3::X, 50.0),
            (Vec3::new(-10.0, 2.0, 0.6), Dir3::X, 50.0),
            (Vec3::new(-10.0, 2.0, 0.0), Dir3::X, 9.0),
            (Vec3::new(-10.0, 2.0, 0.0), Dir3::NEG_X, 50.0),
        ] {
            assert_eq!(trunk.cast(Ray3d::new(origin, dir), max), None, "{origin} {dir:?}");
        }
    }

    #[test]
    fn the_grid_finds_trees_near_a_point_and_along_a_ray_across_cells() {
        let mut world = World::new();
        let ids: Vec<Entity> = (0..11).map(|_| world.spawn_empty().id()).collect();
        let mut index = FloraIndex::default();
        // A row along x, 5 m apart, across several cells, and one standing on a cell corner.
        for (i, &id) in ids[..10].iter().enumerate() {
            index.insert(tree(id, -20.0 + 5.0 * i as f32, 3.0));
        }
        index.insert(tree(ids[10], 8.0, 8.0));
        assert_eq!(index.len(), 11);
        let mut near: Vec<Entity> = index.within(Vec2::new(0.0, 3.0), 5.6).map(|t| t.entity).collect();
        near.sort();
        let mut want = vec![ids[3], ids[4], ids[5]];
        want.sort();
        assert_eq!(near, want, "the trees within 5.6 m of (0, 3)");
        assert_eq!(index.within(Vec2::new(8.0, 8.0), 1.0).count(), 1, "a tree in four cells is met once");
        assert_eq!(index.nearest(Vec2::new(1.0, 4.0), 10.0).map(|t| t.entity), Some(ids[4]));
        assert!(index.nearest(Vec2::new(100.0, 100.0), 10.0).is_none());
        // Along the row from beyond its end: the first trunk it meets, not a farther one.
        let hit = index.cast_ray(Ray3d::new(Vec3::new(40.0, 1.0, 3.0), Dir3::NEG_X), 100.0).expect("a hit");
        assert_eq!(hit.tree, ids[9]);
        // Diagonally across cells to the tree on the corner.
        let from = Vec3::new(-6.0, 1.0, -6.0);
        let toward = Dir3::new(Vec3::new(8.0, 1.0, 8.0) - from).unwrap();
        assert_eq!(index.cast_ray(Ray3d::new(from, toward), 50.0).map(|h| h.tree), Some(ids[10]));
        // Taken out, it is gone from every cell.
        index.remove(ids[10]);
        assert!(index.within(Vec2::new(8.0, 8.0), 1.0).next().is_none());
        assert!(index.cells.values().all(|list| !list.contains(&ids[10])));
    }

    #[test]
    fn the_index_follows_trees_as_they_are_planted_grown_moved_felled_and_despawned() {
        use crate::{FellTree, LodSettings, TreePreset, build_pool};
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::mesh::Mesh;
        use bevy::pbr::StandardMaterial;
        use bevy::time::TimeUpdateStrategy;
        use bevy::transform::components::Transform;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));

        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(10.0, 0.0, 0.0))).id();
        let birch_params = TreePreset::Birch.params();
        let birch = app
            .world_mut()
            .spawn((ProceduralTree(birch_params.clone()), TreeAge(2.0), Transform::from_xyz(-10.0, 0.0, 0.0)))
            .id();
        let pool = build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        let pine = app.world_mut().spawn((PooledTree { pool, variant: 1 }, Transform::from_xyz(0.0, 0.0, 20.0))).id();
        app.update();
        app.update();

        let index = app.world().resource::<FloraIndex>();
        assert_eq!(index.len(), 3);
        assert_eq!(index.nearest(Vec2::new(9.0, 0.0), 5.0).map(|t| t.entity), Some(oak));
        assert_eq!(index.nearest(Vec2::new(0.5, 20.0), 2.0).map(|t| t.entity), Some(pine));
        // The young birch is indexed at its age: a shorter, thinner trunk than its grown one.
        let young = *index.get(birch).expect("the birch is indexed");
        assert!(young.height > 0.0 && young.height < birch_params.trunk.height, "a young trunk {} m tall", young.height);
        // A level ray from the middle meets the oak's bark, facing it.
        let hit = index.cast_ray(Ray3d::new(Vec3::new(0.0, 1.0, 0.0), bevy::math::Dir3::X), 50.0).expect("the oak is hit");
        let oak_tree = *index.get(oak).unwrap();
        assert_eq!(hit.tree, oak);
        assert!((hit.point.x - (10.0 - oak_tree.radius_at(1.0))).abs() < 0.01 && hit.normal.x < -0.9, "{hit:?}");

        // The birch grows: its trunk in the index grows with it.
        app.world_mut().get_mut::<TreeAge>(birch).unwrap().0 = 6.0;
        app.update();
        let grown = *app.world().resource::<FloraIndex>().get(birch).unwrap();
        assert!(grown.height > young.height && grown.base_radius > young.base_radius, "{young:?} -> {grown:?}");
        // The oak moves: it is found where it stands now.
        app.world_mut().get_mut::<Transform>(oak).unwrap().translation = Vec3::new(30.0, 0.0, 0.0);
        app.update();
        let index = app.world().resource::<FloraIndex>();
        assert!(index.nearest(Vec2::new(9.0, 0.0), 5.0).is_none());
        assert_eq!(index.nearest(Vec2::new(29.0, 0.0), 5.0).map(|t| t.entity), Some(oak));

        // The pine is felled: it leaves the index for good.
        app.world_mut().entity_mut(pine).insert(FellTree { speed: 4.0, ..Default::default() });
        for _ in 0..300 {
            app.update();
        }
        assert!(app.world().get::<Felled>(pine).is_some(), "the pine was not felled");
        assert!(app.world().resource::<FloraIndex>().get(pine).is_none(), "a felled tree is still indexed");
        // The oak is despawned: gone.
        app.world_mut().despawn(oak);
        app.update();
        let index = app.world().resource::<FloraIndex>();
        assert!(index.get(oak).is_none());
        assert_eq!(index.len(), 1);
    }
}
