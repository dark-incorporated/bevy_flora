//! One call from parameters + age to CPU meshes.

use crate::foliage::{FoliageMeshes, collect_leaf_points, crown_meshes};
use crate::mesh::MeshData;
use crate::mesh::branch::{apply_pipe_radius, skeleton_mesh};
use crate::params::TreeParams;
use crate::skeleton::{Skeleton, generate_skeleton};

/// Output of [`build_tree`].
#[derive(Clone, Debug)]
pub struct TreeMeshes {
    /// Trunk, collars and branches.
    pub trunk: MeshData,
    /// Leaves; empty when foliage is disabled or nothing has leafed yet.
    pub foliage: MeshData,
    /// A flowering tree's flowers ([`FoliageParams::blossom`](crate::FoliageParams::blossom)),
    /// one at each leaf's stem; empty for a tree without flowers. Drawn with the leaves'
    /// material; [`FloraPlugin`](crate::FloraPlugin) draws them apart from the leaves so that it
    /// can leave them out while [`FloraSeason::blossom`](crate::FloraSeason::blossom) is 0.
    pub flowers: MeshData,
    /// The aged skeleton the meshes were built from.
    pub skeleton: Skeleton,
    /// Age after which the output no longer changes.
    pub mature_at: f32,
}

/// Build the tree described by `params` as it looks at `age`
/// (`f32::INFINITY` for the mature tree).
pub fn build_tree(params: &TreeParams, age: f32) -> TreeMeshes {
    build_aged(&generate_skeleton(params), params, age)
}

/// The skeleton `full` (from [`generate_skeleton`]) as it looks at `age`: the
/// aged nodes, with pipe-model radii when `params.mesh.pipe_radius_enabled`.
pub(crate) fn aged_skeleton(full: &Skeleton, params: &TreeParams, age: f32) -> Skeleton {
    let mut skeleton = full.aged(age, &params.growth);
    if params.mesh.pipe_radius_enabled {
        apply_pipe_radius(
            &mut skeleton,
            params.mesh.pipe_radius_exponent,
            params.mesh.pipe_radius_min,
        );
    }
    skeleton
}

/// [`build_tree`] from the full-grown skeleton `full` that
/// [`generate_skeleton`] made from `params`, so a growing tree can reuse it
/// from frame to frame instead of regenerating it at every age.
pub(crate) fn build_aged(full: &Skeleton, params: &TreeParams, age: f32) -> TreeMeshes {
    let mature_at = full.mature_at(&params.growth);
    let skeleton = aged_skeleton(full, params, age);

    let trunk = skeleton_mesh(&skeleton, params);

    let crown = if params.foliage.enabled {
        let points = collect_leaf_points(
            &skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        );
        crown_meshes(&points, params, trunk_top(&skeleton))
    } else {
        FoliageMeshes::default()
    };

    TreeMeshes {
        trunk,
        foliage: crown.leaves,
        flowers: crown.flowers,
        skeleton,
        mature_at,
    }
}

