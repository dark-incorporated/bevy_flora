//! Branch tube mesher, trunk collars, the pipe-radius model, and the whole-skeleton wood mesh.

use bevy::math::{Vec2, Vec3};

use super::MeshData;
use super::rings::{RingBasis, RingUv, connect_rings, emit_cap, emit_ring, mature_size, u_tiles_around};
use super::smoothing::laplacian_smooth;
use super::trunk::{TrunkPieces, trunk_mesh_with, trunk_pieces};
use crate::params::{BarkScan, TreeParams, TrunkStyle};
use crate::skeleton::{Node, NodeKind, Skeleton};

/// The bark scan's tile size in metres for `params`: that of its scan, `1 x 1` without one.
pub(crate) fn bark_tile(params: &TreeParams) -> Vec2 {
    params.bark.scan.map_or(Vec2::ONE, BarkScan::tile_size)
}

/// Where a node's wood sits in its bark scan's tile space. Lengths and radii are the *mature*
/// ones (the aged values over their progress), so a growing node's texture coordinates never
/// change and the aged mesh agrees with the mature mesh GPU growth shrinks.
#[derive(Clone, Copy, Debug)]
struct BarkLayout {
    /// Size of the scan's tile in metres.
    tile: Vec2,
    /// Whole tiles around every ring of the node.
    u_tiles: f32,
    /// How grown the node's radius is.
    radius_progress: f32,
}

impl BarkLayout {
    /// The layout of `node`'s wood.
    fn new(node: &Node, tile: Vec2) -> Self {
        let radius_progress = node.radius_progress;
        let u_tiles = u_tiles_around(mature_size(node.base_radius, radius_progress), tile.x);
        Self { tile, u_tiles, radius_progress }
    }

    /// The layout of fully grown wood of `radius` metres.
    fn fully_grown(radius: f32, tile: Vec2) -> Self {
        Self { tile, u_tiles: u_tiles_around(radius, tile.x), radius_progress: 1.0 }
    }

    /// A ring `along` metres (of the fully grown wood) from the start of the node's wood.
    fn ring(self, along: f32) -> RingUv {
        RingUv { v: along / self.tile.y, u_tiles: self.u_tiles }
    }

    /// How far a cap of (aged) `radius` reaches from the middle of its disc, in tiles.
    fn cap(self, radius: f32) -> Vec2 {
        let mature = mature_size(radius, self.radius_progress);
        Vec2::new(mature / self.tile.x, mature / self.tile.y)
    }
}

/// Wood at or below this height is the trunk's base on the ground.
const GROUND_EPSILON: f32 = 1.0e-4;

/// Smoothing leaves wood within this many radii of a branch end or of a
/// child's seat untouched (the cap ring and the patch under the child's base).
const SMOOTH_PIN_REACH: f32 = 1.5;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Short tube of `radius` from `root` (a point inside the trunk) to `base`,
/// with both rings perpendicular to `axis` and untwisted, so its `base` ring
/// coincides with the base ring of a tube or collar starting at `base` along
/// `axis`. Extends that wood back into the trunk; empty when `root` is not
/// behind `base` along `axis`.
pub fn root_segment(root: Vec3, base: Vec3, axis: Vec3, segments: u32, radius: f32, tile: Vec2) -> MeshData {
    let mut mesh = MeshData::default();
    append_root_segment(&mut mesh, root, base, axis, segments, radius, BarkLayout::fully_grown(radius, tile));
    mesh
}

/// Whether [`append_root_segment`] emits anything: `root` must lie behind
/// `base` along `axis`.
pub(crate) fn root_segment_emitted(root: Vec3, base: Vec3, axis: Vec3) -> bool {
    (base - root).dot(RingBasis::from_axis(axis).axis()) > 1e-4
}

/// [`root_segment`], appended to `mesh`.
fn append_root_segment(
    mesh: &mut MeshData,
    root: Vec3,
    base: Vec3,
    axis: Vec3,
    segments: u32,
    radius: f32,
    layout: BarkLayout,
) {
    if root_segment_emitted(root, base, axis) {
        let basis = RingBasis::from_axis(axis);
        let segments = segments.max(3);
        let a = emit_ring(mesh, root, basis, segments, 0.0, layout.ring(0.0), |_, _| radius);
        let b = emit_ring(mesh, base, basis, segments, 0.0, layout.ring(0.0), |_, _| radius);
        connect_rings(mesh, a, b, segments);
    }
}

/// Tapered tube along `node`'s (possibly bent) axis with optional twist,
/// capped at the tip.
pub fn tube_mesh(node: &Node, radial_segments: u32, twist_deg: f32, tile: Vec2) -> MeshData {
    let mut mesh = MeshData::default();
    append_tube(&mut mesh, node, radial_segments, twist_deg, u32::MAX, tile);
    mesh
}

/// Rings (axis points) of `node`'s tube: two for straight untwisted wood,
/// the node's [`Node::axis_points`] when it bends or twists. Independent of age.
pub(crate) fn tube_rings(node: &Node, twist_deg: f32) -> u32 {
    if twist_deg.abs() > 0.1 || node.bend > 0.0 { node.axis_points() } else { 2 }
}

