//! Per-node growth state: the only thing written to the GPU each frame.
//!
//! [`GrowthState::update`] ages the full skeleton the way the CPU path does
//! (`aged_skeleton`, with pipe radii) and turns every node into a [`NodeState`]
//! by calling the same helpers the meshers call, so the shader and the CPU
//! meshers agree on which pieces exist at an age and where they sit.

use bevy::math::Vec3;

use crate::foliage::{distributed_leaf_amount, node_leaf_scale};
use crate::mesh::branch::{collar_geometry, primary_root, root_segment_emitted};
use crate::mesh::trunk::{TrunkRing, trunk_cone_lift, trunk_pieces, trunk_rings};
use crate::params::{FoliagePlacement, TreeParams};
use crate::pipeline::aged_skeleton;
use crate::skeleton::{Node, NodeKind, Skeleton};

/// [`NodeState::flags`]: the node's tube and collar exist.
pub(crate) const WOOD: u32 = 1;
/// [`NodeState::flags`]: the primary's collar exists.
pub(crate) const COLLAR: u32 = 2;
/// [`NodeState::flags`]: the primary's base (extension or dome) exists.
pub(crate) const BASE: u32 = 4;
/// [`NodeState::flags`]: the base under the collar exists.
pub(crate) const BASE_COLLAR: u32 = 8;

/// [`GrowthState::trunk_flags`]: the trunk's top cap exists.
pub(crate) const TOP_CAP: u32 = 1;
/// [`GrowthState::trunk_flags`]: the trunk's tip cone exists.
pub(crate) const CONE: u32 = 2;

/// `vec4`s in the buffer ahead of the nodes.
pub(crate) const HEADER_VEC4S: usize = 1;
/// `vec4`s per node in the buffer.
pub(crate) const NODE_VEC4S: usize = 5;
/// `vec4`s per trunk ring in the buffer.
pub(crate) const RING_VEC4S: usize = 2;

/// One node at one age, as the shader reads it (5 `vec4`s):
/// `[start, length]`, `[base_radius, tip_radius, leaf_scale, leaf_count]`,
/// `[collar_start, collar_start_radius]`, `[collar_end, collar_end_radius]`,
/// `[root, flags]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NodeState {
    /// Start of the node's wood.
    pub start: Vec3,
    /// Length of the node's wood.
    pub length: f32,
    /// Radius at the start.
    pub base_radius: f32,
    /// Radius at the tip.
    pub tip_radius: f32,
    /// Growth scale of the node's leaves, 0 to 1.
    pub leaf_scale: f32,
    /// Leaves along the node under `AllBranches`, as a real number (`distributed_leaf_amount`): the
    /// leaf of ordinal `i` shows at `leaf_flush_scale(leaf_count, i)` of its size. Negative: all of
    /// them, at full size.
    pub leaf_count: f32,
    /// Centre of the collar's wide end.
    pub collar_start: Vec3,
    /// Radius of the collar's wide end.
    pub collar_start_radius: f32,
    /// Centre of the collar's narrow end.
    pub collar_end: Vec3,
    /// Radius of the collar's narrow end.
    pub collar_end_radius: f32,
    /// Far end of the primary's extension into the trunk.
    pub root: Vec3,
    /// Which pieces exist: [`WOOD`], [`COLLAR`], [`BASE`], [`BASE_COLLAR`].
    pub flags: u32,
}

impl NodeState {
    /// A node that has not been born: everything collapses onto `at`.
    fn unborn(at: Vec3) -> Self {
        Self {
            start: at,
            leaf_count: -1.0,
            ..Self::default()
        }
    }

    /// The state of the aged `node`.
    fn of(node: &Node, params: &TreeParams) -> Self {
        let mut state = Self {
            start: node.start,
            length: node.length,
            base_radius: node.base_radius,
            tip_radius: node.tip_radius,
            leaf_count: -1.0,
            ..Self::default()
        };
        if node.kind == NodeKind::Trunk {
            return state;
        }

        if params.foliage.enabled && node.length > 0.0 {
            state.leaf_scale = node_leaf_scale(node, &params.growth);
            if params.foliage.placement == FoliagePlacement::AllBranches {
                state.leaf_count = distributed_leaf_amount(&params.foliage, node.kind, node.length, node.height_ratio);
            }
        }

        // The same test `skeleton_mesh` uses to skip a node's wood.
        if node.length <= 1e-4 || node.base_radius <= 0.0 {
            return state;
        }
        state.flags |= WOOD;
        if node.kind != NodeKind::Primary {
            return state;
        }

        let root = primary_root(node);
        let base_exists = |at: Vec3| root.is_none_or(|root| root_segment_emitted(root, at, node.direction));
        state.root = root.unwrap_or(node.start);
        if base_exists(node.start) {
            state.flags |= BASE;
        }
        if params.mesh.collar_enabled {
            let collar = collar_geometry(node, node.base_radius * params.mesh.collar_length);
            state.collar_start = collar.start;
            state.collar_start_radius = collar.start_radius;
            state.collar_end = collar.end;
            state.collar_end_radius = collar.end_radius;
            state.flags |= COLLAR;
            if base_exists(collar.start) {
                state.flags |= BASE_COLLAR;
            }
        }
        state
    }
}

