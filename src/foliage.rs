//! Leaf placement on the (aged) skeleton and leaf geometry.

use bevy::math::{Vec2, Vec3, Vec4};
use std::f32::consts::{PI, TAU};

use crate::crown_hull::{Hull, HullSpec, crown_triangles};
use crate::growth::GrowthParams;
use crate::mesh::MeshData;
use crate::params::{
    CrownStyle, FoliageParams, FoliagePlacement, LeafOrientation, LeafStyle, RenderStyle, TreeParams, TrunkParams,
};
use crate::rng::SeededRng;
use crate::skeleton::{Node, NodeKind, Skeleton, trunk_depth};
#[cfg(feature = "textures")]
use crate::textures::leaves::LEAF_SETS;

/// Separate random stream for leaf tints, so adding them does not change any
/// existing draw (leaf positions, sizes and rotations stay as in 0.1).
const LEAF_TINT_SALT: i32 = 0x7135_1EAF;

/// Separate random stream for leaf hues, for the same reason.
const LEAF_HUE_SALT: i32 = 0x4E0E_1EAF;

/// One leaf: where it sits and how it is oriented.
#[derive(Clone, Debug, PartialEq)]
pub struct LeafPoint {
    /// World-space (tree-local) position of the leaf, in metres.
    pub position: Vec3,
    /// Primary orientation.
    pub direction: Vec3,
    /// Leaf size in metres (base-to-tip length for leaf-shaped cards).
    pub size: f32,
    /// Rotation around `direction`, radians.
    pub rotation: f32,
    /// Per-leaf colour draw in -1..1, scaled by [`FoliageParams::color_variation`].
    pub tint: f32,
    /// Per-leaf hue draw in −1..1, scaled by [`FoliageParams::hue_variation`].
    pub hue: f32,
    /// The clump this leaf belongs to: the `Node::id` of the nearest ancestor of its node (the
    /// node itself included) at recursion depth 0, that is the primary branch, leader or split it
    /// hangs from. The painterly and toon styles shade the leaves of a clump as one soft mass
    /// ([`RenderStyle::Painterly`]).
    pub clump: u32,
}

/// Leaf points for every node of `skeleton`.
///
/// Each node draws from its own RNG stream (keyed by the node's id in the
/// full skeleton), so aging the tree never re-rolls other nodes' leaves.
pub fn collect_leaf_points(
    skeleton: &Skeleton,
    foliage: &FoliageParams,
    growth: &GrowthParams,
    trunk: &TrunkParams,
    seed: i32,
) -> Vec<LeafPoint> {
    let mut points = Vec::new();
    if !foliage.enabled {
        return points;
    }
    for i in 0..skeleton.nodes.len() {
        leaf_points_for_node(skeleton, i, foliage, growth, trunk, seed, &mut points);
    }
    points
}

/// Growth scale of `node`'s leaves: 0 until `progress` passes
/// [`GrowthParams::leaf_threshold`], 1 once the node is fully grown. Leaf size
/// (and a tip cluster's radius) is proportional to it.
pub(crate) fn node_leaf_scale(node: &Node, growth: &GrowthParams) -> f32 {
    if growth.leaf_threshold >= 1.0 {
        if node.progress >= 1.0 { 1.0 } else { 0.0 }
    } else {
        ((node.progress - growth.leaf_threshold) / (1.0 - growth.leaf_threshold)).clamp(0.0, 1.0)
    }
}

/// Leaves along a branch of `length` metres under
/// [`FoliagePlacement::AllBranches`]: at least one, more on longer wood.
pub(crate) fn distributed_leaf_count(foliage: &FoliageParams, length: f32, height_ratio: f32) -> i32 {
    let height_mod = height_modifier(height_ratio, foliage);
    ((foliage.density * length * height_mod) as i32).max(1)
}

/// Append the leaf points of node `i` to `points`.
#[allow(clippy::too_many_arguments)]
pub fn leaf_points_for_node(
    skeleton: &Skeleton,
    i: usize,
    foliage: &FoliageParams,
    growth: &GrowthParams,
    trunk: &TrunkParams,
    seed: i32,
    points: &mut Vec<LeafPoint>,
) {
    let node = &skeleton.nodes[i];
    // A sub-branch shortened to nothing by `keep_out_of_trunk` starts inside
    // the trunk: it has no wood to carry leaves.
    if node.kind == NodeKind::Trunk || node.length <= 0.0 {
        return;
    }

    let leaf_scale = node_leaf_scale(node, growth);
    if leaf_scale <= 0.0 {
        return;
    }

    // Threshold on the mature radius so the decision does not change with age.
    let full_tip_radius = if node.radius_progress > 0.0 {
        node.tip_radius / node.radius_progress
    } else {
        node.tip_radius
    };
    if trunk.radius > 0.0
        && full_tip_radius / trunk.radius > foliage.radius_threshold
        && foliage.placement != FoliagePlacement::AllBranches
    {
        return;
    }

    let mut rng = SeededRng::from_parts(seed, node.id);
    let first = points.len();
    let ctx = NodeCtx {
        wood: node,
        end: skeleton.end(i),
        direction: node.direction,
        length: node.length,
        height_ratio: node.height_ratio,
        trunk,
        seed,
        leaf_scale,
    };

    match foliage.placement {
        FoliagePlacement::TerminalBranches => {
            if skeleton.is_terminal(i) {
                add_terminal_leaves(points, &ctx, foliage, &mut rng);
            }
        }
        FoliagePlacement::AllBranches => add_distributed_leaves(points, &ctx, foliage, &mut rng),
        FoliagePlacement::TipClusters => {
            if skeleton.is_terminal(i) {
                add_cluster_leaves(points, &ctx, foliage, &mut rng);
            }
        }
    }

    // Tints come from their own stream, drawn in leaf order, so a node's
    // leaves keep their tints as the tree ages.
    let mut tint_rng = SeededRng::from_parts(seed ^ LEAF_TINT_SALT, node.id);
    for p in &mut points[first..] {
        p.tint = tint_rng.range(-1.0, 1.0);
    }

    let mut hue_rng = SeededRng::from_parts(seed ^ LEAF_HUE_SALT, node.id);
    for p in &mut points[first..] {
        p.hue = hue_rng.range(-1.0, 1.0);
    }

    // The leaves of a node shade with the rest of the primary branch (leader or split) it hangs
    // from, whatever the age: ids are the full skeleton's.
    let clump = clump_of(skeleton, i);
    for p in &mut points[first..] {
        p.clump = clump;
    }

    if leaf_scale < 1.0 {
        for p in &mut points[first..] {
            p.size *= leaf_scale;
        }
    }
}

/// The clump node `i`'s leaves belong to: the id of its nearest ancestor (itself included) at
/// recursion depth 0, a primary branch, the leader or a split.
fn clump_of(skeleton: &Skeleton, mut i: usize) -> u32 {
    while skeleton.nodes[i].depth > 0 {
        match skeleton.nodes[i].parent {
            Some(parent) => i = parent as usize,
            None => break,
        }
    }
    skeleton.nodes[i].id
}

struct NodeCtx<'a> {
    /// The node itself; leaves along it follow its (bent) axis.
    wood: &'a Node,
    /// Leaves around a tip must not end up inside the trunk.
    trunk: &'a TrunkParams,
    seed: i32,
    end: Vec3,
    direction: Vec3,
    length: f32,
    height_ratio: f32,
    /// Growth scale of this node's leaves (0..1); tip clusters shrink with it.
    leaf_scale: f32,
}

fn add_terminal_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let height_mod = height_modifier(node.height_ratio, foliage);
    let count = ((foliage.density * height_mod) as i32).max(1);
    for _ in 0..count {
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        let direction = leaf_direction(node, foliage, rng);
        let offset = rng.range(0.0, node.length * 0.2);
        let along = if node.length > 0.0 { 1.0 - offset / node.length } else { 1.0 };
        points.push(LeafPoint {
            position: node.wood.point_at(along),
            direction,
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        });
    }
}

fn add_distributed_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let count = distributed_leaf_count(foliage, node.length, node.height_ratio);
    for i in 0..count {
        let t = (i as f32 + rng.range(0.0, 1.0)) / count as f32;
        let biased_t = t * t;
        let position = node.wood.point_at(0.3 + biased_t * 0.7);
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        let direction = leaf_direction(node, foliage, rng);
        points.push(LeafPoint {
            position,
            direction,
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        });
    }
}

fn add_cluster_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let height_mod = height_modifier(node.height_ratio, foliage);
    let count = ((foliage.cluster_size as f32 * height_mod) as i32).max(1);
    // Leaves fill a ball within 3/4 of a leaf of the tip, so each one
    // overlaps the twig end instead of floating on a shell around it. The
    // ball shrinks with the leaves while the node is young.
    let cluster_radius = foliage.leaf_size * 0.75 * node.leaf_scale;
    for _ in 0..count {
        let theta = rng.range(0.0, TAU);
        let phi = rng.range(0.0, PI);
        let r = cluster_radius * rng.range(0.0, 1.0);
        let mut offset = Vec3::new(
            r * phi.sin() * theta.cos(),
            r * phi.sin() * theta.sin(),
            r * phi.cos(),
        );
        // Twigs near the trunk would put part of the sphere inside it; the
        // mirrored point is outside (the tip is, and the trunk is convex).
        if trunk_depth(node.trunk, node.seed, node.end + offset) > 0.0 {
            offset = -offset;
        }
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        points.push(LeafPoint {
            position: node.end + offset,
            direction: offset.normalize_or_zero(),
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        });
    }
}

fn height_modifier(height_ratio: f32, foliage: &FoliageParams) -> f32 {
    if !foliage.use_crown_density {
        return 1.0;
    }
    let base = 1.0 - foliage.height_falloff;
    base + height_ratio * foliage.height_falloff
}

