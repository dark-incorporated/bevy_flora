//! Tree skeleton: an explicit node tree (trunk, primaries, splits, sub-branches)
//! generated deterministically from [`TreeParams`].

use bevy::math::Vec3;
use std::f32::consts::{E, PI, TAU};

use crate::foliage::leaf_bound;
use crate::limits::{MAX_LEAVES, MAX_NODES};
use crate::mesh::rings::perpendicular;
use crate::params::{BranchParams, TreeParams, TrunkParams, TrunkTermination};
use crate::rng::SeededRng;

/// Role of a node in the skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Index 0, the only node without a parent.
    Trunk,
    /// Thin continuation of the trunk above its top.
    Leader,
    /// Branch attached to the trunk.
    Primary,
    /// One of the two halves of a Y-split primary (parent is the stub primary).
    Split,
    /// Recursive sub-branch.
    Sub,
    /// A pendant strand hung from a branch ([`StrandParams`](crate::StrandParams)): one level below
    /// its branch, grown like a sub-branch, straight (no gravity bend), its leaves spread along its
    /// whole length.
    Strand,
}

/// One straight segment of the skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Index of this node in the full (unaged) skeleton; stable across [`Skeleton::aged`].
    pub id: u32,
    /// Role of this node in the tree.
    pub kind: NodeKind,
    /// Parent index; `None` only for the trunk.
    pub parent: Option<u32>,
    /// Fraction along the parent's full length where this node attaches.
    pub attach_t: f32,
    /// Point on the parent's axis this node is attached to. Equals `start`
    /// except for trunk-attached nodes, which start on the trunk surface.
    pub anchor: Vec3,
    /// Start of the segment in tree-local space, in metres.
    pub start: Vec3,
    /// Unit direction the segment points in, before gravity bend.
    pub direction: Vec3,
    /// Segment length in metres: [`Node::full_length`] times `progress`.
    pub length: f32,
    /// Length of the segment once grown, in metres: what `length` is in the skeleton
    /// [`Skeleton::aged`] ages, and stays through every age (`length / progress` is this only to a
    /// rounding error, which a leaf count that lands on a whole number cannot take).
    pub full_length: f32,
    /// Downward bend of the wood toward the tip (`gravity_strength × (1 −
    /// stiffness)`; 0 = straight, always 0 for the trunk). See [`Node::axis`].
    pub bend: f32,
    /// Radius at the start of the segment, in metres.
    pub base_radius: f32,
    /// Radius at the tip of the segment, in metres.
    pub tip_radius: f32,
    /// Recursion depth (0 for trunk, primaries, leader and splits).
    pub depth: u8,
    /// Position within the crown (0 at the branch zone start, 1 at its end).
    pub height_ratio: f32,
    /// Age at which the node starts growing (set by [`crate::growth::assign_birth_times`]).
    pub birth: f32,
    /// Time from `birth` until full length.
    pub duration: f32,
    /// Length growth fraction, 1.0 after generation; set by [`Skeleton::aged`].
    pub progress: f32,
    /// Radius growth fraction, 1.0 after generation; set by [`Skeleton::aged`].
    pub radius_progress: f32,
    /// Number of direct children in the full-grown tree. [`Skeleton::aged`]
    /// keeps it, so terminal-branch decisions (leaf placement) do not change
    /// with age; [`Skeleton::children`] lists the children present now.
    pub child_count: u32,
}

/// Nodes in pre-order: every parent index is smaller than its children's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    /// All nodes; index 0 is the trunk.
    pub nodes: Vec<Node>,
}

impl Skeleton {
    /// True when node `i` has no children in the full-grown tree.
    pub fn is_terminal(&self, i: usize) -> bool {
        self.nodes[i].child_count == 0
    }

    /// End point of node `i` (the tip of its possibly bent wood).
    pub fn end(&self, i: usize) -> Vec3 {
        self.nodes[i].point_at(1.0)
    }

    /// Move every node hanging off a branch onto its parent's (bent) axis at
    /// `attach_t`, parents first so displacements cascade to descendants.
    /// [`Skeleton::aged`] places nodes the same way. With floor avoidance,
    /// each node's bent wood is then kept above `floor` (`floor_level`, raised by the pipe model's
    /// thinnest wood when that is on, which thickens a branch's end past the radius it was
    /// generated with), and a sub-branch's wood is kept out of the trunk.
    fn seat_on_parents(&mut self, branches: &BranchParams, floor: f32, trunk: &TrunkParams, seed: i32) {
        for i in 1..self.nodes.len() {
            let Some(p) = self.nodes[i].parent else { continue };
            let parent = &self.nodes[p as usize];
            if parent.kind != NodeKind::Trunk {
                let shift = parent.point_at(self.nodes[i].attach_t) - self.nodes[i].anchor;
                let node = &mut self.nodes[i];
                node.anchor += shift;
                node.start += shift;
            }
            if branches.floor_avoidance {
                self.nodes[i].keep_above(floor);
            }
            if self.nodes[i].kind == NodeKind::Sub {
                self.nodes[i].keep_out_of_trunk(trunk, seed);
            }
        }
    }

    /// Recompute `child_count` for every node.
    pub fn recount_children(&mut self) {
        for n in &mut self.nodes {
            n.child_count = 0;
        }
        for i in 0..self.nodes.len() {
            if let Some(p) = self.nodes[i].parent {
                self.nodes[p as usize].child_count += 1;
            }
        }
    }

    /// Indices of the direct children of node `i`.
    pub fn children(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let parent = i as u32;
        self.nodes
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.parent == Some(parent))
            .map(|(j, _)| j)
    }
}

/// Fewest axis points (mesh rings) of a bent or twisted branch.
pub const BENT_AXIS_POINTS: u32 = 6;

/// Most axis points a bent branch gets, however far it droops.
pub const MAX_AXIS_POINTS: u32 = 16;

/// Largest change of direction, in radians, between two neighbouring axis
/// points of a bent branch; keeps a drooping curve from showing facets.
const MAX_AXIS_TURN: f32 = 0.08;