/// [`tube_mesh`], appended to `mesh`. A tube with more than `max_rings` rings
/// keeps `max_rings` of them, spread along the axis and including both ends, so
/// every ring still lies on the full-resolution axis.
fn append_tube(mesh: &mut MeshData, node: &Node, radial_segments: u32, twist_deg: f32, max_rings: u32, tile: Vec2) {
    let segments = radial_segments.max(3);
    let full = tube_rings(node, twist_deg);
    let rings = full.min(max_rings.max(2));
    let axis = node.axis(full);
    let layout = BarkLayout::new(node, tile);
    let mature_length = mature_size(node.length, node.progress);

    let mut prev_ring = None;
    for k in 0..rings {
        // `k * (full - 1) / (rings - 1)` is `k` itself when nothing is dropped.
        let source = (k as f32 * (full - 1) as f32 / (rings - 1) as f32).round() as usize;
        let (center, dir) = axis[source];
        let t = source as f32 / (full - 1) as f32;
        let radius = lerp(node.base_radius, node.tip_radius, t);
        let twist = (t * twist_deg).to_radians();
        let basis = RingBasis::from_axis(dir);
        let ring = emit_ring(mesh, center, basis, segments, twist, layout.ring(t * mature_length), |_, _| radius);
        if let Some(prev) = prev_ring {
            connect_rings(mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }

    let (end_pos, end_dir) = axis[full as usize - 1];
    emit_cap(
        mesh,
        end_pos,
        RingBasis::from_axis(end_dir),
        segments,
        twist_deg.to_radians(),
        end_dir,
        layout.cap(node.tip_radius),
        |_, _| node.tip_radius,
    );
}

/// Start of a primary's collar (its wide end) for `collar_mesh` arguments.
pub(crate) fn collar_start(start: Vec3, direction: Vec3, collar_length: f32) -> Vec3 {
    start - direction * collar_length * 0.1
}

/// Width of a collar's wide end relative to the branch radius.
const COLLAR_FLARE: f32 = 1.3;

/// Flared cone blending primary `node` into the trunk surface. It ends half
/// a `collar_length` along the branch (at most at its tip) with the tube's
/// own radius there, so it never stands proud of a tapering branch.
pub fn collar_mesh(node: &Node, collar_length: f32, radial_segments: u32, tile: Vec2) -> MeshData {
    let mut mesh = MeshData::default();
    append_collar(&mut mesh, node, collar_length, radial_segments, tile);
    mesh
}

/// Where a primary's collar runs and how wide it is at each end.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CollarGeometry {
    /// Centre of the wide end (outside the branch's start, along its direction).
    pub start: Vec3,
    /// Centre of the narrow end.
    pub end: Vec3,
    /// Radius of the wide end.
    pub start_radius: f32,
    /// Radius of the narrow end: the tube's own radius there.
    pub end_radius: f32,
}

/// The collar of primary `node`, `collar_length` metres long (see [`collar_mesh`]).
pub(crate) fn collar_geometry(node: &Node, collar_length: f32) -> CollarGeometry {
    let along = (collar_length * 0.5 / node.length).min(1.0);
    CollarGeometry {
        start: collar_start(node.start, node.direction, collar_length),
        end: node.point_at(along),
        start_radius: node.base_radius * COLLAR_FLARE,
        end_radius: lerp(node.base_radius, node.tip_radius, along),
    }
}

/// [`collar_mesh`], appended to `mesh`.
fn append_collar(mesh: &mut MeshData, node: &Node, collar_length: f32, radial_segments: u32, tile: Vec2) {
    let segments = radial_segments.max(3);
    let rings = 3u32;
    let basis = RingBasis::from_axis(node.direction);
    let collar = collar_geometry(node, collar_length);
    // Where the collar's ends lie along the fully grown branch, so its bark carries on the
    // tube's (the same `u_tiles`, and `v` by distance along the branch).
    let layout = BarkLayout::new(node, tile);
    let mature_collar = mature_size(collar_length, node.radius_progress);
    let (from, to) = (-0.1 * mature_collar, (0.5 * mature_collar).min(mature_size(node.length, node.progress)));

    let mut prev_ring = None;
    for k in 0..rings {
        let t = k as f32 / (rings - 1) as f32;
        let center = collar.start.lerp(collar.end, t);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        let radius = lerp(collar.start_radius, collar.end_radius, smooth_t);
        let ring = emit_ring(mesh, center, basis, segments, 0.0, layout.ring(lerp(from, to, t)), |_, _| radius);
        if let Some(prev) = prev_ring {
            connect_rings(mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }
}

/// Parent radius from child radii under the pipe model
/// (`parent^exponent = Σ child^exponent`), at least `min_radius`.
pub fn calculate_pipe_radius(child_radii: &[f32], exponent: f32, min_radius: f32) -> f32 {
    if child_radii.is_empty() {
        return min_radius;
    }
    let accumulated: f32 = child_radii.iter().map(|r| r.powf(exponent)).sum();
    accumulated.powf(1.0 / exponent).max(min_radius)
}

/// Recompute every non-trunk node's radii bottom-up from its children. The
/// `min_radius` floor scales with each node's `radius_progress`, so a branch
/// that has only just been born stays thin instead of becoming a wide,
/// paper-thin disc.
pub fn apply_pipe_radius(skeleton: &mut Skeleton, exponent: f32, min_radius: f32) {
    let n = skeleton.nodes.len();
    if n < 2 {
        return;
    }
    // Σ child_base^exponent per node; children have larger indices, so a
    // reverse sweep sees every child before its parent.
    let mut accumulated = vec![0.0f32; n];
    let mut has_child = vec![false; n];
    for i in (1..n).rev() {
        if has_child[i] {
            let floor = min_radius * skeleton.nodes[i].radius_progress;
            let tip = accumulated[i].powf(1.0 / exponent).max(floor);
            let node = &mut skeleton.nodes[i];
            node.tip_radius = tip;
            if node.base_radius < tip {
                node.base_radius = tip * 1.2;
            }
        }
        let node = &skeleton.nodes[i];
        if let Some(p) = node.parent {
            accumulated[p as usize] += node.base_radius.powf(exponent);
            has_child[p as usize] = true;
        }
    }
    for node in skeleton.nodes.iter_mut().skip(1) {
        if node.tip_radius > node.base_radius {
            node.tip_radius = node.base_radius * 0.8;
        }
        let floor = min_radius * node.radius_progress;
        node.base_radius = node.base_radius.max(floor);
        node.tip_radius = node.tip_radius.max(floor * 0.5);
    }
}

/// Fewest sides of a non-adaptive branch tube; 4 read as flat square planks.
const MIN_BRANCH_SEGMENTS: u32 = 6;

/// Radial segment count for a branch node.
fn radial_segments_for(node: &Node, params: &TreeParams) -> u32 {
    let m = &params.mesh;
    if m.adaptive_resolution {
        let segments = (node.base_radius * m.resolution_scale).round() as u32;
        segments.clamp(m.min_radial_segments, m.max_radial_segments)
    } else if node.depth > 0 {
        MIN_BRANCH_SEGMENTS
    } else {
        (params.trunk.radial_segments / 2).max(MIN_BRANCH_SEGMENTS)
    }
}

/// Whether primary `node` leaves the trunk too steeply to be extended back to
/// the trunk axis: less than 0.5 of its direction points away from the trunk.
fn leaves_steeply(node: &Node) -> bool {
    let radial = (node.start - node.anchor).with_y(0.0);
    node.direction.dot(radial.normalize_or_zero()) < 0.5
}

/// Where a primary's wood begins inside the trunk, so tube and collar have no
/// open, flat-cut end at the trunk surface (`start`). `anchor` is the trunk
/// axis point at the same height.
///
/// A branch leaving the trunk at 30° or more is extended backward along its
/// own axis to the trunk axis (at most twice the trunk radius), so it emerges
/// from the trunk. A steeper branch (see [`leaves_steeply`]) would need a long
/// extension that shows as a ridge down the trunk; `primary_base` closes it
/// with a rounded end instead, and this returns `None`.
pub(crate) fn primary_root(node: &Node) -> Option<Vec3> {
    if leaves_steeply(node) {
        return None;
    }
    let radial = (node.start - node.anchor).with_y(0.0);
    let outward = node.direction.dot(radial.normalize_or_zero());
    let back = (radial.length() / outward).min(0.9 * node.start.y / node.direction.y.max(1e-3));
    Some(node.start - node.direction * back)
}

/// The wood behind a primary's base ring at `at` (radius `radius`, around the
/// branch axis), appended to `mesh`: an extension into the trunk, or for a
/// steep branch a rounded end, so the base ring coincides with the tube's or
/// collar's and nothing shows a flat cut. With `flat_end` a steep branch is
/// closed with a flat disc instead of the rounded end: a few triangles, for the
/// coarse levels of detail, where the knot is too small to see.
fn append_primary_base(
    mesh: &mut MeshData,
    node: &Node,
    at: Vec3,
    radius: f32,
    segments: u32,
    flat_end: bool,
    layout: BarkLayout,
) {
    match primary_root(node) {
        Some(root) => append_root_segment(mesh, root, at, node.direction, segments, radius, layout),
        None if flat_end => {
            let basis = RingBasis::from_axis(node.direction);
            emit_cap(mesh, at, basis, segments.max(3), 0.0, -node.direction, layout.cap(radius), |_, _| radius);
        }
        None => append_root_dome(mesh, at, node.direction, segments, radius, layout),
    }
}

/// A rounded, closed end behind the ring of `radius` at `base`, appended to
/// `mesh`: the base of a branch that hugs the trunk, so it grows out of the
/// trunk like a knot instead of ending in a flat disc. Its last ring
/// (`segments` sides) coincides with the tube's base ring; the four rings
/// behind it have twice the sides, so the knot is rounder than the (often
/// hexagonal) tube it ends. About a hemisphere.
fn append_root_dome(mesh: &mut MeshData, base: Vec3, axis: Vec3, segments: u32, radius: f32, layout: BarkLayout) {
    let basis = RingBasis::from_axis(axis);
    let segments = segments.max(3);
    let round = segments * 2;
    let latitudes = [78.0f32, 62.0, 44.0, 24.0];
    let ring_at = |mesh: &mut MeshData, latitude: f32, sides: u32| -> u32 {
        let (sin, cos) = latitude.to_radians().sin_cos();
        let first = mesh.positions.len();
        let ring = emit_ring(mesh, base - axis * (radius * sin), basis, sides, 0.0, layout.ring(0.0), |_, _| radius * cos);
        for normal in &mut mesh.normals[first..] {
            *normal = (*normal * cos - axis * sin).normalize_or_zero();
        }
        ring
    };
    let (sin, cos) = latitudes[0].to_radians().sin_cos();
    emit_cap(mesh, base - axis * (radius * sin), basis, round, 0.0, -axis, layout.cap(radius), |_, _| radius * cos);
    let mut back = None;
    for latitude in latitudes {
        let ring = ring_at(mesh, latitude, round);
        if let Some(prev) = back {
            connect_rings(mesh, prev, ring, round);
        }
        back = Some(ring);
    }
    // Join the last round ring to the base ring: each base edge (a0, a1) meets
    // three round vertices (b0, b1, b2), the middle one sitting between them.
    let base_ring = ring_at(mesh, 0.0, segments);
    let back = back.expect("at least one round ring");
    for i in 0..segments {
        let (a0, a1) = (base_ring + i, base_ring + i + 1);
        let (b0, b1, b2) = (back + 2 * i, back + 2 * i + 1, back + 2 * i + 2);
        mesh.indices.extend_from_slice(&[b0, b1, a0, b1, a1, a0, b1, b2, a1]);
    }
}

/// Which wood piece a run of vertices belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WoodPiece {
    /// The trunk: rings, caps and tip cone (see [`trunk_mesh_with`]).
    Trunk,
    /// A branch tube: its rings, then the tip cap (centre, then rim).
    Tube,
    /// A primary's collar: three rings.
    Collar,
    /// A primary's extension into the trunk: the far ring, then the near ring.
    /// `collar` tells the base under the collar from the plain one.
    Root { collar: bool },
    /// A primary's rounded base.
    Dome { collar: bool },
}

/// A run of vertices in a wood mesh and the node that produced it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WoodSpan {
    /// Index of the node in the skeleton.
    pub node: usize,
    /// Which piece the vertices form.
    pub piece: WoodPiece,
    /// Index of the span's first vertex in the wood mesh.
    pub first: u32,
    /// Number of vertices in the span.
    pub count: u32,
    /// Radial segments of the piece (0 for the trunk).
    pub segments: u32,
}

fn record(spans: &mut Option<&mut Vec<WoodSpan>>, span: WoodSpan) {
    if span.count > 0
        && let Some(spans) = spans.as_deref_mut()
    {
        spans.push(span);
    }
}

/// The span of a primary's base wood (`first..end` in the roots mesh).
fn base_span(node_index: usize, node: &Node, collar: bool, first: usize, end: usize, segments: u32) -> WoodSpan {
    let piece = if primary_root(node).is_some() {
        WoodPiece::Root { collar }
    } else {
        WoodPiece::Dome { collar }
    };
    WoodSpan {
        node: node_index,
        piece,
        first: first as u32,
        count: (end - first) as u32,
        segments: segments.max(3),
    }
}

/// Wood mesh for the whole (aged) skeleton: trunk, collars and branch tubes,
/// optionally smoothed.
pub fn skeleton_mesh(skeleton: &Skeleton, params: &TreeParams) -> MeshData {
    skeleton_mesh_with(skeleton, params, trunk_pieces(&params.trunk, &skeleton.nodes[0]), None)
}

/// [`skeleton_mesh`] with the trunk's optional `trunk` pieces given. When
/// `spans` is given it receives, in no particular order, every run of vertices
/// and the node and piece it belongs to.
pub(crate) fn skeleton_mesh_with(
    skeleton: &Skeleton,
    params: &TreeParams,
    trunk: TrunkPieces,
    spans: Option<&mut Vec<WoodSpan>>,
) -> MeshData {
    skeleton_mesh_inner(skeleton, params, trunk, spans, None)
}

/// How coarse a level of detail's wood is: no tube, collar or base has more
/// than `sides` sides (at least 3) or `rings` rings (at least 2), and a steep
/// branch's base is a flat disc. A node that already has fewer keeps them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Coarseness {
    pub(crate) sides: u32,
    pub(crate) rings: u32,
}

