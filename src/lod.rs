//! Levels of detail built from the skeleton, not by simplifying a mesh.
//!
//! [`build_lods`] returns three versions of one tree. Level 0 is exactly
//! [`build_tree`](crate::build_tree). The coarser levels drop the thin wood
//! (twigs, and every branch that grows from one), mesh what is left with fewer
//! sides and rings and without collars, and keep only some of the leaves, scaled
//! up so the canopy covers about as much as before. The wood comes from the same
//! meshers as level 0 and no mesh-simplification dependency is needed. The
//! silhouette of the crown is kept: a coarse level stays inside level 0's
//! bounding box (to a centimetre) and keeps the wood and the leaves that reach
//! furthest in each direction.
//!
//! A [`TreePool`](crate::TreePool) draws its trees at these levels, and [`TreeLod`] draws a
//! single [`ProceduralTree`](crate::ProceduralTree) at them.

use bevy::ecs::prelude::{Component, ReflectComponent};
use bevy::math::Vec3;
use bevy::reflect::Reflect;

use crate::foliage::{FoliageMeshes, LeafPoint, collect_leaf_points, crown_meshes_coarse};
use crate::mesh::branch::{Coarseness, skeleton_mesh_coarse};
use crate::params::TreeParams;
use crate::pipeline::{TreeMeshes, build_aged, trunk_top};
use crate::skeleton::{Node, Skeleton, grow_skeleton};

/// How far from the camera each level of detail takes over, in metres: level 1
/// from `lod1`, level 2 from `lod2`, the impostor from `impostor`. Each switch
/// cross-fades over a couple of metres.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct LodDistances {
    /// Distance at which level 0 hands over to level 1.
    pub lod1: f32,
    /// Distance at which level 1 hands over to level 2.
    pub lod2: f32,
    /// Distance at which level 2 hands over to the impostor (a camera-facing
    /// quad). `f32::INFINITY` keeps level 2 at every distance.
    pub impostor: f32,
}

impl Default for LodDistances {
    fn default() -> Self {
        Self {
            lod1: 30.0,
            lod2: 80.0,
            impostor: 200.0,
        }
    }
}

/// How [`build_lods`] coarsens a tree.
///
/// Besides what is set here, each coarse level meshes without collars or
/// smoothing, closes a steep branch's base with a flat disc, halves the trunk's
/// height segments (at least 2) per level, and gives a bent branch at most 4
/// (level 1) or 3 (level 2) rings.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct LodSettings {
    /// Level 1 drops branches thinner than this at their base, in metres, and
    /// everything that grows from them.
    pub lod1_min_radius: f32,
    /// Level 1 keeps every `n`th leaf (at least 1).
    pub lod1_leaf_keep: u32,
    /// Level 2 drops branches thinner than this at their base, in metres, and
    /// everything that grows from them.
    pub lod2_min_radius: f32,
    /// Level 2 keeps every `n`th leaf (at least 1).
    pub lod2_leaf_keep: u32,
    /// Sides of the trunk and the branch tubes at each level. A node that has
    /// fewer sides at level 0 keeps them; the level 0 entry is not used.
    pub radial_segments: [u32; 3],
    /// How much of a hull crown's stack ([`CrownStyle::Hull`](crate::CrownStyle)) each level
    /// keeps: its `tiers` and `sides` times this, at least 2 and 3. Level 0's entry is not used.
    /// The coarse levels' hull is built from every leaf and kept inside level 0's bounds.
    pub hull_detail: [f32; 3],
    /// Where the levels switch, for a pool's trees and a tree with [`TreeLod`].
    pub distances: LodDistances,
    /// How far above the horizon a pool's impostor pictures are taken, in degrees (10 by default,
    /// at most 60): the height the camera that sees the far trees looks down from. A perspective
    /// game keeps 10; an isometric or top-down game with a fixed pitch sets that pitch, so its far
    /// trees show their crowns from above as the near ones do. [`TreeLod`] has no impostor.
    pub impostor_elevation: f32,
    /// Whether a pool's impostors cast shadows (off by default): each casts its tree's
    /// silhouette, the quad turned toward the light, where the shadow cascades reach. Off, a far
    /// forest stands without shadows beside the shadowed near trees, which shows most from above
    /// (an isometric camera); on, every impostor within the cascades is drawn into them too.
    pub impostor_shadows: bool,
}

