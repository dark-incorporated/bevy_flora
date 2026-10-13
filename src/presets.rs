//! Built-in tree presets.
use crate::params::*;
use bevy::color::Color;
use bevy::reflect::Reflect;

use crate::crown::CrownShape;

/// A named, ready-made [`TreeParams`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TreePreset {
    /// Broad, round-crowned deciduous tree.
    Oak,
    /// Conical conifer with a straight leader and needles.
    Pine,
    /// Drooping branches with long trailing foliage.
    Willow,
    /// Slender tree with pale bark and a light crown.
    Birch,
    /// Bare trunk with a tuft of fronds at the top.
    Palm,
    /// Tall, narrow, columnar conifer.
    Cypress,
    /// Small, compact potted-style tree.
    Bonsai,
    /// Rounded deciduous tree with lobed foliage.
    Maple,
    /// Dense, narrow conical conifer.
    Spruce,
    /// Tall, narrow deciduous tree with upswept branches.
    Poplar,
    /// Very thick, short trunk with a sparse crown.
    Baobab,
    /// Thick trunk with an umbrella-like crown.
    DragonTree,
    /// Small tree with a delicate, spreading crown.
    JapaneseMaple,
    /// Leafless, broken-branched tree.
    DeadTree,
    /// Very tall, thick-trunked conifer.
    Redwood,
    /// A low, dense, rounded bush: many thin stems from the ground under small leaves, for
    /// undergrowth, hedges and the edges of a wood.
    Shrub,
    /// A small spreading cherry with smooth reddish bark: pale pink blossom on bare branches in
    /// early spring ([`FoliageParams::blossom`]), then green leaves that turn orange-red.
    Cherry,
    /// A small, round-headed apple tree on a short trunk with grey, flaking bark: white blossom
    /// flushed pink in spring, red apples through summer ([`FoliageParams::fruit`]), dropping as
    /// the leaves turn yellow-brown.
    Apple,
}

impl TreePreset {
    /// Every preset, in display order.
    pub const ALL: [TreePreset; 18] = [
        TreePreset::Oak,
        TreePreset::Pine,
        TreePreset::Willow,
        TreePreset::Birch,
        TreePreset::Palm,
        TreePreset::Cypress,
        TreePreset::Bonsai,
        TreePreset::Maple,
        TreePreset::Spruce,
        TreePreset::Poplar,
        TreePreset::Baobab,
        TreePreset::DragonTree,
        TreePreset::JapaneseMaple,
        TreePreset::DeadTree,
        TreePreset::Redwood,
        TreePreset::Shrub,
        TreePreset::Cherry,
        TreePreset::Apple,
    ];

    /// snake_case name of the preset.
    pub fn name(self) -> &'static str {
        match self {
            TreePreset::Oak => "oak",
            TreePreset::Pine => "pine",
            TreePreset::Willow => "willow",
            TreePreset::Birch => "birch",
            TreePreset::Palm => "palm",
            TreePreset::Cypress => "cypress",
            TreePreset::Bonsai => "bonsai",
            TreePreset::Maple => "maple",
            TreePreset::Spruce => "spruce",
            TreePreset::Poplar => "poplar",
            TreePreset::Baobab => "baobab",
            TreePreset::DragonTree => "dragon_tree",
            TreePreset::JapaneseMaple => "japanese_maple",
            TreePreset::DeadTree => "dead_tree",
            TreePreset::Redwood => "redwood",
            TreePreset::Shrub => "shrub",
            TreePreset::Cherry => "cherry",
            TreePreset::Apple => "apple",
        }
    }

    /// The parameters for this preset.
    pub fn params(self) -> TreeParams {
        match self {
            TreePreset::Oak => oak(),
            TreePreset::Pine => pine(),
            TreePreset::Willow => willow(),
            TreePreset::Birch => birch(),
            TreePreset::Palm => palm(),
            TreePreset::Cypress => cypress(),
            TreePreset::Bonsai => bonsai(),
            TreePreset::Maple => maple(),
            TreePreset::Spruce => spruce(),
            TreePreset::Poplar => poplar(),
            TreePreset::Baobab => baobab(),
            TreePreset::DragonTree => dragon_tree(),
            TreePreset::JapaneseMaple => japanese_maple(),
            TreePreset::DeadTree => dead_tree(),
            TreePreset::Redwood => redwood(),
            TreePreset::Shrub => shrub(),
            TreePreset::Cherry => cherry(),
            TreePreset::Apple => apple(),
        }
    }
}