/// [`skeleton_mesh`] as a coarser level of detail.
pub(crate) fn skeleton_mesh_coarse(skeleton: &Skeleton, params: &TreeParams, coarse: Coarseness) -> MeshData {
    skeleton_mesh_inner(
        skeleton,
        params,
        trunk_pieces(&params.trunk, &skeleton.nodes[0]),
        None,
        Some(coarse),
    )
}

/// The faces of the trunk of a [`TrunkStyle::Prism`] tree (at least 3), `None` for a round one.
pub(crate) fn prism_sides(params: &TreeParams) -> Option<u32> {
    match params.trunk_style {
        TrunkStyle::Prism { sides } => Some(sides.max(3)),
        TrunkStyle::Round => None,
    }
}

fn skeleton_mesh_inner(
    skeleton: &Skeleton,
    params: &TreeParams,
    trunk: TrunkPieces,
    mut spans: Option<&mut Vec<WoodSpan>>,
    coarse: Option<Coarseness>,
) -> MeshData {
    // A prism trunk (`TrunkStyle::Prism`): the trunk has its sides, branch tubes and collars no
    // more, nothing is smoothed and the result is flat-shaded at the end.
    let prism = prism_sides(params);
    let prismatic;
    let params = match prism {
        Some(sides) => {
            let mut p = params.clone();
            p.trunk.radial_segments = sides;
            p.mesh.smooth_iterations = 0;
            prismatic = p;
            &prismatic
        }
        None => params,
    };
    let b = &params.branches;
    let tile = bark_tile(params);
    let mut mesh = trunk_mesh_with(&params.trunk, &skeleton.nodes[0], params.seed, trunk, tile);
    let recording = spans.is_some();
    record(
        &mut spans,
        WoodSpan { node: 0, piece: WoodPiece::Trunk, first: 0, count: mesh.positions.len() as u32, segments: 0 },
    );
    // Primary bases extended into the trunk. Added after smoothing: the bases
    // are open boundaries then, so smoothing keeps them in place and does not
    // pull the junction toward the trunk axis.
    let mut roots = MeshData::default();
    let mut root_spans: Vec<WoodSpan> = Vec::new();

    for (index, node) in skeleton.nodes.iter().enumerate().skip(1) {
        if node.length <= 1e-4 || node.base_radius <= 0.0 {
            continue;
        }
        let segments = match coarse {
            Some(c) => radial_segments_for(node, params).min(c.sides.max(3)),
            None => radial_segments_for(node, params),
        };
        let segments = prism.map_or(segments, |sides| segments.min(sides));
        if node.kind == NodeKind::Primary {
            let first = roots.positions.len();
            append_primary_base(
                &mut roots,
                node,
                node.start,
                node.base_radius,
                segments,
                coarse.is_some(),
                BarkLayout::new(node, tile),
            );
            if recording {
                root_spans.push(base_span(index, node, false, first, roots.positions.len(), segments));
            }
            if params.mesh.collar_enabled {
                let collar_length = node.base_radius * params.mesh.collar_length;
                // Same sides as the tube it wraps.
                let collar_segments = segments;
                let first = mesh.positions.len();
                append_collar(&mut mesh, node, collar_length, collar_segments, tile);
                record(
                    &mut spans,
                    WoodSpan {
                        node: index,
                        piece: WoodPiece::Collar,
                        first: first as u32,
                        count: (mesh.positions.len() - first) as u32,
                        segments: collar_segments.max(3),
                    },
                );
                let first = roots.positions.len();
                append_primary_base(
                    &mut roots,
                    node,
                    collar_start(node.start, node.direction, collar_length),
                    node.base_radius * COLLAR_FLARE,
                    collar_segments,
                    coarse.is_some(),
                    BarkLayout::new(node, tile),
                );
                if recording {
                    root_spans.push(base_span(index, node, true, first, roots.positions.len(), collar_segments));
                }
            }
        }
        let first = mesh.positions.len();
        append_tube(&mut mesh, node, segments, b.twist, coarse.map_or(u32::MAX, |c| c.rings), tile);
        record(
            &mut spans,
            WoodSpan {
                node: index,
                piece: WoodPiece::Tube,
                first: first as u32,
                count: (mesh.positions.len() - first) as u32,
                segments: segments.max(3),
            },
        );
    }

    if params.mesh.smooth_iterations > 0 {
        // Smoothing softens creases but must not move the wood the skeleton
        // defines: the trunk's bottom cap stays on the ground, branch-end caps
        // keep their length, and a parent keeps its surface where a child is
        // seated on it (the child's base or collar edge is an open boundary and
        // stays put, so moving the surface around it would leave a lip).
        let keep: Vec<(Vec3, f32)> = skeleton
            .nodes
            .iter()
            .enumerate()
            .skip(1)
            .flat_map(|(i, node)| {
                let end = (skeleton.end(i), node.tip_radius * SMOOTH_PIN_REACH);
                let seat = node.parent.map(|p| {
                    let parent = &skeleton.nodes[p as usize];
                    if parent.kind == NodeKind::Trunk {
                        (node.start, node.base_radius * COLLAR_FLARE * SMOOTH_PIN_REACH)
                    } else {
                        let radius = parent.base_radius + (parent.tip_radius - parent.base_radius) * node.attach_t;
                        (node.anchor, (radius + node.base_radius) * SMOOTH_PIN_REACH)
                    }
                });
                std::iter::once(end).chain(seat)
            })
            .collect();
        let pins = PinSpheres::new(&keep);
        laplacian_smooth(
            &mut mesh.positions,
            &mut mesh.normals,
            &mesh.indices,
            params.mesh.smooth_iterations,
            params.mesh.smooth_factor,
            |p| p.y <= GROUND_EPSILON || pins.contains(p),
        );
    }
    let roots_at = mesh.positions.len() as u32;
    mesh.extend(&roots);
    for span in root_spans {
        record(&mut spans, WoodSpan { first: span.first + roots_at, ..span });
    }

    // A prism is flat-shaded, which gives every triangle its own vertices: the growth shader's
    // spans (`spans`) would not hold, but a tree with a prism trunk is never grown on the GPU.
    if prism.is_some() {
        return mesh.flat_shaded();
    }
    mesh
}

