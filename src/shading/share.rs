//! Trees that look alike share their [`FloraMaterial`]s.
//!
//! Every material is its own bind group and its own draw state, and Bevy re-specializes every
//! entity of a material whenever that material is modified, so a forest of one material per tree
//! costs far more than the same forest drawing from a handful of shared ones.

use std::collections::HashMap;

use bevy::asset::{AssetId, Assets, Handle};
use bevy::color::ColorToComponents;
use bevy::ecs::prelude::*;
use bevy::math::Vec4;

use super::{FloraMaterial, FloraUniform, FloraWindBuffer, Surface, flora_material};
use crate::params::{BarkScan, LeafScan, RenderStyle};

/// Most entries the cache holds before it drops those whose material no tree uses any more.
const MIN_PRUNE_AT: usize = 64;

/// What makes two flora materials interchangeable: the colour, whether it is leaves, the
/// per-species fields of the uniform and the bark scan (the wind is not part of a material:
/// every material reads it from the shared buffer). Floats are compared by bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Look {
    color: [u32; 4],
    leaf: bool,
    tree: [u32; 4],
    response: [u32; 4],
    leaf_look: [u32; 4],
    tint: [u32; 4],
    bark_radii: [u32; 4],
    bark_mean: [u32; 4],
    autumn: [u32; 4],
    blossom: [u32; 4],
    bark: Option<BarkScan>,
    leaf_scan: Option<LeafScan>,
    venation: bool,
    style: RenderStyle,
    unlit: bool,
}

fn bits(v: Vec4) -> [u32; 4] {
    v.to_array().map(f32::to_bits)
}

impl Look {
    fn new(surface: &Surface) -> Self {
        let FloraUniform { tree, response, leaf, tint, bark, bark_mean, autumn, blossom } = surface.uniform;
        Self {
            color: surface.color.to_linear().to_f32_array().map(f32::to_bits),
            leaf: surface.leaf,
            tree: bits(tree),
            response: bits(response),
            leaf_look: bits(leaf),
            tint: bits(tint),
            autumn: bits(autumn),
            blossom: bits(blossom),
            bark_radii: bits(bark),
            bark_mean: bits(bark_mean),
            bark: surface.bark.as_ref().map(|bark| bark.scan),
            leaf_scan: surface.leaf_scan.as_ref().map(|scan| scan.scan),
            venation: surface.venation.is_some(),
            style: surface.style,
            unlit: surface.unlit,
        }
    }
}

/// The flora materials trees draw with, by look; `rebuild_trees` fills it. It holds ids, not handles: a material lives as
/// long as some tree holds a strong one.
#[derive(Resource, Default)]
pub struct SharedFlora {
    materials: HashMap<Look, AssetId<FloraMaterial>>,
    prune_at: usize,
}

impl SharedFlora {
    /// A strong handle to the material with this `surface`, made with a handle to the shared
    /// `wind` when no live one exists.
    pub(crate) fn resolve(
        &mut self,
        assets: &mut Assets<FloraMaterial>,
        surface: &Surface,
        wind: &FloraWindBuffer,
    ) -> Handle<FloraMaterial> {
        let look = Look::new(surface);
        if let Some(id) = self.materials.get(&look)
            && let Some(strong) = assets.get_strong_handle(*id)
        {
            return strong;
        }
        if self.materials.len() >= self.prune_at.max(MIN_PRUNE_AT) {
            self.materials.retain(|_, id| assets.contains(*id));
            self.prune_at = self.materials.len() * 2;
        }
        let strong = assets.add(flora_material(surface, wind));
        self.materials.insert(look, strong.id());
        strong
    }
}