fn leaf_direction(node: &NodeCtx, foliage: &FoliageParams, rng: &mut SeededRng) -> Vec3 {
    let base_dir = match foliage.orientation {
        LeafOrientation::RadialOutward => {
            let horizontal = Vec3::new(node.end.x, 0.0, node.end.z);
            if horizontal.length_squared() > 0.001 {
                horizontal.normalize()
            } else {
                Vec3::X
            }
        }
        LeafOrientation::FollowBranch => node.direction,
        LeafOrientation::RandomUpward => Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(0.2, 1.0),
            rng.range(-1.0, 1.0),
        )
        .normalize_or_zero(),
        LeafOrientation::HorizontalSpread => {
            let angle = rng.range(0.0, TAU);
            Vec3::new(angle.cos(), 0.0, angle.sin())
        }
    };
    Vec3::new(base_dir.x, base_dir.y - foliage.droop, base_dir.z).normalize_or_zero()
}

/// Geometry for all `points` in `foliage.style`, as [`RenderStyle::Natural`]: see
/// [`foliage_mesh_styled`].
pub fn foliage_mesh(points: &[LeafPoint], foliage: &FoliageParams) -> MeshData {
    foliage_mesh_styled(points, foliage, RenderStyle::Natural)
}

/// Geometry for all `points` in `foliage.style`: the leaves and, for a flowering tree
/// ([`FoliageParams::blossom`]), one flower per leaf after them, in one mesh.
/// [`build_tree`](crate::build_tree) keeps the two apart ([`TreeMeshes::flowers`](crate::TreeMeshes::flowers)).
///
/// Each leaf gets a vertex colour (its tint scaled by
/// [`FoliageParams::color_variation`], its hue by [`FoliageParams::hue_variation`]; a flower's has
/// alpha 0, which is how the shaders tell it), and the leaf shader's inputs are baked in: `uv1.x`
/// flags the upper side of each card, `uv1.y` is the crown visibility
/// ([`FoliageParams::occlusion`]) and `sway` is the flutter vector. Every normal is bent toward
/// pointing out of the crown by [`FoliageParams::normal_bend`] in the natural style; in the
/// painterly and toon styles every leaf takes the normal pointing out of its clump's centre
/// instead (see [`RenderStyle::Painterly`]).
pub fn foliage_mesh_styled(points: &[LeafPoint], foliage: &FoliageParams, style: RenderStyle) -> MeshData {
    foliage_meshes(points, foliage, style).merged()
}

/// A crown's leaf cards and, for a flowering tree ([`FoliageParams::blossom`]), its flowers, one
/// per leaf, as two meshes: the plugin draws the flowers apart so that out of season they cost
/// nothing.
#[derive(Clone, Debug, Default)]
pub(crate) struct FoliageMeshes {
    pub(crate) leaves: MeshData,
    /// Empty for a tree without flowers.
    pub(crate) flowers: MeshData,
    /// Each point's first vertex in `leaves` and in `flowers` (one entry per point, in order;
    /// none in `flowers` for a tree without flowers).
    pub(crate) firsts: [Vec<u32>; 2],
}

impl FoliageMeshes {
    /// The leaves with the flowers after them.
    pub(crate) fn merged(mut self) -> MeshData {
        self.leaves.extend(&self.flowers);
        self.leaves
    }
}

/// The leaves and the flowers of `points`, apart: see [`foliage_mesh_styled`].
pub(crate) fn foliage_meshes(points: &[LeafPoint], foliage: &FoliageParams, style: RenderStyle) -> FoliageMeshes {
    build_foliage(points, foliage, style, false, Crown::Cards)
}

/// [`foliage_meshes`] for the coarse levels of detail: cards cut to a scanned leaf use its
/// coarse outline (6 to 10 points), so a distant leaf costs about what a procedural one does, and
/// flowers are simpler ([`PETAL_COARSE`]).
pub(crate) fn foliage_meshes_coarse(points: &[LeafPoint], foliage: &FoliageParams, style: RenderStyle) -> FoliageMeshes {
    build_foliage(points, foliage, style, true, Crown::Cards)
}

/// How [`build_foliage`] draws the crown.
#[derive(Clone, Copy)]
enum Crown<'a> {
    /// Leaf cards, as the leaf style says.
    Cards,
    /// A faceted hull ([`CrownStyle::Hull`]) in place of the cards, or nothing when the cloud is
    /// too sparse for one. `seed` is the tree's. Fruit and flowers are still drawn.
    Hull { hull: Option<&'a Hull>, seed: i32 },
}

/// The points a hull wraps: every leaf's position and its tip, `size` along its `up`.
fn hull_cloud(points: &[LeafPoint]) -> Vec<Vec3> {
    let mut cloud = Vec::with_capacity(points.len() * 2);
    for point in points {
        let (_, up, _) = build_basis(point.direction, point.rotation);
        cloud.push(point.position);
        cloud.push(point.position + up * point.size);
    }
    cloud
}

/// The hull `params.crown` asks for round `cloud`'s leaves, scaled down by `detail` for a coarse
/// level (see [`HullSpec::scaled`]) and kept inside `bounds`; `None` for leaf cards or a sparse
/// cloud. `trunk_top` is the trunk's top height.
fn hull_for(
    cloud: &[LeafPoint],
    params: &TreeParams,
    trunk_top: f32,
    detail: Option<f32>,
    bounds: Option<(Vec3, Vec3)>,
) -> Option<Hull> {
    let CrownStyle::Hull { tiers, sides, lift, jitter, .. } = params.crown else { return None };
    let spec = HullSpec { tiers, sides, lift, jitter };
    let spec = detail.map_or(spec, |detail| spec.scaled(detail));
    let mut hull = crown_triangles(&hull_cloud(cloud), trunk_top, spec, params.seed)?;
    if let Some((lo, hi)) = bounds {
        hull.clamp_to(lo, hi);
    }
    Some(hull)
}

/// The crown of a tree with `params` at level 0, leaves (or fruit and flowers) and hull as the
/// parameters say: [`foliage_meshes`] for leaf cards. `trunk_top` is the aged trunk's top height.
pub(crate) fn crown_meshes(points: &[LeafPoint], params: &TreeParams, trunk_top: f32) -> FoliageMeshes {
    match params.crown {
        CrownStyle::Leaves => foliage_meshes(points, &params.foliage, params.style),
        CrownStyle::Hull { .. } => {
            let hull = hull_for(points, params, trunk_top, None, None);
            build_foliage(points, &params.foliage, params.style, false, Crown::Hull { hull: hull.as_ref(), seed: params.seed })
        }
    }
}

/// [`crown_meshes`] for a coarse level of detail. `kept` are the leaves it draws (the fruit and
/// flowers of a hull tree); a hull is built from `all` the leaves, with the facets `detail`
/// scales ([`LodSettings::hull_detail`](crate::LodSettings::hull_detail)) and clamped into `bounds`.
pub(crate) fn crown_meshes_coarse(
    kept: &[LeafPoint],
    all: &[LeafPoint],
    params: &TreeParams,
    trunk_top: f32,
    detail: f32,
    bounds: Option<(Vec3, Vec3)>,
) -> FoliageMeshes {
    match params.crown {
        CrownStyle::Leaves => {
            if kept.is_empty() {
                FoliageMeshes::default()
            } else {
                foliage_meshes_coarse(kept, &params.foliage, params.style)
            }
        }
        CrownStyle::Hull { .. } => {
            let hull = hull_for(all, params, trunk_top, Some(detail), bounds);
            build_foliage(kept, &params.foliage, params.style, true, Crown::Hull { hull: hull.as_ref(), seed: params.seed })
        }
    }
}

/// Separate random stream for each hull facet's tint and hue, for the same reason as the leaves'.
const HULL_FACET_SALT: i32 = 0x4641_4345;

/// Separate random stream for each hull face pair's phase.
const HULL_PHASE_SALT: i32 = 0x5048_5345;

/// The vertex colour of a leaf (or facet) with the draws `tint` and `hue` (each in -1..1), alpha 1.
fn leaf_colour(tint: f32, hue: f32, variation: f32, hue_span: f32) -> Vec4 {
    let factor = 1.0 + tint * variation;
    let hue = hue * hue_span;
    Vec4::new(factor * (1.0 + 0.6 * hue), factor * (1.0 + 0.1 * hue), factor * (1.0 - 0.6 * hue), 1.0)
}

/// The flat-shaded mesh of `hull`: three vertices a facet with its face normal, the colour of the
/// leaf formula ([`leaf_colour`]) from draws of the facet's own, `uv1` (1, 1) (a facet is lit on its
/// upper side by the sky, so snow finds it and nothing shades it), uv (0.5, 0.5) and no flutter,
/// only a phase, one for a side face and the underside face on the same two ring points. Every
/// draw is a function of the seed, the tier and the side alone: a hull's colour and phase never
/// change with the tree's age.
fn hull_mesh(hull: &Hull, foliage: &FoliageParams, seed: i32) -> MeshData {
    let variation = foliage.color_variation.clamp(0.0, 0.5);
    let hue_span = foliage.hue_variation.clamp(0.0, 0.5);
    let mut mesh = MeshData::default();
    for facet in &hull.facets {
        let pair = facet.tier * 64 + facet.side;
        let phase = SeededRng::from_parts(seed ^ HULL_PHASE_SALT, pair).next_f32();
        let mut rng = SeededRng::from_parts(seed ^ HULL_FACET_SALT, pair * 2 + u32::from(facet.under));
        let colour = leaf_colour(2.0 * rng.next_f32() - 1.0, 2.0 * rng.next_f32() - 1.0, variation, hue_span);
        let normal = facet.normal();
        let first = mesh.positions.len() as u32;
        mesh.positions.extend_from_slice(&facet.triangle);
        mesh.normals.extend_from_slice(&[normal; 3]);
        mesh.uvs.extend_from_slice(&[Vec2::splat(0.5); 3]);
        mesh.colors.extend_from_slice(&[colour; 3]);
        mesh.uv1.extend_from_slice(&[Vec2::ONE; 3]);
        mesh.sway.extend_from_slice(&[Vec4::new(0.0, 0.0, 0.0, phase); 3]);
        mesh.indices.extend_from_slice(&[first, first + 1, first + 2]);
    }
    mesh
}

