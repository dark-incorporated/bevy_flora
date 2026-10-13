//! Seeing what the camera follows through the trees in front of it: [`SeeThroughTrees`].

use std::sync::RwLock;

use bevy::asset::{Asset, AssetId, Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::math::{Vec3, Vec4};
use bevy::reflect::Reflect;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::Shader;
use bevy::camera::Projection;
use bevy::transform::components::GlobalTransform;

use crate::gpu::GrowthMaterial;
use crate::impostor::ImpostorMaterial;
use crate::shading::{FloraMaterial, FloraWindBuffer, TreeShakes};
use crate::units::FloraUnits;

/// Put on the game's camera to keep what it follows in view: the parts of trees between the
/// camera and [`target`](Self::target) dither out around the line of sight. Under a perspective
/// camera the clear view is a cone [`radius`](Self::radius) wide at the target and a third of
/// that at the camera; under an orthographic one (`Projection::Orthographic`, read from the
/// camera every frame), whose rays are parallel, it is a cylinder `radius` wide along the
/// camera's view. Third-person, top-down and isometric games use it to keep the player visible in
/// a forest; [`keep`](Self::keep), [`inner`](Self::inner), [`in_front`](Self::in_front) and
/// [`floor`](Self::floor) shape the cut to match a fade the game draws on its own meshes.
///
/// Only that camera sees through: shadow maps keep the whole tree (its shadow still falls on the
/// player), and so do other cameras. Every way a tree is drawn takes part (`ProceduralTree`,
/// `GpuGrowth`, pooled levels and impostors), and so do the camera's prepasses (a `DepthPrepass`
/// for TAA, SSAO or depth of field would otherwise keep the trees' depth where they were cut). One
/// camera at a time: the first found with the component. Update `target` as what it follows
/// moves; the camera's own movement is read from its `GlobalTransform`.
///
/// While no camera has it, no pipeline carries the fade: the plugin turns it on in every flora
/// material when one appears (the materials are re-prepared once) and off when it goes. With
/// plain `StandardMaterial`s (no flora shading, see [`FloraPlugin`](crate::FloraPlugin)) there is
/// no fade.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_flora::SeeThroughTrees;
/// // An isometric game's fade: a third of a tree kept at the centre, only what stands more than
/// // 1.2 m before the player's chest, and nothing below knee height.
/// let feet = 0.0;
/// let fade = SeeThroughTrees {
///     target: Vec3::new(0.0, feet + 1.2, 0.0),
///     radius: 6.0,
///     keep: 0.35,
///     inner: 0.55,
///     in_front: Some(1.2),
///     floor: Some(feet + 0.45),
/// };
/// ```
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SeeThroughTrees {
    /// What the camera must keep in view, in world space: the player's chest, a unit, the point
    /// under the cursor.
    pub target: Vec3,
    /// How wide the clear view is at `target`, metres (a radius; [`FloraUnits`] scales it to the
    /// world).
    pub radius: f32,
    /// The share of a tree kept at the centre of the clear view, 0..1: 0 (the default) cuts it
    /// all away, 0.35 leaves a third of it dithered in, so what stands in front still reads.
    pub keep: f32,
    /// Where the edge of the clear view starts to feather, as a share of `radius` (0.7 by
    /// default): the cut is whole inside it and fades out to `radius`.
    pub inner: f32,
    /// Where the cut ends before `target`, metres along the line of sight: `None` (the default)
    /// closes it over the last tenth of the way, `Some(1.2)` cuts only what stands more than
    /// 1.2 m before the target, however far away the camera is ([`FloraUnits`] scales the metres to
    /// the world). Nothing behind the target is
    /// ever cut.
    pub in_front: Option<f32>,
    /// A height in world units (not metres: [`FloraUnits`] does not scale it) below which nothing
    /// is cut, so the trunks standing on the ground beside
    /// the target stay whole: `None` (the default) has no floor.
    pub floor: Option<f32>,
}

impl Default for SeeThroughTrees {
    fn default() -> Self {
        Self { target: Vec3::ZERO, radius: 1.5, keep: 0.0, inner: 0.7, in_front: None, floor: None }
    }
}

/// Where the fade's vectors sit in the shared buffer ([`FloraWindBuffer`]): after the wind,
/// the season and the shakes. `common.wesl`'s `SEE_THROUGH` is this number.
pub(crate) const SEE_THROUGH_SLOT: usize = 7 + 2 * TreeShakes::MAX;

/// How many vectors the fade takes in the shared buffer.
pub(crate) const SEE_THROUGH_VECTORS: usize = 4;

/// What the shaders take for "no floor": lower than any world.
const NO_FLOOR: f32 = -1.0e30;

/// The fade as the shaders read it, all zero while no camera has [`SeeThroughTrees`]:
/// the camera's position and the radius at the target; the target, and 1 under a perspective
/// camera or 2 under an orthographic one; the camera's forward direction and the share kept;
/// where the feather starts, where the cut ends before the target (-1: over the last tenth of
/// the way) and the floor.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SeeThroughCapsule(pub [Vec4; SEE_THROUGH_VECTORS]);

impl SeeThroughCapsule {
    fn of(
        camera: Option<(&SeeThroughTrees, &GlobalTransform, Option<&Projection>)>,
        units: &FloraUnits,
    ) -> Self {
        let Some((fade, at, projection)) = camera else {
            return Self::default();
        };
        let orthographic = matches!(projection, Some(Projection::Orthographic(_)));
        Self([
            at.translation().extend(units.world(fade.radius.max(0.0))),
            fade.target.extend(if orthographic { 2.0 } else { 1.0 }),
            at.forward().as_vec3().extend(fade.keep.clamp(0.0, 1.0)),
            Vec4::new(
                fade.inner.clamp(0.0, 0.99),
                fade.in_front.map_or(-1.0, |d| units.world(d.max(0.0))),
                fade.floor.unwrap_or(NO_FLOOR),
                0.0,
            ),
        ])
    }
}

/// Write the fading camera's capsule into the shared buffer, in place, when it changed. Runs
/// after transform propagation, so the camera's position is the one this frame is drawn from:
/// the shaders cut only in the view drawn from exactly there.
pub(crate) fn sync_see_through(
    cameras: Query<(&SeeThroughTrees, &GlobalTransform, Option<&Projection>)>,
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
        for (slot, vector) in slice[SEE_THROUGH_SLOT..].iter_mut().zip(now.0) {
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

        let camera = app
            .world_mut()
            .spawn((
                SeeThroughTrees { target: Vec3::new(4.0, 5.0, 6.0), radius: 2.0, ..Default::default() },
                GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            ))
            .id();
        app.update();
        assert!(flags(&app).iter().all(|on| *on));
        // A perspective cut, with the defaults: no share kept, the feather from 0.7 of the radius,
        // closing over the last tenth of the way (-1), no floor.
        assert_eq!(
            slots(&app),
            [[1.0, 2.0, 3.0, 2.0], [4.0, 5.0, 6.0, 1.0], [0.0, 0.0, -1.0, 0.0], [0.7, -1.0, NO_FLOOR, 0.0]]
        );

        // A tree of another look planted meanwhile gets materials of its own, turned on too; a
        // change of wind rewrites the whole buffer and keeps the fade.
        app.world_mut().spawn(ProceduralTree(TreePreset::Birch.params()));
        app.world_mut().resource_mut::<FloraWind>().strength = 0.9;
        app.update();
        app.update();
        assert!(flags(&app).len() > 2 && flags(&app).iter().all(|on| *on));
        assert_eq!(slots(&app)[1], [4.0, 5.0, 6.0, 1.0]);

        // An orthographic camera cuts a cylinder along its view; the shape's numbers go along.
        let fade = SeeThroughTrees {
            target: Vec3::new(4.0, 5.0, 6.0),
            radius: 2.0,
            keep: 0.35,
            inner: 0.55,
            in_front: Some(1.2),
            floor: Some(0.45),
        };
        let projection = Projection::Orthographic(bevy::camera::OrthographicProjection::default_3d());
        app.world_mut().entity_mut(camera).insert((fade, projection));
        app.update();
        assert_eq!(slots(&app)[1..], [[4.0, 5.0, 6.0, 2.0], [0.0, 0.0, -1.0, 0.35], [0.55, 1.2, 0.45, 0.0]]);

        app.world_mut().entity_mut(camera).remove::<SeeThroughTrees>();
        app.update();
        assert!(flags(&app).iter().all(|on| !on), "off again once no camera sees through");
        assert_eq!(slots(&app), [[0.0; 4]; SEE_THROUGH_VECTORS]);
    }

    #[test]
    fn the_radius_and_the_distance_in_front_are_metres_the_floor_is_not() {
        let fade = SeeThroughTrees { radius: 2.0, in_front: Some(1.5), floor: Some(0.45), ..Default::default() };
        let at = GlobalTransform::default();
        let metres = SeeThroughCapsule::of(Some((&fade, &at, None)), &FloraUnits::default());
        let feet = SeeThroughCapsule::of(Some((&fade, &at, None)), &FloraUnits { units_per_metre: 3.0 });
        assert_eq!((metres.0[0].w, metres.0[3].y, metres.0[3].z), (2.0, 1.5, 0.45));
        assert_eq!((feet.0[0].w, feet.0[3].y, feet.0[3].z), (6.0, 4.5, 0.45));
    }
}