impl Node {
    /// `points` samples of the wood's axis, base to tip: `(center, direction)`.
    ///
    /// The direction at fraction `t` is `direction` with `t² · bend · h` taken
    /// off its vertical component, where `h` is the direction's horizontal
    /// length: gravity's lever arm, so a vertical branch does not bend and a
    /// near-vertical one cannot fold back on itself. The centers integrate
    /// those directions in equal steps of `length / (points − 1)`.
    pub fn axis(&self, points: u32) -> Vec<(Vec3, Vec3)> {
        let points = points.max(2);
        let step = self.length / (points - 1) as f32;
        let mut center = self.start;
        (0..points)
            .map(|k| {
                let t = k as f32 / (points - 1) as f32;
                let dir = self.bent_direction(t);
                let sample = (center, dir);
                center += dir * step;
                sample
            })
            .collect()
    }

    /// How many axis points (mesh rings) this branch needs: `BENT_AXIS_POINTS`,
    /// or more when gravity turns its direction by more than `MAX_AXIS_TURN`
    /// between two points, up to `MAX_AXIS_POINTS`. Wood, children, leaves and
    /// the floor test all sample the axis with this count, so they agree.
    pub fn axis_points(&self) -> u32 {
        if self.bend <= 0.0 {
            return BENT_AXIS_POINTS;
        }
        // The direction turns fastest at the tip (d/dt of t² · bend · lever), so
        // the last segment decides. Start from the linear estimate, then check.
        let lever = Vec3::new(self.direction.x, 0.0, self.direction.z).length();
        let estimate = (2.0 * self.bend * lever / MAX_AXIS_TURN).ceil() as u32 + 1;
        let mut points = estimate.clamp(BENT_AXIS_POINTS, MAX_AXIS_POINTS);
        let tip = self.bent_direction(1.0);
        while points < MAX_AXIS_POINTS {
            let before = self.bent_direction(1.0 - 1.0 / (points - 1) as f32);
            if tip.dot(before).clamp(-1.0, 1.0).acos() <= MAX_AXIS_TURN {
                break;
            }
            points += 1;
        }
        points
    }

    /// Point at fraction `f` (clamped to 0–1) of the way along the wood's axis.
    pub fn point_at(&self, f: f32) -> Vec3 {
        let f = f.clamp(0.0, 1.0);
        if self.bend <= 0.0 {
            return self.start + self.direction * (self.length * f);
        }
        let axis = self.axis(self.axis_points());
        let x = f * (axis.len() - 1) as f32;
        let k = (x as usize).min(axis.len() - 2);
        axis[k].0.lerp(axis[k + 1].0, x - k as f32)
    }

    fn bent_direction(&self, t: f32) -> Vec3 {
        if self.bend <= 0.0 {
            return self.direction;
        }
        let d = self.direction;
        let lever = Vec3::new(d.x, 0.0, d.z).length();
        Vec3::new(d.x, d.y - t * t * self.bend * lever, d.z).normalize_or_zero()
    }

    /// Lowest point of the wood's surface (axis height minus the radius there).
    fn lowest(&self) -> f32 {
        let axis = self.axis(self.axis_points());
        let last = (axis.len() - 1) as f32;
        axis.iter()
            .enumerate()
            .map(|(k, (c, _))| c.y - lerp(self.base_radius, self.tip_radius, k as f32 / last))
            .fold(f32::MAX, f32::min)
    }