/// Spheres bucketed on a coarse grid, so asking whether a point lies in any of
/// them checks only the spheres overlapping its cell (smoothing asks for every
/// vertex, and big trees have hundreds of pins).
struct PinSpheres<'a> {
    spheres: &'a [(Vec3, f32)],
    cells: std::collections::HashMap<[i32; 3], Vec<u32>>,
}

impl<'a> PinSpheres<'a> {
    const CELL: f32 = 0.25;

    fn new(spheres: &'a [(Vec3, f32)]) -> Self {
        let mut cells: std::collections::HashMap<[i32; 3], Vec<u32>> = std::collections::HashMap::new();
        for (i, &(center, radius)) in spheres.iter().enumerate() {
            let lo = Self::cell(center - Vec3::splat(radius));
            let hi = Self::cell(center + Vec3::splat(radius));
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Self { spheres, cells }
    }

    fn cell(p: Vec3) -> [i32; 3] {
        (p / Self::CELL).floor().as_ivec3().to_array()
    }

    fn contains(&self, p: Vec3) -> bool {
        self.cells.get(&Self::cell(p)).is_some_and(|list| {
            list.iter().any(|&i| {
                let (c, r) = self.spheres[i as usize];
                p.distance_squared(c) <= r * r
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::{assert_faces_outward, assert_indices_valid};
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    fn primary_base(node: &Node, at: Vec3, radius: f32, segments: u32) -> MeshData {
        let mut mesh = MeshData::default();
        append_primary_base(&mut mesh, node, at, radius, segments, false, BarkLayout::new(node, Vec2::ONE));
        mesh
    }

    fn root_dome(base: Vec3, axis: Vec3, segments: u32, radius: f32) -> MeshData {
        let mut mesh = MeshData::default();
        append_root_dome(&mut mesh, base, axis, segments, radius, BarkLayout::fully_grown(radius, Vec2::ONE));
        mesh
    }

    #[test]
    fn bark_coordinates_are_metric_whole_tiles_around_and_anchored_to_the_mature_node() {
        let params = TreePreset::Oak.params();
        let tile = BarkScan::Oak.tile_size();
        let full = generate_skeleton(&params);
        let aged = full.aged(3.0, &params.growth);
        let segments = 6usize;
        let mut checked = 0;
        for node in aged.nodes.iter().skip(1).filter(|n| n.kind == NodeKind::Primary && n.progress > 0.05) {
            let mature = &full.nodes[node.id as usize];
            let mesh = tube_mesh(node, segments as u32, params.branches.twist, tile);
            let rings = tube_rings(node, params.branches.twist) as usize;
            let u_tiles = u_tiles_around(mature.base_radius, tile.x);
            for ring in 0..rings {
                let vertices = &mesh.uvs[ring * (segments + 1)..][..segments + 1];
                // Metres of the fully grown branch over the tile's length, whatever the age.
                let v = ring as f32 / (rings - 1) as f32 * mature.length / tile.y;
                assert!(vertices.iter().all(|uv| (uv.y - v).abs() < 1e-3), "node {} ring {ring}", node.id);
                // Whole tiles around, so the seam is invisible.
                assert_eq!((vertices[0].x, vertices[segments].x), (0.0, u_tiles), "node {} ring {ring}", node.id);
            }
            assert!(u_tiles >= 1.0 && u_tiles.fract() == 0.0);
            checked += 1;
        }
        assert!(checked > 5, "only {checked} primaries were aged enough to check");
    }

    #[test]
    fn wood_mesh_faces_outward_for_all_presets() {
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 0;
            let skeleton = generate_skeleton(&params);
            let mesh = skeleton_mesh(&skeleton, &params);
            assert_indices_valid(&mesh, preset.name());
            assert_faces_outward(&mesh, preset.name());
        }
    }

    #[test]
    fn collar_and_bent_tube_face_outward() {
        let params = TreePreset::Willow.params(); // gravity + twist
        let skeleton = generate_skeleton(&params);
        let node = skeleton
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Primary)
            .unwrap();
        assert!(node.bend > 0.0);
        let tube = tube_mesh(node, 6, 45.0, Vec2::ONE);
        assert_indices_valid(&tube, "tube");
        assert_faces_outward(&tube, "tube");
        let collar = collar_mesh(node, 0.3, 5, Vec2::ONE);
        assert_indices_valid(&collar, "collar");
        assert_faces_outward(&collar, "collar");
        let root = primary_base(node, node.start, node.base_radius, 6);
        assert_indices_valid(&root, "root");
        assert_faces_outward(&root, "root");
    }

    #[test]
    fn collar_never_stands_proud_of_its_branch() {
        // The collar's narrow end must lie on or inside the tube it wraps,
        // even on short, fast-tapering branches (baobab, dead tree) and with
        // collars longer than the branch.
        for (label, collar_length) in [("default", None), ("long collar", Some(6.0))] {
            for preset in TreePreset::ALL {
                let mut params = preset.params();
                if let Some(c) = collar_length {
                    params.mesh.collar_length = c;
                }
                let skeleton = generate_skeleton(&params);
                for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary && n.length > 1e-4) {
                    let collar = collar_mesh(node, node.base_radius * params.mesh.collar_length, 6, Vec2::ONE);
                    let end_ring = &collar.positions[collar.positions.len() - 7..];
                    let axis: Vec<Vec3> = node.axis(node.axis_points()).into_iter().map(|(c, _)| c).collect();
                    for &p in end_ring {
                        let (dist, f) = axis
                            .windows(2)
                            .enumerate()
                            .map(|(k, w)| {
                                let ab = w[1] - w[0];
                                let t = ((p - w[0]).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
                                (p.distance(w[0] + ab * t), (k as f32 + t) / (axis.len() - 1) as f32)
                            })
                            .fold((f32::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                        let tube_radius = lerp(node.base_radius, node.tip_radius, f);
                        assert!(
                            dist <= tube_radius + 2e-3,
                            "{label} {}: primary {} collar end {dist} from the axis, tube radius {tube_radius}",
                            preset.name(),
                            node.id
                        );
                    }
                }
            }
        }
    }

    /// Vertices on edges used by exactly one triangle (open boundaries), with
    /// coincident vertices (seams, cap rings) treated as one.
    fn open_boundary_vertices(mesh: &MeshData) -> Vec<Vec3> {
        let key = |i: u32| (mesh.positions[i as usize] * 1.0e4).round().as_ivec3().to_array();
        let mut uses: std::collections::HashMap<([i32; 3], [i32; 3]), u32> = Default::default();
        for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
            for (u, v) in [(a, b), (b, c), (c, a)] {
                let (ku, kv) = (key(u), key(v));
                if ku != kv {
                    *uses.entry((ku.min(kv), ku.max(kv))).or_insert(0) += 1;
                }
            }
        }
        uses.into_iter()
            .filter(|&(_, n)| n == 1)
            .flat_map(|((u, v), _)| [u, v])
            .map(|k| Vec3::new(k[0] as f32, k[1] as f32, k[2] as f32) / 1.0e4)
            .collect()
    }

    #[test]
    fn primary_bases_are_buried_in_the_trunk() {
        // Tube plus root segment is open only at the root; that opening must
        // be inside the trunk, or a shallow junction shows a flat, open end.
        for preset in TreePreset::ALL {
            let params = preset.params();
            let skeleton = generate_skeleton(&params);
            for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary) {
                let mut wood = tube_mesh(node, 4, params.branches.twist, Vec2::ONE);
                wood.extend(&primary_base(node, node.start, node.base_radius, 4));
                assert_faces_outward(&wood, preset.name());
                for p in open_boundary_vertices(&wood) {
                    let depth = crate::skeleton::trunk_depth(&params.trunk, params.seed, p);
                    assert!(depth > 0.0, "{}: primary {} has an open end {depth} m outside the trunk", preset.name(), node.id);
                }
            }
        }
    }

    #[test]
    fn steep_primaries_end_in_a_closed_rounded_base() {
        // D-032: a branch leaving the trunk steeply was closed by a flat,
        // sheared prism toward the trunk axis; its base is now a closed dome
        // that reaches back along the branch.
        let mut steep = 0;
        for preset in TreePreset::ALL {
            let params = preset.params();
            let skeleton = generate_skeleton(&params);
            for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary && n.length > 1e-4) {
                if !leaves_steeply(node) {
                    continue;
                }
                steep += 1;
                let mut wood = tube_mesh(node, 6, params.branches.twist, Vec2::ONE);
                wood.extend(&primary_base(node, node.start, node.base_radius, 6));
                assert_faces_outward(&wood, preset.name());
                assert!(
                    open_boundary_vertices(&wood).is_empty(),
                    "{}: steep primary {} has an open base",
                    preset.name(),
                    node.id
                );
                let back = wood
                    .positions
                    .iter()
                    .map(|p| (node.start - *p).dot(node.direction))
                    .fold(f32::MIN, f32::max);
                assert!(back > 0.8 * node.base_radius, "{}: base reaches back only {back} m", preset.name());
            }
        }
        assert!(steep > 10, "only {steep} steep primaries in all presets");
    }

    #[test]
    fn the_knot_behind_a_hexagonal_branch_is_round() {
        // D-051: the rounded base of a 6-sided branch was itself a 6-sided
        // faceted knot. The ring just behind the base must be close to a
        // circle: neighbouring vertices at most 31 degrees apart (a hexagon's
        // are 60 apart, an octagon's 45).
        let dome = root_dome(Vec3::ZERO, Vec3::Y, 6, 1.0);
        let (sin, cos) = 24.0f32.to_radians().sin_cos();
        let mut angles: Vec<f32> = dome
            .positions
            .iter()
            .filter(|p| (p.y + sin).abs() < 1e-4 && (p.x.hypot(p.z) - cos).abs() < 1e-4)
            .map(|p| p.z.atan2(p.x))
            .collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-3); // the seam vertex is duplicated
        let widest_gap = angles.windows(2).map(|w| w[1] - w[0]).fold(0.0f32, f32::max);
        assert!(
            angles.len() >= 12 && widest_gap.to_degrees() < 31.0,
            "{} distinct vertices, widest gap {} degrees",
            angles.len(),
            widest_gap.to_degrees()
        );
    }

    #[test]
    fn smoothing_keeps_primary_junctions_closed() {
        // Each primary's root segment must meet its (smoothed) tube and collar:
        // every vertex of the root segment's outer ring appears again in the
        // wood mesh, on the tube's or collar's base ring.
        let mut params = TreePreset::Oak.params();
        params.mesh.smooth_iterations = 2;
        let skeleton = generate_skeleton(&params);
        let mesh = skeleton_mesh(&skeleton, &params);
        let copies = |p: Vec3| mesh.positions.iter().filter(|q| q.distance(p) < 1e-5).count();
        for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary && n.length > 1e-4) {
            let segments = radial_segments_for(node, &params);
            let root = primary_base(node, node.start, node.base_radius, segments);
            let outer_ring = &root.positions[root.positions.len() - (segments as usize + 1)..];
            for &p in outer_ring {
                assert!(copies(p) >= 2, "primary {}: junction vertex {p:?} has no matching tube vertex", node.id);
            }
        }
    }

    #[test]
    fn gravity_bent_wood_ends_where_the_skeleton_says() {
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 0.8;
        params.branches.stiffness = 0.1;
        // Mid-growth (parents still lengthening) and mature.
        for age in [6.0, f32::INFINITY] {
            let out = crate::pipeline::build_tree(&params, age);
            let skeleton = &out.skeleton;

            let mut visibly_bent = 0;
            for (i, node) in skeleton.nodes.iter().enumerate().skip(1) {
                if node.length <= 1e-4 || node.base_radius <= 0.0 {
                    continue; // no wood emitted
                }
                let end = skeleton.end(i);
                if end.distance(node.start + node.direction * node.length) > node.base_radius {
                    visibly_bent += 1;
                }
                // Child branches and leaves sit on `end`; the bent wood must reach it.
                let nearest = out
                    .trunk
                    .positions
                    .iter()
                    .map(|p| p.distance(end))
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest <= node.tip_radius + 1e-3,
                    "age {age}, node {i}: wood tip is {nearest} from where its leaves and children sit"
                );
                // Every child starts on this node's wood axis.
                for c in skeleton.children(i) {
                    let axis = node.axis(node.axis_points());
                    let on_axis = axis
                        .windows(2)
                        .map(|w| segment_distance(skeleton.nodes[c].start, w[0].0, w[1].0))
                        .fold(f32::MAX, f32::min);
                    assert!(on_axis < 1e-4, "age {age}: child {c} of {i} is {on_axis} off the bent axis");
                }
            }
            assert!(visibly_bent > 0, "age {age}: the gravity toggle should bend some branch");
        }
    }