fn oak() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 0.6,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.2,
            randomness: 0.05,
            root_flare_count: 5,
            root_flare_spread: 0.6,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.1,
            leader_taper: 0.15,
            leader_has_branches: true,
            twist: 15.0,
        },
        branches: BranchParams {
            start: 0.35,
            end: 0.85,
            density: 3.0,
            length: 0.65,
            angle: 55.0,
            radius_ratio: 0.35,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.25,
            up_attraction: 0.15,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.2,
            apical_dominance: 0.3,
            flatness: 0.3,
            angle_curve: 0.0,
            crown_angle_variation: -0.2,
            twist: 10.0,
            gravity_strength: 0.3,
            stiffness: 0.6,
            break_chance: 0.05,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.9,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 0.3,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 6.0,
            cluster_size: 5,
            leaf_size: 0.33,
            leaf_size_variation: 0.2,
            radius_threshold: 0.2,
            height_falloff: 0.25,
            droop: 0.15,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.176, 0.314, 0.086),
            autumn: Some(Color::srgb(0.50, 0.24, 0.04)),
            scan: Some(LeafScan::Oak),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.365, 0.227, 0.102),
        bark: BarkParams { scan: Some(BarkScan::Oak), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.8, flutter: 1.0, sway_frequency: 1.0, flutter_frequency: 1.0 },
    }
}

fn pine() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 8.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 5,
            taper: 0.1,
            taper_curve: 0.7,
            flare: 1.1,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.2,
            leader_taper: 0.05,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.25,
            end: 0.95,
            density: 3.6,
            length: 0.45,
            angle: 70.0,
            radius_ratio: 0.25,
            taper: 0.8,
            phyllotaxis_angle: 72.0,
            randomness: 0.15,
            up_attraction: -0.1,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.8,
            flatness: 0.0,
            angle_curve: 0.3,
            crown_angle_variation: -0.3,
            twist: 5.0,
            gravity_strength: 0.05,
            stiffness: 0.8,
            break_chance: 0.0,
            crown_shape: CrownShape::Conical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 5.0,
            cluster_size: 6,
            leaf_size: 0.25,
            leaf_size_variation: 0.1,
            radius_threshold: 0.25,
            height_falloff: 0.2,
            droop: 0.05,
            rotation_variation: 0.3,
            use_crown_density: true,
            color: Color::srgb(0.106, 0.302, 0.243),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.420, 0.267, 0.137),
        bark: BarkParams { scan: Some(BarkScan::Pine), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.7, flutter: 0.5, sway_frequency: 0.8, flutter_frequency: 1.4 },
    }
}

fn willow() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.4,
            taper_curve: 0.4,
            flare: 1.0,
            randomness: 0.1,
            root_flare_count: 4,
            root_flare_spread: 0.4,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 10.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 1.0,
            density: 4.0,
            length: 0.7,
            angle: 50.0,
            radius_ratio: 0.2,
            taper: 0.85,
            phyllotaxis_angle: 137.5,
            randomness: 0.3,
            up_attraction: -0.5,
            recursion: 3,
            sub_branch_count: 2,
            sub_branch_scale: 0.9,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.2,
            flatness: 0.2,
            angle_curve: -0.4,
            crown_angle_variation: -0.3,
            twist: 15.0,
            gravity_strength: 2.5,
            stiffness: 0.1,
            break_chance: 0.0,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.8,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.003,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::SingleQuad,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 10.0,
            cluster_size: 4,
            leaf_size: 0.26,
            leaf_size_variation: 0.25,
            radius_threshold: 0.15,
            height_falloff: 0.4,
            droop: 0.6,
            rotation_variation: 0.8,
            use_crown_density: true,
            color: Color::srgb(0.565, 0.690, 0.376),
            autumn: Some(Color::srgb(0.74, 0.62, 0.10)),
            scan: Some(LeafScan::Willow),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.478, 0.361, 0.239),
        bark: BarkParams { scan: Some(BarkScan::Willow), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.4, flutter: 1.6, sway_frequency: 0.7, flutter_frequency: 0.8 },
    }
}

fn birch() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 7.0,
            radius: 0.3,
            radial_segments: 6,
            height_segments: 5,
            taper: 0.2,
            taper_curve: 0.5,
            flare: 1.0,
            randomness: 0.15,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.4,
            end: 0.95,
            density: 3.0,
            length: 0.35,
            angle: 40.0,
            radius_ratio: 0.2,
            taper: 0.75,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.3,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.5,
            length_variation: 0.15,
            sub_branch_position_bias: 0.1,
            apical_dominance: 0.55,
            flatness: 0.1,
            angle_curve: 0.2,
            crown_angle_variation: 0.1,
            twist: 8.0,
            gravity_strength: 0.1,
            stiffness: 0.7,
            break_chance: 0.1,
            crown_shape: CrownShape::TaperedCylindrical,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 0.2,
            angle: 25.0,
            position: 0.5,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RandomUpward,
            density: 3.5,
            cluster_size: 4,
            leaf_size: 0.26,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.35,
            droop: 0.1,
            rotation_variation: 0.5,
            use_crown_density: true,
            color: Color::srgb(0.486, 0.804, 0.486),
            autumn: Some(Color::srgb(0.95, 0.70, 0.08)),
            scan: Some(LeafScan::Birch),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.961, 0.961, 0.863),
        bark: BarkParams { scan: Some(BarkScan::Birch), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.2, flutter: 1.3, sway_frequency: 1.1, flutter_frequency: 1.3 },
    }
}