impl Default for LodSettings {
    fn default() -> Self {
        Self {
            lod1_min_radius: 0.015,
            lod1_leaf_keep: 3,
            lod2_min_radius: 0.05,
            lod2_leaf_keep: 8,
            radial_segments: [8, 5, 4],
            hull_detail: [1.0, 0.7, 0.5],
            distances: LodDistances::default(),
            impostor_elevation: 10.0,
            impostor_shadows: false,
        }
    }
}

/// Draw a [`ProceduralTree`](crate::ProceduralTree) at three levels of detail, as a pool draws
/// its trees: the full tree up close, then [`build_lods`]' coarser levels from
/// `distances.lod1` and `distances.lod2` on (30 and 80 m by default), cross-fading over a couple of
/// metres. Individual trees have no impostor: level 2 is drawn at every distance beyond `lod2`
/// (`distances.impostor` is not used). Without it a tree is drawn in full at every distance.
/// 1 000 oaks draw at 6.5 ms a frame with it and 24 ms without (1080p, Apple M4 Pro).
///
/// The coarse levels are rebuilt whenever the tree is (a new age of a [`Growing`](crate::Growing)
/// tree included), from the same skeleton: 200 oaks growing on the CPU take 14 ms a frame with it
/// and 10 ms without. Its children are the full tree's (see [`TreeRender`](crate::TreeRender)) plus
/// a wood and a leaf entity per coarse level ([`TreeLodMesh`](crate::plugin::TreeLodMesh)), whose
/// leaves carry a flowering tree's flowers. A tree grown on the GPU ([`GpuGrowth`](crate::GpuGrowth))
/// ignores it, and a felled tree drops its coarse levels when it is cut.
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use bevy_flora::{LodDistances, LodSettings, ProceduralTree, TreeLod, TreePreset};
///
/// fn plant(mut commands: Commands) {
///     let lod = LodSettings { distances: LodDistances { lod1: 20.0, lod2: 50.0, ..default() }, ..default() };
///     commands.spawn((ProceduralTree(TreePreset::Maple.params()), TreeLod(lod), Transform::default()));
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeLod(pub LodSettings);

/// Three levels of detail of the tree `params` describes at `age`
/// (`f32::INFINITY` for the mature tree). Index 0 is the full tree, 2 the
/// coarsest; see [`LodSettings`] for what each level drops.
///
/// The leaves of the coarse levels are chosen from the full tree's leaves, so
/// thin wood that is dropped does not take its foliage with it. The
/// [`TreeMeshes::skeleton`] of a coarse level is the skeleton it was meshed from. As
/// for [`build_tree`](crate::build_tree), any `params` and `age` give a tree
/// ([`TreeParams::sanitized`]).
pub fn build_lods(params: &TreeParams, age: f32, lod: &LodSettings) -> [TreeMeshes; 3] {
    let params = &*params.sanitized();
    let full = grow_skeleton(params);
    let fine = build_aged(&full, params, age);
    let [lod1, lod2] = coarse_levels(&fine, params, lod);
    [fine, lod1, lod2]
}

/// Levels 1 and 2 of the tree whose level 0 (`build_aged`'s output for `params`) is `fine`.
pub(crate) fn coarse_levels(fine: &TreeMeshes, params: &TreeParams, lod: &LodSettings) -> [TreeMeshes; 2] {
    let points = if params.foliage.enabled {
        collect_leaf_points(
            &fine.skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        )
    } else {
        Vec::new()
    };
    let source = Source {
        fine,
        points: &points,
        params,
        bounds: bounds(fine),
        reaching_nodes: reaching_nodes(&fine.skeleton),
        reaching_leaves: extremes(points.len(), |i| points[i].position, |i| points[i].position),
    };
    let lod1 = source.coarse(
        1,
        lod.lod1_min_radius,
        lod.lod1_leaf_keep,
        lod.radial_segments[1],
        lod.hull_detail[1],
    );
    let lod2 = source.coarse(
        2,
        lod.lod2_min_radius,
        lod.lod2_leaf_keep,
        lod.radial_segments[2],
        lod.hull_detail[2],
    );
    [lod1, lod2]
}

/// The full tree and what the coarse levels are chosen from.
struct Source<'a> {
    fine: &'a TreeMeshes,
    /// Every leaf of the full tree.
    points: &'a [LeafPoint],
    params: &'a TreeParams,
    /// Box around the full tree's wood and leaves; `None` for an empty tree.
    bounds: Option<(Vec3, Vec3)>,
    /// Nodes that reach furthest along each axis, either way.
    reaching_nodes: Vec<usize>,
    /// The leaves that reach furthest along each axis, either way:
    /// `[x low, x high, y low, y high, z low, z high]`.
    reaching_leaves: Option<[usize; 6]>,
}

