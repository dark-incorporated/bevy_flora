//! Recolouring trees that are not pooled.
//!
//! A [`PooledTree`](crate::PooledTree) is recoloured through its pool ([`RecolorPool`](crate::RecolorPool),
//! [`PooledLook`](crate::PooledLook)). A tree that left its pool (felled, notched) or never had
//! one (a [`ProceduralTree`]) has materials of its own, which [`RecolorTree`] swaps for those of
//! the new colours: no mesh is rebuilt, and trees with equal colours share a material again
//! ([`SharedFlora`]).

use std::sync::Arc;

use bevy::ecs::prelude::*;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::asset::Assets;

use crate::felling::{CutFace, Felled, calm_params};
use crate::params::TreeParams;
use crate::plugin::{Paint, ProceduralTree, TreeRender, repaint_flora, set_color};
use crate::pool::PoolColors;
use crate::shading::{FloraMaterial, FloraMaterialSupport, FloraWindBuffer, SharedFlora, TextureLibrary};

/// Dress a tree that is not pooled in new colours: the wood and the leaves of a
/// [`ProceduralTree`] (every level of detail of a [`TreeLod`](crate::TreeLod) tree), its cut faces,
/// and, for a felled one, its stump; or a [`TreeStump`](crate::TreeStump) or
/// [`TreeLog`](crate::TreeLog) itself, which outlive the tree (send one per entity: a bucked tree's
/// stump and logs are separate entities). `colors` are as for a pool's look
/// ([`PoolColors`]: scans keep their pattern, an unlit look draws the end grain unlit too).
///
/// The meshes are not rebuilt. The tree keeps the colours: its later rebuilds use them over the
/// ones in its [`TreeParams`], and a pooled tree felled in a look begins with that look's
/// colours. A felled tree that is lying on the ground keeps its still (non-swaying) materials.
/// A tree that grows on the GPU ([`GpuGrowth`](crate::GpuGrowth)) is left as it is: it takes its
/// colours from its parameters.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RecolorTree {
    /// The tree, stump or log.
    pub tree: Entity,
    /// The new colours.
    pub colors: PoolColors,
}

/// The colours a tree is drawn in over those of its [`TreeParams`]: put on a [`ProceduralTree`] by
/// [`RecolorTree`], and on one that was a pooled tree in a look. Its absence means the
/// parameters' own, lit.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct TreeColors(pub(crate) PoolColors);

/// What a stump or a log was cut from, so [`RecolorTree`] can dress it when its tree is gone: the
/// parameters of the tree in the colours it had, whether it is unlit, and whether its wood has
/// the materials that do not sway (a log's).
#[derive(Component, Clone)]
pub(crate) struct CutWood {
    pub(crate) params: Arc<TreeParams>,
    pub(crate) unlit: bool,
    pub(crate) calm: bool,
}

/// Apply every [`RecolorTree`].
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn recolor_trees(
    mut commands: Commands,
    mut messages: MessageReader<RecolorTree>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut textures: TextureLibrary,
    mut cut_face: ResMut<CutFace>,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    mut trees: Query<(&ProceduralTree, &mut TreeRender, Option<&mut Felled>, Option<&Children>)>,
    mut cuts: Query<(&mut CutWood, &Children)>,
    plain: Query<&MeshMaterial3d<StandardMaterial>>,
    flora_children: Query<(), With<MeshMaterial3d<FloraMaterial>>>,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());
    let mut work: Vec<(Entity, PoolColors)> = messages.read().map(|m| (m.tree, m.colors)).collect();
    let mut next = 0;
    while next < work.len() {
        let (entity, colors) = work[next];
        next += 1;
        let unlit = colors.unlit;
        if let Ok((tree, mut render, mut felled, children)) = trees.get_mut(entity) {
            if matches!(render.paint, Paint::Growth(_)) {
                continue;
            }
            commands.entity(entity).insert(TreeColors(colors));
            let painted = colors.apply(&tree.0);
            let standard = match &render.paint {
                Paint::Standard { trunk, foliage } => Some((trunk.clone(), foliage.clone())),
                _ => None,
            };
            if let Some((trunk, foliage)) = standard {
                set_color(&mut materials, &trunk, colors.wood, unlit);
                set_color(&mut materials, &foliage, colors.leaves, unlit);
            } else if let Some(wind) = wind {
                let mut resolve = |params: &TreeParams| {
                    (
                        shared.resolve(&mut flora, &textures.wood(params).with_unlit(unlit), wind),
                        shared.resolve(&mut flora, &textures.leaves(params).with_unlit(unlit), wind),
                    )
                };
                if felled.as_deref().is_some_and(Felled::landed) {
                    // Lying on the ground: the children wear the materials that do not sway.
                    let (wood, leaves) = resolve(&calm_params(&painted));
                    let level0 = render.foliage.iter().chain(&render.flowers).map(|child| child.0);
                    for child in level0 {
                        commands.entity(child).insert(MeshMaterial3d(leaves.clone()));
                    }
                    commands.entity(render.trunk).insert(MeshMaterial3d(wood));
                } else {
                    let (wood, leaves) = resolve(&painted);
                    repaint_flora(&mut commands, &mut render, wood, leaves, felled.is_none());
                    if let Some(felled) = felled.as_deref_mut() {
                        let (wood, leaves) = resolve(&calm_params(&painted));
                        felled.restyle_still(render.trunk, &wood, &leaves);
                    }
                }
            }
            for child in children.into_iter().flat_map(|children| children.iter()) {
                if let Ok(current) = plain.get(child)
                    && cut_face.is_face(&current.0)
                {
                    let face = cut_face.get(&mut textures, &mut materials, unlit);
                    if current.0 != face {
                        commands.entity(child).insert(MeshMaterial3d(face));
                    }
                }
            }
            if let Some(felled) = felled.as_deref() {
                work.push((felled.stump, colors));
            }
        } else if let Ok((mut cut, children)) = cuts.get_mut(entity) {
            let painted = colors.apply(&cut.params);
            let params = if cut.calm { calm_params(&painted) } else { painted.clone() };
            cut.params = Arc::new(painted);
            cut.unlit = unlit;
            let wood = wind
                .map(|wind| shared.resolve(&mut flora, &textures.wood(&params).with_unlit(unlit), wind));
            for child in children.iter() {
                if flora_children.contains(child) {
                    if let Some(wood) = &wood {
                        commands.entity(child).insert(MeshMaterial3d(wood.clone()));
                    }
                } else if let Ok(current) = plain.get(child) {
                    if cut_face.is_face(&current.0) {
                        let face = cut_face.get(&mut textures, &mut materials, unlit);
                        if current.0 != face {
                            commands.entity(child).insert(MeshMaterial3d(face));
                        }
                    } else {
                        set_color(&mut materials, &current.0.clone(), colors.wood, unlit);
                    }
                }
            }
        }
    }
}
