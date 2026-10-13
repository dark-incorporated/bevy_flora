//! Seeing what the camera follows through the trees in front of it: [`SeeThroughTrees`].

use std::sync::RwLock;

use bevy::asset::{Asset, AssetId, Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::math::{Vec3, Vec4};
use bevy::reflect::Reflect;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::Shader;
use bevy::transform::components::GlobalTransform;

use crate::gpu::GrowthMaterial;
use crate::impostor::ImpostorMaterial;
use crate::shading::{FloraMaterial, FloraWindBuffer, TreeShakes};
use crate::units::FloraUnits;

/// Put on the game's camera to keep what it follows in view: the parts of trees between the
/// camera and [`target`](Self::target) dither out around the line of sight. Third-person and
/// top-down games use it to keep the player visible in a forest.
///
/// The clear view is a cone from a perspective camera, [`radius`](Self::radius) wide at the target
/// and a third of that at the camera, and a cylinder along the view direction for an orthographic
/// one, where every ray is parallel (the shader tells them apart by the projection). Through it
/// a tree keeps `mix(keep, 1, smoothstep(inner * radius, radius, distance from the line of
/// sight))` of its surface, the rest dithered away with a 4x4 ordered pattern: nothing kept at the
/// centre by default, all of it outside the radius. Perspective closes over the last tenth of the
/// way to the target, so nothing behind it is cut; [`in_front`](Self::in_front) gives that limit as
/// a distance instead, and orthographic cuts only what stands in front of the target by more than
/// it. Nothing lower than [`above_feet`](Self::above_feet) over the target's
/// [`feet`](Self::feet) is cut, so the foot of a trunk beside the player stays.
///
/// Only that camera sees through: shadow maps keep the whole tree (its shadow still falls on the
/// player), and so do other cameras. Every way a tree is drawn takes part (`ProceduralTree`,
/// `GpuGrowth`, pooled levels and impostors), and so do the camera's prepasses (a `DepthPrepass`
/// for TAA, SSAO or depth of field would otherwise keep the trees' depth where they were cut). One
/// camera at a time: the first found with the component. Update `target` as what it follows
/// moves; the camera's own movement is read from its `GlobalTransform`.
///
/// `radius`, `in_front` and `above_feet` are metres ([`FloraUnits`] scales them to the world);
/// `target` and `feet` are positions, in world units.
///
/// While no camera has it, no pipeline carries the fade: the plugin turns it on in every flora
/// material when one appears (the materials are re-prepared once) and off when it goes. With
/// plain `StandardMaterial`s (no flora shading, see [`FloraPlugin`](crate::FloraPlugin)) there is
/// no fade.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SeeThroughTrees {
    /// What the camera must keep in view, in world space: the player's chest, a unit, the point
    /// under the cursor.
    pub target: Vec3,
    /// How wide the clear view is at `target`, metres (a radius). With a perspective camera the
    /// view closes over the last tenth of the way to the target (or at `in_front`), so nothing
    /// behind it is cut.
    pub radius: f32,
    /// The share of a tree that stays drawn at the centre of the clear view, 0 (all of it cut:
    /// the default) to 1 (none), so a wall or a crown between the camera and the player stays
    /// readable behind the hole.
    pub keep: f32,
    /// How far in front of `target` (toward the camera) the cut begins, metres: along the view
    /// direction with an orthographic camera, along the line of sight with a perspective one,
    /// where it replaces the closing over the last tenth of the way. 0, the default, is as
    /// before: the whole way up to the last tenth (perspective), or everything in front of the
    /// target (orthographic).
    pub in_front: f32,
    /// The height of the target's feet, world units: what `above_feet` is measured from.
    pub feet: f32,
    /// Nothing lower than this over `feet` is cut, metres: the foot of a trunk beside the
    /// player stays whole. 0, the default, cuts at every height.
    pub above_feet: f32,
    /// Where the feather of the clear view starts, as a share of `radius` (0 to 1): the view is
    /// clearest within it and fades to nothing at `radius`.
    pub inner: f32,
}

impl Default for SeeThroughTrees {
    fn default() -> Self {
        Self { target: Vec3::ZERO, radius: 1.5, keep: 0.0, in_front: 0.0, feet: 0.0, above_feet: 0.0, inner: 0.7 }
    }
}