fn palm() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.35,
            taper_curve: 0.3,
            flare: 1.3,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.85,
            end: 0.98,
            density: 8.0,
            length: 0.6,
            angle: 60.0,
            radius_ratio: 0.28,
            taper: 0.9,
            phyllotaxis_angle: 45.0,
            randomness: 0.1,
            up_attraction: -0.2,
            recursion: 0,
            sub_branch_count: 0,
            sub_branch_scale: 0.5,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.9,
            flatness: 0.0,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 0.0,
            gravity_strength: 0.15,
            stiffness: 0.5,
            break_chance: 0.0,
            crown_shape: CrownShape::Cylindrical,
            crown_influence: 0.5,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::SingleQuad,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 1.5,
            cluster_size: 2,
            leaf_size: 1.05,
            leaf_size_variation: 0.2,
            radius_threshold: 0.3,
            height_falloff: 0.1,
            droop: 0.4,
            rotation_variation: 0.2,
            use_crown_density: false,
            color: Color::srgb(0.133, 0.545, 0.133),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.545, 0.451, 0.333),
        bark: BarkParams { scan: Some(BarkScan::Palm), smooth_below: 0.0, fissured_above: 0.05 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.5, flutter: 1.2, sway_frequency: 0.6, flutter_frequency: 0.9 },
    }
}

fn cypress() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 10.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.15,
            taper_curve: 0.6,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::PointedTip,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.1,
            end: 0.95,
            density: 3.0,
            length: 0.25,
            angle: 18.0,
            radius_ratio: 0.28,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.1,
            up_attraction: 0.85,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.4,
            length_variation: 0.05,
            sub_branch_position_bias: -0.2,
            apical_dominance: 0.9,
            flatness: 0.0,
            angle_curve: 0.5,
            crown_angle_variation: 0.4,
            twist: 3.0,
            gravity_strength: 0.0,
            stiffness: 0.9,
            break_chance: 0.0,
            crown_shape: CrownShape::Flame,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::ClusterSphere,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 4.0,
            cluster_size: 8,
            leaf_size: 0.15,
            leaf_size_variation: 0.1,
            radius_threshold: 0.18,
            height_falloff: 0.15,
            droop: 0.05,
            rotation_variation: 0.4,
            use_crown_density: true,
            color: Color::srgb(0.208, 0.369, 0.231),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.290, 0.235, 0.165),
        bark: BarkParams { scan: Some(BarkScan::Cedar), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.6, flutter: 0.6, sway_frequency: 1.0, flutter_frequency: 1.2 },
    }
}

fn bonsai() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 2.0,
            radius: 0.25,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.25,
            taper_curve: 0.4,
            flare: 1.4,
            randomness: 0.25,
            root_flare_count: 3,
            root_flare_spread: 0.8,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 30.0,
        },
        branches: BranchParams {
            start: 0.3,
            end: 0.8,
            density: 2.5,
            length: 0.5,
            angle: 60.0,
            radius_ratio: 0.3,
            taper: 0.6,
            phyllotaxis_angle: 90.0,
            randomness: 0.5,
            up_attraction: 0.0,
            recursion: 2,
            sub_branch_count: 2,
            sub_branch_scale: 0.6,
            length_variation: 0.3,
            sub_branch_position_bias: 0.4,
            apical_dominance: 0.4,
            flatness: 0.4,
            angle_curve: -0.2,
            crown_angle_variation: -0.1,
            twist: 20.0,
            gravity_strength: 0.15,
            stiffness: 0.4,
            break_chance: 0.1,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.6,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.0025,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.4,
            angle: 35.0,
            position: 0.5,
            radius_threshold: 0.06,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::StarBurst,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::HorizontalSpread,
            density: 2.5,
            cluster_size: 6,
            leaf_size: 0.33,
            leaf_size_variation: 0.2,
            radius_threshold: 0.2,
            height_falloff: 0.2,
            droop: 0.15,
            rotation_variation: 0.7,
            use_crown_density: true,
            color: Color::srgb(0.208, 0.369, 0.231),
            autumn: Some(Color::srgb(0.70, 0.22, 0.03)),
            scan: Some(LeafScan::Bonsai),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.361, 0.251, 0.200),
        bark: BarkParams { scan: Some(BarkScan::Zelkova), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.2, flutter: 0.6, sway_frequency: 1.5, flutter_frequency: 1.2 },
    }
}

fn maple() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 8.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.35,
            taper_curve: 0.5,
            flare: 1.15,
            randomness: 0.05,
            root_flare_count: 4,
            root_flare_spread: 0.5,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.1,
            leader_taper: 0.12,
            leader_has_branches: true,
            twist: 10.0,
        },
        branches: BranchParams {
            start: 0.35,
            end: 0.9,
            density: 3.0,
            length: 0.55,
            angle: 52.0,
            radius_ratio: 0.32,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.1,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.15,
            apical_dominance: 0.45,
            flatness: 0.25,
            angle_curve: 0.0,
            crown_angle_variation: -0.15,
            twist: 8.0,
            gravity_strength: 0.25,
            stiffness: 0.55,
            break_chance: 0.05,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.85,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 0.25,
            angle: 28.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 4.5,
            cluster_size: 5,
            leaf_size: 0.39,
            leaf_size_variation: 0.2,
            radius_threshold: 0.18,
            height_falloff: 0.25,
            droop: 0.12,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.133, 0.322, 0.133),
            autumn: Some(Color::srgb(0.88, 0.25, 0.02)),
            scan: Some(LeafScan::Maple),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.32, 0.21, 0.12),
        bark: BarkParams { scan: Some(BarkScan::Maple), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.9, flutter: 1.0, sway_frequency: 1.0, flutter_frequency: 1.0 },
    }
}