/// Where the trunk chain ends: the top of the trunk or of its leader. A hull crown
/// ([`CrownStyle::Hull`](crate::CrownStyle)) is never lower than this.
pub(crate) fn trunk_top(skeleton: &Skeleton) -> f32 {
    skeleton
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.kind, crate::skeleton::NodeKind::Trunk | crate::skeleton::NodeKind::Leader))
        .map(|(i, _)| skeleton.end(i).y)
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::assert_indices_valid;
    use crate::presets::TreePreset;

    #[test]
    fn build_tree_all_presets_mature() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            for age in [f32::INFINITY, 3.0] {
                let out = build_tree(&params, age);
                let what = format!("{} @ {age}", preset.name());
                assert!(!out.trunk.is_empty(), "{what}: empty trunk");
                assert_indices_valid(&out.trunk, &what);
                assert_indices_valid(&out.foliage, &what);
                assert_indices_valid(&out.flowers, &what);
                assert!(out.mature_at.is_finite() && out.mature_at > 0.0);
                // Bonsai's density (0.8) yields zero primaries over its branch zone, as in the
                // original generator, so it is a bare trunk with no leaves.
                if age.is_infinite() && params.foliage.enabled && out.skeleton.nodes.len() > 1 {
                    assert!(!out.foliage.is_empty(), "{what}: mature tree has no leaves");
                }
                assert_eq!(out.flowers.is_empty(), params.foliage.blossom.is_none() || out.foliage.is_empty(), "{what}");
            }
        }
    }

    #[test]
    fn pipe_radius_path_builds() {
        let mut params = TreePreset::Oak.params();
        params.mesh.pipe_radius_enabled = true;
        params.mesh.smooth_iterations = 2;
        params.mesh.adaptive_resolution = true;
        let out = build_tree(&params, f32::INFINITY);
        assert_indices_valid(&out.trunk, "pipe+smooth+adaptive");
    }

    use crate::crown_hull::hull_spruce;
    use crate::params::{CrownStyle, TrunkStyle};
    use bevy::math::{Vec2, Vec3, Vec4};

    #[test]
    fn no_preset_has_a_hull_or_a_prism_and_every_default_is_leaves_and_round() {
        assert_eq!((TreeParams::default().crown, TreeParams::default().trunk_style), (CrownStyle::Leaves, TrunkStyle::Round));
        for preset in TreePreset::ALL {
            let params = preset.params();
            assert_eq!((params.crown, params.trunk_style), (CrownStyle::Leaves, TrunkStyle::Round), "{}", preset.name());
            assert!(params.gpu_growable(), "{}", preset.name());
        }
        assert!(!hull_spruce().gpu_growable());
        let mut prism = TreePreset::Oak.params();
        prism.trunk_style = TrunkStyle::Prism { sides: 5 };
        assert!(!prism.gpu_growable());
    }

    #[test]
    fn a_hull_crown_is_tiers_times_sides_times_two_flat_facets_in_the_leaf_mesh() {
        let params = hull_spruce();
        let out = build_tree(&params, f32::INFINITY);
        let facets = out.foliage.indices.len() / 3;
        assert_eq!(facets, 5 * 7 * 2, "the crown's triangles");
        assert!(out.flowers.is_empty());
        assert_indices_valid(&out.foliage, "hull");
        let leaves = &out.foliage;
        assert_eq!(leaves.positions.len(), facets * 3, "three vertices a facet");
        for (f, tri) in leaves.indices.as_chunks::<3>().0.iter().enumerate() {
            let [a, b, c] = tri.map(|i| leaves.positions[i as usize]);
            let geometric = (b - a).cross(c - a).normalize();
            for &i in tri {
                let i = i as usize;
                assert!(leaves.normals[i].distance(geometric) < 1e-4, "facet {f} is not flat");
                assert_eq!(leaves.colors[i].w, 1.0, "facet {f}: a leaf's alpha");
                assert_eq!(leaves.uv1[i], Vec2::ONE, "facet {f}: lit by the whole sky");
                assert_eq!(leaves.uvs[i], Vec2::splat(0.5));
                assert_eq!(leaves.sway[i].truncate(), Vec3::ZERO, "facet {f} has no flutter");
            }
            assert_eq!(leaves.sway[tri[0] as usize].w, leaves.sway[tri[2] as usize].w);
        }
        // The same parameters, the same bytes.
        let again = build_tree(&params, f32::INFINITY);
        assert_eq!((again.foliage, again.trunk), (out.foliage, out.trunk));
    }

    #[test]
    fn a_hulls_colour_and_phase_never_change_with_age() {
        let params = hull_spruce();
        let (young, grown) = (build_tree(&params, 6.0), build_tree(&params, f32::INFINITY));
        for out in [&young, &grown] {
            assert_eq!(out.foliage.indices.len() / 3, 70, "both ages have a crown to compare");
        }
        let key = |m: &MeshData| -> Vec<(Vec4, u32)> { m.colors.iter().zip(&m.sway).map(|(c, s)| (*c, s.w.to_bits())).collect() };
        assert_eq!(key(&young.foliage), key(&grown.foliage));
        assert_ne!(young.foliage.positions, grown.foliage.positions, "the crown itself grows");
        // The side face and the underside face on the same two ring points share a phase.
        let phases: std::collections::HashSet<u32> = grown.foliage.sway.iter().map(|s| s.w.to_bits()).collect();
        assert_eq!(phases.len(), 5 * 7, "one phase for each pair of faces");
    }

    #[test]
    fn a_tree_too_sparse_for_a_hull_has_none() {
        let out = build_tree(&hull_spruce(), 0.0);
        assert!(out.foliage.is_empty(), "a seedling has {} crown triangles", out.foliage.indices.len() / 3);
        let mut params = hull_spruce();
        params.foliage.enabled = false;
        assert!(build_tree(&params, f32::INFINITY).foliage.is_empty());
    }

    #[test]
    fn a_prism_trunk_is_flat_shaded_with_the_sides_it_was_given_and_faces_out() {
        for sides in [3_u32, 5, 8] {
            let mut params = hull_spruce();
            params.trunk_style = TrunkStyle::Prism { sides };
            params.trunk.root_flare_count = 0;
            let wood = build_tree(&params, f32::INFINITY).trunk;
            assert_indices_valid(&wood, "prism");
            assert_eq!(wood.positions.len(), wood.indices.len(), "every triangle has its own vertices");
            for tri in wood.indices.as_chunks::<3>().0 {
                let [a, b, c] = tri.map(|i| wood.positions[i as usize]);
                let geometric = (b - a).cross(c - a);
                if geometric.length_squared() < 1e-12 {
                    continue;
                }
                let n = geometric.normalize();
                assert!(tri.iter().all(|&i| wood.normals[i as usize].distance(n) < 1e-4), "wood is not flat");
            }
            // The ground ring: the trunk's `sides` corners (and the bottom cap's centre).
            let ring: std::collections::HashSet<[i32; 2]> = wood
                .positions
                .iter()
                .filter(|p| p.y.abs() < 1e-5 && p.x.hypot(p.z) > 1e-4)
                .map(|p| [(p.x * 1e4).round() as i32, (p.z * 1e4).round() as i32])
                .collect();
            assert_eq!(ring.len() as u32, sides, "a trunk with {sides} sides");
            // The trunk's faces below the first branch (within its own radius of the axis, so no
            // branch or collar) point away from its axis.
            let near = params.trunk.radius * 1.5;
            let first_branch = params.trunk.height * params.branches.start;
            let mut checked = 0;
            for tri in wood.indices.as_chunks::<3>().0 {
                let [a, b, c] = tri.map(|i| wood.positions[i as usize]);
                let centroid = (a + b + c) / 3.0;
                let n = (b - a).cross(c - a);
                if centroid.y > 0.05 && centroid.y < first_branch && centroid.x.hypot(centroid.z) < near && n.length_squared() > 1e-12 {
                    assert!(n.x * centroid.x + n.z * centroid.z > 0.0, "{sides} sides: a trunk face at {centroid} faces in");
                    checked += 1;
                }
            }
            assert!(checked >= sides as usize, "{sides} sides: only {checked} triangles checked");
        }
    }

    #[test]
    fn branch_tubes_and_collars_have_at_most_the_prisms_sides() {
        use crate::mesh::branch::{WoodPiece, WoodSpan, skeleton_mesh_with};
        use crate::mesh::trunk::trunk_pieces;
        for sides in [3_u32, 4] {
            let mut params = TreePreset::Oak.params();
            params.mesh.smooth_iterations = 2;
            params.mesh.adaptive_resolution = true;
            params.trunk_style = TrunkStyle::Prism { sides };
            let skeleton = crate::skeleton::generate_skeleton(&params).aged(f32::INFINITY, &params.growth);
            let mut spans: Vec<WoodSpan> = Vec::new();
            skeleton_mesh_with(&skeleton, &params, trunk_pieces(&params.trunk, &skeleton.nodes[0]), Some(&mut spans));
            let tubes: Vec<u32> = spans
                .iter()
                .filter(|s| matches!(s.piece, WoodPiece::Tube | WoodPiece::Collar))
                .map(|s| s.segments)
                .collect();
            assert!(tubes.len() > 10, "{sides} sides: the oak has {} tubes and collars", tubes.len());
            assert!(tubes.iter().all(|&s| s <= sides), "{sides} sides: a tube has {:?}", tubes.iter().max());
        }
    }
}