fn build_foliage(points: &[LeafPoint], foliage: &FoliageParams, style: RenderStyle, coarse: bool, crown: Crown) -> FoliageMeshes {
    let mut out = FoliageMeshes::default();
    let hull = match crown {
        Crown::Cards => None,
        Crown::Hull { hull, .. } => hull,
    };
    if points.is_empty() && hull.is_none() {
        return out;
    }
    let variation = foliage.color_variation.clamp(0.0, 0.5);
    let hue_span = foliage.hue_variation.clamp(0.0, 0.5);
    let flowering = foliage.blossom.is_some();
    // A fruit's colour over the leaf colour the material multiplies it by (vertex colours scale
    // the material's), as the flowers' blossom colour is in the shaders.
    let fruit = foliage.fruit.as_ref().map(|fruit| (fruit, crate::shading::colour_ratio(fruit.color, foliage.color).truncate()));
    let rigid = foliage.style == LeafStyle::ClusterSphere;
    // Each point's vertices, in the leaves and in the flowers.
    let mut ranges = [Vec::with_capacity(points.len()), Vec::new()];
    for point in points {
        let FoliageMeshes { leaves, flowers, firsts } = &mut out;
        let first = leaves.positions.len();
        firsts[0].push(first as u32);
        // A hull tree has no cards: its crown is the hull, appended last.
        if matches!(crown, Crown::Cards) {
            let card = scanned_card(foliage, point, coarse);
            match foliage.style {
                LeafStyle::CrossedPlanes => crossed_planes(leaves, point, card.as_ref()),
                LeafStyle::SingleQuad => single_quad(leaves, point, card.as_ref()),
                LeafStyle::ClusterSphere => cluster_sphere(leaves, point),
                LeafStyle::StarBurst => star_burst(leaves, point, card.as_ref()),
                LeafStyle::NeedleCluster => needle_cluster(leaves, point),
            }
        }
        let factor = 1.0 + point.tint * variation;
        let colour = leaf_colour(point.tint, point.hue, variation, hue_span);
        leaves.colors.resize(leaves.positions.len(), colour);
        if matches!(crown, Crown::Cards) {
            bake_leaf_inputs(leaves, point, first, rigid);
        }
        // A fruit-bearing leaf's fruit, hanging below its stem, in the leaf mesh after the leaf: it
        // shares the leaf's vertex range (crown visibility; `GpuGrowth` grows it with its leaf).
        if let Some((params, ratio)) = fruit
            && fruit_draw(point) < params.share
        {
            let start = leaves.positions.len();
            fruit_sphere(leaves, point, params.radius * point.size / foliage.leaf_size.max(1e-3), coarse);
            // Darker underneath, as a fruit shades its own underside: in the crown's shade, where
            // only the even ambient light reaches it, it still reads as round.
            let normals = &leaves.normals[start..];
            leaves.colors.extend(normals.iter().map(|n| {
                let shade = FRUIT_UNDERSIDE + (1.0 - FRUIT_UNDERSIDE) * (0.5 + 0.5 * n.y);
                (ratio * factor * shade).extend(FRUIT_ALPHA)
            }));
            bake_fruit_inputs(leaves, point, start);
        }
        ranges[0].push((first, leaves.positions.len()));

        // A flowering tree's flower, at the leaf's stem, in the leaf's colour and variation.
        if flowering {
            let first = flowers.positions.len();
            firsts[1].push(first as u32);
            let side = flower(flowers, point, coarse);
            // A flower's alpha is 0, which the shaders tell it by; its centre is a little darker.
            for k in first..flowers.positions.len() {
                let shade = if (k - first) % side == 0 { FLOWER_CENTRE } else { 1.0 };
                flowers.colors.push((colour.truncate() * shade).extend(0.0));
            }
            bake_leaf_inputs(flowers, point, first, rigid);
            ranges[1].push((first, flowers.positions.len()));
        }
    }

    if !points.is_empty() {
        let center = points.iter().map(|p| p.position).sum::<Vec3>() / points.len() as f32;
        let [leaf_ranges, flower_ranges] = &ranges;
        shade_crown(&mut out.leaves, points, leaf_ranges, center, foliage, style);
        shade_crown(&mut out.flowers, points, flower_ranges, center, foliage, style);
    }
    if let (Some(hull), Crown::Hull { seed, .. }) = (hull, crown) {
        out.leaves.extend(&hull_mesh(hull, foliage, seed));
    }
    out
}

/// The leaf shader's per-vertex inputs for the vertices of `point` from `first` on: which side of
/// the card (`uv1.x`; the crown visibility, `uv1.y`, is set by [`shade_crown`]) and the flutter.
fn bake_leaf_inputs(mesh: &mut MeshData, point: &LeafPoint, first: usize, rigid: bool) {
    // Normals are still the cards' own here, which is what tells the sides apart.
    let MeshData { positions, normals, uv1, sway, .. } = mesh;
    uv1.extend(normals[first..].iter().map(|&n| Vec2::new(if is_upper(n) { 1.0 } else { 0.0 }, 1.0)));

    // One flutter direction and phase per leaf; both faces, both crossed cards and its flower
    // share them. The phase also picks the moment the leaf turns and falls in autumn and its
    // flower opens in spring (`FloraSeason`), so it hashes the leaf's own draws, which never
    // change as the tree grows, not its position, which does: a growing leaf keeps its phase, the
    // same as on the mature mesh `GpuGrowth` shrinks and on the levels of detail.
    let (_, _, forward) = build_basis(point.direction, point.rotation);
    let phase = leaf_phase(point);
    for &vertex in &positions[first..] {
        let lever = if rigid { point.size * 0.4 } else { (vertex - point.position).length() };
        sway.push((forward * lever).extend(phase));
    }
}

/// Crown-wide shading baked into `mesh`, whose vertices `ranges` assigns to `points`: leaves deep
/// in the crown see less sky, and normals point out of the crown (natural) or out of the leaf's
/// clump (painterly, toon).
fn shade_crown(
    mesh: &mut MeshData,
    points: &[LeafPoint],
    ranges: &[(usize, usize)],
    center: Vec3,
    foliage: &FoliageParams,
    style: RenderStyle,
) {
    if mesh.is_empty() {
        return;
    }
    if points.len() >= 2 {
        let occlusion = foliage.occlusion.clamp(0.0, 1.0);
        let extent = points
            .iter()
            .fold(Vec3::ZERO, |m, p| m.max((p.position - center).abs()))
            .max(Vec3::splat(1e-3));
        for (point, &(first, end)) in points.iter().zip(ranges) {
            let rel = (point.position - center) / extent;
            let d = rel.length().min(1.0);
            let up = rel.y.clamp(-1.0, 1.0);
            let openness = (0.8 * d + 0.2 * (0.5 * up + 0.5)).clamp(0.0, 1.0);
            let visibility = 1.0 - occlusion * (1.0 - openness);
            for uv in &mut mesh.uv1[first..end] {
                uv.y = visibility;
            }
        }
    }

    // A fruit keeps its own round normals: bent out of the crown it would shade flat.
    let leaf = |colors: &[Vec4], k: usize| colors.get(k).is_none_or(|c| !is_fruit_alpha(c.w));
    if style == RenderStyle::Natural {
        let bend = foliage.normal_bend.clamp(0.0, 1.0);
        if bend > 0.0 {
            for (k, (normal, &position)) in mesh.normals.iter_mut().zip(&mesh.positions).enumerate() {
                if leaf(&mesh.colors, k) {
                    let out = (position - center).normalize_or(Vec3::Y);
                    *normal = normal.lerp(out, bend).normalize_or(out);
                }
            }
        }
    } else {
        // Painted clumps: every leaf of a clump takes the normal that points out of the clump's
        // own centre (the mean of its leaves), fully: `normal_bend` toward the crown's centre does
        // not apply. The clump of a leaf is the primary branch, leader or split it hangs from.
        let mut sums: std::collections::HashMap<u32, (Vec3, f32)> = std::collections::HashMap::new();
        for point in points {
            let entry = sums.entry(point.clump).or_insert((Vec3::ZERO, 0.0));
            entry.0 += point.position;
            entry.1 += 1.0;
        }
        for (point, &(first, end)) in points.iter().zip(ranges) {
            let (sum, count) = sums[&point.clump];
            let clump_center = sum / count;
            for k in (first..end).filter(|&k| leaf(&mesh.colors, k)) {
                mesh.normals[k] = (mesh.positions[k] - clump_center).normalize_or(Vec3::Y);
            }
        }
    }
}

/// Two leaf cards at 90° sharing `up`. Their bases meet at the leaf point and
/// the tips reach one `size` along `up`, so the leaf grows out of its stem
/// instead of being threaded on it.
fn crossed_planes(mesh: &mut MeshData, point: &LeafPoint, card: Option<&Card>) {
    let half = point.size * 0.5;
    let (right, up, forward) = build_basis(point.direction, point.rotation);
    let center = point.position + up * half;
    add_double_sided_leaf(mesh, center, right * half, up * half, card);
    add_double_sided_leaf(mesh, center, forward * half, up * half, card);
}

/// One leaf card, base at the leaf point and tip one `size` along `up`.
fn single_quad(mesh: &mut MeshData, point: &LeafPoint, card: Option<&Card>) {
    let half = point.size * 0.5;
    let (right, up, _) = build_basis(point.direction, point.rotation);
    add_double_sided_leaf(mesh, point.position + up * half, right * half, up * half, card);
}

