//! All tunable parameters of a tree, grouped by subsystem.
//!
//! Every struct implements [`Default`]; `TreeParams::default()` is the
//! generic "oak-ish" tree. Presets in [`crate::presets`] are plain
//! `TreeParams` values.

use bevy::asset::Asset;
use bevy::color::Color;
use bevy::math::Vec2;
use bevy::reflect::Reflect;

pub use crate::crown::CrownShape;
pub use crate::growth::GrowthParams;

/// What happens at the top of the trunk.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrunkTermination {
    /// Flat disc closes the trunk top.
    #[default]
    FlatCap,
    /// Trunk tapers to a point; no cap.
    PointedTip,
    /// A thin leader branch continues upward from the trunk top.
    LeaderBranch,
}

/// Leaf geometry style.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LeafStyle {
    /// Two leaf-shaped cards at 90° (classic pixel-art leaf).
    #[default]
    CrossedPlanes,
    /// One leaf-shaped card.
    SingleQuad,
    /// Octahedron approximation of a leaf cluster.
    ClusterSphere,
    /// Three leaf-shaped cards at 60°.
    StarBurst,
    /// Six thin radiating quads (pine needles).
    NeedleCluster,
}

/// Where leaves are placed on the skeleton.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FoliagePlacement {
    /// Only at the tips of branches without children in the full-grown tree.
    /// A young branch that will grow twigs stays leafless, so leaves never
    /// appear and then vanish as the tree ages.
    #[default]
    TerminalBranches,
    /// Distributed along every branch, toward its base. A leaf keeps its place on
    /// its branch as the branch grows: a young branch shows some of the leaves of
    /// the grown one, spread over its whole length, at the same fractions of it,
    /// and gains the rest in the gaps, so no leaf hops along the wood when another
    /// one appears. A leaf that appears grows from nothing to its size over the
    /// growth of branch the next leaf takes to appear, so none pops in. (And
    /// [`GpuGrowth`](crate::GpuGrowth) puts each leaf where the CPU does, at the
    /// same size.)
    AllBranches,
    /// Spherical clusters at the tips of branches without children in the
    /// full-grown tree.
    TipClusters,
}

/// Leaf orientation mode.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LeafOrientation {
    /// Horizontally away from the trunk axis.
    #[default]
    RadialOutward,
    /// Along the branch direction.
    FollowBranch,
    /// Random with an upward bias.
    RandomUpward,
    /// Flat with a random yaw.
    HorizontalSpread,
}

/// Trunk shape.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct TrunkParams {
    /// Trunk height in metres.
    pub height: f32,
    /// Base trunk radius in metres.
    pub radius: f32,
    /// Number of vertices around each trunk ring.
    pub radial_segments: u32,
    /// Number of ring subdivisions along the trunk height.
    pub height_segments: u32,
    /// Top radius as a fraction of `radius` (0 = point, 1 = no taper).
    pub taper: f32,
    /// Taper curve exponent (0.5 = linear-ish, <0.5 early taper, >0.5 gradual).
    pub taper_curve: f32,
    /// Base radius multiplier (1.0 = none, 1.5 = 50% wider base).
    pub flare: f32,
    /// Lean/wobble amount (0 = straight).
    pub randomness: f32,
    /// Number of root ridges around the base (0 = none).
    pub root_flare_count: u32,
    /// Radial extent of root ridges relative to the base radius.
    pub root_flare_spread: f32,
    /// Height of the root ridges as a fraction of trunk height.
    pub root_flare_height: f32,
    /// How the trunk top is finished.
    pub termination: TrunkTermination,
    /// Leader length as a fraction of trunk height.
    pub leader_length: f32,
    /// Leader tip radius as a fraction of its base.
    pub leader_taper: f32,
    /// Grow sub-branches on the leader.
    pub leader_has_branches: bool,
    /// Twist around the trunk axis from base to top, in degrees.
    pub twist: f32,
}

impl Default for TrunkParams {
    fn default() -> Self {
        Self {
            height: 5.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 4,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.5,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 0.0,
        }
    }
}