/// Most rings a branch tube has at each level; a bent branch is a polyline of
/// that many points, a straight one has two whatever this says.
const AXIS_POINTS: [u32; 3] = [u32::MAX, 4, 3];

/// A coarse level keeps the leaf that reaches furthest in some direction only if
/// the leaves it already has fall short of it by more than this share of the
/// crown's extent along that axis.
const REACH_TOLERANCE: f32 = 0.08;

impl Source<'_> {
    /// Level `level` of the tree.
    fn coarse(&self, level: u32, min_radius: f32, leaf_keep: u32, sides: u32, hull_detail: f32) -> TreeMeshes {
        let skeleton = prune(&self.fine.skeleton, min_radius, &self.reaching_nodes);

        // No smoothing, collars or adaptive sides, and a coarser trunk: the
        // point of a coarse level is few triangles.
        let mut wood_params = self.params.clone();
        wood_params.mesh.smooth_iterations = 0;
        wood_params.mesh.adaptive_resolution = false;
        wood_params.mesh.collar_enabled = false;
        // Root flares are lobes around the trunk: with fewer sides than the
        // trunk has they flatten out and the base of a fat trunk shrinks.
        let trunk = &self.params.trunk;
        if !(trunk.root_flare_count > 0 && trunk.root_flare_spread > 0.0) {
            wood_params.trunk.radial_segments = sides.min(trunk.radial_segments).max(3);
        }
        wood_params.trunk.height_segments = (self.params.trunk.height_segments >> level).max(2);
        let rings = AXIS_POINTS[level as usize];
        let trunk = skeleton_mesh_coarse(&skeleton, &wood_params, Coarseness { sides, rings });

        let crown = self.coarse_leaves(leaf_keep, hull_detail);
        TreeMeshes {
            trunk,
            foliage: crown.leaves,
            flowers: crown.flowers,
            skeleton,
            mature_at: self.fine.mature_at,
        }
    }

    /// Every `keep`th leaf, plus the ones that reach furthest where the others
    /// fall short, each as large as the room to the full tree's bounds allows, up
    /// to `sqrt(keep)` times its size: fewer, bigger leaves cover about as much of
    /// the crown.
    fn coarse_leaves(&self, keep: u32, hull_detail: f32) -> FoliageMeshes {
        let keep = keep.max(1) as usize;
        let scale = (keep as f32).sqrt();
        let mut chosen: Vec<bool> = (0..self.points.len()).map(|i| i % keep == 0).collect();
        if let Some(reaching) = self.reaching_leaves {
            for (k, &leaf) in reaching.iter().enumerate() {
                let (axis, high) = (k / 2, k % 2 == 1);
                let at = |i: usize| self.points[i].position[axis];
                let extent = at(reaching[axis * 2 + 1]) - at(reaching[axis * 2]);
                let chosen_reach = chosen
                    .iter()
                    .enumerate()
                    .filter(|&(_, &c)| c)
                    .map(|(i, _)| at(i));
                let shortfall = if high {
                    at(leaf) - chosen_reach.fold(f32::NEG_INFINITY, f32::max)
                } else {
                    chosen_reach.fold(f32::INFINITY, f32::min) - at(leaf)
                };
                if shortfall > REACH_TOLERANCE * extent {
                    chosen[leaf] = true;
                }
            }
        }
        let kept: Vec<LeafPoint> = self
            .points
            .iter()
            .zip(&chosen)
            .filter(|&(_, &c)| c)
            .map(|(p, _)| LeafPoint {
                size: p.size * self.fit(p, scale),
                ..p.clone()
            })
            .collect();
        crown_meshes_coarse(
            &kept,
            self.points,
            self.params,
            trunk_top(&self.fine.skeleton),
            hull_detail,
            self.bounds,
        )
    }

    /// How much `point`'s leaf may be scaled (between 1 and `scale`) without
    /// reaching outside the full tree's bounds. A leaf reaches at most its
    /// `size` from its position.
    fn fit(&self, point: &LeafPoint, scale: f32) -> f32 {
        let Some((lo, hi)) = self.bounds else {
            return 1.0;
        };
        let room = (point.position - lo).min(hi - point.position).min_element();
        (room / point.size.max(1e-6)).clamp(1.0, scale)
    }
}