/// A flower's width as a share of its leaf's length: a card stands for a little cluster of
/// blossom.
const FLOWER_SIZE: f32 = 0.8;
/// How much darker a flower's centre is than its petals.
const FLOWER_CENTRE: f32 = 0.8;
/// One petal's outline, from its left side to the gap before the next petal: (angle from the
/// petal's middle, in degrees; reach, a share of the flower's radius). A rounded petal with a
/// notched tip, as a cherry's; the coarse levels of detail take its tip and the gap alone.
const PETAL: [(f32, f32); 6] = [(-26.0, 0.78), (-12.0, 0.99), (0.0, 0.88), (12.0, 0.99), (26.0, 0.78), (36.0, 0.42)];
const PETAL_COARSE: [(f32, f32); 2] = [(0.0, 0.95), (36.0, 0.45)];

/// A five-petalled flower at the leaf's stem, [`FLOWER_SIZE`] of the leaf's length across,
/// facing out along the leaf and up: a fan over the five petals' outlines ([`PETAL`], or
/// [`PETAL_COARSE`] for a coarse level), both sides. Its centre is the first vertex of each side;
/// returns the vertices a side has.
fn flower(mesh: &mut MeshData, point: &LeafPoint, coarse: bool) -> usize {
    let petal: &[(f32, f32)] = if coarse { &PETAL_COARSE } else { &PETAL };
    let radius = point.size * FLOWER_SIZE * 0.5;
    let (_, up, _) = build_basis(point.direction, point.rotation);
    let facing = (up + Vec3::Y * 0.7).normalize_or(Vec3::Y);
    let a = facing.any_orthogonal_vector().normalize();
    let b = facing.cross(a);
    let centre = point.position + up * radius * 0.6;
    let n = 5 * petal.len() as u32;
    for (normal, front) in [(facing, true), (-facing, false)] {
        let base = mesh.positions.len() as u32;
        mesh.positions.push(centre);
        mesh.normals.push(normal);
        mesh.uvs.push(Vec2::splat(0.5));
        for k in 0..5 {
            let middle = point.rotation + k as f32 * TAU / 5.0;
            for &(angle, reach) in petal {
                let (sin, cos) = (middle + angle.to_radians()).sin_cos();
                mesh.positions.push(centre + (a * cos + b * sin) * radius * reach);
                mesh.normals.push(normal);
                mesh.uvs.push(Vec2::new(0.5 + 0.5 * cos * reach, 0.5 + 0.5 * sin * reach));
            }
        }
        // Counter-clockwise seen from `facing` (a x b = facing) is the front.
        for k in 0..n {
            let (i, j) = (base + 1 + k, base + 1 + (k + 1) % n);
            mesh.indices.extend_from_slice(&if front { [base, i, j] } else { [base, j, i] });
        }
    }
    n as usize + 1
}

/// A fruit's vertex alpha, which the shaders (`is_fruit`) and [`shade_crown`] tell it by: a leaf's
/// is 1, a flower's 0.
pub(crate) const FRUIT_ALPHA: f32 = 0.25;

/// Whether a vertex colour's alpha is a fruit's ([`FRUIT_ALPHA`]).
pub(crate) fn is_fruit_alpha(alpha: f32) -> bool {
    (0.125..0.5).contains(&alpha)
}

/// A fruit's rings of latitude and meridians: a sphere of 8 sides near, of 6 on a coarse level.
const FRUIT_GRID: (u32, u32) = (6, 8);
const FRUIT_GRID_COARSE: (u32, u32) = (4, 6);
/// A fruit's height over its width: an apple is a little squat.
const FRUIT_SQUAT: f32 = 0.9;
/// What a fruit keeps of its colour on its underside (full on top), baked into its vertices.
const FRUIT_UNDERSIDE: f32 = 0.55;
/// How far a fruit's flutter reaches, a share of its leaf's length: it hangs and swings with its
/// twig rather than flapping as its leaf does.
const FRUIT_LEVER: f32 = 0.15;

/// The leaf's own draw (0..1) of whether it bears a fruit ([`FruitParams::share`](crate::FruitParams::share)): a hash of the
/// leaf's tint and hue draws, which never change as the tree grows (as [`leaf_phase`], with
/// other constants, so which leaves bear fruit has nothing to do with when they turn).
fn fruit_draw(point: &LeafPoint) -> f32 {
    let mut h = point.tint.to_bits().wrapping_mul(0x5BD1_E995) ^ point.hue.to_bits().wrapping_mul(0x1B87_3593);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// A fruit of `radius` hanging just below the leaf's stem: a sphere, a little squat
/// ([`FRUIT_SQUAT`]), with its own outward normals ([`FRUIT_GRID`], or [`FRUIT_GRID_COARSE`] for a
/// coarse level).
fn fruit_sphere(mesh: &mut MeshData, point: &LeafPoint, radius: f32, coarse: bool) {
    let (rings, sides) = if coarse { FRUIT_GRID_COARSE } else { FRUIT_GRID };
    let centre = point.position - Vec3::Y * radius * FRUIT_SQUAT * 1.1;
    let base = mesh.positions.len() as u32;
    for ring in 0..=rings {
        let (sin_lat, cos_lat) = (PI * ring as f32 / rings as f32).sin_cos();
        for side in 0..=sides {
            let (sin_lon, cos_lon) = (point.rotation + TAU * side as f32 / sides as f32).sin_cos();
            let normal = Vec3::new(sin_lat * cos_lon, cos_lat, sin_lat * sin_lon);
            mesh.positions.push(centre + Vec3::new(normal.x, normal.y * FRUIT_SQUAT, normal.z) * radius);
            mesh.normals.push(normal);
            mesh.uvs.push(Vec2::splat(0.5));
        }
    }
    let row = sides + 1;
    for ring in 0..rings {
        for side in 0..sides {
            let (a, b) = (base + ring * row + side, base + (ring + 1) * row + side);
            // Counter-clockwise seen from outside; the poles' degenerate halves are left out.
            if ring > 0 {
                mesh.indices.extend_from_slice(&[a, a + 1, b]);
            }
            if ring + 1 < rings {
                mesh.indices.extend_from_slice(&[a + 1, b + 1, b]);
            }
        }
    }
}

/// The leaf shader's per-vertex inputs for a fruit from `first` on: `uv1` as a leaf's (the crown
/// visibility is set by [`shade_crown`]) and a short, rigid flutter ([`FRUIT_LEVER`]) at its own
/// phase, its leaf's shifted by a half: the fruit's own moment in the season, and a phase apart
/// from its leaf's, which is what tells one card of the leaf mesh from the next (`falling.rs`).
fn bake_fruit_inputs(mesh: &mut MeshData, point: &LeafPoint, first: usize) {
    let MeshData { positions, uv1, sway, .. } = mesh;
    debug_assert_eq!((uv1.len(), sway.len()), (first, first));
    let (_, _, forward) = build_basis(point.direction, point.rotation);
    let phase = (leaf_phase(point) + 0.5).fract();
    uv1.resize(positions.len(), Vec2::new(1.0, 1.0));
    sway.resize(positions.len(), (forward * point.size * FRUIT_LEVER).extend(phase));
}

/// Octahedron.
fn cluster_sphere(mesh: &mut MeshData, point: &LeafPoint) {
    let size = point.size * 0.4;
    let c = point.position;
    let top = c + Vec3::Y * size;
    let bottom = c - Vec3::Y * size;
    let front = c + Vec3::NEG_Z * size;
    let back = c + Vec3::Z * size;
    let left = c + Vec3::NEG_X * size;
    let right = c + Vec3::X * size;

    add_triangle_outward(mesh, c, top, front, right);
    add_triangle_outward(mesh, c, top, right, back);
    add_triangle_outward(mesh, c, top, back, left);
    add_triangle_outward(mesh, c, top, left, front);
    add_triangle_outward(mesh, c, bottom, right, front);
    add_triangle_outward(mesh, c, bottom, back, right);
    add_triangle_outward(mesh, c, bottom, left, back);
    add_triangle_outward(mesh, c, bottom, front, left);
}

/// Three quads at 60°.
fn star_burst(mesh: &mut MeshData, point: &LeafPoint, card: Option<&Card>) {
    let half = point.size * 0.5;
    let (_, up, _) = build_basis(point.direction, point.rotation);
    for i in 0..3 {
        let angle = (i as f32 / 3.0) * TAU + point.rotation;
        let right = Vec3::new(angle.cos(), 0.0, angle.sin());
        add_double_sided_leaf(mesh, point.position, right * half, up * half, card);
    }
}

/// Six thin radiating quads.
fn needle_cluster(mesh: &mut MeshData, point: &LeafPoint) {
    let length = point.size;
    let width = point.size * 0.1;
    let (_, _, forward) = build_basis(point.direction, point.rotation);
    for i in 0..6 {
        let angle = (i as f32 / 6.0) * TAU + point.rotation;
        let needle_dir = Vec3::new(angle.cos(), 0.3, angle.sin()).normalize();
        let needle_right = forward.cross(needle_dir).normalize_or_zero();
        add_double_sided_quad(
            mesh,
            point.position + needle_dir * length * 0.5,
            needle_right * width,
            needle_dir * length * 0.5,
        );
    }
}

/// Orthonormal `(right, up, forward)` with `forward = direction`, rotated by
/// `rotation` around `direction`.
fn build_basis(direction: Vec3, rotation: f32) -> (Vec3, Vec3, Vec3) {
    let world_up = if direction.y.abs() > 0.9 { Vec3::NEG_Z } else { Vec3::Y };
    let right = direction.cross(world_up).normalize_or_zero();
    let up = right.cross(direction).normalize_or_zero();
    let (sin_r, cos_r) = rotation.sin_cos();
    (right * cos_r + up * sin_r, up * cos_r - right * sin_r, direction)
}

/// Whether a card face with normal `n` is the leaf's upper side. Decided on the first of
/// `y, x, z` that is not zero, so exactly one of a double-sided card's two copies is the
/// upper one, even for a vertical card.
fn is_upper(n: Vec3) -> bool {
    [n.y, n.x, n.z].into_iter().find(|c| c.abs() > 1e-4).is_none_or(|c| c > 0.0)
}

/// Quad spanned by `half_right`/`half_up` around `center`, emitted twice with
/// opposite windings; each side's normal is derived from its winding.
fn add_double_sided_quad(mesh: &mut MeshData, center: Vec3, half_right: Vec3, half_up: Vec3) {
    let v0 = center - half_right - half_up;
    let v1 = center + half_right - half_up;
    let v2 = center + half_right + half_up;
    let v3 = center - half_right + half_up;
    let normal = half_right.cross(half_up).normalize_or_zero();
    let uvs = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];

    let front = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[v0, v1, v2, v3]);
    mesh.normals.extend_from_slice(&[normal; 4]);
    mesh.uvs.extend_from_slice(&uvs);
    mesh.indices
        .extend_from_slice(&[front, front + 1, front + 2, front, front + 2, front + 3]);

    let back = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[v0, v1, v2, v3]);
    mesh.normals.extend_from_slice(&[-normal; 4]);
    mesh.uvs.extend_from_slice(&uvs);
    mesh.indices
        .extend_from_slice(&[back, back + 2, back + 1, back, back + 3, back + 2]);
}