/// Branch placement, shape and recursion.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct BranchParams {
    /// Start of the branch zone as a fraction of trunk height.
    pub start: f32,
    /// End of the branch zone as a fraction of trunk height.
    pub end: f32,
    /// Primary branches per unit of trunk length.
    pub density: f32,
    /// Primary branch length relative to trunk height.
    pub length: f32,
    /// Angle from the trunk in degrees (0 = up, 90 = horizontal).
    pub angle: f32,
    /// Branch base radius relative to the parent radius at the attachment point.
    pub radius_ratio: f32,
    /// Taper from base to tip (0 = none, 1 = point).
    pub taper: f32,
    /// Spiral angle between consecutive primaries (137.5° = golden angle).
    pub phyllotaxis_angle: f32,
    /// Direction randomness.
    pub randomness: f32,
    /// Upward growth tendency.
    pub up_attraction: f32,
    /// Sub-branch recursion depth (0 = none).
    pub recursion: u32,
    /// Sub-branches per branch.
    pub sub_branch_count: u32,
    /// Length multiplier per recursion level.
    pub sub_branch_scale: f32,
    /// Random length variation (0 = uniform).
    pub length_variation: f32,
    /// Sub-branch position bias (-1 = base, 0 = uniform, 1 = tip).
    pub sub_branch_position_bias: f32,
    /// Suppression of upper branches (0 = equal, 1 = strong leader).
    pub apical_dominance: f32,
    /// Horizontal spread (0 = natural, 1 = flat canopy).
    pub flatness: f32,
    /// Angle change with height (-1 = droop at top, +1 = reach up at top).
    pub angle_curve: f32,
    /// Crown-based angle variation (-1 = lower horizontal, +1 = lower vertical).
    pub crown_angle_variation: f32,
    /// Twist along each branch, in degrees.
    pub twist: f32,
    /// Downward bend toward the tip.
    pub gravity_strength: f32,
    /// Resistance to gravity (0 = flexible, 1 = stiff).
    pub stiffness: f32,
    /// Chance that a branch is missing ("broke off").
    pub break_chance: f32,
    /// Shape of the crown envelope that scales branch length by height.
    pub crown_shape: CrownShape,
    /// How strongly `crown_shape` affects branch length (0 = uniform).
    pub crown_influence: f32,
    /// Prevent branches from ending below `floor_level`.
    pub floor_avoidance: bool,
    /// World-space y of the floor, in metres, used when `floor_avoidance` is on.
    pub floor_level: f32,
    /// Thinnest a branch's base may be, in metres; its tip is floored at half of
    /// this, so twigs still taper. Generated radii are raised to these floors
    /// (never above the radius of the wood a branch grows from), so twigs on a
    /// large tree read as twigs instead of hairlines; `0` leaves every radius
    /// as the ratios and tapers give it.
    pub min_twig_radius: f32,
    /// Pendant strands hung from the outer part of every branch: a weeping habit. Off by default
    /// (every preset but the willow); see [`StrandParams`].
    pub strands: StrandParams,
}

/// Pendant strands: slender twigs that hang straight down from the outer part of a branch and keep
/// hanging for a set length, the long, rope-like twigs of a weeping willow. A strand is a node of the
/// skeleton one level below the branch it hangs from ([`NodeKind::Strand`](crate::NodeKind::Strand)),
/// so it grows with it, carries leaves along its whole length, sways and burns like any twig; it is a
/// straight rope a few millimetres thick (`min_twig_radius`), drawn with two rings of wood. `density`
/// zero (the default) makes none, and a tree without strands is the tree it was before this field
/// existed. A strand that would hang lower than `clearance` above the floor is shortened to stop
/// there, one that would pass through the trunk is shortened clear of it.
///
/// ```
/// use bevy_flora::{NodeKind, StrandParams, TreePreset, build_tree};
///
/// // An oak that weeps a little: ropes of its own twigs hang from the outer half of every branch.
/// let mut oak = TreePreset::Oak.params();
/// oak.branches.strands = StrandParams { density: 1.0, length: 0.4, ..Default::default() };
/// let tree = build_tree(&oak, f32::INFINITY);
/// assert!(tree.skeleton.nodes.iter().any(|n| n.kind == NodeKind::Strand));
/// ```
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct StrandParams {
    /// Strands per metre of branch, hung along the part of it past `start`. `0` makes none; the
    /// count a branch gets is this times its length times `1 - start`, rounded at random.
    pub density: f32,
    /// Where along a branch strands begin, as a share of its length (0 = from its base, 0.5 = on its
    /// outer half).
    pub start: f32,
    /// The longest a strand hangs, as a share of the trunk's height; each strand is between half of
    /// this and all of it.
    pub length: f32,
    /// How far a strand may lean off plumb where it leaves its branch, in degrees (0 = every strand
    /// plumb). Each strand leans by a draw of its own, in a direction of its own.
    pub spread: f32,
    /// With floor avoidance on, how far above `floor_level` a strand stops, in metres (a strand that
    /// would reach lower is shortened, not removed).
    pub clearance: f32,
}