    #[test]
    fn drooping_willow_twigs_start_on_their_parents_wood() {
        // D-030: willow twigs leave along the parent's bent tangent, and the
        // forked willow has halves hanging from stubs; every child must still
        // start on its parent's bent axis, so nothing floats beside the wood.
        let willow = TreePreset::Willow.params();
        let mut forked = willow.clone();
        forked.split.enabled = true;
        forked.split.probability = 1.0;
        forked.split.radius_threshold = 0.0;
        for params in [willow, forked] {
            let skeleton = crate::pipeline::build_tree(&params, f32::INFINITY).skeleton;
            let mut checked = 0;
            for (i, node) in skeleton.nodes.iter().enumerate().skip(1) {
                let axis = node.axis(node.axis_points());
                for c in skeleton.children(i) {
                    let on_axis = axis
                        .windows(2)
                        .map(|w| segment_distance(skeleton.nodes[c].start, w[0].0, w[1].0))
                        .fold(f32::MAX, f32::min);
                    assert!(on_axis < 1e-4, "child {c} of {i} is {on_axis} off the bent axis");
                    checked += 1;
                }
            }
            assert!(checked > 100, "only {checked} parent-child pairs");
        }
    }

    fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
        p.distance(a + ab * t)
    }

    #[test]
    fn smoothed_wood_stays_attached_and_seamless() {
        let mut params = TreePreset::Oak.params();
        params.mesh.smooth_iterations = 0;
        let skeleton = generate_skeleton(&params);
        let raw = skeleton_mesh(&skeleton, &params);
        params.mesh.smooth_iterations = 2;
        let smooth = skeleton_mesh(&skeleton, &params);
        assert_eq!(raw.positions.len(), smooth.positions.len());

        // Vertices that coincide (UV seams, cap rings) must still coincide.
        let cell = |p: Vec3| (p * 1.0e5).round().as_ivec3();
        let mut groups: std::collections::HashMap<_, Vec<usize>> = Default::default();
        for (i, p) in raw.positions.iter().enumerate() {
            groups.entry(cell(*p)).or_default().push(i);
        }
        for group in groups.values().filter(|g| g.len() > 1) {
            let first = smooth.positions[group[0]];
            for &i in group {
                let gap = (smooth.positions[i] - first).length();
                assert!(gap < 1e-3, "vertices {} and {i} torn apart by {gap}", group[0]);
            }
        }

        // The trunk base stays on the ground instead of being pulled up.
        for (a, b) in raw.positions.iter().zip(&smooth.positions) {
            if a.y <= 1e-4 {
                assert!(b.y <= 1e-3, "ground vertex {a:?} lifted to {b:?}");
            }
        }
        assert_indices_valid(&smooth, "smoothed oak");
    }

    #[test]
    fn smoothing_keeps_branch_ends_and_attachments() {
        // D-040: plain Laplacian smoothing shrank the wood: branch ends
        // retracted (willow 0.18 m at 2 passes) and twigs were left hanging
        // off their thinned parents.
        for preset in [TreePreset::Willow, TreePreset::Redwood, TreePreset::Oak, TreePreset::Pine] {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 2;
            let skeleton = generate_skeleton(&params);
            let mesh = skeleton_mesh(&skeleton, &params);
            let near = |q: Vec3| mesh.positions.iter().map(|w| w.distance(q)).fold(f32::MAX, f32::min);
            for (i, n) in skeleton.nodes.iter().enumerate().skip(1) {
                let gap = near(skeleton.end(i)) - n.tip_radius;
                assert!(gap < 0.02, "{} {:?} {}: end retracted {gap} m", preset.name(), n.kind, n.id);
                let Some(p) = n.parent else { continue };
                let parent = &skeleton.nodes[p as usize];
                if parent.kind == NodeKind::Trunk {
                    continue;
                }
                let radius = parent.base_radius + (parent.tip_radius - parent.base_radius) * n.attach_t;
                let gap = near(n.anchor) - radius;
                assert!(gap < 0.01, "{} {:?} {}: anchor {gap} m off its parent's wood", preset.name(), n.kind, n.id);
            }
        }
    }

    #[test]
    fn smoothing_leaves_trunk_junctions_in_place() {
        // D-041: the trunk surface around a primary's collar moved up to
        // 1.9 cm while the collar's open edge stayed pinned, leaving a lip.
        for preset in [TreePreset::Oak, TreePreset::Redwood, TreePreset::Maple] {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 0;
            let skeleton = generate_skeleton(&params);
            let raw = skeleton_mesh(&skeleton, &params);
            params.mesh.smooth_iterations = 2;
            let smooth = skeleton_mesh(&skeleton, &params);
            for n in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary) {
                for (a, b) in raw.positions.iter().zip(&smooth.positions) {
                    if a.distance(n.start) < n.base_radius * COLLAR_FLARE {
                        let moved = a.distance(*b);
                        assert!(moved < 2e-3, "{} primary {}: junction vertex moved {moved} m", preset.name(), n.id);
                    }
                }
            }
        }
    }

    #[test]
    fn smoothing_never_folds_thin_twigs() {
        // D-042: welding by a fixed 0.1 mm merged distinct ring vertices of
        // sub-millimetre twigs, so smoothing folded them inside out (willow
        // 132 triangles). Seams must still weld at large coordinates.
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 2;
            let mesh = skeleton_mesh(&generate_skeleton(&params), &params);
            assert_faces_outward(&mesh, preset.name());
        }
        let mut giant = TreePreset::Oak.params();
        giant.trunk.height = 40.0;
        giant.trunk.radius = 3.0;
        giant.mesh.smooth_iterations = 2;
        let mesh = skeleton_mesh(&generate_skeleton(&giant), &giant);
        assert_faces_outward(&mesh, "giant oak");
        let cell = |p: Vec3| (p * 1.0e3).round().as_ivec3();
        let mut groups: std::collections::HashMap<_, Vec<usize>> = Default::default();
        for (i, p) in mesh.positions.iter().enumerate() {
            groups.entry(cell(*p)).or_default().push(i);
        }
        for group in groups.values() {
            for &i in group {
                let gap = mesh.positions[i].distance(mesh.positions[group[0]]);
                assert!(gap < 1e-3, "giant oak: seam torn by {gap} m");
            }
        }
    }

    #[test]
    fn smoothing_keeps_normals_where_nothing_moved() {
        // D-043: normals were recomputed from faces over the whole mesh, so
        // unmoved collar ends got face-tilted normals and lit up as a rim
        // against their branch (dragon tree closeup).
        for preset in [TreePreset::DragonTree, TreePreset::Oak] {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 0;
            let skeleton = generate_skeleton(&params);
            let raw = skeleton_mesh(&skeleton, &params);
            params.mesh.smooth_iterations = 2;
            let smooth = skeleton_mesh(&skeleton, &params);
            let moved: Vec<bool> = raw.positions.iter().zip(&smooth.positions).map(|(a, b)| a != b).collect();
            let mut touched = vec![false; raw.positions.len()];
            for &[a, b, c] in raw.indices.as_chunks::<3>().0 {
                if moved[a as usize] || moved[b as usize] || moved[c as usize] {
                    for v in [a, b, c] {
                        touched[v as usize] = true;
                    }
                }
            }
            let cell = |p: Vec3| (p * 1.0e5).round().as_ivec3();
            let mut twins: std::collections::HashMap<_, Vec<usize>> = Default::default();
            for (i, p) in raw.positions.iter().enumerate() {
                twins.entry(cell(*p)).or_default().push(i);
            }
            let agree = |a: usize, b: usize| raw.normals[a].dot(raw.normals[b]) > 0.5;
            // A vertex keeps its generated normal unless it, or a twin it
            // shades with, borders a moved face.
            let mut kept = 0;
            for (v, p) in raw.positions.iter().enumerate() {
                if twins[&cell(*p)].iter().any(|&u| touched[u] && agree(u, v)) {
                    continue;
                }
                assert_eq!(raw.normals[v], smooth.normals[v], "{} vertex {v}: normal changed though nothing moved", preset.name());
                kept += 1;
            }
            assert!(kept > 0, "{}: smoothing moved every vertex", preset.name());
            // UV seam twins next to moved wood must shade alike. (Root segments
            // are added after smoothing, inside the trunk, and never border a
            // moved face.)
            for group in twins.values() {
                for &a in group {
                    for &b in group.iter().filter(|&&b| touched[a] && touched[b] && raw.normals[a].dot(raw.normals[b]) > 0.99) {
                        let gap = smooth.normals[a].distance(smooth.normals[b]);
                        assert!(gap < 1e-3, "{} vertices {a} and {b}: seam normals differ by {gap}", preset.name());
                    }
                }
            }
        }
    }

    #[test]
    fn pipe_radius_combines_children() {
        assert_eq!(calculate_pipe_radius(&[], 2.0, 0.1), 0.1);
        let r = calculate_pipe_radius(&[0.3, 0.4], 2.0, 0.01);
        assert!((r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_pipe_radius_keeps_tip_below_base_and_above_min() {
        let mut params = TreePreset::Oak.params();
        params.mesh.pipe_radius_enabled = true;
        let mut skeleton = generate_skeleton(&params);
        let trunk_before = skeleton.nodes[0].clone();
        apply_pipe_radius(&mut skeleton, 2.0, 0.02);
        assert_eq!(skeleton.nodes[0], trunk_before);
        for (i, n) in skeleton.nodes.iter().enumerate().skip(1) {
            assert!(n.tip_radius <= n.base_radius, "node {i}");
            assert!(n.base_radius >= 0.02, "node {i}");
            assert!(n.tip_radius >= 0.01, "node {i}");
        }
    }

    #[test]
    fn growing_trees_with_pipe_radii_face_outward() {
        // A just-born branch (1 mm long) was floored to the full 2 cm pipe
        // radius, a paper-thin disc whose faces folded inside out (oak seed 3
        // at age 9, willow seed 2 at age 5).
        for (preset, seed, age) in [(TreePreset::Oak, 3, 9.0), (TreePreset::Willow, 2, 5.0)] {
            let mut params = preset.params();
            params.seed = seed;
            params.mesh.pipe_radius_enabled = true;
            let out = crate::pipeline::build_tree(&params, age);
            crate::mesh::rings::tests::assert_faces_outward(&out.trunk, preset.name());
        }
    }
}