/// Pointed oval, base to tip, about half the card's area. Units are
/// half-extents: x along `half_right`, y along `half_up`, base at y = -1, tip
/// at y = 1. Counter-clockwise seen from the front (`half_right × half_up`).
pub(crate) const LEAF_OUTLINE: [Vec2; 8] = [
    Vec2::new(0.0, -1.0),
    Vec2::new(0.55, -0.6),
    Vec2::new(0.8, -0.05),
    Vec2::new(0.6, 0.5),
    Vec2::new(0.0, 1.0),
    Vec2::new(-0.6, 0.5),
    Vec2::new(-0.8, -0.05),
    Vec2::new(-0.55, -0.6),
];

/// A scanned leaf's card: the leaf's real outline (card units, as [`LEAF_OUTLINE`]), its atlas
/// coordinates, its triangulation and its width over length.
#[cfg_attr(not(feature = "textures"), allow(dead_code))]
struct Card {
    aspect: f32,
    outline: &'static [[f32; 2]],
    uv: &'static [[f32; 2]],
    triangles: &'static [u16],
}

/// Whether the leaf cards of `foliage` are cut to a scanned leaf's outline and wear its
/// textures: a [`FoliageParams::scan`] is chosen, the crate has the scans (the `textures`
/// feature) and the style is a leaf-shaped one.
pub(crate) fn uses_scan(foliage: &FoliageParams) -> bool {
    cfg!(feature = "textures")
        && foliage.scan.is_some()
        && matches!(foliage.style, LeafStyle::CrossedPlanes | LeafStyle::SingleQuad | LeafStyle::StarBurst)
}

/// The scanned leaf `point`'s cards are cut to: one of the four leaves of the scan, picked by a
/// hash of the leaf's own random draws (its tint and hue), which never change as the tree ages,
/// so a leaf keeps its shape as the tree grows and `GpuGrowth` draws the same leaf.
#[cfg(feature = "textures")]
fn scanned_card(foliage: &FoliageParams, point: &LeafPoint, coarse: bool) -> Option<Card> {
    if !uses_scan(foliage) {
        return None;
    }
    let hash = point.tint.to_bits().wrapping_mul(0x9E37_79B1) ^ point.hue.to_bits().wrapping_mul(0x85EB_CA6B);
    let cell = ((hash ^ (hash >> 15)) % 4) as usize;
    let leaf = &LEAF_SETS[foliage.scan? as usize].leaves[cell];
    Some(if coarse {
        Card { aspect: leaf.aspect, outline: leaf.coarse_outline, uv: leaf.coarse_uv, triangles: leaf.coarse_triangles }
    } else {
        Card { aspect: leaf.aspect, outline: leaf.outline, uv: leaf.uv, triangles: leaf.triangles }
    })
}

#[cfg(not(feature = "textures"))]
fn scanned_card(_foliage: &FoliageParams, _point: &LeafPoint, _coarse: bool) -> Option<Card> {
    None
}