impl SeeThroughTrees {
    /// How much of a tree's surface at `world` this fade leaves drawn to the camera it is on, 0
    /// (cut) to 1 (whole), worked out as the shaders do (`see_through` in `common.wesl`): the
    /// share of its pixels the dither keeps is this number. `camera` is that camera's transform,
    /// `orthographic` whether its projection is, and `units` the world's, as the plugin was given
    /// them.
    ///
    /// This is what the camera's own views see. Shadow maps and other cameras always see the whole
    /// tree: `see_through` returns at once in any view that is not drawn from exactly the fading
    /// camera's position.
    pub fn opacity_at(&self, world: Vec3, camera: &GlobalTransform, orthographic: bool, units: &FloraUnits) -> f32 {
        let packed = SeeThroughCapsule::of(Some((self, camera)), units);
        opacity(&packed.0, world, camera.forward().as_vec3(), orthographic)
    }
}

/// Where the fade's vectors sit in the shared buffer ([`FloraWindBuffer`]): after the wind,
/// the season and the shakes. `common.wesl`'s `SEE_THROUGH` is this number.
pub(crate) const SEE_THROUGH_SLOT: usize = 7 + 2 * TreeShakes::MAX;

/// How many `Vec4`s the fade takes in the shared buffer: see [`SeeThroughCapsule`].
pub(crate) const SEE_THROUGH_VECTORS: usize = 4;

/// The fade as the shaders read it, all zero while no camera has [`SeeThroughTrees`]:
///
/// 0. the camera's position and the radius at the target (world units),
/// 1. the target and 1,
/// 2. what is kept at the centre, how far in front of the target the cut begins (world units),
///    the height below which nothing is cut (`f32::MIN` for no floor) and where the feather
///    starts (a share of the radius),
/// 3. spare.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SeeThroughCapsule(pub [Vec4; SEE_THROUGH_VECTORS]);

impl SeeThroughCapsule {
    fn of(camera: Option<(&SeeThroughTrees, &GlobalTransform)>, units: &FloraUnits) -> Self {
        let Some((fade, at)) = camera else { return Self::default() };
        let floor = if fade.above_feet > 0.0 { fade.feet + units.world(fade.above_feet) } else { f32::MIN };
        Self([
            at.translation().extend(units.world(fade.radius.max(0.0))),
            fade.target.extend(1.0),
            Vec4::new(
                fade.keep.clamp(0.0, 1.0),
                units.world(fade.in_front.max(0.0)),
                floor,
                // A feather as wide as nothing would divide by zero in the shader's smoothstep.
                fade.inner.clamp(0.0, 0.99),
            ),
            Vec4::ZERO,
        ])
    }
}

/// `smoothstep(low, high, x)` as in WGSL, 0 or 1 where the edges meet.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = if high > low { ((x - low) / (high - low)).clamp(0.0, 1.0) } else { f32::from(x >= high) };
    t * t * (3.0 - 2.0 * t)
}

/// `see_through` in `common.wesl`, for the fade as packed in `capsule`: how much of the surface at
/// `world` stays drawn, 0 to 1, seen from a camera looking along `forward`.
fn opacity(capsule: &[Vec4; SEE_THROUGH_VECTORS], world: Vec3, forward: Vec3, orthographic: bool) -> f32 {
    let [eye, goal, shape, _] = *capsule;
    let (keep, in_front, floor, inner) = (shape.x, shape.y, shape.z, shape.w);
    if goal.w < 0.5 || world.y < floor {
        return 1.0;
    }
    let (radius, across, closing) = if orthographic {
        // A cylinder along the view, cutting only what is more than `in_front` before the target.
        let dir = forward.normalize_or_zero();
        let rel = world - goal.truncate();
        let along = rel.dot(dir);
        if -along <= in_front.max(0.0) {
            return 1.0;
        }
        (eye.w, (rel - dir * along).length(), 1.0)
    } else {
        // A cone from the camera, a third as wide there as at the target.
        let line = goal.truncate() - eye.truncate();
        let t = (world - eye.truncate()).dot(line) / line.dot(line).max(1.0e-6);
        if t <= 0.0 || t >= 1.0 {
            return 1.0;
        }
        let closing = if in_front > 0.0 {
            if (1.0 - t) * line.length() <= in_front {
                return 1.0;
            }
            1.0
        } else {
            1.0 - smoothstep(0.9, 1.0, t)
        };
        let radius = eye.w * (1.0 / 3.0 + (1.0 - 1.0 / 3.0) * t);
        (radius, world.distance(eye.truncate() + line * t), closing)
    };
    let feather = smoothstep(inner * radius, radius, across);
    // `mix(keep, 1.0, feather)` as WGSL computes it: exactly 1 where the feather is 1.
    let kept = keep * (1.0 - feather) + feather;
    1.0 - (1.0 - kept) * closing
}