impl Default for StrandParams {
    fn default() -> Self {
        Self { density: 0.0, start: 0.4, length: 0.5, spread: 12.0, clearance: 0.15 }
    }
}

impl Default for BranchParams {
    fn default() -> Self {
        Self {
            start: 0.3,
            end: 0.9,
            density: 1.0,
            length: 0.4,
            angle: 45.0,
            radius_ratio: 0.3,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.1,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.5,
            length_variation: 0.15,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.5,
            flatness: 0.0,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 0.0,
            gravity_strength: 0.0,
            stiffness: 0.5,
            break_chance: 0.0,
            crown_shape: CrownShape::Cylindrical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.005,
            strands: StrandParams::default(),
        }
    }
}

/// Y-junction splitting of primary branches.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct SplitParams {
    /// Whether primary branches may split into Y-junctions.
    pub enabled: bool,
    /// Chance that a primary splits.
    pub probability: f32,
    /// Angle between the two split branches, in degrees.
    pub angle: f32,
    /// Where along the primary the split occurs (0–1).
    pub position: f32,
    /// Minimum base radius for a branch to split.
    pub radius_threshold: f32,
}

impl Default for SplitParams {
    fn default() -> Self {
        Self {
            enabled: false,
            probability: 0.3,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.05,
        }
    }
}

/// Mesh construction options.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct MeshParams {
    /// Smoothing passes on the wood mesh (0 = off). Volume-preserving (Taubin)
    /// smoothing that softens creases but keeps the ground ring, branch ends,
    /// twig seats, the trunk under collars and hairline twigs in place.
    pub smooth_iterations: u32,
    /// Smoothing strength per pass (0–1).
    pub smooth_factor: f32,
    /// Pick radial segments per branch from its radius.
    pub adaptive_resolution: bool,
    /// Segments per unit radius when `adaptive_resolution` is on.
    pub resolution_scale: f32,
    /// Lower bound on radial segments when `adaptive_resolution` is on. Above
    /// `max_radial_segments` it gives way: the upper bound wins.
    pub min_radial_segments: u32,
    /// Upper bound on radial segments when `adaptive_resolution` is on. It always holds, even
    /// below `min_radial_segments`, so it caps the wood's vertex cost; a tube never has fewer than
    /// 3 sides.
    pub max_radial_segments: u32,
    /// Recompute parent radii from children (Da Vinci's pipe model).
    pub pipe_radius_enabled: bool,
    /// Pipe model exponent (2.0 = Da Vinci, ~2.5 = Murray).
    pub pipe_radius_exponent: f32,
    /// Minimum branch radius under the pipe model.
    pub pipe_radius_min: f32,
    /// Emit a flared collar where primaries meet the trunk.
    pub collar_enabled: bool,
    /// Collar length as a multiple of the branch base radius.
    pub collar_length: f32,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            smooth_iterations: 0,
            smooth_factor: 0.5,
            adaptive_resolution: false,
            resolution_scale: 16.0,
            min_radial_segments: 4,
            max_radial_segments: 16,
            pipe_radius_enabled: false,
            pipe_radius_exponent: 2.0,
            pipe_radius_min: 0.02,
            collar_enabled: true,
            collar_length: 1.5,
        }
    }
}

/// A bundled, CC0-licensed leaf scan: four real leaves photographed flat, as one atlas of colour
/// and normal maps with each leaf's outline, so [`FoliageParams::scan`] can cut leaf cards to
/// the shape of a real leaf and paint them with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LeafScan {
    /// Lobed oak leaves (ambientCG `LeafSet016`).
    Oak,
    /// Long, narrow willow leaves (ambientCG `LeafSet013`).
    Willow,
    /// Birch leaves (ambientCG `LeafSet014`).
    Birch,
    /// Broad evergreen leaves for the bonsai (ambientCG `LeafSet022`).
    Bonsai,
    /// Maple leaves (ambientCG `LeafSet010`).
    Maple,
    /// Rounded poplar leaves (ambientCG `LeafSet024`).
    Poplar,
    /// Small yellow Japanese maple leaves (ambientCG `LeafSet021`).
    JapaneseMaple,
}