/// A leaf's flutter phase and season moment, 0..1: a hash of its colour and hue draws (not of
/// the hash [`scanned_card`] takes its cell from, so the two are unrelated).
fn leaf_phase(point: &LeafPoint) -> f32 {
    let mut h = point.tint.to_bits().wrapping_mul(0x2C1B_3C6D) ^ point.hue.to_bits().wrapping_mul(0x297A_2D39);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Leaf-shaped card spanned by `half_right`/`half_up` around `center`: a fan over
/// [`LEAF_OUTLINE`], or, with a scanned `card`, the scan's outline (as wide as its width over
/// length says) and triangulation, with the scan's atlas coordinates as UVs. Emitted twice with
/// opposite windings; each side's normal is derived from its winding.
fn add_double_sided_leaf(mesh: &mut MeshData, center: Vec3, half_right: Vec3, half_up: Vec3, card: Option<&Card>) {
    let normal = half_right.cross(half_up).normalize_or_zero();
    if let Some(card) = card {
        let half_right = half_right * card.aspect;
        for (side_normal, front) in [(normal, true), (-normal, false)] {
            let base = mesh.positions.len() as u32;
            for (p, uv) in card.outline.iter().zip(card.uv) {
                mesh.positions.push(center + half_right * p[0] + half_up * p[1]);
                mesh.uvs.push(Vec2::from_array(*uv));
            }
            mesh.normals.extend(std::iter::repeat_n(side_normal, card.outline.len()));
            for triangle in card.triangles.as_chunks::<3>().0 {
                let [a, b, c] = triangle.map(|i| base + u32::from(i));
                if front {
                    mesh.indices.extend_from_slice(&[a, b, c]);
                } else {
                    mesh.indices.extend_from_slice(&[a, c, b]);
                }
            }
        }
        return;
    }
    let n = LEAF_OUTLINE.len() as u32;
    for (side_normal, front) in [(normal, true), (-normal, false)] {
        let base = mesh.positions.len() as u32;
        mesh.positions.push(center);
        mesh.uvs.push(Vec2::splat(0.5));
        for p in LEAF_OUTLINE {
            mesh.positions.push(center + half_right * p.x + half_up * p.y);
            mesh.uvs.push((p + Vec2::ONE) * 0.5);
        }
        mesh.normals
            .extend(std::iter::repeat_n(side_normal, n as usize + 1));
        for i in 0..n {
            let a = base + 1 + i;
            let b = base + 1 + (i + 1) % n;
            if front {
                mesh.indices.extend_from_slice(&[base, a, b]);
            } else {
                mesh.indices.extend_from_slice(&[base, b, a]);
            }
        }
    }
}

/// Triangle wound so its face points away from `center`.
fn add_triangle_outward(mesh: &mut MeshData, center: Vec3, a: Vec3, b: Vec3, c: Vec3) {
    let geometric = (b - a).cross(c - a);
    let outward = (a + b + c) / 3.0 - center;
    let (v1, v2) = if geometric.dot(outward) >= 0.0 { (b, c) } else { (c, b) };
    let normal = (v1 - a).cross(v2 - a).normalize_or_zero();
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[a, v1, v2]);
    mesh.normals.extend_from_slice(&[normal; 3]);
    mesh.uvs.extend_from_slice(&[
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.5, 1.0),
    ]);
    mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::{assert_faces_outward, assert_indices_valid};
    use crate::params::TreeParams;
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    fn point() -> LeafPoint {
        LeafPoint {
            position: Vec3::ZERO,
            direction: Vec3::Y,
            size: 1.0,
            rotation: 0.0,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        }
    }

    /// Foliage settings with flat per-card normals and uniform colour.
    fn flat(style: LeafStyle) -> FoliageParams {
        FoliageParams {
            style,
            normal_bend: 0.0,
            color_variation: 0.0,
            hue_variation: 0.0,
            ..FoliageParams::default()
        }
    }

    #[test]
    fn build_basis_produces_orthonormal() {
        for dir in [Vec3::Y, Vec3::X, Vec3::new(0.3, 0.9, 0.1).normalize()] {
            let (right, up, forward) = build_basis(dir, 0.7);
            assert!((right.length() - 1.0).abs() < 1e-5);
            assert!((up.length() - 1.0).abs() < 1e-5);
            assert!(right.dot(up).abs() < 1e-5);
            assert!(right.dot(forward).abs() < 1e-5);
            assert!(up.dot(forward).abs() < 1e-5);
        }
    }

    /// Per leaf, the number of vertices its cards have, of a mesh built with `foliage`.
    #[cfg(feature = "textures")]
    fn vertices_per_leaf(points: &[LeafPoint], foliage: &FoliageParams) -> Vec<usize> {
        let built = foliage_meshes(points, foliage, RenderStyle::Natural);
        let firsts = &built.firsts[0];
        let ends = firsts.iter().skip(1).copied().chain([built.leaves.positions.len() as u32]);
        firsts.iter().zip(ends).map(|(&a, b)| (b - a) as usize).collect()
    }

    #[cfg(feature = "textures")]
    #[test]
    fn scanned_cards_use_the_scans_outline_atlas_coordinates_and_all_four_leaves() {
        let (mut params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        params.foliage.scan = Some(crate::params::LeafScan::Oak);
        let foliage = FoliageParams { normal_bend: 0.0, ..params.foliage.clone() };
        assert!(uses_scan(&foliage));
        let mesh = foliage_mesh(&points, &foliage);
        assert_indices_valid(&mesh, "scanned oak");
        // The procedural card is a fan of 9 vertices a side; a scanned one has its outline's.
        let counts = vertices_per_leaf(&points, &foliage);
        let outline_counts: std::collections::BTreeSet<usize> = crate::textures::leaves::LEAF_SETS
            [crate::params::LeafScan::Oak as usize]
            .leaves
            .iter()
            .map(|leaf| 2 * 2 * leaf.outline.len())
            .collect();
        assert!(counts.iter().all(|c| outline_counts.contains(c)), "leaf vertex counts {counts:?} vs {outline_counts:?}");
        let used: std::collections::BTreeSet<usize> = counts.iter().copied().collect();
        assert_eq!(used, outline_counts, "every one of the set's leaves is used");
        // Atlas coordinates, not card coordinates: inside the atlas, and not the card's own.
        assert!(mesh.uvs.iter().all(|uv| (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y)));
        let procedural = foliage_mesh(&points, &FoliageParams { scan: None, ..foliage.clone() });
        assert_ne!(mesh.uvs[0], procedural.uvs[0]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn a_leaf_keeps_its_scanned_shape_as_the_tree_ages() {
        let (mut params, mature) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        params.foliage.scan = Some(crate::params::LeafScan::Maple);
        let foliage = FoliageParams { normal_bend: 0.0, ..params.foliage.clone() };
        let skeleton = generate_skeleton(&params).aged(6.0, &params.growth);
        let young = collect_leaf_points(&skeleton, &foliage, &params.growth, &params.trunk, params.seed);
        assert!(!young.is_empty() && young.len() < mature.len());
        let by_tint: std::collections::HashMap<(u32, u32), usize> = mature
            .iter()
            .zip(vertices_per_leaf(&mature, &foliage))
            .map(|(p, n)| ((p.tint.to_bits(), p.hue.to_bits()), n))
            .collect();
        for (p, n) in young.iter().zip(vertices_per_leaf(&young, &foliage)) {
            assert_eq!(by_tint.get(&(p.tint.to_bits(), p.hue.to_bits())), Some(&n), "a leaf changed shape with age");
        }
    }

    #[test]
    fn a_leaf_keeps_its_phase_as_the_tree_ages() {
        // The phase is when a leaf flutters and when it turns and falls in autumn, so a growing
        // tree must not reshuffle it while its leaves move outward.
        let (params, mature) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let skeleton = generate_skeleton(&params).aged(6.0, &params.growth);
        let young = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
        assert!(!young.is_empty() && young.len() < mature.len());
        let phases = |points: &[LeafPoint]| {
            let built = foliage_meshes(points, &params.foliage, RenderStyle::Natural);
            let (mesh, firsts) = (built.leaves, built.firsts[0].clone());
            let key = |p: &LeafPoint| (p.tint.to_bits(), p.hue.to_bits());
            points
                .iter()
                .zip(firsts)
                .map(|(p, first)| (key(p), (p.position, mesh.sway[first as usize].w)))
                .collect::<std::collections::HashMap<_, _>>()
        };
        let (old, new) = (phases(&mature), phases(&young));
        let mut moved = 0;
        for (key, (position, phase)) in &new {
            let (mature_position, mature_phase) = old[key];
            assert_eq!(phase.to_bits(), mature_phase.to_bits(), "a leaf changed its phase with age");
            moved += usize::from(position.distance(mature_position) > 1e-3);
        }
        assert!(moved > new.len() / 2, "the young leaves should sit elsewhere than the mature ones");
        let distinct: std::collections::BTreeSet<u32> = old.values().map(|(_, phase)| phase.to_bits()).collect();
        assert!(distinct.len() > old.len() * 9 / 10, "leaves share phases");
    }

    #[test]
    fn only_leaf_shaped_styles_use_a_scan() {
        let mut foliage = FoliageParams { scan: Some(crate::params::LeafScan::Birch), ..FoliageParams::default() };
        for (style, uses) in [
            (LeafStyle::CrossedPlanes, true),
            (LeafStyle::SingleQuad, true),
            (LeafStyle::StarBurst, true),
            (LeafStyle::ClusterSphere, false),
            (LeafStyle::NeedleCluster, false),
        ] {
            foliage.style = style;
            assert_eq!(uses_scan(&foliage), uses && cfg!(feature = "textures"), "{style:?}");
        }
        foliage.style = LeafStyle::CrossedPlanes;
        foliage.scan = None;
        assert!(!uses_scan(&foliage));
    }

    #[test]
    fn leaf_vertex_counts() {
        let p = point();
        let count = |style| foliage_mesh(std::slice::from_ref(&p), &flat(style)).positions.len();
        assert_eq!(count(LeafStyle::CrossedPlanes), 36);
        assert_eq!(count(LeafStyle::SingleQuad), 18);
        assert_eq!(count(LeafStyle::ClusterSphere), 24);
        assert_eq!(count(LeafStyle::StarBurst), 54);
        assert_eq!(count(LeafStyle::NeedleCluster), 48);
    }

    #[test]
    fn leaf_meshes_face_outward() {
        // Flat normals: bent normals deliberately disagree with the winding.
        let p = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        };
        for style in [
            LeafStyle::CrossedPlanes,
            LeafStyle::SingleQuad,
            LeafStyle::ClusterSphere,
            LeafStyle::StarBurst,
            LeafStyle::NeedleCluster,
        ] {
            let mesh = foliage_mesh(std::slice::from_ref(&p), &flat(style));
            assert_indices_valid(&mesh, &format!("{style:?}"));
            assert_faces_outward(&mesh, &format!("{style:?}"));
        }
    }

    #[test]
    fn leaf_cards_grow_out_of_their_attachment_point() {
        // D-045: cards centred on the leaf point were threaded on their twig
        // and read as green rings around thick limbs.
        let p = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            tint: 0.0,
            hue: 0.0,
            clump: 0,
        };
        let (_, up, _) = build_basis(p.direction, p.rotation);
        for style in [LeafStyle::CrossedPlanes, LeafStyle::SingleQuad] {
            let mesh = foliage_mesh(std::slice::from_ref(&p), &flat(style));
            let (min, max) = mesh
                .positions
                .iter()
                .map(|v| (*v - p.position).dot(up))
                .fold((f32::MAX, f32::MIN), |(lo, hi), a| (lo.min(a), hi.max(a)));
            assert!(min > -1e-5, "{style:?}: card reaches {min} m behind the leaf point");
            assert!((max - p.size).abs() < 1e-5, "{style:?}: tip {max} m out, leaf size {}", p.size);
        }
    }

    fn oak_leaf_points(placement: FoliagePlacement, age: f32) -> (TreeParams, Vec<LeafPoint>) {
        let mut params = TreePreset::Oak.params();
        // The tests that read it count vertices per leaf: every procedural card has the same
        // number, a scanned one has the count of its outline.
        params.foliage.scan = None;
        params.foliage.placement = placement;
        let skeleton = generate_skeleton(&params).aged(age, &params.growth);
        let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
        (params, points)
    }

    #[test]
    fn a_flowering_tree_has_a_flower_per_leaf_in_a_mesh_of_its_own() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let plain = foliage_meshes(&points, &params.foliage, RenderStyle::Natural);
        assert!(plain.flowers.is_empty() && plain.firsts[1].is_empty(), "a tree without blossom has flowers");
        let flowering = FoliageParams { blossom: Some(bevy::color::Color::srgb(1.0, 0.8, 0.9)), ..params.foliage.clone() };
        let built = foliage_meshes(&points, &flowering, RenderStyle::Natural);
        // The leaves are the same with or without flowers, and opaque.
        assert_eq!(built.leaves, plain.leaves);
        assert!(built.leaves.colors.iter().all(|c| c.w == 1.0));
        assert_indices_valid(&built.flowers, "flowering oak");
        let flower = 2 * (5 * PETAL.len() + 1);
        assert_eq!(built.flowers.positions.len(), points.len() * flower);
        assert_eq!(built.firsts[1].len(), points.len());
        assert!(built.flowers.colors.iter().all(|c| c.w == 0.0), "a flower is flagged by its alpha");
        for (p, (&leaf, &first)) in built.firsts[0].iter().zip(&built.firsts[1]).enumerate() {
            assert_eq!(first as usize, p * flower);
            // A flower opens, sways and grows with its leaf: it shares the leaf's phase.
            let phase = built.leaves.sway[leaf as usize].w;
            assert!(built.flowers.sway[first as usize..first as usize + flower].iter().all(|s| s.w == phase));
        }
        // In one mesh, the flowers come after all the leaves.
        let merged = foliage_mesh(&points, &flowering);
        assert_eq!(merged.positions.len(), built.leaves.positions.len() + built.flowers.positions.len());
        assert_eq!(merged.positions[..built.leaves.positions.len()], built.leaves.positions[..]);
    }

    #[test]
    fn a_fruiting_tree_hangs_round_fruit_below_a_share_of_its_leaves() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let fruiting = FoliageParams {
            fruit: Some(crate::params::FruitParams { share: 0.25, ..Default::default() }),
            ..params.foliage.clone()
        };
        for style in [RenderStyle::Natural, RenderStyle::Painterly] {
            let plain = foliage_meshes(&points, &params.foliage, style);
            let built = foliage_meshes(&points, &fruiting, style);
            assert_indices_valid(&built.leaves, "fruiting oak");
            let range = |m: &FoliageMeshes, p: usize| {
                m.firsts[0][p] as usize..m.firsts[0].get(p + 1).map_or(m.leaves.positions.len(), |&n| n as usize)
            };
            let mut bearing = 0;
            for (p, point) in points.iter().enumerate() {
                // Each leaf's vertices are as without fruit; its fruit, if any, follows them.
                let (bare, full) = (range(&plain, p), range(&built, p));
                let leaf = full.start..full.start + bare.len();
                assert_eq!(built.leaves.positions[leaf.clone()], plain.leaves.positions[bare.clone()]);
                assert_eq!(built.leaves.normals[leaf.clone()], plain.leaves.normals[bare.clone()]);
                let fruit = leaf.end..full.end;
                if fruit.is_empty() {
                    continue;
                }
                bearing += 1;
                assert!(built.leaves.colors[fruit.clone()].iter().all(|c| c.w == FRUIT_ALPHA), "{style:?}: a fruit is flagged by its alpha");
                // A phase of its own, apart from its leaf's: the falling leaves tell cards apart by it.
                let phase = built.leaves.sway[fruit.start].w;
                assert!(built.leaves.sway[fruit.clone()].iter().all(|s| s.w == phase));
                assert_ne!(phase, built.leaves.sway[leaf.start].w);
                // Round and hanging below the stem: its normals and faces point out of its centre,
                // whatever the style does to the leaves' normals.
                let centre = built.leaves.positions[fruit.clone()].iter().sum::<Vec3>() / fruit.len() as f32;
                assert!(centre.y < point.position.y, "{style:?}: a fruit above its stem");
                for k in fruit.clone() {
                    assert!(built.leaves.normals[k].dot(built.leaves.positions[k] - centre) > 0.0, "{style:?}: a fruit's normal points in");
                }
                for triangle in built.leaves.indices.as_chunks::<3>().0 {
                    let [a, b, c] = triangle.map(|i| built.leaves.positions[i as usize]);
                    if fruit.contains(&(triangle[0] as usize)) {
                        assert!((b - a).cross(c - a).dot((a + b + c) / 3.0 - centre) > 0.0, "{style:?}: a fruit's face points in");
                    }
                }
            }
            let share = bearing as f32 / points.len() as f32;
            assert!((0.2..0.3).contains(&share), "{style:?}: {share} of the leaves bear fruit");
        }
    }

    #[test]
    fn bent_normals_point_out_of_the_crown() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        assert!(!points.is_empty());
        let center = points.iter().map(|p| p.position).sum::<Vec3>() / points.len() as f32;
        for normal_bend in [params.foliage.normal_bend, 1.0] {
            let foliage = FoliageParams { normal_bend, ..params.foliage.clone() };
            let mesh = foliage_mesh(&points, &foliage);
            assert_indices_valid(&mesh, "oak");
            for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
                assert!((normal.length() - 1.0).abs() < 1e-4, "bend {normal_bend}: non-unit normal {normal:?}");
                assert!(
                    normal.dot(*position - center) > 0.0,
                    "bend {normal_bend}: normal {normal:?} at {position:?} points into the crown"
                );
            }
        }
    }

    #[test]
    fn painterly_normals_point_out_of_the_leafs_own_clump() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let mut sums = std::collections::HashMap::new();
        for p in &points {
            let entry = sums.entry(p.clump).or_insert((Vec3::ZERO, 0.0f32));
            entry.0 += p.position;
            entry.1 += 1.0;
        }
        assert!(sums.len() > 3, "an oak has several clumps, found {}", sums.len());

        let built = foliage_meshes(&points, &params.foliage, RenderStyle::Painterly);
        let (painted, firsts) = (&built.leaves, &built.firsts[0]);
        assert_indices_valid(painted, "painterly oak");
        for (i, point) in points.iter().enumerate() {
            let end = firsts.get(i + 1).map_or(painted.positions.len(), |&n| n as usize);
            let (sum, count) = sums[&point.clump];
            let center = sum / count;
            for k in firsts[i] as usize..end {
                let expected = (painted.positions[k] - center).normalize_or(Vec3::Y);
                assert!(
                    painted.normals[k].dot(expected) > 0.9999,
                    "normal {:?} does not point out of clump {} (expected {expected:?})",
                    painted.normals[k],
                    point.clump
                );
            }
        }

        // Only the normals differ from the natural style's; toon bends them the same way.
        let natural = foliage_mesh(&points, &params.foliage);
        assert_eq!(painted.positions, natural.positions);
        assert_eq!(painted.colors, natural.colors);
        assert_eq!(painted.uv1, natural.uv1);
        assert_ne!(painted.normals, natural.normals);
        assert_eq!(foliage_mesh_styled(&points, &params.foliage, RenderStyle::Toon).normals, painted.normals);
    }

    #[test]
    fn a_clump_is_a_primary_branch_leader_or_split_with_everything_that_grows_from_it() {
        let skeleton = generate_skeleton(&TreePreset::Oak.params());
        let mut subs = 0;
        for (i, node) in skeleton.nodes.iter().enumerate() {
            let clump = clump_of(&skeleton, i);
            let owner = &skeleton.nodes[clump as usize];
            assert_eq!(owner.id, clump, "ids index the full skeleton");
            assert_eq!(owner.depth, 0, "node {i} belongs to clump {clump}, which is not a depth 0 node");
            match node.parent {
                Some(parent) if node.depth > 0 => {
                    subs += 1;
                    assert_eq!(clump, clump_of(&skeleton, parent as usize), "a sub-branch joins its parent's clump");
                }
                _ => assert_eq!(clump, node.id, "a primary branch, leader or split is its own clump"),
            }
        }
        assert!(subs > 0, "the oak has sub-branches");
    }

    #[test]
    fn leaf_colors_vary_per_leaf_within_bounds() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        assert_eq!(params.foliage.style, LeafStyle::CrossedPlanes);
        let leaf_vertices = 36;

        let varied = FoliageParams { color_variation: 0.2, hue_variation: 0.0, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &varied);
        assert_eq!(mesh.colors.len(), mesh.positions.len());
        assert_eq!(mesh.colors.len(), points.len() * leaf_vertices);
        let mut distinct = std::collections::BTreeSet::new();
        for leaf in mesh.colors.chunks(leaf_vertices) {
            assert!(leaf.iter().all(|c| *c == leaf[0]), "one leaf, several colours");
            let c = leaf[0];
            assert!((0.8 - 1e-5..=1.2 + 1e-5).contains(&c.x), "factor {} out of bounds", c.x);
            assert!(c.x == c.y && c.y == c.z && c.w == 1.0, "tint must be grey and opaque: {c:?}");
            distinct.insert(c.x.to_bits());
        }
        assert!(distinct.len() > 1, "all leaves share one colour");

        let plain = FoliageParams { color_variation: 0.0, hue_variation: 0.0, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &plain);
        assert!(mesh.colors.iter().all(|c| *c == Vec4::ONE));
    }

    #[test]
    fn leaf_hues_vary_per_leaf() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let foliage = FoliageParams { hue_variation: 0.5, color_variation: 0.0, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &foliage);
        let leaf_vertices = 36;
        let (mut warm, mut cool) = (false, false);
        for leaf in mesh.colors.chunks(leaf_vertices) {
            assert!(leaf.iter().all(|c| *c == leaf[0]), "one leaf, several colours");
            let c = leaf[0];
            assert!((c.x - 1.0).abs() <= 0.3 + 1e-5, "red {} out of bounds", c.x);
            warm |= c.x > c.z;
            cool |= c.x < c.z;
        }
        assert!(warm && cool, "hues do not lean both ways");
    }

    #[test]
    fn upper_flag_marks_the_face_that_looks_up() {
        let tilted = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            ..point()
        };
        let sideways = LeafPoint { direction: Vec3::X, ..point() };
        for style in [
            LeafStyle::CrossedPlanes,
            LeafStyle::SingleQuad,
            LeafStyle::ClusterSphere,
            LeafStyle::StarBurst,
            LeafStyle::NeedleCluster,
        ] {
            for p in [point(), tilted.clone(), sideways.clone()] {
                let mesh = foliage_mesh(std::slice::from_ref(&p), &flat(style));
                assert_eq!(mesh.uv1.len(), mesh.positions.len());
                for (uv, &normal) in mesh.uv1.iter().zip(&mesh.normals) {
                    assert!(uv.x == 0.0 || uv.x == 1.0, "{style:?}: flag {}", uv.x);
                    assert_eq!(uv.x == 1.0, is_upper(normal), "{style:?}: flag against normal {normal:?}");
                }
                // A card is emitted twice, back to back; exactly one copy is the upper side.
                let face = match style {
                    LeafStyle::ClusterSphere => continue,
                    LeafStyle::NeedleCluster => 4,
                    _ => 9,
                };
                for card in mesh.uv1.chunks(2 * face) {
                    assert!(
                        card[..face].iter().all(|uv| uv.x != card[face].x)
                            && card[..face].iter().all(|uv| uv.x == card[0].x)
                            && card[face..].iter().all(|uv| uv.x == card[face].x),
                        "{style:?}: a card's two faces share a side"
                    );
                }
            }
        }
    }

    #[test]
    fn crown_occlusion_is_darkest_inside() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        let leaf_vertices = 36;
        let center = points.iter().map(|p| p.position).sum::<Vec3>() / points.len() as f32;
        let by_distance = |a: &&(usize, &LeafPoint), b: &&(usize, &LeafPoint)| {
            (a.1.position - center).length().total_cmp(&(b.1.position - center).length())
        };
        let indexed: Vec<_> = points.iter().enumerate().collect();
        let nearest = indexed.iter().min_by(by_distance).unwrap().0;
        let farthest = indexed.iter().max_by(by_distance).unwrap().0;

        let dark = FoliageParams { occlusion: 0.6, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &dark);
        assert!(mesh.uv1.iter().all(|uv| (0.4 - 1e-5..=1.0).contains(&uv.y)), "visibility out of [0.4, 1]");
        assert!(
            mesh.uv1[nearest * leaf_vertices].y < mesh.uv1[farthest * leaf_vertices].y,
            "the leaf at the crown's heart is not darker than the outermost one"
        );

        let open = FoliageParams { occlusion: 0.0, ..params.foliage.clone() };
        assert!(foliage_mesh(&points, &open).uv1.iter().all(|uv| uv.y == 1.0));
    }

    #[test]
    fn leaf_sway_is_one_motion_per_leaf() {
        let a = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            ..point()
        };
        // Two leaves with their own colour and hue draws, as every real leaf has: the phase
        // comes from them.
        let b = LeafPoint { position: Vec3::new(-2.0, 3.0, 1.0), tint: 0.3, hue: -0.6, ..a.clone() };
        for style in [
            LeafStyle::CrossedPlanes,
            LeafStyle::SingleQuad,
            LeafStyle::ClusterSphere,
            LeafStyle::StarBurst,
            LeafStyle::NeedleCluster,
        ] {
            let points = [a.clone(), b.clone()];
            let mesh = foliage_mesh(&points, &flat(style));
            assert_eq!(mesh.sway.len(), mesh.positions.len());
            let per_leaf = mesh.positions.len() / 2;
            let mut phases = Vec::new();
            for (i, p) in points.iter().enumerate() {
                let range = i * per_leaf..(i + 1) * per_leaf;
                let (_, _, forward) = build_basis(p.direction, p.rotation);
                let phase = mesh.sway[range.start].w;
                assert!((0.0..1.0).contains(&phase), "{style:?}: phase {phase}");
                phases.push(phase);
                for (sway, &vertex) in mesh.sway[range.clone()].iter().zip(&mesh.positions[range]) {
                    assert_eq!(sway.w, phase, "{style:?}: a leaf with two phases");
                    let offset = sway.truncate();
                    assert!(offset.cross(forward).length() < 1e-4, "{style:?}: flutter not along the leaf's direction");
                    match style {
                        LeafStyle::CrossedPlanes | LeafStyle::SingleQuad => {
                            assert!((offset.length() - (vertex - p.position).length()).abs() < 1e-5, "{style:?}: lever");
                        }
                        LeafStyle::ClusterSphere => {
                            assert!((offset.length() - p.size * 0.4).abs() < 1e-5, "{style:?}: a cluster must move as one");
                        }
                        _ => {}
                    }
                }
            }
            assert_ne!(phases[0], phases[1], "{style:?}: two leaves flutter in step");
        }
    }

    #[test]
    fn branches_shortened_into_the_trunk_carry_no_leaves() {
        // A cypress on a wobbly trunk has sub-branches whose parent runs inside
        // the trunk; they are shortened to nothing, and their leaves sat
        // inside the trunk.
        let mut params = crate::presets::TreePreset::Cypress.params();
        params.seed = 1;
        params.trunk.randomness = 0.5;
        params.trunk.twist = 180.0;
        params.branches.twist = 90.0;
        let s = crate::skeleton::generate_skeleton(&params);
        let points = collect_leaf_points(&s, &params.foliage, &params.growth, &params.trunk, params.seed);
        for p in &points {
            let depth = trunk_depth(&params.trunk, params.seed, p.position);
            assert!(depth <= p.size * 0.5, "leaf {depth} m inside the trunk at {:?}", p.position);
        }
    }

    #[test]
    fn leaf_tints_never_reroll_as_the_tree_ages() {
        let tints = |age: f32| -> std::collections::BTreeSet<u32> {
            let (_, points) = oak_leaf_points(FoliagePlacement::TerminalBranches, age);
            points.iter().map(|p| p.tint.to_bits()).collect()
        };
        let young = tints(10.0);
        let mature = tints(f32::INFINITY);
        assert!(young.len() > 1, "no tints drawn at age 10");
        assert!(young.is_subset(&mature), "a leaf's tint changed while the tree aged");
    }

    #[test]
    fn collect_leaf_points_disabled() {
        let params = TreePreset::Oak.params();
        let skeleton = generate_skeleton(&params);
        let foliage = FoliageParams {
            enabled: false,
            ..params.foliage.clone()
        };
        let points = collect_leaf_points(&skeleton, &foliage, &params.growth, &params.trunk, 42);
        assert!(points.is_empty());
    }

    #[test]
    fn leaves_scale_with_age() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let leaves_at = |age: f32| {
            let aged = full.aged(age, &params.growth);
            collect_leaf_points(&aged, &params.foliage, &params.growth, &params.trunk, params.seed)
        };
        let young = leaves_at(2.0);
        let mid = leaves_at(params.growth.maturity_age * 0.3);
        let mature = leaves_at(f32::INFINITY);
        assert!(!mature.is_empty());
        assert!(young.len() < mature.len(), "{} vs {}", young.len(), mature.len());
        let max_size = |pts: &[LeafPoint]| pts.iter().map(|p| p.size).fold(0.0, f32::max);
        assert!(max_size(&mid) < max_size(&mature));
    }

    #[test]
    fn leaf_points_stable_under_aging() {
        let mut params = TreePreset::Oak.params();
        params.foliage.placement = FoliagePlacement::AllBranches;
        params.foliage.orientation = LeafOrientation::FollowBranch;
        let full = generate_skeleton(&params);
        let (idx, primary) = full
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.kind == NodeKind::Primary)
            .unwrap();
        let id = primary.id;
        let relative_leaves = |age: f32| {
            let aged = full.aged(age, &params.growth);
            let i = aged.nodes.iter().position(|n| n.id == id).expect("primary alive");
            let mut pts = Vec::new();
            leaf_points_for_node(&aged, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
            let end = aged.end(i);
            pts.into_iter()
                .map(|p| (p.position - end, p.direction, p.size, p.rotation))
                .collect::<Vec<_>>()
        };
        let a = relative_leaves(primary.birth + primary.duration);
        let b = relative_leaves(f32::INFINITY);
        assert!(!a.is_empty(), "node {idx} should have leaves");
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert!(x.0.distance(y.0) < 1e-4);
            assert_eq!(x.1, y.1);
            assert_eq!(x.2, y.2);
            assert_eq!(x.3, y.3);
        }
    }

    #[test]
    fn leaves_sit_on_bent_wood() {
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 0.8;
        params.branches.stiffness = 0.1;
        let skeleton = generate_skeleton(&params);
        let axes: Vec<Vec<Vec3>> = skeleton
            .nodes
            .iter()
            .skip(1)
            .map(|n| n.axis(n.axis_points()).into_iter().map(|(c, _)| c).collect())
            .collect();
        let off_wood = |p: Vec3| {
            axes.iter()
                .flat_map(|axis| axis.windows(2))
                .map(|w| {
                    let ab = w[1] - w[0];
                    let t = ((p - w[0]).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
                    p.distance(w[0] + ab * t)
                })
                .fold(f32::MAX, f32::min)
        };
        for placement in [FoliagePlacement::AllBranches, FoliagePlacement::TerminalBranches] {
            params.foliage.placement = placement;
            let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
            assert!(!points.is_empty());
            for p in &points {
                let d = off_wood(p.position);
                assert!(d < 1e-4, "{placement:?}: leaf at {:?} is {d} off the bent wood", p.position);
            }
        }
    }

    #[test]
    fn no_leaf_inside_the_trunk() {
        // Cypress twigs hug the trunk, so its tip clusters reach toward it.
        for preset in [TreePreset::Cypress, TreePreset::Poplar, TreePreset::Oak] {
            for placement in [FoliagePlacement::TipClusters, FoliagePlacement::TerminalBranches, FoliagePlacement::AllBranches] {
                let mut params = preset.params();
                params.foliage.placement = placement;
                let skeleton = generate_skeleton(&params);
                let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
                for p in &points {
                    let depth = trunk_depth(&params.trunk, params.seed, p.position);
                    assert!(depth <= 0.0, "{} {placement:?}: leaf {depth} m inside the trunk", preset.name());
                }
            }
        }
    }

    #[test]
    fn young_tip_clusters_stay_on_their_tips() {
        // D-037: the cluster ball must shrink with its leaves while the tree
        // is young, or a 1 cm leaf floats tens of cm off its twig.
        for preset in [TreePreset::DragonTree, TreePreset::Oak, TreePreset::Cypress] {
            let mut params = preset.params();
            params.foliage.placement = FoliagePlacement::TipClusters;
            let mature = generate_skeleton(&params);
            for age in [4.0, 6.0, 9.0, 12.0] {
                let skeleton = mature.aged(age, &params.growth);
                let tips: Vec<Vec3> = (0..skeleton.nodes.len())
                    .filter(|&i| skeleton.nodes[i].kind != NodeKind::Trunk && skeleton.is_terminal(i))
                    .map(|i| skeleton.end(i))
                    .collect();
                let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
                let max_ratio = 0.75 / (1.0 - params.foliage.leaf_size_variation) + 1e-3;
                for p in &points {
                    let d = tips.iter().map(|t| t.distance(p.position)).fold(f32::MAX, f32::min);
                    assert!(
                        d <= p.size * max_ratio + 1e-5,
                        "{} age {age}: leaf of size {} sits {d} m from the nearest tip",
                        preset.name(),
                        p.size
                    );
                }
            }
        }
    }

    #[test]
    fn leaves_never_disappear_as_a_tree_ages() {
        // D-039: tip placements must not drop a branch's leaves when its
        // first (still leafless) twigs are born; baobab went from 80 leaves
        // at age 6 to none at age 8.
        for preset in TreePreset::ALL {
            for placement in [FoliagePlacement::TerminalBranches, FoliagePlacement::TipClusters] {
                let mut params = preset.params();
                params.foliage.placement = placement;
                let mature = generate_skeleton(&params);
                let mut previous = 0;
                for step in 0..=40 {
                    let age = step as f32 * 0.5;
                    let skeleton = mature.aged(age, &params.growth);
                    let count = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed).len();
                    assert!(
                        count >= previous,
                        "{} {placement:?}: {previous} leaves before age {age}, {count} at it",
                        preset.name()
                    );
                    previous = count;
                }
            }
        }
    }
}
