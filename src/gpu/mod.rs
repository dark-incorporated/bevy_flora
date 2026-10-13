//! Growing trees in the vertex shader.
//!
//! A tree with [`GpuGrowth`] is built once, at maturity, and uploaded once.
//! Every vertex carries which node it belongs to and where it sits relative to
//! that node's axis; each frame the CPU ages the skeleton ([`Skeleton::aged`])
//! and writes one small record per node into a storage buffer, and the vertex
//! shader (`growth.wesl`) moves every vertex to where the CPU path would have
//! put it. Nothing else is uploaded.
//!
//! [`Skeleton::aged`]: crate::Skeleton::aged

use bevy::app::App;
use bevy::asset::{AssetApp, AssetServer, Assets};
use bevy::ecs::prelude::*;
use bevy::mesh::{MeshVertexAttribute, VertexFormat};
use bevy::pbr::{MaterialPlugin, PbrPlugin};
use bevy::reflect::Reflect;
use bevy::render::RenderApp;
use bevy::render::storage::ShaderBuffer;

mod frame;
mod material;
mod mesh;
pub(crate) mod render;
mod state;

#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod tests;

pub use material::{GrowthExtension, GrowthMaterial};

/// Per-vertex: the node (`Node::id`) in the low word, the placement class
/// (low 8 bits) and a leaf's ordinal (the rest) in the high word.
pub(crate) const ATTRIBUTE_GROWTH_ID: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthId", 0x666C_6F72_6101, VertexFormat::Uint32x2);
/// Per-vertex: the anchor on the node's axis in node lengths (`xyz`), and the
/// fraction along the piece or the trunk ring (`w`).
pub(crate) const ATTRIBUTE_GROWTH_AXIS: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthAxis", 0x666C_6F72_6102, VertexFormat::Float32x4);
/// Per-vertex: the offset from the anchor in units of the piece's scale (`xyz`),
/// and the trunk's unit root-ridge bulge (`w`).
pub(crate) const ATTRIBUTE_GROWTH_OFFSET: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthOffset", 0x666C_6F72_6103, VertexFormat::Float32x4);

/// Grow this tree in the vertex shader instead of rebuilding its meshes every
/// frame. Add it to an entity that has [`ProceduralTree`](crate::ProceduralTree)
/// and [`TreeAge`](crate::TreeAge) (and, to make the tree grow,
/// [`Growing`](crate::Growing)).
///
/// The mature tree is built once per change of [`ProceduralTree`](crate::ProceduralTree)
/// and uploaded once; each frame only a few kilobytes of per-node state are
/// written. A tree on this path costs the CPU microseconds per frame instead of
/// the 0.1-0.4 ms the CPU path takes, which makes growing a forest affordable.
///
/// Without this component a tree behaves exactly as before.
///
/// # What differs from the CPU path
///
/// The tree is the same at every age in its overall shape, but it is a
/// deformation of the mature mesh, not a rebuild:
///
/// - **Smoothing** ([`MeshParams::smooth_iterations`](crate::MeshParams::smooth_iterations))
///   is computed once, on the mature mesh, and deformed with the tree; the CPU
///   path smooths every age afresh.
/// - **Ring and side counts are the mature ones.** With
///   [`MeshParams::adaptive_resolution`](crate::MeshParams::adaptive_resolution)
///   a young thin branch keeps the sides of the mature branch.
/// - **Unborn parts have zero size**, instead of being absent; their triangles
///   collapse to a point and draw nothing.
/// - With [`FoliagePlacement::AllBranches`](crate::FoliagePlacement::AllBranches),
///   leaves keep their mature positions along a branch and the first ones appear
///   as it lengthens; the CPU path respaces them.
/// - Tip-cluster leaves that are mirrored away from the trunk
///   ([`FoliagePlacement::TipClusters`](crate::FoliagePlacement::TipClusters))
///   keep the mirroring the mature tree chose.
/// - Leaf normals and the shape of primary bases follow the mature tree.
/// - A growing pointed trunk ends in a flat cap; with smoothing on, the CPU
///   path softens that cap into a point while the tree is young.
/// - The [`Mesh`](bevy::mesh::Mesh) assets on the tree's children, and the
///   bounding box derived from them, are the **mature** tree at every age:
///   picking, physics and frustum culling see the grown size, not the current one.
/// - Growth does not feed motion vectors (temporal anti-aliasing sees a growing
///   branch as still).
///
/// A tree with a hull crown ([`CrownStyle::Hull`](crate::CrownStyle)) or a prism trunk
/// ([`TrunkStyle::Prism`](crate::TrunkStyle)) is never grown on the GPU: it ignores this
/// component and is built on the CPU, with a warning logged once ([`TreeParams::gpu_growable`](crate::TreeParams::gpu_growable)).
///
/// # Platform support
///
/// The per-node state lives in a storage buffer, which WebGL2 does not have. On
/// such a device (and until the renderer is ready) the tree falls back to the
/// CPU path and a warning is logged once. The fallback tree is drawn with plain materials
/// (no leaf shading, no wind) when the device lacks vertex-stage storage buffers. The same
/// happens if
/// [`FloraPlugin`](crate::FloraPlugin) was added before the renderer's plugins
/// (`DefaultPlugins`): it works in any order, but only registers
/// [`GrowthMaterial`] with a renderer that is already there, so add it after.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct GpuGrowth;