impl LeafScan {
    /// Every scan, in the order of the generated outline table.
    pub const ALL: [LeafScan; 7] = [
        LeafScan::Oak,
        LeafScan::Willow,
        LeafScan::Birch,
        LeafScan::Bonsai,
        LeafScan::Maple,
        LeafScan::Poplar,
        LeafScan::JapaneseMaple,
    ];
}

/// Leaf placement and geometry.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct FoliageParams {
    /// Whether leaves are generated at all.
    pub enabled: bool,
    /// Leaf mesh style.
    pub style: LeafStyle,
    /// Where leaves are placed on the skeleton.
    pub placement: FoliagePlacement,
    /// How leaves are oriented relative to their branch.
    pub orientation: LeafOrientation,
    /// Leaves per branch (terminal) or per unit length (all branches).
    pub density: f32,
    /// Leaves per cluster for [`FoliagePlacement::TipClusters`].
    pub cluster_size: u32,
    /// Leaf length (base to tip) in metres; leaf cards are leaf-shaped, about half the area of a square of that size.
    pub leaf_size: f32,
    /// Random size variation (0–0.5).
    pub leaf_size_variation: f32,
    /// How many leaves a strand ([`StrandParams`]) bears, as a share of [`density`](Self::density): a
    /// long thin twig carries a row of leaves of its own spacing, not the rate of a branch (1 = as
    /// every branch). Under [`FoliagePlacement::AllBranches`] a strand of length `l` has
    /// `density * strand_density * l` leaves, before the height falloff.
    pub strand_density: f32,
    /// How long a strand's leaves are, as a share of [`leaf_size`](Self::leaf_size) (1 = as every
    /// leaf): a row of many leaves is a row of small ones. A strand's leaves are cut as a
    /// four-point lance whatever the scan, so a curtain of them costs a quarter of the triangles
    /// a scanned card does.
    pub strand_size: f32,
    /// Branches thicker than this fraction of the trunk radius get no leaves.
    pub radius_threshold: f32,
    /// Density falloff from the bottom to the top of the crown.
    pub height_falloff: f32,
    /// Downward tilt of leaves.
    pub droop: f32,
    /// Random rotation around the leaf direction (0–1).
    pub rotation_variation: f32,
    /// Modulate density by crown height.
    pub use_crown_density: bool,
    /// How far leaf normals bend from each card's own facing toward pointing out of the crown
    /// (0 = flat cards, 1 = a soft round crown); baked into the mesh. The painterly and toon
    /// styles ([`RenderStyle`]) ignore it: their normals point out of each leaf's own clump.
    pub normal_bend: f32,
    /// Per-leaf brightness variation (0–0.5): each leaf's colour is scaled by a factor in 1 ± this.
    pub color_variation: f32,
    /// Per-leaf hue shift (0–0.5): each leaf leans toward yellow-green or blue-green by up to
    /// this fraction.
    pub hue_variation: f32,
    /// How much darker leaves deep inside the crown are under ambient light (0–1); baked into
    /// the mesh.
    pub occlusion: f32,
    /// Leaf base color.
    pub color: Color,
    /// How leaves take light: gloss, veins, the light that comes through them.
    pub shading: LeafShading,
    /// A bundled leaf scan whose real outline the leaf cards are cut to and whose colour and
    /// normal map they wear, or `None` for the procedural leaf with veins grown by space
    /// colonisation ([`LeafShading::veins`]). Only the leaf-shaped styles ([`LeafStyle::CrossedPlanes`],
    /// [`LeafStyle::SingleQuad`], [`LeafStyle::StarBurst`]) use it. Without the crate's `textures`
    /// feature every scan is absent.
    pub scan: Option<LeafScan>,
    /// The colour the leaves turn in autumn, or `None` for an evergreen. With a colour, the
    /// leaves turn as [`FloraSeason::autumn`](crate::FloraSeason::autumn) rises, each at its own
    /// moment, and drop as [`FloraSeason::leaf_fall`](crate::FloraSeason::leaf_fall) rises, in the
    /// order they turned. Drawn by the flora shaders; plain materials ignore it.
    pub autumn: Option<Color>,
    /// The colour of the tree's flowers, or `None` for a tree without them. With a colour, every
    /// leaf has a five-petalled flower at its stem that opens as
    /// [`FloraSeason::blossom`](crate::FloraSeason::blossom) rises, each at its own moment, and
    /// falls as it drops. The flowers are a mesh of their own
    /// ([`TreeMeshes::flowers`](crate::TreeMeshes::flowers)) that the plugin does not draw while
    /// the blossom is 0, so out of season a tree drawn as a `ProceduralTree` costs what it would
    /// without them; a pool keeps them in its leaf meshes, where the shaders hide them. Drawn by
    /// the flora shaders: with plain materials every flower is out while the blossom is above 0
    /// (a pool's all year).
    pub blossom: Option<Color>,
    /// The tree's fruit, or `None` for a tree without. With fruit, some of the leaves
    /// ([`FruitParams::share`]) have one hanging below their stem, in the leaf mesh: it sways
    /// with its leaf, is not on the tree while the blossom is out
    /// ([`FloraSeason::blossom`](crate::FloraSeason::blossom) above 0) and drops in the first part
    /// of the leaf fall ([`FloraSeason::leaf_fall`](crate::FloraSeason::leaf_fall)), each at its
    /// own moment. Drawn by the flora shaders: with plain materials every fruit is on the tree
    /// all year, in the leaves' colour.
    pub fruit: Option<FruitParams>,
}