/// `skeleton` without every non-trunk node whose base radius is below
/// `min_radius` and without its descendants, except for the `keep` nodes and
/// their ancestors. `child_count` is kept as it was, so the decisions that
/// depend on it (which branches are terminal) do not change.
fn prune(skeleton: &Skeleton, min_radius: f32, keep: &[usize]) -> Skeleton {
    // Nodes come in pre-order: a parent has a smaller index than its children.
    let mut required = vec![false; skeleton.nodes.len()];
    for &i in keep {
        required[i] = true;
    }
    for i in (1..skeleton.nodes.len()).rev() {
        if required[i]
            && let Some(parent) = skeleton.nodes[i].parent
        {
            required[parent as usize] = true;
        }
    }

    let mut new_index: Vec<Option<u32>> = vec![None; skeleton.nodes.len()];
    let mut nodes = Vec::with_capacity(skeleton.nodes.len());
    for (i, node) in skeleton.nodes.iter().enumerate() {
        let parent = match node.parent {
            None => None,
            Some(p) => match new_index[p as usize] {
                Some(kept) if required[i] || node.base_radius >= min_radius => Some(kept),
                _ => continue,
            },
        };
        new_index[i] = Some(nodes.len() as u32);
        nodes.push(Node {
            parent,
            ..node.clone()
        });
    }
    Skeleton { nodes }
}

/// Box around the wood and the leaves of `meshes`.
fn bounds(meshes: &TreeMeshes) -> Option<(Vec3, Vec3)> {
    let mut points = meshes
        .trunk
        .positions
        .iter()
        .chain(&meshes.foliage.positions)
        .chain(&meshes.flowers.positions)
        .copied();
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
}

/// The items (of `count`, `None` if there are none) that reach furthest along
/// each axis, either way: `[x low, x high, y low, y high, z low, z high]`. An
/// item reaches as low as `low` and as high as `high`.
fn extremes(
    count: usize,
    low: impl Fn(usize) -> Vec3,
    high: impl Fn(usize) -> Vec3,
) -> Option<[usize; 6]> {
    if count == 0 {
        return None;
    }
    let mut out = [0; 6];
    for axis in 0..3 {
        out[axis * 2] = (0..count).min_by(|&a, &b| low(a)[axis].total_cmp(&low(b)[axis]))?;
        out[axis * 2 + 1] = (0..count).max_by(|&a, &b| high(a)[axis].total_cmp(&high(b)[axis]))?;
    }
    Some(out)
}