/// Whether a tree with (`has`) or without [`GpuGrowth`] and `params` grows in the vertex shader
/// on this device: `has`, the device supports it and the tree can ([`TreeParams::gpu_growable`]).
/// A tree with a hull crown or a prism trunk asks and is built on the CPU, with a warning the
/// first time.
pub(crate) fn grows_on_gpu(has: bool, support: &GpuGrowthSupport, params: &crate::params::TreeParams) -> bool {
    if !(has && support.0) {
        return false;
    }
    if !params.gpu_growable() {
        bevy::log::warn_once!(
            "a tree with `GpuGrowth` has a hull crown or a prism trunk (`CrownStyle::Hull`, `TrunkStyle::Prism`), which grow on \
             the CPU only; it is built there"
        );
        return false;
    }
    true
}

/// Whether this device can grow trees in the vertex shader. [`FloraPlugin`](crate::FloraPlugin)
/// sets it once the renderer is ready, from the device's limits: `true` where
/// the vertex stage can read a storage buffer, `false` on WebGL2. While it is
/// `false`, trees with [`GpuGrowth`] grow on the CPU. Insert it yourself only
/// where there is no renderer (a headless test).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuGrowthSupport(pub bool);

/// Set when [`GrowthMaterial`] was registered with the renderer, which only
/// happens if the plugin was added after the renderer's plugins.
#[derive(Resource)]
pub(crate) struct GrowthMaterialAdded;

/// Register what GPU growth needs: the support flag, and, when the renderer's
/// plugins have already been added, the material and its shaders.
///
/// Plugins may be added in any order. Added ahead of the renderer's (or of the
/// asset plugin), this registers nothing that needs them; [`finish`] covers the
/// asset types, and `GpuGrowthSupport` stays `false`, so trees grow on the CPU.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<GpuGrowthSupport>().register_type::<GpuGrowth>();
    if app.get_sub_app(RenderApp).is_some() && app.is_plugin_added::<PbrPlugin>() {
        material::add_shaders(app);
        app.add_plugins(MaterialPlugin::<GrowthMaterial>::default())
            .insert_resource(GrowthMaterialAdded);
    } else {
        add_asset_types(app);
    }
}

/// Once every plugin is built, make sure the asset types the systems take exist
/// even if the plugin was added too early to register them.
pub(crate) fn finish(app: &mut App) {
    add_asset_types(app);
}

/// Register [`GrowthMaterial`] and [`ShaderBuffer`] as assets where the asset
/// plugin is present and nobody has yet (the renderer's plugins do it when they
/// are).
fn add_asset_types(app: &mut App) {
    if !app.world().contains_resource::<AssetServer>() {
        return;
    }
    if !app.world().contains_resource::<Assets<GrowthMaterial>>() {
        app.init_asset::<GrowthMaterial>();
    }
    if !app.world().contains_resource::<Assets<ShaderBuffer>>() {
        app.init_asset::<ShaderBuffer>();
    }
}