/// Everything the shader needs at one age.
#[derive(Clone, Debug, Default)]
pub(crate) struct GrowthState {
    /// One state per node of the full skeleton, by `Node::id`.
    pub nodes: Vec<NodeState>,
    /// The trunk's rings, bottom to top.
    pub rings: Vec<TrunkRing>,
    /// [`TOP_CAP`] and [`CONE`]: which optional trunk pieces exist.
    pub trunk_flags: u32,
    /// Height of the tip cone above the top ring.
    pub cone_lift: f32,
    /// Growth progress of the trunk, 0 to 1.
    pub trunk_progress: f32,
    present: Vec<bool>,
}

impl GrowthState {
    /// Recompute for `full` (the full-grown skeleton of `params`) at `age`,
    /// reusing this state's allocations.
    pub fn update(&mut self, full: &Skeleton, params: &TreeParams, age: f32) {
        let aged = aged_skeleton(full, params, age);
        let count = full.nodes.len();
        self.present.clear();
        self.present.resize(count, false);
        self.nodes.clear();
        self.nodes.resize(count, NodeState::default());

        for node in &aged.nodes {
            let id = node.id as usize;
            self.nodes[id] = NodeState::of(node, params);
            self.present[id] = true;
        }
        // Parents come first, so an unborn node can sit on its parent's start.
        for id in 1..count {
            if !self.present[id] {
                let parent = full.nodes[id].parent.map_or(0, |p| p as usize);
                self.nodes[id] = NodeState::unborn(self.nodes[parent].start);
            }
        }

        let trunk = &aged.nodes[0];
        trunk_rings(&params.trunk, trunk, params.seed, &mut self.rings);
        let pieces = trunk_pieces(&params.trunk, trunk);
        self.trunk_flags = if pieces.top_cap { TOP_CAP } else { 0 } | if pieces.cone { CONE } else { 0 };
        self.cone_lift = trunk_cone_lift(&params.trunk, trunk);
        self.trunk_progress = trunk.progress;
    }

    /// Number of `vec4`s [`GrowthState::encode`] writes.
    pub fn vec4_len(&self) -> usize {
        HEADER_VEC4S + NODE_VEC4S * self.nodes.len() + RING_VEC4S * self.rings.len()
    }

    /// Write the buffer the shader reads: a header `[node count, ring count,
    /// trunk flags, cone lift]`, the nodes, then the trunk rings
    /// (`[center, radius]`, `[bulge, twist, 0, 0]`). `out` is [`vec4_len`](Self::vec4_len) long.
    pub fn encode(&self, out: &mut [[f32; 4]]) {
        assert_eq!(out.len(), self.vec4_len(), "growth buffer has the wrong size");
        let v = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
        out[0] = [
            self.nodes.len() as f32,
            self.rings.len() as f32,
            self.trunk_flags as f32,
            self.cone_lift,
        ];
        let (nodes, rings) = out[HEADER_VEC4S..].split_at_mut(NODE_VEC4S * self.nodes.len());
        for (slot, node) in nodes.as_chunks_mut::<NODE_VEC4S>().0.iter_mut().zip(&self.nodes) {
            slot[0] = v(node.start, node.length);
            slot[1] = [node.base_radius, node.tip_radius, node.leaf_scale, node.leaf_count];
            slot[2] = v(node.collar_start, node.collar_start_radius);
            slot[3] = v(node.collar_end, node.collar_end_radius);
            slot[4] = v(node.root, node.flags as f32);
        }
        for (slot, ring) in rings.as_chunks_mut::<RING_VEC4S>().0.iter_mut().zip(&self.rings) {
            slot[0] = v(ring.center, ring.radius);
            slot[1] = [ring.bulge, ring.twist, 0.0, 0.0];
        }
    }
}