/// A tree's fruit ([`FoliageParams::fruit`]): round, hanging below the stem of some of its leaves.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct FruitParams {
    /// The fruit's colour; each fruit's brightness varies by [`FoliageParams::color_variation`]
    /// as its leaf's does.
    pub color: Color,
    /// A fruit's radius in metres at a leaf of [`FoliageParams::leaf_size`]: it grows with its
    /// leaf, so a young tree's fruit is small, and a coarse level of detail's, whose leaves are
    /// fewer and larger, is larger too.
    pub radius: f32,
    /// The share of the leaves that bear a fruit (0–1), each leaf by its own fixed draw.
    pub share: f32,
}

impl Default for FruitParams {
    /// Red apples, 4.5 cm across the middle, on one leaf in eight.
    fn default() -> Self {
        Self { color: Color::srgb(0.72, 0.08, 0.06), radius: 0.045, share: 0.125 }
    }
}

impl Default for FoliageParams {
    fn default() -> Self {
        Self {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 3.0,
            cluster_size: 4,
            leaf_size: 0.3,
            leaf_size_variation: 0.15,
            strand_density: 1.0,
            strand_size: 1.0,
            radius_threshold: 0.15,
            height_falloff: 0.3,
            droop: 0.2,
            rotation_variation: 0.5,
            use_crown_density: true,
            normal_bend: 0.8,
            color_variation: 0.3,
            hue_variation: 0.3,
            occlusion: 0.6,
            color: Color::srgb(0.133, 0.545, 0.133),
            shading: LeafShading::default(),
            scan: None,
            autumn: None,
            blossom: None,
            fruit: None,
        }
    }
}

/// How leaves take light, shared by every leaf of a tree.
///
/// Read by the shaders of [`FloraMaterial`](crate::FloraMaterial); trees drawn with a plain
/// `StandardMaterial` ignore it.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct LeafShading {
    /// Strength of the light coming through a leaf lit from behind (0 = none).
    pub translucency: f32,
    /// Multiplies the leaf colour for the light coming through it.
    pub transmission_tint: Color,
    /// Perceptual roughness of the upper side; the lower side is 0.3 rougher, capped at 1.
    /// The gloss fades as the camera turns to face the sun, where a crown glows through its
    /// leaves instead of reflecting the sun off them (which turned treetops white).
    pub roughness: f32,
    /// How visible the midrib and side veins are (0–1); leaf-shaped cards only.
    pub veins: f32,
    /// How much paler the lower side is than the upper (0–1).
    pub underside: f32,
}

impl Default for LeafShading {
    fn default() -> Self {
        Self {
            translucency: 1.0,
            transmission_tint: Color::srgb(1.0, 0.95, 0.55),
            roughness: 0.55,
            veins: 0.5,
            underside: 0.25,
        }
    }
}

