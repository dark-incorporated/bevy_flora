//! Foliage that parts around a body moving through it: [`FoliagePusher`].
//!
//! Put a `FoliagePusher` on the player, an NPC or a cart and the leaves and thin stems it walks
//! through bend aside, around its body, and spring back behind it; trunks and thick limbs stay
//! where they are, and a plant stays rooted at its foot. Every way a tree is drawn up close takes
//! part (`ProceduralTree`, `TreeLod`, pooled levels, `GpuGrowth`), shadows included; impostors are
//! too far to matter. The shaders do it, from the shared buffer the wind is in
//! ([`FloraWindBuffer`]): the plugin writes the [`MAX_PUSHERS`] pushers nearest the camera there
//! when they move, each with where it was over the last half second, so what it parted closes up
//! behind it as it goes.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_flora::FoliagePusher;
//!
//! fn spawn_player(mut commands: Commands) {
//!     // The player's translation is the middle of its body: 1.8 m tall, 0.45 m round.
//!     commands.spawn((Transform::from_xyz(0.0, 0.9, 0.0), FoliagePusher::default()));
//! }
//! ```
//!
//! While no entity has a `FoliagePusher` no shader carries the push; the plugin turns it on in every
//! flora material when one appears (the materials are re-prepared once) and off when the last goes.

use std::collections::VecDeque;

use bevy::asset::Assets;
use bevy::camera::Camera3d;
use bevy::ecs::prelude::*;
use bevy::math::{Vec2, Vec3};
use bevy::platform::collections::HashMap;
use bevy::reflect::Reflect;
use bevy::render::storage::ShaderBuffer;
use bevy::time::Time;
use bevy::transform::components::GlobalTransform;

use crate::gpu::GrowthMaterial;
use crate::see_through::{SEE_THROUGH_SLOT, SEE_THROUGH_VECTORS};
use crate::shading::{FloraMaterial, FloraWindBuffer};
use crate::units::FloraUnits;

/// A body that pushes foliage aside as it moves through it: put it on an entity with a transform
/// (its translation is the middle of the body). Leaves and thin stems within `radius` of it, and
/// within its height, bend out of its way; see the [module](crate::push). `radius` and `height`
/// are metres: [`FloraUnits`] scales them to the world's unit.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FoliagePusher {
    /// How far round the body is, metres ([`FloraUnits`] scales it to the world): what is nearer
    /// its axis is pushed out to about here.
    pub radius: f32,
    /// How tall the body is, metres ([`FloraUnits`] scales it to the world), centred on its
    /// translation.
    pub height: f32,
    /// How far foliage gives way, 0..1: 1 out of the body, 0.5 halfway.
    pub strength: f32,
}

impl Default for FoliagePusher {
    fn default() -> Self {
        Self { radius: 0.45, height: 1.8, strength: 1.0 }
    }
}

/// The most pushers the shaders see at once: the nearest to the camera.
pub const MAX_PUSHERS: usize = 4;

/// Where each pusher is drawn from: now, and as it was `TRAIL_STEP`, twice and three times that
/// ago, ever weaker, so the foliage it parted closes up behind it.
const TRAIL: usize = 4;
const TRAIL_STEP: f32 = 0.15;
const TRAIL_WEIGHTS: [f32; TRAIL] = [1.0, 0.6, 0.3, 0.1];

/// Where the pushers sit in the shared buffer ([`FloraWindBuffer`]): after the see-through fade.
/// `common.wesl`'s `PUSH` is this number.
pub(crate) const PUSH_SLOT: usize = SEE_THROUGH_SLOT + SEE_THROUGH_VECTORS;

/// `Vec4`s of the pushers' part of the shared buffer: a header (how many samples follow, and the
/// x, z centre and radius of the circle across the ground everything they push lies in), then two
/// per sample: (x, y, z of the body's middle, radius) and (half its height, its strength times the
/// sample's weight, 0, 0).
pub(crate) const PUSH_VECTORS: usize = 1 + 2 * MAX_PUSHERS * TRAIL;

/// The pushers as the shaders read them; `sync_wind` keeps them when it rewrites the weather.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct PushSlots(pub [[f32; 4]; PUSH_VECTORS]);

impl Default for PushSlots {
    fn default() -> Self {
        Self([[0.0; 4]; PUSH_VECTORS])
    }
}

/// Where a pusher has been: (time, middle of its body), newest first, the last half second.
#[derive(Default)]
pub(crate) struct Trails(HashMap<Entity, VecDeque<(f32, Vec3)>>);

impl Trails {
    /// Where the pusher whose trail is `trail` was `age` seconds ago: the sample nearest that time
    /// (its oldest, when it has not been there that long).
    fn at(trail: &VecDeque<(f32, Vec3)>, now: f32, age: f32) -> Vec3 {
        let wanted = now - age;
        trail.iter().find(|(time, _)| *time <= wanted).or_else(|| trail.back()).map_or(Vec3::ZERO, |(_, at)| *at)
    }
}