fn spruce() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 10.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.12,
            taper_curve: 0.8,
            flare: 1.05,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.25,
            leader_taper: 0.03,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.12,
            end: 0.95,
            density: 4.4,
            length: 0.5,
            angle: 78.0,
            radius_ratio: 0.22,
            taper: 0.82,
            phyllotaxis_angle: 72.0,
            randomness: 0.1,
            up_attraction: -0.15,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.35,
            length_variation: 0.08,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.88,
            flatness: 0.0,
            angle_curve: 0.35,
            crown_angle_variation: -0.3,
            twist: 3.0,
            gravity_strength: 0.08,
            stiffness: 0.75,
            break_chance: 0.0,
            crown_shape: CrownShape::Conical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 5.5,
            cluster_size: 7,
            leaf_size: 0.22,
            leaf_size_variation: 0.08,
            radius_threshold: 0.22,
            height_falloff: 0.18,
            droop: 0.08,
            rotation_variation: 0.25,
            use_crown_density: true,
            color: Color::srgb(0.086, 0.278, 0.212),
            ..Default::default()
        },
        growth: GrowthParams {
            trunk_fraction: 0.6,
            branch_fraction: 0.6,
            thickening_lag: 1.2,
            leaf_threshold: 0.15,
            ..Default::default()
        },
        trunk_color: Color::srgb(0.380, 0.243, 0.141),
        bark: BarkParams { scan: Some(BarkScan::KnottedPine), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.7, flutter: 0.5, sway_frequency: 0.8, flutter_frequency: 1.4 },
    }
}

fn poplar() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 12.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 7,
            taper: 0.12,
            taper_curve: 0.7,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.12,
            leader_taper: 0.05,
            leader_has_branches: true,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.12,
            end: 0.95,
            density: 2.5,
            length: 0.35,
            angle: 25.0,
            radius_ratio: 0.2,
            taper: 0.75,
            phyllotaxis_angle: 137.5,
            randomness: 0.05,
            up_attraction: 0.6,
            recursion: 1,
            sub_branch_count: 4,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: -0.1,
            apical_dominance: 0.92,
            flatness: 0.0,
            angle_curve: 0.4,
            crown_angle_variation: 0.3,
            twist: 2.0,
            gravity_strength: 0.0,
            stiffness: 0.9,
            break_chance: 0.0,
            crown_shape: CrownShape::Flame,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 20.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RandomUpward,
            density: 10.0,
            cluster_size: 4,
            leaf_size: 0.29,
            leaf_size_variation: 0.12,
            radius_threshold: 0.15,
            height_falloff: 0.3,
            droop: 0.05,
            rotation_variation: 0.45,
            use_crown_density: true,
            color: Color::srgb(0.404, 0.545, 0.306),
            autumn: Some(Color::srgb(0.92, 0.72, 0.05)),
            scan: Some(LeafScan::Poplar),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.553, 0.514, 0.435),
        bark: BarkParams { scan: Some(BarkScan::Sakura), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.1, flutter: 1.8, sway_frequency: 1.1, flutter_frequency: 1.8 },
    }
}

fn baobab() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 1.5,
            radial_segments: 12,
            height_segments: 6,
            taper: 0.5,
            taper_curve: 0.3,
            flare: 1.6,
            randomness: 0.08,
            root_flare_count: 6,
            root_flare_spread: 0.7,
            root_flare_height: 0.2,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.2,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.75,
            end: 0.95,
            density: 3.0,
            length: 0.45,
            angle: 55.0,
            radius_ratio: 0.35,
            taper: 0.6,
            phyllotaxis_angle: 137.5,
            randomness: 0.35,
            up_attraction: 0.2,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.5,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.25,
            flatness: 0.3,
            angle_curve: -0.2,
            crown_angle_variation: -0.2,
            twist: 12.0,
            gravity_strength: 0.15,
            stiffness: 0.5,
            break_chance: 0.1,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 0.3,
            angle: 35.0,
            position: 0.6,
            radius_threshold: 0.15,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.8,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::ClusterSphere,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 2.0,
            cluster_size: 5,
            leaf_size: 0.4,
            leaf_size_variation: 0.25,
            radius_threshold: 0.25,
            height_falloff: 0.15,
            droop: 0.2,
            rotation_variation: 0.5,
            use_crown_density: false,
            color: Color::srgb(0.306, 0.463, 0.235),
            autumn: Some(Color::srgb(0.62, 0.48, 0.08)),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.506, 0.424, 0.369),
        bark: BarkParams { scan: Some(BarkScan::PalmTree), smooth_below: 0.20, fissured_above: 0.80 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.3, flutter: 0.8, sway_frequency: 0.8, flutter_frequency: 1.0 },
    }
}