/// How a species moves in the wind. Multiplies what [`FloraWind`](crate::FloraWind) asks for, so a
/// willow sways more than an oak in the same weather.
///
/// Read by the shaders of [`FloraMaterial`](crate::FloraMaterial); trees drawn with a plain
/// `StandardMaterial` do not move.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct WindResponse {
    /// How far the whole tree bends (0 = rigid): a multiplier on the main bend, which leans every
    /// plant by a share of its own height, so a shrub and a pine with the same `sway` lean through
    /// the same angle, at whatever `Transform` scale or [`FloraUnits`](crate::FloraUnits) they are
    /// drawn at. The presets keep it between 0.9 (the oak, the conifers, the baobab and the other
    /// stiff species) and 1.6 (the willow).
    pub sway: f32,
    /// How much its leaves flutter (0 = not at all).
    pub flutter: f32,
    /// How fast it sways and its gusts come and go.
    pub sway_frequency: f32,
    /// How fast its leaves flutter.
    pub flutter_frequency: f32,
}

impl Default for WindResponse {
    fn default() -> Self {
        Self { sway: 1.0, flutter: 1.0, sway_frequency: 1.0, flutter_frequency: 1.0 }
    }
}

/// A bundled, CC0-licensed bark scan (colour and normal map) trunks and branches can wear: see
/// [`BarkParams`]. Without the crate's `textures` feature every scan is absent and wood is plain
/// colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BarkScan {
    /// Deeply furrowed oak (Poly Haven `jolcham_oak_bark_01`).
    Oak,
    /// Plated pine (Poly Haven `pine_bark`).
    Pine,
    /// Rough willow (Poly Haven `bark_willow`).
    Willow,
    /// White papery birch with dark lenticels (TextureCan `wood_0027`).
    Birch,
    /// Ringed palm (Poly Haven `palm_bark`).
    Palm,
    /// Stringy Japanese cedar (Poly Haven `japanese_cedar_bark`).
    Cedar,
    /// Japanese zelkova (Poly Haven `japanese_zelkova_bark`).
    Zelkova,
    /// Trident maple (Poly Haven `trident_maple_bark`).
    Maple,
    /// Knotted pine (Poly Haven `knotted_pine_bark`).
    KnottedPine,
    /// Cherry (Poly Haven `sakura_bark`).
    Sakura,
    /// A palm tree's fibrous bark (Poly Haven `palm_tree_bark`).
    PalmTree,
    /// Chinese hackberry (Poly Haven `chinese_hackberry_bark`).
    Hackberry,
    /// Brown weathered bark (Poly Haven `bark_brown_02`).
    Brown,
    /// Dawn redwood (Poly Haven `metasequoia_bark`).
    Metasequoia,
}

impl BarkScan {
    /// Every scan.
    pub const ALL: [BarkScan; 14] = [
        BarkScan::Oak,
        BarkScan::Pine,
        BarkScan::Willow,
        BarkScan::Birch,
        BarkScan::Palm,
        BarkScan::Cedar,
        BarkScan::Zelkova,
        BarkScan::Maple,
        BarkScan::KnottedPine,
        BarkScan::Sakura,
        BarkScan::PalmTree,
        BarkScan::Hackberry,
        BarkScan::Brown,
        BarkScan::Metasequoia,
    ];

    /// The real size of the scan's tile in metres (across, along): the wood's texture
    /// coordinates are in tiles, so bark features keep their real-world size on any branch.
    pub fn tile_size(self) -> Vec2 {
        match self {
            BarkScan::Oak | BarkScan::Cedar => Vec2::new(1.0, 2.0),
            BarkScan::Pine => Vec2::new(2.0, 2.0),
            BarkScan::Willow | BarkScan::Brown => Vec2::ONE,
            BarkScan::Birch => Vec2::splat(0.6),
            BarkScan::Palm => Vec2::splat(1.86),
            BarkScan::Zelkova | BarkScan::Maple => Vec2::splat(1.8),
            BarkScan::Hackberry => Vec2::new(1.8, 3.6),
            BarkScan::KnottedPine => Vec2::splat(1.7),
            BarkScan::Sakura => Vec2::splat(1.6),
            BarkScan::PalmTree => Vec2::new(1.3, 2.6),
            BarkScan::Metasequoia => Vec2::splat(1.94),
        }
    }
}

/// The bark a tree's wood wears, and how it follows the wood's growth.
///
/// The scan is laid out in metres, anchored to the *mature* tree, so a growing tree's bark does
/// not slide as it lengthens and thickens. Young wood is smooth (the scan's average colour, no
/// relief); as a branch's radius passes `smooth_below` the fissures fade in, and above
/// `fissured_above` the whole scan shows. The scan's average colour is divided out of the
/// wood's base colour, so a textured tree has the same colour as an untextured one from afar.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct BarkParams {
    /// The scan to wear, or `None` for plain colour.
    pub scan: Option<BarkScan>,
    /// Wood thinner than this radius, in metres, is smooth.
    pub smooth_below: f32,
    /// Wood thicker than this radius, in metres, shows the full scan (smoothstep in between).
    pub fissured_above: f32,
}