    /// Keep the wood at or above `floor`: relax the gravity bend just enough
    /// (the lowest point only drops as the bend grows), and if the straight
    /// branch still points below the floor, shorten it to end on the floor.
    /// A branch whose base is already below the floor only has to stay above
    /// its base, so it keeps the droop it can.
    fn keep_above(&mut self, floor: f32) {
        let floor = if self.start.y < floor { floor.min(self.start.y - self.base_radius) } else { floor };
        if self.lowest() >= floor {
            return;
        }
        let full = self.bend;
        self.bend = 0.0;
        if self.lowest() >= floor {
            let (mut lo, mut hi) = (0.0, full);
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                self.bend = mid;
                if self.lowest() >= floor {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            self.bend = lo;
        } else if self.direction.y < 0.0 {
            let drop = self.start.y - floor - self.tip_radius;
            self.length = (drop / -self.direction.y).max(0.0);
        }
    }

    /// Keep the wood out of the trunk: a sub-branch is checked against the
    /// trunk when it is generated, as a straight chord, but seating it on a
    /// bent parent and its own gravity bend can move it inside. Relax the
    /// bend just enough, and if the straight branch is still inside, shorten
    /// it until it clears.
    fn keep_out_of_trunk(&mut self, trunk: &TrunkParams, seed: i32) {
        let inside = |n: &Node| {
            (1..=10).any(|k| trunk_depth(trunk, seed, n.point_at(k as f32 / 10.0)) > 0.0)
        };
        if !inside(self) {
            return;
        }
        let full = self.bend;
        self.bend = 0.0;
        if !inside(self) {
            let (mut lo, mut hi) = (0.0, full);
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                self.bend = mid;
                if inside(self) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            self.bend = lo;
            return;
        }
        let (mut lo, mut hi) = (0.0, self.length);
        for _ in 0..16 {
            let mid = 0.5 * (lo + hi);
            self.length = mid;
            if inside(self) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        self.length = lo;
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `(base, tip)` radii of a new branch under `BranchParams::min_twig_radius`:
/// at least `floor` at the base and `floor / 2` at the tip, so twigs still
/// taper instead of ending in flat discs, and never thicker than `parent`, the
/// radius of the wood they grow from.
fn floored_radii(base: f32, tip: f32, floor: f32, parent: f32) -> (f32, f32) {
    let base = base.max(floor).min(parent);
    (base, tip.max(0.5 * floor).min(base))
}

fn random_vec(rng: &mut SeededRng) -> Vec3 {
    Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
    )
    .normalize_or_zero()
}

/// Horizontal offset of the trunk axis at normalized height `t`.
///
/// Shared by the trunk mesher and branch placement so branches sit on the
/// wobbling trunk surface.
pub fn trunk_wobble(trunk: &TrunkParams, seed: i32, t: f32) -> (f32, f32) {
    if trunk.randomness <= 0.0 {
        return (0.0, 0.0);
    }
    let seed_f = seed as f32;
    let scale = trunk.randomness * t * trunk.height * 0.3;
    (
        (t * PI + seed_f * 0.1).sin() * scale,
        (t * E + seed_f * 0.2).cos() * scale,
    )
}

/// Trunk radius at normalized height `t` (taper only, no root flare).
pub fn trunk_radius_at(trunk: &TrunkParams, t: f32) -> f32 {
    let taper_factor = t.powf(trunk.taper_curve);
    lerp(trunk.radius * trunk.flare, trunk.radius * trunk.taper, taper_factor)
}

/// How far `point` lies inside the trunk's tapered, wobbling cylinder
/// (positive = inside; root flares are ignored). Points above the trunk top
/// or below the ground are never inside.
pub fn trunk_depth(trunk: &TrunkParams, seed: i32, point: Vec3) -> f32 {
    let t = point.y / trunk.height;
    if !(0.0..=1.0).contains(&t) {
        return f32::NEG_INFINITY;
    }
    let (wx, wz) = trunk_wobble(trunk, seed, t);
    trunk_radius_at(trunk, t) - Vec3::new(point.x - wx, 0.0, point.z - wz).length()
}

/// Generate the full (mature) skeleton for `params`, including birth times.
///
/// `params` is [`TreeParams::sanitized`] first, and the skeleton stops at [`MAX_NODES`] nodes and at
/// the leaves of [`MAX_LEAVES`] (the last nodes are dropped to fit), so any parameters give a
/// tree in bounded time and memory.
pub fn generate_skeleton(params: &TreeParams) -> Skeleton {
    grow_skeleton(&params.sanitized())
}

/// [`generate_skeleton`] for parameters that are already [`TreeParams::sanitized`].
pub(crate) fn grow_skeleton(params: &TreeParams) -> Skeleton {
    let mut rng = SeededRng::new(params.seed);
    let trunk = &params.trunk;

    let mut skeleton = Skeleton {
        nodes: vec![Node {
            id: 0,
            kind: NodeKind::Trunk,
            parent: None,
            attach_t: 0.0,
            anchor: Vec3::ZERO,
            start: Vec3::ZERO,
            direction: Vec3::Y,
            length: trunk.height,
            full_length: trunk.height,
            bend: 0.0,
            base_radius: trunk.radius * trunk.flare,
            tip_radius: trunk.radius * trunk.taper,
            depth: 0,
            height_ratio: 0.0,
            birth: 0.0,
            duration: 0.0,
            progress: 1.0,
            radius_progress: 1.0,
            child_count: 0,
        }],
    };

    // Draw order matches the original generator: all primaries first, then the
    // leader (and its sub-branches), then per-primary splits and sub-branches.
    let primaries = generate_primaries(params, &mut rng);

    if trunk.termination == TrunkTermination::LeaderBranch {
        let top_radius = trunk.radius * trunk.taper;
        let (wx, wz) = trunk_wobble(trunk, params.seed, 1.0);
        let start = Vec3::new(wx, trunk.height, wz);
        let leader_idx = push_node(
            &mut skeleton,
            Node {
                kind: NodeKind::Leader,
                parent: Some(0),
                attach_t: 1.0,
                anchor: start,
                start,
                direction: Vec3::Y,
                length: trunk.height * trunk.leader_length,
                base_radius: top_radius,
                tip_radius: top_radius * trunk.leader_taper,
                depth: 0,
                height_ratio: 1.0,
                ..blank_node()
            },
        );
        if trunk.leader_has_branches {
            generate_subs(&mut skeleton, leader_idx, params, &mut rng);
        }
    }

    for primary in primaries {
        // A primary, and the two halves of its split: all or none, so the cap holds.
        if skeleton.nodes.len() + 3 > MAX_NODES {
            break;
        }
        let idx = push_node(&mut skeleton, primary);
        if let Some((child_a, child_b)) = try_split(&mut skeleton, idx, params, &mut rng) {
            let a = push_node(&mut skeleton, child_a);
            generate_subs(&mut skeleton, a, params, &mut rng);
            let b = push_node(&mut skeleton, child_b);
            generate_subs(&mut skeleton, b, params, &mut rng);
        } else {
            generate_subs(&mut skeleton, idx, params, &mut rng);
        }
    }

    skeleton.recount_children();
    let b = &params.branches;
    if b.gravity_strength > 0.0 {
        let bend = (b.gravity_strength * (1.0 - b.stiffness)).max(0.0);
        for node in skeleton.nodes.iter_mut().skip(1) {
            node.bend = bend;
        }
    }
    let floor = params.branches.floor_level + if params.mesh.pipe_radius_enabled { params.mesh.pipe_radius_min } else { 0.0 };
    skeleton.seat_on_parents(&params.branches, floor, &params.trunk, params.seed);
    hang_strands(&mut skeleton, params);
    fit_leaves(&mut skeleton, &params.foliage);
    for node in &mut skeleton.nodes {
        node.full_length = node.length;
    }
    crate::growth::assign_birth_times(&mut skeleton, &params.growth);
    skeleton
}

/// Separate random stream for strands, so a tree without them draws nothing and one with them keeps
/// every other branch where it was.
const STRAND_SALT: i32 = 0x57A4_D5E7;

/// The most strands one branch hangs, whatever its length and the density: a branch that would hang more is
/// at its limit long before the tree is at [`MAX_NODES`].
const MAX_STRANDS_PER_BRANCH: u32 = 256;

/// The shortest a strand is, as a share of the longest it could be there.
const STRAND_SHORTEST: f32 = 0.5;

/// The thinnest a strand's base is, in metres, whatever `min_twig_radius` says: a strand of no
/// thickness would be a hairline of degenerate triangles.
const STRAND_THINNEST: f32 = 0.001;

/// Hang [`StrandParams`](crate::StrandParams) strands from the outer part of every branch of the seated
/// `skeleton`: each is a [`NodeKind::Strand`] one level below its branch, appended after every other node
/// (a parent still comes before its children), born when its branch reaches the point it hangs from. A
/// strand is a straight rope (no gravity bend, and its twist is not drawn: two rings of wood). The
/// leader, which only continues the trunk, carries none. Each branch draws from a stream of its own
/// (keyed by its id), so the strands of one branch never move another's.
fn hang_strands(skeleton: &mut Skeleton, params: &TreeParams) {
    let s = &params.branches.strands;
    if s.density <= 0.0 || s.length <= 0.0 {
        return;
    }
    let b = &params.branches;
    let start = s.start.clamp(0.0, 1.0);
    let spread = s.spread.clamp(0.0, 90.0).to_radians();
    let longest = s.length * params.trunk.height;
    let floor = b.floor_level + s.clearance;
    let branches = skeleton.nodes.len();
    for i in 1..branches {
        if skeleton.nodes.len() >= MAX_NODES {
            break;
        }
        let parent = skeleton.nodes[i].clone();
        if parent.kind == NodeKind::Leader || parent.length <= 0.0 {
            continue;
        }
        let mut rng = SeededRng::from_parts(params.seed ^ STRAND_SALT, parent.id);
        let expected = s.density * parent.length * (1.0 - start);
        let count = (expected.floor() as u32 + u32::from(rng.next_f32() < expected.fract())).min(MAX_STRANDS_PER_BRANCH);
        for _ in 0..count {
            if skeleton.nodes.len() >= MAX_NODES {
                break;
            }
            let t = rng.range(start, 1.0);
            let lean = spread * rng.next_f32().sqrt();
            let around = rng.range(0.0, TAU);
            let direction = Vec3::new(lean.sin() * around.cos(), -lean.cos(), lean.sin() * around.sin());
            let at = parent.point_at(t);
            // As far as it can hang: all of `longest`, or down to the floor, whichever is less.
            let reach = if b.floor_avoidance { longest.min((at.y - floor).max(0.0) / -direction.y) } else { longest };
            let length = reach * rng.range(STRAND_SHORTEST, 1.0);
            let parent_radius = lerp(parent.base_radius, parent.tip_radius, t);
            let (base_radius, tip_radius) = floored_radii(0.0, 0.0, b.min_twig_radius.max(STRAND_THINNEST), parent_radius);
            let mut strand = Node {
                kind: NodeKind::Strand,
                parent: Some(parent.id),
                attach_t: t,
                anchor: at,
                start: at,
                direction,
                length,
                bend: 0.0,
                base_radius,
                tip_radius,
                depth: parent.depth.saturating_add(1),
                height_ratio: parent.height_ratio,
                ..blank_node()
            };
            if b.floor_avoidance {
                strand.keep_above(floor);
            }
            strand.keep_out_of_trunk(&params.trunk, params.seed);
            if strand.length > 0.05 {
                push_node(skeleton, strand);
            }
        }
    }
    if skeleton.nodes.len() > branches {
        skeleton.recount_children();
    }
}

/// Drop the last nodes of `skeleton` until the leaves they could bear fit [`MAX_LEAVES`]. A node is
/// counted as if it bore leaves whether or not it is terminal (a node that loses its children becomes
/// terminal), so the bound holds for the skeleton that is left. Nodes are in generation order, a parent
/// before its children, so a cut leaves a tree.
fn fit_leaves(skeleton: &mut Skeleton, foliage: &crate::params::FoliageParams) {
    if !foliage.enabled {
        return;
    }
    let mut total = 0usize;
    let mut keep = skeleton.nodes.len();
    for (i, node) in skeleton.nodes.iter().enumerate() {
        total = total.saturating_add(leaf_bound(foliage, node.kind, node.length, node.height_ratio));
        if total > MAX_LEAVES {
            keep = i.max(1);
            break;
        }
    }
    if keep < skeleton.nodes.len() {
        skeleton.nodes.truncate(keep);
        skeleton.recount_children();
    }
}

/// Placeholder values for fields filled in later (`..blank_node()`).
fn blank_node() -> Node {
    Node {
        id: 0,
        kind: NodeKind::Sub,
        parent: None,
        attach_t: 0.0,
        anchor: Vec3::ZERO,
        start: Vec3::ZERO,
        direction: Vec3::Y,
        length: 0.0,
        full_length: 0.0,
        bend: 0.0,
        base_radius: 0.0,
        tip_radius: 0.0,
        depth: 0,
        height_ratio: 0.0,
        birth: 0.0,
        duration: 0.0,
        progress: 1.0,
        radius_progress: 1.0,
        child_count: 0,
    }
}

fn push_node(skeleton: &mut Skeleton, mut node: Node) -> usize {
    node.id = skeleton.nodes.len() as u32;
    skeleton.nodes.push(node);
    skeleton.nodes.len() - 1
}

/// Primary branches along the trunk on a phyllotaxis spiral.
fn generate_primaries(params: &TreeParams, rng: &mut SeededRng) -> Vec<Node> {
    let trunk = &params.trunk;
    let b = &params.branches;
    let mut out = Vec::new();
    let mut current_angle = 0.0f32;

    let start_height = trunk.height * b.start;
    let end_height = trunk.height * b.end;
    let zone_length = end_height - start_height;
    if zone_length <= 0.0 {
        return out;
    }

    let spacing = 1.0 / (b.density + 0.001);
    let branch_count = ((zone_length / spacing).floor() as i32).min(MAX_NODES as i32);

    for i in 0..branch_count {
        let base_height = start_height + spacing * i as f32;
        let height = base_height + rng.range(0.0, spacing * 0.5);
        if height > end_height {
            break;
        }

        if b.break_chance > 0.0 && rng.next_f32() < b.break_chance {
            continue;
        }

        current_angle += b.phyllotaxis_angle + rng.range(-10.0, 10.0);
        let angle_rad = current_angle.to_radians();

        let t = height / trunk.height;
        let trunk_r = trunk_radius_at(trunk, t);
        let (wobble_x, wobble_z) = trunk_wobble(trunk, params.seed, t);

        let anchor = Vec3::new(wobble_x, height, wobble_z);
        let start = Vec3::new(
            angle_rad.cos() * trunk_r + wobble_x,
            height,
            angle_rad.sin() * trunk_r + wobble_z,
        );

        let height_ratio = (height - start_height) / zone_length;
        let angle_adjustment = b.crown_angle_variation * (height_ratio - 0.5) * 30.0;
        let effective_angle = (b.angle + angle_adjustment).clamp(0.0, 90.0);

        let outward = Vec3::new(angle_rad.cos(), 0.0, angle_rad.sin());
        let base_dir = Vec3::Y.lerp(outward, effective_angle / 90.0);

        let flattened_dir = if b.flatness > 0.0 {
            Vec3::new(base_dir.x, base_dir.y * (1.0 - b.flatness), base_dir.z).normalize_or_zero()
        } else {
            base_dir
        };

        let random_offset = random_vec(rng) * b.randomness;
        let up_offset = Vec3::Y * b.up_attraction;
        let direction = (flattened_dir + random_offset + up_offset).normalize_or_zero();

        let (base_radius, tip_radius) = floored_radii(
            trunk_r * b.radius_ratio,
            trunk_r * b.radius_ratio * (1.0 - b.taper),
            b.min_twig_radius,
            trunk_r,
        );

        let shape_mult = b.crown_shape.length_multiplier(height_ratio);
        let final_mult = lerp(1.0, shape_mult, b.crown_influence);
        let variation_min = 1.0 - b.length_variation;
        let dominance_mult = 1.0 - (b.apical_dominance * height_ratio * 0.5);
        let length = trunk.height
            * b.length
            * final_mult
            * dominance_mult
            * rng.range(variation_min, 1.0);

        let direction = if b.angle_curve.abs() > 0.001 {
            let angle_adjustment = b.angle_curve * height_ratio * 30.0f32.to_radians();
            Vec3::new(direction.x, direction.y + angle_adjustment.sin(), direction.z)
                .normalize_or_zero()
        } else {
            direction
        };

        let (direction, length) = if b.floor_avoidance {
            let end_y = start.y + direction.y * length;
            if end_y < b.floor_level {
                if direction.y < -0.3 {
                    continue;
                } else if direction.y < 0.0 {
                    let clamped_dir = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
                    let max_length = if clamped_dir.y.abs() < 0.001 {
                        length
                    } else {
                        ((start.y - b.floor_level) / -clamped_dir.y).max(0.1)
                    };
                    (clamped_dir, length.min(max_length))
                } else {
                    (direction, length)
                }
            } else {
                (direction, length)
            }
        } else {
            (direction, length)
        };

        out.push(Node {
            kind: NodeKind::Primary,
            parent: Some(0),
            attach_t: t,
            anchor,
            start,
            direction,
            length,
            base_radius,
            tip_radius,
            depth: 0,
            height_ratio,
            ..blank_node()
        });
    }

    out
}

/// Y-split of the primary at `idx`. On success the primary is shortened to the
/// stub and the two diverging children are returned (not yet pushed).
fn try_split(
    skeleton: &mut Skeleton,
    idx: usize,
    params: &TreeParams,
    rng: &mut SeededRng,
) -> Option<(Node, Node)> {
    let split = &params.split;
    if !split.enabled || rng.next_f32() > split.probability {
        return None;
    }
    let parent = &skeleton.nodes[idx];
    if parent.base_radius < split.radius_threshold {
        return None;
    }

    let split_t = split.position;
    let split_point = parent.start + parent.direction * parent.length * split_t;
    let remaining_length = parent.length * (1.0 - split_t);
    let split_radius = lerp(parent.base_radius, parent.tip_radius, split_t);

    let half_angle = (split.angle / 2.0).to_radians();
    let perp = perpendicular(parent.direction);
    let rotation = rng.range(0.0, TAU);
    let perp2 = parent.direction.cross(perp);
    let rotated_perp = perp * rotation.cos() + perp2 * rotation.sin();

    let dir1 = (parent.direction * half_angle.cos() + rotated_perp * half_angle.sin())
        .normalize_or_zero();
    let dir2 = (parent.direction * half_angle.cos() - rotated_perp * half_angle.sin())
        .normalize_or_zero();

    let min_radius = params.branches.min_twig_radius;
    let branch_tip_radius = parent.tip_radius * 0.9;
    let (split_base, split_tip) =
        floored_radii(split_radius * 0.7, branch_tip_radius * 0.7, min_radius, split_radius);
    let template = Node {
        kind: NodeKind::Split,
        parent: Some(idx as u32),
        attach_t: 1.0,
        anchor: split_point,
        start: split_point,
        base_radius: split_base,
        tip_radius: split_tip,
        depth: parent.depth,
        height_ratio: parent.height_ratio,
        ..blank_node()
    };
    let child_a = Node {
        direction: dir1,
        length: remaining_length * rng.range(0.9, 1.1),
        ..template.clone()
    };
    let child_b = Node {
        direction: dir2,
        length: remaining_length * rng.range(0.9, 1.1),
        ..template
    };

    // The stub tapers into its halves, so the fork shows no ledge.
    let stub = &mut skeleton.nodes[idx];
    stub.length *= split_t;
    stub.tip_radius = split_base;

    Some((child_a, child_b))
}

/// Recursively generate sub-branches of `parent_idx`, pushing in pre-order.
fn generate_subs(
    skeleton: &mut Skeleton,
    parent_idx: usize,
    params: &TreeParams,
    rng: &mut SeededRng,
) {
    let b = &params.branches;
    let depth = skeleton.nodes[parent_idx].depth as u32;
    if depth >= b.recursion {
        return;
    }

    for _ in 0..b.sub_branch_count {
        if skeleton.nodes.len() >= MAX_NODES {
            return;
        }
        if b.break_chance > 0.0 && rng.next_f32() < b.break_chance {
            continue;
        }

        let parent = skeleton.nodes[parent_idx].clone();

        let base_t = rng.range(0.3, 0.8);
        let t = if b.sub_branch_position_bias > 0.0 {
            lerp(base_t, 0.8, b.sub_branch_position_bias)
        } else {
            lerp(base_t, 0.3, -b.sub_branch_position_bias)
        };
        let start = parent.start + parent.direction * parent.length * t;
        // A drooping parent hands its children its direction where they attach
        // (the tangent of the bent wood), so twigs hang from arching limbs
        // instead of leaving as if the limb were straight.
        let tangent = {
            let mut bent = parent.clone();
            bent.bend = (b.gravity_strength * (1.0 - b.stiffness)).max(0.0);
            bent.bent_direction(t)
        };

        let spread = rng.range(30.0, 60.0).to_radians();
        let rotation = rng.range(0.0, TAU);

        let perp = perpendicular(tangent);
        let perp2 = tangent.cross(perp);
        let offset = perp * rotation.cos() + perp2 * rotation.sin();
        let spread_direction = |side: f32| {
            let d = (tangent * spread.cos() + offset * (side * spread.sin()))
                .normalize_or_zero();
            if b.flatness > 0.0 {
                Vec3::new(d.x, d.y * (1.0 - b.flatness), d.z).normalize_or_zero()
            } else {
                d
            }
        };

        let dominance_mult = 1.0 - (b.apical_dominance * parent.height_ratio * 0.4);
        let length = parent.length * b.sub_branch_scale * dominance_mult;
        let parent_radius = lerp(parent.base_radius, parent.tip_radius, t);
        let (base_radius, tip_radius) = floored_radii(
            parent_radius * b.radius_ratio,
            parent_radius * b.radius_ratio * (1.0 - b.taper),
            b.min_twig_radius,
            parent_radius,
        );

        // A sub-branch spreading toward the trunk would grow through it (steep
        // primaries hug the trunk); spread to the other side of the parent instead.
        let into_trunk = |dir: Vec3| {
            (1..=10).any(|k| {
                let p = start + dir * (length * k as f32 / 10.0);
                trunk_depth(&params.trunk, params.seed, p) > 0.0
            })
        };
        let mut direction = spread_direction(1.0);
        if into_trunk(direction) {
            direction = spread_direction(-1.0);
        }

        let (direction, force_terminal) = if b.floor_avoidance {
            let end_y = start.y + direction.y * length;
            if end_y < b.floor_level {
                if direction.y < -0.3 {
                    continue;
                } else if direction.y < 0.0 {
                    (
                        Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero(),
                        true,
                    )
                } else {
                    (direction, false)
                }
            } else {
                (direction, false)
            }
        } else {
            (direction, false)
        };

        let idx = push_node(
            skeleton,
            Node {
                kind: NodeKind::Sub,
                parent: Some(parent_idx as u32),
                attach_t: t,
                anchor: start,
                start,
                direction,
                length,
                base_radius,
                tip_radius,
                depth: (depth + 1) as u8,
                height_ratio: parent.height_ratio,
                ..blank_node()
            },
        );

        if !force_terminal {
            generate_subs(skeleton, idx, params, rng);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;

    #[test]
    fn skeleton_is_preorder() {
        for preset in TreePreset::ALL {
            let s = generate_skeleton(&preset.params());
            assert_eq!(s.nodes[0].kind, NodeKind::Trunk);
            for (i, n) in s.nodes.iter().enumerate().skip(1) {
                let p = n.parent.expect("non-trunk node has a parent") as usize;
                assert!(p < i, "{}: node {i} parent {p}", preset.name());
            }
        }
    }

    #[test]
    fn seed_determinism() {
        let params = TreePreset::Maple.params();
        let a = generate_skeleton(&params);
        let b = generate_skeleton(&params);
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (x, y) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!(x.start.to_array(), y.start.to_array());
            assert_eq!(x.length.to_bits(), y.length.to_bits());
        }
        let mut other = params.clone();
        other.seed += 1;
        let c = generate_skeleton(&other);
        assert!(a != c);
    }

    #[test]
    fn trunk_only_when_density_zero_zone() {
        let mut params = TreeParams::default();
        params.branches.start = 0.5;
        params.branches.end = 0.5;
        let s = generate_skeleton(&params);
        assert_eq!(s.nodes.len(), 1);
        assert!(s.is_terminal(0));
    }

    #[test]
    fn split_marks_primary_as_stub() {
        let mut unsplit = TreeParams::default();
        unsplit.branches.recursion = 0;
        unsplit.split.enabled = false;
        let mut split = unsplit.clone();
        split.split.enabled = true;
        split.split.probability = 1.0;
        split.split.radius_threshold = 0.0;

        let a = generate_skeleton(&unsplit);
        let b = generate_skeleton(&split);
        let primaries_a: Vec<&Node> = a.nodes.iter().filter(|n| n.kind == NodeKind::Primary).collect();
        let primaries_b: Vec<(usize, &Node)> = b
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::Primary)
            .collect();
        assert!(!primaries_a.is_empty());
        assert_eq!(primaries_a.len(), primaries_b.len());
        for (orig, (idx, stub)) in primaries_a.iter().zip(&primaries_b) {
            assert!((stub.length - orig.length * split.split.position).abs() < 1e-5);
            assert_eq!(stub.child_count, 2);
            let kinds: Vec<NodeKind> = b.children(*idx).map(|c| b.nodes[c].kind).collect();
            assert_eq!(kinds, vec![NodeKind::Split, NodeKind::Split]);
        }
    }

    #[test]
    fn sub_branch_radius_follows_parent_at_attachment() {
        // `BranchParams::radius_ratio`: base radius relative to the parent
        // radius at the attachment point (as for primaries on the trunk), but
        // never below `min_twig_radius` and never above the parent's radius.
        for preset in TreePreset::ALL {
            let params = preset.params();
            let s = generate_skeleton(&params);
            for n in s.nodes.iter().filter(|n| n.kind == NodeKind::Sub) {
                let parent = &s.nodes[n.parent.unwrap() as usize];
                let parent_r = lerp(parent.base_radius, parent.tip_radius, n.attach_t);
                let expected = (parent_r * params.branches.radius_ratio)
                    .max(params.branches.min_twig_radius)
                    .min(parent_r);
                assert!(
                    (n.base_radius - expected).abs() <= 1e-6,
                    "{}: sub {} base radius {} != {expected}",
                    preset.name(),
                    n.id,
                    n.base_radius
                );
            }
        }
    }

    #[test]
    fn no_branch_is_thinner_than_the_twig_floor() {
        // D-036: second-level twigs came out at 0.4 mm (willow: 0.0 mm) and
        // read as hairlines, some of them bare in mid-air. The floor applies
        // in full at the base and by half at the tip, so twigs still taper,
        // and a twig is never thicker than the wood it grows from.
        for preset in TreePreset::ALL {
            for floor in [0.002, 0.004] {
                let mut params = preset.params();
                params.branches.min_twig_radius = floor;
                params.split.enabled = true;
                params.split.probability = 1.0;
                params.split.radius_threshold = 0.0;
                let s = generate_skeleton(&params);
                // The leader is the trunk's pointed extension, not a twig.
                for (i, n) in s.nodes.iter().enumerate().skip(1).filter(|(_, n)| n.kind != NodeKind::Leader) {
                    let what = format!("{} floor {floor}: {:?} {i} radii {} -> {}", preset.name(), n.kind, n.base_radius, n.tip_radius);
                    assert!(n.tip_radius >= 0.5 * floor - 1e-7 && n.base_radius >= n.tip_radius - 1e-7, "{what}");
                    let parent = &s.nodes[n.parent.unwrap() as usize];
                    if parent.kind != NodeKind::Trunk {
                        let parent_r = lerp(parent.base_radius, parent.tip_radius, n.attach_t);
                        assert!(n.base_radius <= parent_r + 1e-7, "{what}: parent is {parent_r} where it attaches");
                    }
                }
            }
        }
    }

    #[test]
    fn sub_branches_spread_around_the_bent_tangent() {
        // D-030: children took the direction their drooping parent left the
        // trunk with, so twigs stuck out of arching limbs instead of hanging.
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 2.0;
        params.branches.stiffness = 0.0;
        params.branches.flatness = 0.0;
        params.branches.floor_avoidance = false;
        let s = generate_skeleton(&params);
        let mut checked = 0;
        for n in s.nodes.iter().filter(|n| n.kind == NodeKind::Sub) {
            let parent = &s.nodes[n.parent.unwrap() as usize];
            let tangent = parent.bent_direction(n.attach_t);
            let spread = n.direction.dot(tangent).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(
                (29.9..=60.1).contains(&spread),
                "sub {} leaves its parent's tangent at {spread} degrees",
                n.id
            );
            checked += 1;
        }
        assert!(checked > 20, "only {checked} sub-branches");
    }

    #[test]
    fn sub_branches_do_not_grow_into_the_trunk() {
        for preset in TreePreset::ALL {
            for seed in [42, 1, 2, 3, 4, 5, 6, 7] {
                let mut params = preset.params();
                params.seed = seed;
                let s = generate_skeleton(&params);
                for n in s.nodes.iter().filter(|n| n.kind == NodeKind::Sub) {
                    for k in 1..=10 {
                        let p = n.point_at(k as f32 / 10.0);
                        let depth = trunk_depth(&params.trunk, seed, p);
                        assert!(
                            depth <= 0.0,
                            "{} seed {seed}: sub {} is {depth} m inside the trunk at {p:?}",
                            preset.name(),
                            n.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn strong_gravity_never_folds_a_branch() {
        // Gravity bends wood toward the ground; it must never turn a branch
        // back on itself (consecutive axis directions more than 90° apart),
        // which pinches its tube inside out.
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 2.0;
        params.branches.stiffness = 0.0;
        let s = generate_skeleton(&params);
        for n in s.nodes.iter().skip(1) {
            let axis = n.axis(n.axis_points());
            for w in axis.windows(2) {
                assert!(
                    w[0].1.dot(w[1].1) > 0.0,
                    "{:?} {} folds: {:?} -> {:?}",
                    n.kind,
                    n.id,
                    w[0].1,
                    w[1].1
                );
            }
        }
    }

    #[test]
    fn drooping_branches_get_enough_rings_to_look_smooth() {
        // D-034: every bent branch was meshed on 6 rings, so a long willow limb
        // turned up to 9° at each ring and showed facets along its curve.
        for (gravity, stiffness) in [(0.5, 0.2), (1.0, 0.2), (2.0, 0.0)] {
            let mut params = TreePreset::Willow.params();
            params.branches.gravity_strength = gravity;
            params.branches.stiffness = stiffness;
            let s = generate_skeleton(&params);
            let mut extra = false;
            for n in s.nodes.iter().skip(1) {
                let axis = n.axis(n.axis_points());
                extra |= axis.len() > BENT_AXIS_POINTS as usize;
                if axis.len() == MAX_AXIS_POINTS as usize {
                    continue; // capped: the most rings a branch gets
                }
                for w in axis.windows(2) {
                    let turn = w[0].1.dot(w[1].1).clamp(-1.0, 1.0).acos();
                    assert!(turn < 0.1, "gravity {gravity}: {:?} {} turns {turn} rad per ring", n.kind, n.id);
                }
            }
            assert!(extra, "gravity {gravity}: no branch got more than {BENT_AXIS_POINTS} rings");
        }
    }

    #[test]
    fn bent_wood_stays_above_the_floor() {
        // Floor avoidance must hold for the bent axis, not just the straight
        // one: drooping willow twigs and downward branches low on the trunk.
        let mut low = TreePreset::Oak.params();
        low.branches.start = 0.05;
        low.branches.gravity_strength = 2.0;
        low.branches.stiffness = 0.0;
        low.branches.up_attraction = -1.0;
        let mut cases = vec![("droop low".to_string(), low)];
        for seed in 1..=8 {
            let mut willow = TreePreset::Willow.params();
            willow.seed = seed;
            cases.push((format!("willow seed {seed}"), willow));
        }
        for (label, params) in cases {
            let s = generate_skeleton(&params);
            for n in s.nodes.iter().skip(1) {
                let lowest = n.lowest();
                assert!(
                    lowest >= params.branches.floor_level - 1e-4,
                    "{label}: {:?} {} reaches {lowest} below the floor",
                    n.kind,
                    n.id
                );
            }
        }
    }

    #[test]
    fn branches_below_a_raised_floor_keep_their_droop() {
        // D-038: a branch that starts below the floor cannot satisfy it, so
        // it must not drop below its own base; it must not lose its whole
        // gravity bend either.
        let mut params = TreePreset::Oak.params();
        params.branches.floor_level = 2.0;
        params.branches.start = 0.1;
        let s = generate_skeleton(&params);
        let mut below = 0;
        for n in s.nodes.iter().skip(1).filter(|n| n.start.y < params.branches.floor_level) {
            below += 1;
            let floor = n.start.y - n.base_radius;
            assert!(n.lowest() >= floor - 1e-4, "{:?} {} dips below its own base", n.kind, n.id);
            if n.direction.y > 0.0 {
                assert!(n.bend > 0.0, "{:?} {} lost its gravity bend", n.kind, n.id);
            }
        }
        assert!(below > 0, "no branch starts below the raised floor");
    }

    #[test]
    fn split_stub_flows_into_its_halves() {
        // At a Y-split the stub must end no wider than the halves start, or
        // its end shows as a ledge around the fork.
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.split.enabled = true;
            params.split.probability = 1.0;
            params.split.radius_threshold = 0.0;
            let s = generate_skeleton(&params);
            for (i, stub) in s.nodes.iter().enumerate() {
                for c in s.children(i).filter(|&c| s.nodes[c].kind == NodeKind::Split) {
                    let half = &s.nodes[c];
                    assert!(
                        stub.tip_radius <= half.base_radius + 1e-6,
                        "{}: stub {} ends at radius {} but half {} starts at {}",
                        preset.name(),
                        stub.id,
                        stub.tip_radius,
                        half.id,
                        half.base_radius
                    );
                }
            }
        }
    }

    #[test]
    fn strands_hang_from_the_wood_and_stop_above_the_floor() {
        // D-052: the willow's crown was a mound two thirds of its height; a strand keeps hanging past
        // the arch of its branch, plumb within `spread`, and stops `clearance` above the floor.
        let params = TreePreset::Willow.params();
        let strands = &params.branches.strands;
        let s = generate_skeleton(&params);
        let plumb = strands.spread.to_radians().cos();
        let floor = params.branches.floor_level + strands.clearance;
        let mut count = 0;
        for (i, n) in s.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Strand) {
            count += 1;
            let p = n.parent.expect("a strand hangs from a branch") as usize;
            let parent = &s.nodes[p];
            assert!(p < i, "strand {i} comes before its branch {p}");
            assert_ne!(parent.kind, NodeKind::Strand, "strand {i} hangs from a strand");
            assert_eq!(n.depth, parent.depth + 1);
            assert!((strands.start..=1.0).contains(&n.attach_t), "strand {i} hangs from {} of its branch", n.attach_t);
            assert!(n.start.distance(parent.point_at(n.attach_t)) < 1e-4, "strand {i} starts off its branch's wood");
            assert!(n.direction.y <= -plumb + 1e-4, "strand {i} leans {} off plumb", n.direction.y.acos());
            let end = s.end(i);
            assert!(end.y < n.start.y, "strand {i} does not hang");
            assert!(end.y - n.tip_radius >= floor - 1e-3, "strand {i} ends at {} under the floor {floor}", end.y);
        }
        assert!(count >= 200, "only {count} strands on the willow");
    }

    #[test]
    fn strands_leave_every_other_branch_where_it_was() {
        // Strands draw from streams of their own and come after every other node: a tree with them is
        // the tree without them plus ropes, so no branch moves, and none is born at another age.
        let params = TreePreset::Willow.params();
        let mut bare = params.clone();
        bare.branches.strands.density = 0.0;
        let with = generate_skeleton(&params);
        let without = generate_skeleton(&bare);
        assert!(without.nodes.iter().all(|n| n.kind != NodeKind::Strand));
        assert!(with.nodes.len() > without.nodes.len());
        for (a, b) in with.nodes.iter().zip(&without.nodes) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.start.to_array().map(f32::to_bits), b.start.to_array().map(f32::to_bits), "node {} moved", a.id);
            assert_eq!(a.length.to_bits(), b.length.to_bits(), "node {} changed length", a.id);
            assert_eq!(a.birth.to_bits(), b.birth.to_bits(), "node {} changed birth", a.id);
        }
        let again = generate_skeleton(&params);
        assert_eq!(with, again, "the same seed gave another tree");
    }

    #[test]
    fn a_tree_that_asks_for_no_strands_has_none() {
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.branches.strands.density = 0.0;
            let s = generate_skeleton(&params);
            assert!(s.nodes.iter().all(|n| n.kind != NodeKind::Strand), "{} has strands at density 0", preset.name());
        }
    }

    #[test]
    fn strands_on_any_tree_keep_off_the_trunk_and_the_floor() {
        // The oak with ropes from the base of every branch, plumb, on a raised floor: nothing hangs
        // through the trunk or below the clearance.
        let mut params = TreePreset::Oak.params();
        params.branches.floor_level = 2.0;
        params.branches.start = 0.1;
        params.branches.strands = crate::StrandParams { density: 3.0, start: 0.0, length: 1.0, spread: 0.0, clearance: 0.3 };
        let s = generate_skeleton(&params);
        let mut count = 0;
        for (i, n) in s.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Strand) {
            count += 1;
            for k in 1..=10 {
                let p = n.point_at(k as f32 / 10.0);
                assert!(trunk_depth(&params.trunk, params.seed, p) <= 1e-4, "strand {i} runs through the trunk at {p}");
            }
            assert!(s.end(i).y >= 2.3 - n.tip_radius - 1e-3, "strand {i} ends at {} under the raised floor", s.end(i).y);
        }
        assert!(count > 50, "only {count} strands");
    }
}