/// Write the pushers nearest the camera into the shared buffer, in place, when they moved: each as
/// it is and as it was over the last half second.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_pushers(
    pushers: Query<(Entity, &FoliagePusher, &GlobalTransform)>,
    units: Res<FloraUnits>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    time: Res<Time>,
    mut trails: Local<Trails>,
    mut slots: ResMut<PushSlots>,
    buffer: Option<Res<FloraWindBuffer>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
) {
    let now = time.elapsed_secs();
    let keep = TRAIL_STEP * TRAIL as f32 + 0.1;
    trails.0.retain(|entity, _| pushers.contains(*entity));
    for (entity, _, at) in &pushers {
        let trail = trails.0.entry(entity).or_default();
        trail.push_front((now, at.translation()));
        while trail.len() > 1 && trail.back().is_some_and(|(time, _)| now - time > keep) {
            trail.pop_back();
        }
    }
    let eye = cameras.iter().next().map_or(Vec3::ZERO, GlobalTransform::translation);
    let mut nearest: Vec<(Entity, &FoliagePusher, f32)> =
        pushers.iter().map(|(entity, pusher, at)| (entity, pusher, at.translation().distance_squared(eye))).collect();
    nearest.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut packed = PushSlots::default();
    let mut count = 0;
    let metre = units.units_per_metre;
    for (entity, pusher, _) in nearest.into_iter().take(MAX_PUSHERS) {
        let Some(trail) = trails.0.get(&entity) else { continue };
        for (k, weight) in TRAIL_WEIGHTS.iter().enumerate() {
            let at = Trails::at(trail, now, k as f32 * TRAIL_STEP);
            packed.0[1 + 2 * count] = [at.x, at.y, at.z, pusher.radius.max(0.0) * metre];
            packed.0[2 + 2 * count] =
                [0.5 * pusher.height.max(0.0) * metre, pusher.strength.clamp(0.0, 1.0) * weight, 0.0, 0.0];
            count += 1;
        }
    }
    // The circle (across the ground) around everything the samples reach, so a vertex outside it
    // reads one vector and no sample.
    let samples: Vec<[f32; 4]> = (0..count).map(|i| packed.0[1 + 2 * i]).collect();
    let centre = samples.iter().fold(Vec2::ZERO, |sum, s| sum + Vec2::new(s[0], s[2])) / count.max(1) as f32;
    let reach = samples.iter().map(|s| centre.distance(Vec2::new(s[0], s[2])) + 1.5 * s[3]).fold(0.0, f32::max);
    packed.0[0] = [count as f32, centre.x, centre.y, reach];
    // Look before borrowing mutably: a mutable borrow marks it changed.
    if *slots == packed {
        return;
    }
    *slots = packed;
    let (Some(buffer), Some(mut buffers)) = (buffer, buffers) else { return };
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    if let Some(slice) = shader_buffer.cast_slice_mut::<[f32; 4]>()
        && slice.len() >= PUSH_SLOT + PUSH_VECTORS
    {
        slice[PUSH_SLOT..PUSH_SLOT + PUSH_VECTORS].copy_from_slice(&slots.0);
    }
}