impl Default for BarkParams {
    fn default() -> Self {
        Self { scan: None, smooth_below: 0.03, fissured_above: 0.15 }
    }
}

/// A stylised way to draw a tree, derived from its skeleton: see [`TreeParams::style`].
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderStyle {
    /// Physically based lighting on crown-bent normals (the crown shades as one soft volume).
    #[default]
    Natural,
    /// Painterly clumps: the leaves of each primary branch (or leader, or split) shade as one
    /// soft clump of their own: every leaf's normal is bent fully toward pointing out of its
    /// clump's centre, not the crown's, so a crown reads as a cluster of rounded masses with a
    /// lit side and a shaded side each. Lighting is as in [`RenderStyle::Natural`].
    Painterly,
    /// Toon shading: each directional light's `N·L` is cut into three bands (shaded, middle and
    /// lit), light through backlit leaves is kept, and an ink rim darkens the surface toward its
    /// silhouette. Leaves keep the painterly clumps' normals.
    Toon,
}

/// How a tree's crown is drawn: as the cards of its leaves, or as one faceted hull.
///
/// `Hull` is the stylised conifer of a low-poly game: the leaf cloud is not drawn; a stack of
/// `tiers` cones of `sides` facets each is wrapped around it instead ([`crate::crown_hull`]), so
/// the crown reads as a few big flat planes. [`FoliageParams::enabled`] still decides whether
/// there is a crown (leaf points are what the hull wraps); fruit and flowers are drawn as ever.
/// The hull has `tiers * sides * 2` triangles and no leaf cards, scan or veins; its facets are
/// lit matte and flat, and a tree that has one is never grown on the GPU
/// ([`TreeParams::gpu_growable`]).
#[derive(Default, Clone, Copy, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrownStyle {
    /// Leaf cards, as [`FoliageParams::style`] says.
    #[default]
    Leaves,
    /// A faceted stack of cones around the leaf cloud.
    Hull {
        /// Cones stacked from the crown's base to its top (at least 1).
        tiers: u32,
        /// Facets round each cone (at least 3).
        sides: u32,
        /// The crown starts at least this share of its top's height above the root, so a trunk
        /// shows under it and a body can walk beneath (0..1).
        lift: f32,
        /// How far each tier's ring is knocked about, 0 (regular cones) to 1: the ring's radius
        /// by up to 16%, its height by up to 12% of a tier, and each tier's turn.
        jitter: f32,
        /// Draw the tree's wood as its trunk chain only (the trunk and its leader, no side
        /// branches), which the crown hides in a stylised conifer: a few dozen triangles instead
        /// of thousands, and no twig poking out of a facet. The tree's skeleton keeps its
        /// branches (limbs it rests on when felled, its shape, its fire fuel); only the mesh
        /// leaves them out. `false` draws every branch.
        trunk_only: bool,
    },
}

/// How a tree's trunk and branches are meshed.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrunkStyle {
    /// Round, smooth-shaded wood, as [`TrunkParams::radial_segments`] and [`MeshParams`] say.
    #[default]
    Round,
    /// Flat-shaded prisms: the trunk has `sides` faces (at least 3), branch tubes and collars at
    /// most that many, no smoothing, and every triangle has its own vertices and face normal.
    Prism {
        /// Faces round the trunk.
        sides: u32,
    },
}