fn dragon_tree() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.1,
            randomness: 0.05,
            root_flare_count: 3,
            root_flare_spread: 0.4,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.15,
            leader_has_branches: false,
            twist: 8.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 0.9,
            density: 1.0,
            length: 0.4,
            angle: 45.0,
            radius_ratio: 0.3,
            taper: 0.5,
            phyllotaxis_angle: 180.0,
            randomness: 0.15,
            up_attraction: 0.25,
            recursion: 3,
            sub_branch_count: 2,
            sub_branch_scale: 0.7,
            length_variation: 0.15,
            sub_branch_position_bias: 0.5,
            apical_dominance: 0.35,
            flatness: 0.15,
            angle_curve: 0.1,
            crown_angle_variation: 0.0,
            twist: 5.0,
            gravity_strength: 0.1,
            stiffness: 0.65,
            break_chance: 0.0,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.6,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 1.0,
            angle: 40.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.6,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::StarBurst,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 3.0,
            cluster_size: 8,
            leaf_size: 0.46,
            leaf_size_variation: 0.15,
            radius_threshold: 0.2,
            height_falloff: 0.1,
            droop: 0.1,
            rotation_variation: 0.4,
            use_crown_density: false,
            color: Color::srgb(0.286, 0.443, 0.255),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.463, 0.396, 0.322),
        bark: BarkParams { scan: Some(BarkScan::Hackberry), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.3, flutter: 0.7, sway_frequency: 0.9, flutter_frequency: 0.9 },
    }
}

fn japanese_maple() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 3.5,
            radius: 0.15,
            radial_segments: 6,
            height_segments: 12,
            taper: 0.25,
            taper_curve: 0.4,
            flare: 1.1,
            randomness: 0.15,
            root_flare_count: 3,
            root_flare_spread: 0.5,
            root_flare_height: 0.1,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.08,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 15.0,
        },
        branches: BranchParams {
            start: 0.25,
            end: 0.85,
            density: 2.5,
            length: 0.45,
            angle: 65.0,
            radius_ratio: 0.25,
            taper: 0.7,
            phyllotaxis_angle: 180.0,
            randomness: 0.25,
            up_attraction: -0.15,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.2,
            apical_dominance: 0.35,
            flatness: 0.65,
            angle_curve: -0.3,
            crown_angle_variation: -0.25,
            twist: 12.0,
            gravity_strength: 0.2,
            stiffness: 0.45,
            break_chance: 0.05,
            crown_shape: CrownShape::Spreading,
            crown_influence: 0.8,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.0025,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.2,
            angle: 30.0,
            position: 0.4,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.4,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::HorizontalSpread,
            density: 4.0,
            cluster_size: 4,
            leaf_size: 0.23,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.35,
            droop: 0.18,
            rotation_variation: 0.65,
            use_crown_density: true,
            color: Color::srgb(0.698, 0.133, 0.133),
            autumn: Some(Color::srgb(0.45, 0.02, 0.04)),
            scan: Some(LeafScan::JapaneseMaple),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.376, 0.282, 0.212),
        bark: BarkParams { scan: Some(BarkScan::Maple), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.0, flutter: 1.2, sway_frequency: 1.1, flutter_frequency: 1.1 },
    }
}

fn cherry() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 2.0,
            radius: 0.22,
            radial_segments: 8,
            height_segments: 10,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.15,
            randomness: 0.12,
            root_flare_count: 4,
            root_flare_spread: 0.4,
            root_flare_height: 0.08,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.1,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 10.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 1.0,
            density: 5.0,
            length: 1.4,
            angle: 40.0,
            radius_ratio: 0.35,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.25,
            up_attraction: 0.25,
            recursion: 2,
            sub_branch_count: 5,
            sub_branch_scale: 0.55,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.3,
            flatness: 0.3,
            angle_curve: -0.2,
            crown_angle_variation: 0.0,
            twist: 8.0,
            gravity_strength: 0.15,
            stiffness: 0.5,
            break_chance: 0.03,
            crown_shape: CrownShape::Spreading,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.003,
        },
        split: SplitParams { enabled: true, probability: 0.5, angle: 28.0, position: 0.45, radius_threshold: 0.06 },
        mesh: MeshParams { collar_enabled: true, collar_length: 1.4, ..Default::default() },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 7.0,
            cluster_size: 4,
            leaf_size: 0.2,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.3,
            droop: 0.15,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.20, 0.40, 0.12),
            autumn: Some(Color::srgb(0.86, 0.36, 0.10)),
            blossom: Some(Color::srgb(0.98, 0.72, 0.82)),
            scan: Some(LeafScan::Birch),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.33, 0.20, 0.16),
        bark: BarkParams { scan: Some(BarkScan::Sakura), smooth_below: 0.02, fissured_above: 0.12 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.9, flutter: 1.1, sway_frequency: 1.0, flutter_frequency: 1.0 },
    }
}