/// The branch nodes whose tips (and the wood around them) reach furthest along
/// each axis, either way.
fn reaching_nodes(skeleton: &Skeleton) -> Vec<usize> {
    let reach = |i: usize| Vec3::splat(skeleton.nodes[i].tip_radius);
    extremes(
        skeleton.nodes.len(),
        |i| skeleton.end(i) - reach(i),
        |i| skeleton.end(i) + reach(i),
    )
    .into_iter()
    .flatten()
    .filter(|&i| i > 0)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::assert_indices_valid;
    use crate::presets::TreePreset;

    /// Triangles a level 2 tree may have whatever level 0 had.
    const SMALL_LEVEL_2_TRIANGLES: usize = 500;

    fn triangles(m: &TreeMeshes) -> usize {
        (m.trunk.indices.len() + m.foliage.indices.len() + m.flowers.indices.len()) / 3
    }

    #[test]
    fn levels_get_cheaper_and_keep_the_crown() {
        let settings = LodSettings::default();
        let mut problems = Vec::new();
        for preset in TreePreset::ALL {
            let params = preset.params();
            let lods = build_lods(&params, f32::INFINITY, &settings);
            let name = preset.name();
            let counts = lods.each_ref().map(triangles);
            if !(counts[0] > counts[1] && counts[1] > counts[2]) {
                problems.push(format!("{name}: triangles {counts:?} do not decrease"));
            }
            // A tree that is a few hundred triangles to begin with (a palm, a
            // dead tree) is a trunk and a handful of branches; there is nothing
            // to drop that is not the tree.
            let budget = (0.15 * counts[0] as f32).max(SMALL_LEVEL_2_TRIANGLES as f32);
            if counts[2] as f32 > budget {
                problems.push(format!(
                    "{name}: level 2 has {} of {} triangles",
                    counts[2], counts[0]
                ));
            }

            let (fine_lo, fine_hi) = bounds(&lods[0]).expect("tree has geometry");
            let fine = fine_hi - fine_lo;
            for (level, lod) in lods.iter().enumerate().skip(1) {
                let (lo, hi) = bounds(lod).expect("tree has geometry");
                let size = hi - lo;
                for axis in 0..3 {
                    let ratio = size[axis] / fine[axis];
                    if !(0.9..=1.1).contains(&ratio) {
                        problems.push(format!(
                            "{name}: level {level} is {ratio:.3} of level 0 along axis {axis}"
                        ));
                    }
                }
                // A coarse level is a part of the crown, not a bigger one: the
                // culling box of level 0 holds it (to a centimetre).
                let overshoot = (fine_lo - lo).max(hi - fine_hi).max_element();
                if overshoot > 0.01 {
                    problems.push(format!(
                        "{name}: level {level} reaches {overshoot:.3} m beyond level 0"
                    ));
                }
                assert_indices_valid(&lod.trunk, &format!("{name} level {level} wood"));
                assert_indices_valid(&lod.foliage, &format!("{name} level {level} leaves"));
                assert_indices_valid(&lod.flowers, &format!("{name} level {level} flowers"));
                assert_eq!(lod.flowers.is_empty(), params.foliage.blossom.is_none() || lod.foliage.is_empty(), "{name}");
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn level_zero_is_build_tree() {
        for preset in [TreePreset::Oak, TreePreset::Cherry] {
            let params = preset.params();
            let lods = build_lods(&params, f32::INFINITY, &LodSettings::default());
            let fine = crate::build_tree(&params, f32::INFINITY);
            assert_eq!(lods[0].trunk, fine.trunk);
            assert_eq!(lods[0].foliage, fine.foliage);
            assert_eq!(lods[0].flowers, fine.flowers);
        }
    }

    #[test]
    fn pruning_keeps_parents_consistent() {
        let params = TreePreset::Oak.params();
        let fine = crate::build_tree(&params, f32::INFINITY).skeleton;
        let reaching = reaching_nodes(&fine);
        let pruned = prune(&fine, 0.05, &reaching);
        assert!(pruned.nodes.len() < fine.nodes.len());
        let kept: Vec<u32> = pruned.nodes.iter().map(|n| n.id).collect();
        for &i in &reaching {
            assert!(
                kept.contains(&(i as u32)),
                "node {i} reaches furthest and must stay"
            );
        }
        for (i, node) in pruned.nodes.iter().enumerate() {
            match node.parent {
                None => assert_eq!(i, 0),
                Some(p) => {
                    assert!((p as usize) < i, "parent {p} of node {i} is not before it");
                    let parent = &pruned.nodes[p as usize];
                    assert_eq!(fine.nodes[node.id as usize].parent, Some(parent.id));
                }
            }
        }
    }

    #[test]
    fn a_hull_tree_has_a_smaller_hull_at_each_level_inside_level_zeros_bounds() {
        let params = crate::crown_hull::hull_spruce();
        let lods = build_lods(&params, f32::INFINITY, &LodSettings::default());
        // 5 x 7 at level 0; level 1 keeps 0.7 of it (4 x 5), level 2 half (3 x 4).
        let facets: Vec<usize> = lods.iter().map(|m| m.foliage.indices.len() / 3).collect();
        assert_eq!(facets, [70, 40, 24]);
        let counts = lods.each_ref().map(triangles);
        assert!(counts[0] > counts[1] && counts[1] > counts[2], "{counts:?} do not decrease");
        let (fine_lo, fine_hi) = bounds(&lods[0]).expect("tree has geometry");
        for (level, lod) in lods.iter().enumerate().skip(1) {
            let (lo, hi) = bounds(lod).expect("tree has geometry");
            let overshoot = (fine_lo - lo).max(hi - fine_hi).max_element();
            assert!(overshoot <= 0.01, "level {level} reaches {overshoot} m beyond level 0");
            assert_indices_valid(&lod.foliage, &format!("hull level {level}"));
            assert_indices_valid(&lod.trunk, &format!("prism level {level}"));
            // The crown is still a crown: as tall as level 0's.
            let height = |m: &TreeMeshes| m.foliage.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            assert!((height(lod) - height(&lods[0])).abs() < 0.1 * height(&lods[0]), "level {level} is not as tall");
        }
        // Level 0 is `build_tree` as ever, hull and prism included.
        let fine = crate::build_tree(&params, f32::INFINITY);
        assert_eq!((&lods[0].trunk, &lods[0].foliage), (&fine.trunk, &fine.foliage));
        // `hull_detail` sets how much of the stack a level keeps.
        let full = LodSettings { hull_detail: [1.0; 3], ..LodSettings::default() };
        let same: Vec<usize> = build_lods(&params, f32::INFINITY, &full).iter().map(|m| m.foliage.indices.len() / 3).collect();
        assert_eq!(same, [70, 70, 70]);
    }
}