/// Complete description of a tree.
///
/// It is also an [`Asset`]: a [`ProceduralTreeAsset`](crate::ProceduralTreeAsset) draws a tree
/// from a handle, so trees that share one follow every change to it, and with the `ron` feature
/// it loads from `.tree.ron` files (`TreeParams::to_ron` writes one).
#[derive(Asset, Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct TreeParams {
    /// Random seed; the same params and seed always generate the same tree.
    pub seed: i32,
    /// Trunk shape.
    pub trunk: TrunkParams,
    /// Branch generation.
    pub branches: BranchParams,
    /// Y-junction splitting.
    pub split: SplitParams,
    /// Mesh generation.
    pub mesh: MeshParams,
    /// Leaf placement and geometry.
    pub foliage: FoliageParams,
    /// Growth timing.
    pub growth: GrowthParams,
    /// How the tree moves in the wind.
    pub wind: WindResponse,
    /// Trunk and branch color.
    pub trunk_color: Color,
    /// Surface detail of the wood: a bundled bark scan that fades in as a branch thickens.
    pub bark: BarkParams,
    /// How the tree is drawn: naturally lit, as painted clumps, or as a toon with an ink rim.
    pub style: RenderStyle,
    /// How the crown is drawn: leaf cards (the default) or a faceted hull ([`CrownStyle`]).
    pub crown: CrownStyle,
    /// How the wood is meshed: round and smooth (the default) or flat-shaded prisms ([`TrunkStyle`]).
    pub trunk_style: TrunkStyle,
    /// A plant of the forest floor (a shrub, a juniper, a fern) rather than a tree: nothing in how
    /// it is drawn changes, but [`FloraIndex`](crate::FloraIndex) tells it apart
    /// ([`IndexedTree::undergrowth`](crate::IndexedTree::undergrowth)), so a game's axe, collision
    /// or AI can pass it by. True for the `Shrub`, `Juniper` and `Fern` presets.
    pub undergrowth: bool,
}

impl TreeParams {
    /// Whether this tree can grow on the GPU ([`GpuGrowth`](crate::GpuGrowth)): only a round
    /// trunk under leaf cards can. A hull crown is rebuilt from the leaf cloud at every age and a
    /// prism trunk is flat-shaded per triangle, neither of which the growth shader reproduces,
    /// so such a tree is built on the CPU whatever its components say.
    pub fn gpu_growable(&self) -> bool {
        matches!(self.crown, CrownStyle::Leaves) && matches!(self.trunk_style, TrunkStyle::Round)
    }

    /// How far the grown trunk reaches from its axis at the ground, in metres: its radius
    /// widened by the flare, and by the root ridges when it has them. The footprint
    /// [`FloraGround::footing`](crate::FloraGround::footing) stands a tree on.
    pub fn base_radius(&self) -> f32 {
        let flared = self.trunk.radius * self.trunk.flare;
        if self.trunk.root_flare_count > 0 && self.trunk.root_flare_spread > 0.0 {
            flared * (1.0 + self.trunk.root_flare_spread)
        } else {
            flared
        }
    }

    /// The age, in the growth units of [`TreeParams::growth`], at which this tree is fully grown:
    /// the age [`Growing`](crate::Growing) runs it to and [`TreeMature`](crate::TreeMature)
    /// reports, and above which a [`TreeAge`](crate::TreeAge) changes nothing (at that very age
    /// the last wood is a float's rounding short of its full radius, under 1e-6 m). A tree's clock
    /// is its own (an oak and a palm take different numbers), so a caller that places trees at a
    /// share of their growth, or shows an age range in an editor, asks the tree.
    ///
    /// It is read off the skeleton the seed grows (the last branch to thicken, so there is no
    /// formula from [`GrowthParams`] alone) and costs that, not a mesh: the same number as
    /// `build_tree(&params, f32::INFINITY).mature_at` without the wood and leaves, 0.2 ms for the
    /// oak and 0.7 ms for the willow in a release build. Ask once for a kind of tree, not every frame.
    ///
    /// ```
    /// use bevy_flora::{TreeAge, TreePreset};
    ///
    /// let params = TreePreset::Oak.params();
    /// // An oak half way through its growing time (not half its size):
    /// let age = TreeAge(0.5 * params.mature_at());
    /// assert!(age.0 > 0.0 && age.0.is_finite());
    /// ```
    pub fn mature_at(&self) -> f32 {
        crate::skeleton::generate_skeleton(self).mature_at(&self.growth)
    }
}

impl Default for TreeParams {
    fn default() -> Self {
        Self {
            seed: 42,
            trunk: TrunkParams::default(),
            branches: BranchParams::default(),
            split: SplitParams::default(),
            mesh: MeshParams::default(),
            foliage: FoliageParams::default(),
            growth: GrowthParams::default(),
            wind: WindResponse::default(),
            trunk_color: Color::srgb(0.545, 0.271, 0.075),
            bark: BarkParams::default(),
            style: RenderStyle::Natural,
            crown: CrownStyle::Leaves,
            trunk_style: TrunkStyle::Round,
            undergrowth: false,
        }
    }
}