/// Write the fading camera's capsule into the shared buffer, in place, when it changed. Runs
/// after transform propagation, so the camera's position is the one this frame is drawn from:
/// the shaders cut only in the view drawn from exactly there.
pub(crate) fn sync_see_through(
    cameras: Query<(&SeeThroughTrees, &GlobalTransform)>,
    units: Res<FloraUnits>,
    mut capsule: ResMut<SeeThroughCapsule>,
    buffer: Option<Res<FloraWindBuffer>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
) {
    let now = SeeThroughCapsule::of(cameras.iter().next(), &units);
    // Look before borrowing mutably: a mutable borrow marks it changed.
    if *capsule == now {
        return;
    }
    *capsule = now;
    let (Some(buffer), Some(mut buffers)) = (buffer, buffers) else { return };
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    if let Some(slice) = shader_buffer.cast_slice_mut::<[f32; 4]>()
        && slice.len() >= SEE_THROUGH_SLOT + SEE_THROUGH_VECTORS
    {
        for (slot, vector) in slice[SEE_THROUGH_SLOT..SEE_THROUGH_SLOT + SEE_THROUGH_VECTORS].iter_mut().zip(now.0) {
            *slot = vector.to_array();
        }
    }
}

/// Turn the fade on in every flora material while a camera has [`SeeThroughTrees`], and off when
/// none has: the flag is part of the materials' pipeline keys, so their shaders carry the
/// fade's discard (and the prepass its fragment stage) only then. Materials made while it is on
/// (a new look, a new pool) are turned on the frame after.
pub(crate) fn arm_see_through(
    cameras: Query<(), With<SeeThroughTrees>>,
    flora: Option<ResMut<Assets<FloraMaterial>>>,
    growth: Option<ResMut<Assets<GrowthMaterial>>>,
    impostors: Option<ResMut<Assets<ImpostorMaterial>>>,
) {
    let on = !cameras.is_empty();
    if let Some(mut assets) = flora {
        arm(&mut assets, on, |m| m.extension.see_through, |m, on| m.extension.see_through = on);
    }
    if let Some(mut assets) = growth {
        arm(&mut assets, on, |m| m.extension.see_through, |m, on| m.extension.see_through = on);
    }
    if let Some(mut assets) = impostors {
        arm(&mut assets, on, |m| m.see_through, |m, on| m.see_through = on);
    }
}

/// Set the flag of the materials in `assets` whose flag is not `on` (only those: a material
/// borrowed mutably is re-prepared). The fire arms its flag the same way.
pub(crate) fn arm<A: Asset>(assets: &mut Assets<A>, on: bool, flag: fn(&A) -> bool, set: fn(&mut A, bool)) {
    let stale: Vec<AssetId<A>> = assets.iter().filter(|(_, material)| flag(material) != on).map(|(id, _)| id).collect();
    for id in stale {
        if let Some(mut material) = assets.get_mut(id) {
            set(&mut material, on);
        }
    }
}

/// The prepass fragment stage of a flora pipeline while the fade is on (`see_through_prepass.wesl`).
/// Pipelines are specialized without access to the world, so the handle loaded when the shaders
/// are embedded is kept here.
static PREPASS_FRAGMENT: RwLock<Option<Handle<Shader>>> = RwLock::new(None);

/// Keep the handle of the prepass fragment shader for [`prepass_fragment`].
pub(crate) fn set_prepass_fragment(shader: Handle<Shader>) {
    if let Ok(mut slot) = PREPASS_FRAGMENT.write() {
        *slot = Some(shader);
    }
}