/// Turn the push on in every flora material while an entity has a [`FoliagePusher`], and off when
/// none has: the flag is part of the materials' pipeline keys, so their shaders carry the push only
/// then. Materials made while it is on are turned on the frame after.
pub(crate) fn arm_push(
    pushers: Query<(), With<FoliagePusher>>,
    flora: Option<ResMut<Assets<FloraMaterial>>>,
    growth: Option<ResMut<Assets<GrowthMaterial>>>,
) {
    let on = !pushers.is_empty();
    if let Some(mut assets) = flora {
        crate::see_through::arm(&mut assets, on, |m| m.extension.push, |m, on| m.extension.push = on);
    }
    if let Some(mut assets) = growth {
        crate::see_through::arm(&mut assets, on, |m| m.extension.push, |m, on| m.extension.push = on);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::mesh::Mesh;
    use bevy::pbr::StandardMaterial;
    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::shading::FloraMaterialSupport;
    use crate::{FloraPlugin, ProceduralTree, TreePreset};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        // No renderer: draw the trees with flora materials anyway.
        app.insert_resource(FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)));
        app
    }

    fn armed(app: &App) -> Vec<bool> {
        app.world().resource::<Assets<FloraMaterial>>().iter().map(|(_, m)| m.extension.push).collect()
    }

    fn slots(app: &App) -> Vec<[f32; 4]> {
        let buffer = app.world().resource::<FloraWindBuffer>();
        let data = app.world().resource::<Assets<ShaderBuffer>>().get(&buffer.0).unwrap().cast_slice::<[f32; 4]>().unwrap();
        data[PUSH_SLOT..PUSH_SLOT + PUSH_VECTORS].to_vec()
    }

    #[test]
    fn a_pusher_bends_foliage_from_where_it_is_and_was_until_it_goes() {
        let mut app = app();
        app.world_mut().spawn(ProceduralTree(TreePreset::Shrub.params()));
        app.update();
        assert!(!armed(&app).is_empty() && armed(&app).iter().all(|on| !on), "off without a pusher");
        assert_eq!(slots(&app)[0][0], 0.0);

        // A body: the materials carry the push, and the shaders see it four times where it stands.
        let pusher = FoliagePusher { radius: 0.5, height: 2.0, strength: 0.8 };
        let body = app.world_mut().spawn((pusher, GlobalTransform::from_translation(Vec3::new(1.0, 1.0, 0.0)))).id();
        app.update();
        assert!(armed(&app).iter().all(|on| *on));
        let s = slots(&app);
        // Four samples on the spot: the circle all they push lies in is 1.5 radii round it.
        assert_eq!(s[0], [4.0, 1.0, 0.0, 0.75]);
        assert_eq!((s[1], s[2]), ([1.0, 1.0, 0.0, 0.5], [1.0, 0.8, 0.0, 0.0]));

        // Walking along +x at 2 m/s: the newest sample where it is, the older ones behind it every
        // 0.15 s (0.3 m), ever weaker.
        for step in 1..=12 {
            *app.world_mut().get_mut::<GlobalTransform>(body).unwrap() =
                GlobalTransform::from_translation(Vec3::new(1.0 + 0.1 * step as f32, 1.0, 0.0));
            app.update();
        }
        let s = slots(&app);
        for (k, behind) in [0.0, 0.3, 0.6, 0.9].into_iter().enumerate() {
            let x = s[1 + 2 * k][0];
            assert!((x - (2.2 - behind)).abs() < 0.11, "sample {k} at x {x}, not {} m behind", behind);
        }
        assert!((1..4).all(|k| s[2 + 2 * k][1] < s[2 * k][1]), "older samples not weaker: {s:?}");

        // Six bodies: the four nearest the camera are seen.
        app.world_mut().spawn((bevy::camera::Camera3d::default(), GlobalTransform::default()));
        for x in [3.0, 6.0, 4.0, 5.0, 7.0] {
            app.world_mut().spawn((pusher, GlobalTransform::from_translation(Vec3::new(x, 1.0, 0.0))));
        }
        app.update();
        let s = slots(&app);
        assert_eq!(s[0][0], 16.0);
        let nearest: Vec<f32> = (0..MAX_PUSHERS).map(|p| s[1 + 2 * TRAIL * p][0]).collect();
        assert_eq!(nearest, vec![2.2, 3.0, 4.0, 5.0]);

        // All gone: no pusher left, and no shader carries the push.
        let bodies: Vec<Entity> =
            app.world_mut().query_filtered::<Entity, With<FoliagePusher>>().iter(app.world()).collect();
        for entity in bodies {
            app.world_mut().despawn(entity);
        }
        app.update();
        assert!(armed(&app).iter().all(|on| !on), "on with no pusher left");
        assert_eq!(slots(&app)[0][0], 0.0);
    }

    #[test]
    fn the_default_pusher_covers_the_same_share_of_a_shrub_in_a_world_in_feet() {
        const FEET: f32 = 3.2808;
        let shrub = crate::build_tree(&TreePreset::Shrub.params(), f32::INFINITY);
        let points: Vec<Vec3> = shrub.trunk.positions.iter().chain(&shrub.foliage.positions).copied().collect();
        // The shrub at the origin and a default body (0.45 m round, 1.8 m tall) at its side, in a
        // world `world` times the metre's size that the plugin is told is `units` to the metre:
        // what the shader's reach test (`push_offset`) takes from the buffer, the samples within
        // 1.5 radii across the ground and half a radius past the body's ends.
        let share = |world: f32, units: f32| {
            let mut app = app();
            app.insert_resource(FloraUnits { units_per_metre: units });
            let at = Vec3::new(0.4 * world, 0.9 * world, 0.0);
            app.world_mut().spawn((FoliagePusher::default(), GlobalTransform::from_translation(at)));
            app.update();
            let s = slots(&app);
            let (body, half_height) = (s[1], s[2][0]);
            let covered = points
                .iter()
                .map(|p| *p * world)
                .filter(|p| {
                    let across = (Vec2::new(p.x, p.z) - Vec2::new(body[0], body[2])).length();
                    across < 1.5 * body[3] && (p.y - body[1]).abs() - half_height < 0.5 * body[3]
                })
                .count();
            covered as f32 / points.len() as f32
        };
        let (metres, feet) = (share(1.0, 1.0), share(FEET, FEET));
        assert!(metres > 0.02 && metres < 0.98, "the body covers {metres} of the shrub: not a test of anything");
        assert!((metres - feet).abs() < 1.0e-3, "{metres} of the shrub in metres, {feet} in feet");
        // Told nothing of the unit, the same body is 3.2808 times too small for the shrub.
        let unaware = share(FEET, 1.0);
        assert!(unaware < metres * 0.8, "an unaware body covers {unaware} of the shrub, {metres} in metres");
    }
}