fn apple() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 1.6,
            radius: 0.25,
            radial_segments: 8,
            height_segments: 10,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.15,
            randomness: 0.15,
            root_flare_count: 4,
            root_flare_spread: 0.4,
            root_flare_height: 0.08,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.1,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 12.0,
        },
        branches: BranchParams {
            start: 0.6,
            end: 1.0,
            density: 8.0,
            length: 1.8,
            angle: 55.0,
            radius_ratio: 0.5,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.3,
            up_attraction: 0.35,
            recursion: 2,
            sub_branch_count: 6,
            sub_branch_scale: 0.6,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.25,
            flatness: 0.2,
            angle_curve: -0.2,
            crown_angle_variation: 0.0,
            twist: 10.0,
            gravity_strength: 0.2,
            stiffness: 0.5,
            break_chance: 0.03,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.003,
        },
        split: SplitParams { enabled: false, probability: 0.0, angle: 30.0, position: 0.45, radius_threshold: 0.06 },
        mesh: MeshParams { collar_enabled: true, collar_length: 1.4, ..Default::default() },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 7.0,
            cluster_size: 4,
            leaf_size: 0.22,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.3,
            droop: 0.2,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.22, 0.42, 0.14),
            autumn: Some(Color::srgb(0.72, 0.58, 0.18)),
            blossom: Some(Color::srgb(0.98, 0.88, 0.90)),
            fruit: Some(FruitParams::default()),
            scan: Some(LeafScan::Birch),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.36, 0.32, 0.28),
        bark: BarkParams { scan: Some(BarkScan::Zelkova), smooth_below: 0.02, fissured_above: 0.12 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.8, flutter: 1.0, sway_frequency: 1.0, flutter_frequency: 1.0 },
    }
}

fn dead_tree() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.35,
            taper_curve: 0.5,
            flare: 1.2,
            randomness: 0.2,
            root_flare_count: 4,
            root_flare_spread: 0.6,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.2,
            leader_has_branches: false,
            twist: 40.0,
        },
        branches: BranchParams {
            start: 0.2,
            end: 0.7,
            density: 0.8,
            length: 0.4,
            angle: 50.0,
            radius_ratio: 0.35,
            taper: 0.6,
            phyllotaxis_angle: 137.5,
            randomness: 0.45,
            up_attraction: 0.0,
            recursion: 2,
            sub_branch_count: 2,
            sub_branch_scale: 0.5,
            length_variation: 0.35,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.3,
            flatness: 0.2,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 25.0,
            gravity_strength: 0.4,
            stiffness: 0.35,
            break_chance: 0.35,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.4,
            floor_avoidance: false,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: true,
            probability: 0.25,
            angle: 40.0,
            position: 0.4,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.6,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: false,
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.388, 0.361, 0.325),
        bark: BarkParams { scan: Some(BarkScan::Brown), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.6, flutter: 0.0, sway_frequency: 1.2, flutter_frequency: 1.0 },
    }
}

fn redwood() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 20.0,
            radius: 1.0,
            radial_segments: 12,
            height_segments: 10,
            taper: 0.05,
            taper_curve: 0.8,
            flare: 1.3,
            randomness: 0.02,
            root_flare_count: 6,
            root_flare_spread: 0.5,
            root_flare_height: 0.2,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.15,
            leader_taper: 0.05,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 0.95,
            density: 2.4,
            length: 0.35,
            angle: 75.0,
            radius_ratio: 0.25,
            taper: 0.8,
            phyllotaxis_angle: 137.5,
            randomness: 0.1,
            up_attraction: -0.1,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.85,
            flatness: 0.0,
            angle_curve: 0.25,
            crown_angle_variation: 0.2,
            twist: 3.0,
            gravity_strength: 0.15,
            stiffness: 0.85,
            break_chance: 0.02,
            crown_shape: CrownShape::Conical,
            crown_influence: 0.9,
            floor_avoidance: true,
            floor_level: 0.0,
            ..Default::default()
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 3.0,
            cluster_size: 5,
            leaf_size: 0.18,
            leaf_size_variation: 0.1,
            radius_threshold: 0.15,
            height_falloff: 0.25,
            droop: 0.1,
            rotation_variation: 0.3,
            use_crown_density: true,
            color: Color::srgb(0.12, 0.32, 0.21),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.463, 0.263, 0.161),
        bark: BarkParams { scan: Some(BarkScan::Metasequoia), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 0.5, flutter: 0.6, sway_frequency: 0.6, flutter_frequency: 1.0 },
    }
}