/// The prepass fragment shader of the fade, once the shaders are embedded.
pub(crate) fn prepass_fragment() -> Option<Handle<Shader>> {
    PREPASS_FRAGMENT.read().ok()?.clone()
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::mesh::Mesh;
    use bevy::pbr::StandardMaterial;
    use bevy::transform::components::Transform;

    use super::*;
    use crate::shading::{FloraMaterialSupport, FloraWind};
    use crate::{FloraPlugin, ProceduralTree, TreePreset};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        // No renderer: draw the trees with flora materials anyway.
        app.insert_resource(FloraMaterialSupport);
        app
    }

    fn flags(app: &App) -> Vec<bool> {
        app.world().resource::<Assets<FloraMaterial>>().iter().map(|(_, m)| m.extension.see_through).collect()
    }

    fn slots(app: &App) -> [[f32; 4]; SEE_THROUGH_VECTORS] {
        let buffer = app.world().resource::<FloraWindBuffer>();
        let data = app.world().resource::<Assets<ShaderBuffer>>().get(&buffer.0).unwrap().cast_slice::<[f32; 4]>().unwrap();
        std::array::from_fn(|i| data[SEE_THROUGH_SLOT + i])
    }

    #[test]
    fn a_camera_that_sees_through_turns_the_fade_on_in_every_flora_material_until_it_goes() {
        let mut app = app();
        app.world_mut().spawn(ProceduralTree(TreePreset::Oak.params()));
        app.update();
        assert!(!flags(&app).is_empty() && flags(&app).iter().all(|on| !on), "off without a camera");
        assert_eq!(slots(&app), [[0.0; 4]; SEE_THROUGH_VECTORS]);

        let fade = SeeThroughTrees {
            target: Vec3::new(4.0, 5.0, 6.0),
            radius: 2.0,
            keep: 0.4,
            in_front: 1.5,
            feet: 0.5,
            above_feet: 0.25,
            inner: 0.5,
        };
        let camera = app
            .world_mut()
            .spawn((fade, GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0))))
            .id();
        app.update();
        assert!(flags(&app).iter().all(|on| *on));
        let shape = [[1.0, 2.0, 3.0, 2.0], [4.0, 5.0, 6.0, 1.0], [0.4, 1.5, 0.75, 0.5], [0.0; 4]];
        assert_eq!(slots(&app), shape);

        // A tree of another look planted meanwhile gets materials of its own, turned on too; a
        // change of wind rewrites the whole buffer and keeps the fade.
        app.world_mut().spawn(ProceduralTree(TreePreset::Birch.params()));
        app.world_mut().resource_mut::<FloraWind>().strength = 0.9;
        app.update();
        app.update();
        assert!(flags(&app).len() > 2 && flags(&app).iter().all(|on| *on));
        assert_eq!(slots(&app), shape);

        // A game in feet: the fade follows the unit (what is metres: radius, `in_front` and
        // `above_feet`) without being touched.
        app.world_mut().insert_resource(FloraUnits { units_per_metre: 4.0 });
        app.update();
        assert_eq!(slots(&app), [[1.0, 2.0, 3.0, 8.0], [4.0, 5.0, 6.0, 1.0], [0.4, 6.0, 1.5, 0.5], [0.0; 4]]);

        app.world_mut().entity_mut(camera).remove::<SeeThroughTrees>();
        app.update();
        assert!(flags(&app).iter().all(|on| !on), "off again once no camera sees through");
        assert_eq!(slots(&app), [[0.0; 4]; SEE_THROUGH_VECTORS]);
    }

    #[test]
    fn the_defaults_and_the_odd_values_pack_as_the_shader_can_use_them() {
        let camera = GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let metres = FloraUnits::default();
        // The defaults: nothing kept, the whole way, at every height, a feather from 70%.
        let plain = SeeThroughTrees { target: Vec3::new(4.0, 5.0, 6.0), radius: 2.0, ..Default::default() };
        let packed = SeeThroughCapsule::of(Some((&plain, &camera)), &metres).0;
        assert_eq!(packed[0], Vec4::new(1.0, 2.0, 3.0, 2.0));
        assert_eq!(packed[1], Vec4::new(4.0, 5.0, 6.0, 1.0));
        assert_eq!(packed[2], Vec4::new(0.0, 0.0, f32::MIN, 0.7));
        assert_eq!(packed[3], Vec4::ZERO);
        // Out of range values are made usable: a share is 0..1, a distance is not negative, and a
        // feather as wide as nothing would divide by zero.
        let odd = SeeThroughTrees { keep: 1.5, in_front: -2.0, above_feet: -1.0, inner: 3.0, ..plain };
        assert_eq!(SeeThroughCapsule::of(Some((&odd, &camera)), &metres).0[2], Vec4::new(1.0, 0.0, f32::MIN, 0.99));
        // In feet: the metres are scaled, the positions are not.
        let feet = FloraUnits { units_per_metre: 3.2808 };
        let told = SeeThroughTrees { in_front: 4.0, feet: 10.0, above_feet: 1.5, ..plain };
        let packed = SeeThroughCapsule::of(Some((&told, &camera)), &feet).0;
        assert!((packed[0].w - 2.0 * 3.2808).abs() < 1.0e-4, "{}", packed[0].w);
        assert_eq!(packed[0].truncate(), Vec3::new(1.0, 2.0, 3.0));
        assert!((packed[2].y - 4.0 * 3.2808).abs() < 1.0e-4, "{}", packed[2].y);
        assert!((packed[2].z - (10.0 + 1.5 * 3.2808)).abs() < 1.0e-4, "{}", packed[2].z);
        assert_eq!(SeeThroughCapsule::of(None, &feet), SeeThroughCapsule::default());
    }

    /// The camera of the issue (#52): orthographic at a 35° pitch, in a world in feet, looking at
    /// the player's chest 5 ft up; the clear view 30 ft across and feathered from 55% of that, 35%
    /// of an occluder kept at its centre, nothing within 4 ft of the chest and nothing under 1.5 ft
    /// above the feet cut. Also where "toward the camera" is, along the view.
    fn the_issue() -> (SeeThroughTrees, GlobalTransform, FloraUnits, Vec3) {
        let feet = FloraUnits { units_per_metre: 3.2808 };
        let target = Vec3::new(10.0, 5.0, -3.0);
        let pitch = 35f32.to_radians();
        let toward = Vec3::new(0.0, pitch.sin(), pitch.cos());
        let camera =
            GlobalTransform::from(Transform::from_translation(target + toward * 400.0).looking_at(target, Vec3::Y));
        let fade = SeeThroughTrees {
            target,
            radius: 30.0 / 3.2808,
            keep: 0.35,
            in_front: 4.0 / 3.2808,
            feet: 0.0,
            above_feet: 1.5 / 3.2808,
            inner: 0.55,
        };
        (fade, camera, feet, toward)
    }

    #[test]
    fn an_orthographic_camera_cuts_a_cylinder_that_keeps_a_share_at_its_centre() {
        let (fade, camera, units, toward) = the_issue();
        let at = |world: Vec3| fade.opacity_at(world, &camera, true, &units);
        // A tree 10 ft in front of the target along the view: 35% kept at the centre, however far
        // along the view (a cylinder does not narrow toward the camera).
        let tree = fade.target + toward * 10.0;
        assert!((at(tree) - 0.35).abs() < 1.0e-5, "{}", at(tree));
        assert!((at(fade.target + toward * 100.0) - 0.35).abs() < 1.0e-5);
        // Across it: kept at 0.35 within 55% of the radius, feathered to whole at the radius.
        let side = Vec3::X;
        assert!((at(tree + side * 16.0) - 0.35).abs() < 1.0e-5);
        assert!((at(tree + side * 30.0 * 0.775) - (0.35 + 0.65 * 0.5)).abs() < 1.0e-4);
        assert!((at(tree + side * 30.0) - 1.0).abs() < 1.0e-3);
        assert_eq!(at(tree - side * 45.0), 1.0);
        // The same cut seen the other way round: the ordered dither keeps this share of its pixels.
        assert!(at(tree) < at(tree + side * 20.0) && at(tree + side * 20.0) < at(tree + side * 28.0));
    }

    #[test]
    fn whatever_is_behind_the_player_or_beside_his_feet_is_whole() {
        let (fade, camera, units, toward) = the_issue();
        let at = |fade: &SeeThroughTrees, world: Vec3| fade.opacity_at(world, &camera, true, &units);
        // Behind the target along the view, near it or far (without the floor: far is under it).
        assert_eq!(at(&fade, fade.target - toward * 3.0), 1.0);
        let no_floor = SeeThroughTrees { above_feet: 0.0, ..fade };
        assert_eq!(at(&no_floor, fade.target - toward * 40.0), 1.0);
        // Within `in_front` of the chest is whole, past it cut.
        assert_eq!(at(&fade, fade.target + toward * 3.9), 1.0);
        assert!((at(&fade, fade.target + toward * 4.1) - 0.35).abs() < 1.0e-5);
        // The foot of a trunk 1 ft over the feet, well in front of the player and well inside the
        // clear view, stays: the same point 2 ft up is cut.
        let foot = Vec3::new(fade.target.x, 1.0, fade.target.z + 12.0);
        assert_eq!(at(&fade, foot), 1.0);
        assert!((at(&fade, foot + Vec3::Y) - 0.35).abs() < 1.0e-5, "{}", at(&fade, foot + Vec3::Y));
        // The floor is measured from the feet, not from the ground the world's origin stands on.
        let raised = SeeThroughTrees { feet: 3.0, ..fade };
        assert_eq!(at(&raised, foot + Vec3::Y), 1.0);
        // Nothing is cut where no camera has the fade: the packed target's last lane is 0.
        let none = SeeThroughCapsule::default().0;
        assert_eq!(opacity(&none, foot, Vec3::NEG_Z, true), 1.0);
    }

    #[test]
    fn a_perspective_camera_cuts_a_cone_that_closes_over_the_last_tenth_unless_told_how_far() {
        let units = FloraUnits::default();
        let target = Vec3::new(0.0, 2.0, 0.0);
        let camera = GlobalTransform::from(Transform::from_xyz(0.0, 2.0, 100.0).looking_at(target, Vec3::Y));
        let fade = SeeThroughTrees { target, radius: 3.0, ..Default::default() };
        let at = |fade: &SeeThroughTrees, z: f32, x: f32| fade.opacity_at(Vec3::new(x, 2.0, z), &camera, false, &units);
        // Half way the cone is two thirds of the radius wide: cut within 70% of it, whole at its edge.
        assert_eq!(at(&fade, 50.0, 0.0), 0.0);
        assert_eq!(at(&fade, 50.0, 1.3), 0.0);
        assert!((at(&fade, 50.0, 2.0) - 1.0).abs() < 1.0e-3);
        assert_eq!(at(&fade, 50.0, 2.5), 1.0);
        // A third as wide at the camera: 1 m wide a metre from it, cut at 0.5 m from the line, whole at 1.
        assert_eq!(at(&fade, 99.0, 0.5), 0.0);
        assert_eq!(at(&fade, 99.0, 1.1), 1.0);
        // At the target, behind it and behind the camera: whole. Over the last tenth: closing.
        assert_eq!(at(&fade, 0.0, 0.0), 1.0);
        assert_eq!(at(&fade, -5.0, 0.0), 1.0);
        assert_eq!(at(&fade, 101.0, 0.0), 1.0);
        assert!((at(&fade, 5.0, 0.0) - 0.5).abs() < 1.0e-4);
        // `in_front` gives that limit as a distance instead: nothing within 5 m of the target is
        // cut, and what is further is cut right up to it.
        let told = SeeThroughTrees { in_front: 5.0, ..fade };
        assert_eq!(at(&told, 4.9, 0.0), 1.0);
        assert_eq!(at(&told, 5.1, 0.0), 0.0);
        // And a share kept at the centre.
        let kept = SeeThroughTrees { keep: 0.35, ..fade };
        assert!((at(&kept, 50.0, 0.0) - 0.35).abs() < 1.0e-6);
    }

    #[test]
    fn the_shader_returns_at_once_in_a_view_that_is_not_the_fading_cameras() {
        // Shadow maps and other cameras keep the whole tree, and this cannot be seen in a test that
        // draws nothing: `see_through` must compare the position the view is drawn from with the
        // fading camera's before it does anything else.
        let source = include_str!("shading/common.wesl");
        let body = source.split("fn see_through(").nth(1).expect("common.wesl has see_through");
        let before_first_return = body.split("return;").next().unwrap_or_default();
        assert!(
            before_first_return.contains("any(abs(view_position - eye.xyz) > vec3<f32>(1.0e-3))"),
            "see_through no longer starts with the check on the view's position"
        );
    }
}