/// A dozen stems from a stub of a trunk 0.5 m tall (the leader is one more), leaning out and
/// arching down under gravity, the longest at 40% of the height (`Spreading`), each with a few
/// side shoots and every one in leaves: a mound about 1.4 m high and 1.9 m across.
fn shrub() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 0.5,
            radius: 0.07,
            radial_segments: 8,
            height_segments: 4,
            taper: 0.6,
            taper_curve: 0.5,
            flare: 1.15,
            randomness: 0.15,
            root_flare_count: 0,
            root_flare_spread: 0.5,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 1.6,
            leader_taper: 0.15,
            leader_has_branches: true,
            twist: 0.0,
        },
        branches: BranchParams {
            // Low enough for leaves near the ground, high enough that the lowest stems' roots,
            // buried in the trunk, stay above it.
            start: 0.22,
            end: 1.0,
            density: 30.0,
            length: 2.2,
            angle: 55.0,
            radius_ratio: 0.4,
            taper: 0.75,
            phyllotaxis_angle: 137.5,
            randomness: 0.4,
            up_attraction: 0.1,
            recursion: 1,
            sub_branch_count: 4,
            sub_branch_scale: 0.55,
            length_variation: 0.3,
            sub_branch_position_bias: 0.2,
            apical_dominance: 0.0,
            flatness: 0.0,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 0.0,
            gravity_strength: 0.5,
            stiffness: 0.4,
            break_chance: 0.0,
            crown_shape: CrownShape::Spreading,
            crown_influence: 0.5,
            floor_avoidance: true,
            floor_level: 0.0,
            min_twig_radius: 0.003,
        },
        split: SplitParams::default(),
        mesh: MeshParams::default(),
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 18.0,
            cluster_size: 4,
            leaf_size: 0.15,
            leaf_size_variation: 0.25,
            radius_threshold: 0.06,
            height_falloff: 0.0,
            droop: 0.2,
            rotation_variation: 0.5,
            use_crown_density: true,
            color: Color::srgb(0.16, 0.33, 0.12),
            autumn: Some(Color::srgb(0.70, 0.25, 0.05)),
            scan: Some(LeafScan::Poplar),
            ..Default::default()
        },
        growth: GrowthParams { maturity_age: 6.0, ..Default::default() },
        trunk_color: Color::srgb(0.30, 0.24, 0.18),
        bark: BarkParams { scan: Some(BarkScan::Brown), smooth_below: 0.03, fissured_above: 0.15 },
        style: RenderStyle::Natural,
        crown: CrownStyle::Leaves,
        trunk_style: TrunkStyle::Round,
        wind: WindResponse { sway: 1.2, flutter: 1.2, sway_frequency: 1.0, flutter_frequency: 1.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oak_values() {
        let oak = TreePreset::Oak.params();
        assert_eq!(oak.branches.crown_shape, CrownShape::Spherical);
        assert!(oak.branches.angle > 45.0);
    }

    #[test]
    fn pine_values() {
        let pine = TreePreset::Pine.params();
        assert_eq!(pine.branches.crown_shape, CrownShape::Conical);
        assert!(pine.branches.up_attraction < 0.0);
    }

    #[test]
    fn willow_values() {
        let willow = TreePreset::Willow.params();
        assert_eq!(willow.branches.crown_shape, CrownShape::Hemispherical);
        assert!(willow.branches.up_attraction < -0.3);
    }

    #[test]
    fn willow_branches_hang_at_their_tips() {
        // D-030: the willow spread sideways and barely weeped; only 46 of its
        // 200 branches ended within 45° of straight down.
        let s = crate::skeleton::generate_skeleton(&TreePreset::Willow.params());
        let tips: Vec<f32> = s
            .nodes
            .iter()
            .skip(1)
            .filter(|n| n.kind != crate::skeleton::NodeKind::Leader)
            .map(|n| n.axis(n.axis_points()).last().unwrap().1.y)
            .collect();
        let hanging = tips.iter().filter(|&&y| y < -0.7).count();
        assert!(hanging * 10 >= tips.len() * 7, "{hanging} of {} branch tips hang", tips.len());
    }

    #[test]
    fn willow_crown_reaches_the_top_of_the_trunk() {
        // D-049: the limbs stopped at 90% of the trunk, so the crown sat low
        // under a bare top.
        let params = TreePreset::Willow.params();
        let s = crate::skeleton::generate_skeleton(&params);
        let top = params.trunk.height;
        let highest = s
            .nodes
            .iter()
            .filter(|n| n.kind == crate::skeleton::NodeKind::Primary)
            .map(|n| n.start.y)
            .fold(f32::MIN, f32::max);
        assert!(highest >= 0.93 * top, "highest limb leaves the trunk at {highest} m of {top} m");
    }

    #[test]
    fn willow_leaves_hang_along_their_twigs() {
        // D-050: leaves pointed randomly upward on a thick tangle of 400
        // branches, so the crown was a leafy mound; they now follow thin,
        // long twigs that hang, so they are strung in strands.
        let params = TreePreset::Willow.params();
        let tree = crate::pipeline::build_tree(&params, f32::INFINITY);
        let leaves = crate::foliage::collect_leaf_points(
            &tree.skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        );
        let hanging = leaves.iter().filter(|l| l.direction.y < -0.5).count();
        assert!(hanging * 10 >= leaves.len() * 6, "{hanging} of {} leaves hang", leaves.len());
    }

    #[test]
    fn young_spruce_is_a_sapling_not_a_pole_of_wires() {
        // D-031: at age 2 the spruce was 5 m tall with bare, hair-thin primaries
        // up to 41% of its height (length / base radius up to 144).
        let params = TreePreset::Spruce.params();
        let tree = crate::pipeline::build_tree(&params, 2.0);
        let height = tree.skeleton.nodes[0].length;
        let primaries: Vec<_> = tree
            .skeleton
            .nodes
            .iter()
            .filter(|n| n.kind == crate::skeleton::NodeKind::Primary)
            .collect();
        assert!(primaries.len() >= 5, "only {} primaries at age 2", primaries.len());
        for n in &primaries {
            assert!(n.length < 0.35 * height, "primary {} is {} m on a {height} m sapling", n.id, n.length);
            assert!(n.length < 110.0 * n.base_radius, "primary {} is {} m long and {} m thick", n.id, n.length, n.base_radius);
        }
    }

    #[test]
    fn palm_values() {
        let palm = TreePreset::Palm.params();
        assert!(palm.branches.start > 0.8);
        assert_eq!(palm.branches.recursion, 0);
    }

    #[test]
    fn cypress_values() {
        let cypress = TreePreset::Cypress.params();
        assert_eq!(cypress.branches.crown_shape, CrownShape::Flame);
        assert!(cypress.branches.up_attraction > 0.5);
        assert!(cypress.branches.angle < 30.0);
    }

    #[test]
    fn spruce_whorled_branching() {
        let spruce = TreePreset::Spruce.params();
        assert_eq!(spruce.branches.phyllotaxis_angle, 72.0);
        assert!(spruce.branches.angle > 70.0);
        assert!(spruce.branches.apical_dominance > 0.85);
    }

    #[test]
    fn poplar_is_a_leafy_column() {
        // D-048: short branches hugging the trunk left a 14 m pole with 71 leaves, and a
        // 2.4 m bare leader stuck out above the crown like an antenna.
        let params = TreePreset::Poplar.params();
        let tree = crate::pipeline::build_tree(&params, f32::INFINITY);
        let points = crate::foliage::collect_leaf_points(
            &tree.skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        );
        assert!(points.len() >= 400, "{} leaves", points.len());
        let tip = tree.trunk.positions.iter().fold(f32::MIN, |hi, p| hi.max(p.y));
        let crowned = points.iter().filter(|p| p.position.y > 0.85 * tip).count();
        assert!(crowned >= 25, "only {crowned} leaves in the top 15% of the tree");
        let (min, max) = tree
            .foliage
            .positions
            .iter()
            .fold((bevy::math::Vec3::MAX, bevy::math::Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let width = (max.x - min.x).max(max.z - min.z);
        assert!(width < 0.3 * tip, "crown {width} m wide on a {tip} m tree");
    }

    #[test]
    fn baobab_bottle_trunk() {
        let baobab = TreePreset::Baobab.params();
        assert!(baobab.trunk.radius > 1.0);
        assert!(baobab.branches.start > 0.7);
    }

    #[test]
    fn dragon_tree_forking() {
        let dragon = TreePreset::DragonTree.params();
        assert_eq!(dragon.split.probability, 1.0);
        assert_eq!(dragon.branches.sub_branch_count, 2);
    }

    #[test]
    fn japanese_maple_ornamental() {
        let jmaple = TreePreset::JapaneseMaple.params();
        assert!(jmaple.branches.flatness > 0.5);
        assert_eq!(jmaple.branches.phyllotaxis_angle, 180.0);
        assert_eq!(jmaple.branches.crown_shape, CrownShape::Spreading);
    }

    #[test]
    fn dead_tree_no_foliage() {
        let dead = TreePreset::DeadTree.params();
        assert!(!dead.foliage.enabled);
        assert!(dead.branches.break_chance > 0.3);
        assert!(dead.trunk.twist > 30.0);
    }

    #[test]
    fn redwood_massive() {
        let redwood = TreePreset::Redwood.params();
        assert!(redwood.trunk.height >= 20.0);
        assert!(redwood.trunk.radius >= 1.0);
        assert!(redwood.trunk.taper < 0.15);
    }

    #[test]
    fn foliage_preset_oak() {
        let oak = TreePreset::Oak.params().foliage;
        assert_eq!(oak.style, LeafStyle::CrossedPlanes);
        assert_eq!(oak.placement, FoliagePlacement::AllBranches);
        assert!(oak.density > 3.0);
    }

    #[test]
    fn foliage_preset_pine() {
        let pine = TreePreset::Pine.params().foliage;
        assert_eq!(pine.style, LeafStyle::NeedleCluster);
        assert_eq!(pine.orientation, LeafOrientation::FollowBranch);
    }

    #[test]
    fn foliage_preset_palm() {
        let palm = TreePreset::Palm.params().foliage;
        assert_eq!(palm.placement, FoliagePlacement::TerminalBranches);
        assert!(palm.leaf_size > 0.5);
        assert!(palm.density < 2.0);
    }

    #[test]
    fn all_presets_have_distinct_names() {
        let mut names: Vec<_> = TreePreset::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TreePreset::ALL.len());
    }
}
