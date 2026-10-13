//! Scripted gallery renderer for visual QA.
//!
//! Renders every scenario (presets, growth ages, seeds, parameter toggles,
//! leaf styles, foliage placements) to one PNG with four views of the tree
//! (front, side, top, junction close-up) and appends geometry statistics and
//! sanity flags to `stats.tsv`.
//!
//! ```text
//! cargo run --example gallery -- [--out <dir>] [--filter <substr>]... [--no-shadows] [--gpu]
//!     [--warmup-secs <s>]
//! ```
//!
//! `--out` defaults to `target/gallery/latest`. `--filter` keeps scenarios whose
//! name contains the substring; repeat it to match any of several. `--no-shadows`
//! disables shadow maps, to tell shadows from geometry defects. `--gpu` grows
//! every tree in the vertex shader (`GpuGrowth`); compare its PNGs with a CPU
//! render of the same scenarios using the `compare_gallery` example.
//! `--warmup-secs <s>` renders for at least that long before the first capture:
//! shaders compile in the background, and on a slow (software) renderer the first
//! capture would otherwise come out empty.
//!
//! The wind is off (`FloraWind` strength 0), so captures repeat. The views render into an
//! offscreen image, so captures do not depend on the
//! window being visible (the window itself stays blank). A capture that comes
//! back as a single flat color is flagged `BLANK_CAPTURE`.

use std::f32::consts::PI;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::camera::{RenderTarget, Viewport};
use bevy::image::TextureFormatPixelInfo;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::ui::UiTargetCamera;
use bevy::window::WindowResolution;
use bevy_flora::{
    CrownShape, CrownStyle, FloraPlugin, FloraSeason, FloraWind, FoliagePlacement, GpuGrowth, GpuGrowthSupport,
    LeafOrientation, LeafStyle, MeshData, NodeKind, ProceduralTree, RenderStyle, TreeAge, TreeMeshes, TreeParams,
    TreePreset, TrunkStyle, TrunkTermination, build_tree,
};

const WINDOW: UVec2 = UVec2::new(1280, 960);
const VIEW: UVec2 = UVec2::new(640, 480);
const WARMUP_FRAMES: u32 = 10;
const SETTLE_FRAMES: u32 = 5;
const CAPTURE_TIMEOUT_FRAMES: u32 = 120;
const TSV_HEADER: &str = "name\tage\tseed\tnodes\ttrunk_verts\ttrunk_tris\tfoliage_verts\tfoliage_tris\tbbox_min\tbbox_max\tflags";

/// Mutation applied to the oak preset by a toggle scenario.
type Edit = fn(&mut TreeParams);

struct Scenario {
    name: String,
    params: TreeParams,
    age: f32,
    /// The season the scenario is rendered in; summer unless it is one of the season scenarios.
    season: FloraSeason,
}

fn preset(preset: TreePreset) -> TreeParams {
    let mut params = preset.params();
    params.seed = 42;
    params
}

fn scenarios() -> Vec<Scenario> {
    let inf = f32::INFINITY;
    let mut list = Vec::new();
    let mut add = |name: String, params: TreeParams, age: f32| {
        list.push(Scenario { name, params, age, season: FloraSeason::SUMMER });
    };

    for p in TreePreset::ALL {
        add(format!("mature_{}", p.name()), preset(p), inf);
    }

    for p in [TreePreset::Oak, TreePreset::Pine] {
        for age in [0.0, 0.5, 2.0, 4.0, 6.0, 8.0, inf] {
            add(format!("grow_{}_{age}", p.name()), preset(p), age);
        }
    }

    for p in [TreePreset::Oak, TreePreset::Maple] {
        for seed in 1..=3 {
            let mut params = preset(p);
            params.seed = seed;
            add(format!("seed_{}_{seed}", p.name()), params, inf);
        }
    }

    let toggles: [(&str, Edit); 12] = [
        ("oak_split", |p| {
            p.split.enabled = true;
            p.split.probability = 1.0;
            p.split.radius_threshold = 0.0;
        }),
        ("oak_pipe", |p| p.mesh.pipe_radius_enabled = true),
        ("oak_smooth", |p| p.mesh.smooth_iterations = 2),
        ("oak_adaptive", |p| p.mesh.adaptive_resolution = true),
        ("oak_gravity", |p| {
            p.branches.gravity_strength = 0.8;
            p.branches.stiffness = 0.1;
        }),
        ("oak_wobble", |p| p.trunk.randomness = 0.4),
        ("oak_roots", |p| {
            p.trunk.root_flare_count = 6;
            p.trunk.root_flare_spread = 1.0;
        }),
        ("oak_flatcap", |p| p.trunk.termination = TrunkTermination::FlatCap),
        ("oak_pointed", |p| p.trunk.termination = TrunkTermination::PointedTip),
        ("oak_leader", |p| p.trunk.termination = TrunkTermination::LeaderBranch),
        ("oak_twist", |p| {
            p.trunk.twist = 180.0;
            p.branches.twist = 90.0;
        }),
        ("oak_nocollar", |p| p.mesh.collar_enabled = false),
    ];
    for (name, edit) in toggles {
        let mut params = preset(TreePreset::Oak);
        edit(&mut params);
        add(name.to_string(), params, inf);
    }

    for style in [
        LeafStyle::CrossedPlanes,
        LeafStyle::SingleQuad,
        LeafStyle::ClusterSphere,
        LeafStyle::StarBurst,
        LeafStyle::NeedleCluster,
    ] {
        let mut params = preset(TreePreset::Oak);
        params.foliage.style = style;
        add(format!("leaf_{style:?}"), params, inf);
    }

    for placement in [
        FoliagePlacement::TerminalBranches,
        FoliagePlacement::AllBranches,
        FoliagePlacement::TipClusters,
    ] {
        let mut params = preset(TreePreset::Oak);
        params.foliage.placement = placement;
        add(format!("place_{placement:?}"), params, inf);
    }

    // Wider coverage. Every other preset while growing and with more seeds.
    let others = |skip: [TreePreset; 2]| TreePreset::ALL.into_iter().filter(move |p| !skip.contains(p));
    for p in others([TreePreset::Oak, TreePreset::Pine]) {
        for age in [2.0, 4.0, 6.0, 8.0, 12.0] {
            add(format!("age_{}_{age}", p.name()), preset(p), age);
        }
    }
    for p in others([TreePreset::Oak, TreePreset::Maple]) {
        for seed in 1..=3 {
            let mut params = preset(p);
            params.seed = seed;
            add(format!("seed_{}_{seed}", p.name()), params, inf);
        }
    }

    // Every leaf style x placement on a second preset.
    for style in [
        LeafStyle::CrossedPlanes,
        LeafStyle::SingleQuad,
        LeafStyle::ClusterSphere,
        LeafStyle::StarBurst,
        LeafStyle::NeedleCluster,
    ] {
        for placement in [
            FoliagePlacement::TerminalBranches,
            FoliagePlacement::AllBranches,
            FoliagePlacement::TipClusters,
        ] {
            let mut params = preset(TreePreset::Maple);
            params.foliage.style = style;
            params.foliage.placement = placement;
            add(format!("leafplace_{style:?}_{placement:?}"), params, inf);
        }
    }

    // Extreme parameter values on the oak.
    let extremes: &[(&str, Edit)] = &[
        ("extreme_droop", |p| {
            p.branches.gravity_strength = 2.0;
            p.branches.stiffness = 0.0;
        }),
        ("extreme_droop_split", |p| {
            p.branches.gravity_strength = 1.0;
            p.branches.stiffness = 0.0;
            p.split.enabled = true;
            p.split.probability = 1.0;
            p.split.radius_threshold = 0.0;
        }),
        ("extreme_droop_low", |p| {
            // Floor avoidance against bent wood near the ground.
            p.branches.start = 0.05;
            p.branches.gravity_strength = 2.0;
            p.branches.stiffness = 0.0;
            p.branches.up_attraction = -1.0;
        }),
        ("extreme_wobble", |p| p.trunk.randomness = 1.0),
        ("extreme_twist", |p| {
            p.trunk.twist = 720.0;
            p.branches.twist = 360.0;
        }),
        ("extreme_random", |p| p.branches.randomness = 1.0),
        ("extreme_recursion", |p| {
            p.branches.recursion = 3;
            p.branches.sub_branch_count = 4;
        }),
        ("extreme_flat", |p| p.branches.flatness = 1.0),
        ("extreme_horizontal", |p| {
            p.branches.angle = 90.0;
            p.branches.up_attraction = 0.0;
        }),
        ("extreme_vertical", |p| p.branches.angle = 0.0),
        ("extreme_mesh", |p| {
            p.mesh.pipe_radius_enabled = true;
            p.mesh.smooth_iterations = 3;
            p.mesh.adaptive_resolution = true;
        }),
        ("extreme_lowpoly", |p| {
            p.trunk.radial_segments = 3;
            p.trunk.height_segments = 1;
        }),
        ("extreme_highpoly", |p| {
            p.trunk.radial_segments = 32;
            p.trunk.height_segments = 32;
        }),
        ("extreme_noleaves", |p| p.foliage.enabled = false),
        ("extreme_bigcollar", |p| p.mesh.collar_length = 6.0),
    ];
    for (name, edit) in extremes {
        let mut params = preset(TreePreset::Oak);
        edit(&mut params);
        add(name.to_string(), params, inf);
    }

    // Leaf orientations and crown envelopes, each on the oak.
    for orientation in [
        LeafOrientation::RadialOutward,
        LeafOrientation::FollowBranch,
        LeafOrientation::RandomUpward,
        LeafOrientation::HorizontalSpread,
    ] {
        let mut params = preset(TreePreset::Oak);
        params.foliage.orientation = orientation;
        add(format!("orient_{orientation:?}"), params, inf);
    }
    for shape in [
        CrownShape::Cylindrical,
        CrownShape::Conical,
        CrownShape::Spherical,
        CrownShape::Hemispherical,
        CrownShape::TaperedCylindrical,
        CrownShape::Flame,
        CrownShape::InverseConical,
        CrownShape::TendFlame,
        CrownShape::Spreading,
    ] {
        let mut params = preset(TreePreset::Oak);
        params.branches.crown_shape = shape;
        params.branches.crown_influence = 1.0;
        add(format!("crown_{shape:?}"), params, inf);
    }

    // Forced Y-splits on every other preset.
    for p in TreePreset::ALL.into_iter().filter(|&p| p != TreePreset::Oak) {
        let mut params = preset(p);
        params.split.enabled = true;
        params.split.probability = 1.0;
        params.split.radius_threshold = 0.0;
        add(format!("split_{}", p.name()), params, inf);
    }

    // Two smoothing passes on every other preset.
    for p in TreePreset::ALL.into_iter().filter(|&p| p != TreePreset::Oak) {
        let mut params = preset(p);
        params.mesh.smooth_iterations = 2;
        add(format!("smooth_{}", p.name()), params, inf);
    }

    // Smoothing on young trees, where thin new wood is most likely to kink.
    for p in TreePreset::ALL {
        let mut params = preset(p);
        params.mesh.smooth_iterations = 2;
        add(format!("smoothyoung_{}_4", p.name()), params, 4.0);
    }

    // Per-node pipe radii on every other preset.
    for p in TreePreset::ALL.into_iter().filter(|&p| p != TreePreset::Oak) {
        let mut params = preset(p);
        params.mesh.pipe_radius_enabled = true;
        add(format!("pipe_{}", p.name()), params, inf);
    }

    // Pipe radii while growing: just-born branches meet the radius floor (D-053).
    for p in TreePreset::ALL {
        let mut params = preset(p);
        params.mesh.pipe_radius_enabled = true;
        add(format!("pipeyoung_{}_5", p.name()), params, 5.0);
    }

    // A wobbly, twisted trunk under every preset: limbs that hug it can run
    // back into it (D-054).
    for p in TreePreset::ALL {
        let mut params = preset(p);
        params.trunk.randomness = 0.5;
        params.trunk.twist = 180.0;
        params.branches.twist = 90.0;
        add(format!("wobble_{}", p.name()), params, inf);
    }

    // The painterly and toon looks on every preset, and while growing (CPU and GPU meshes bake
    // their clumps from different leaves).
    for (name, style) in [("painterly", RenderStyle::Painterly), ("toon", RenderStyle::Toon)] {
        for p in TreePreset::ALL {
            let mut params = preset(p);
            params.style = style;
            add(format!("{name}_{}", p.name()), params, inf);
        }
        for p in [TreePreset::Oak, TreePreset::Maple] {
            for age in [2.0, 5.0] {
                let mut params = preset(p);
                params.style = style;
                add(format!("{name}young_{}_{age}", p.name()), params, age);
            }
        }
    }

    // Growth series at odd ages for drooping and forking presets.
    for p in [TreePreset::Willow, TreePreset::DragonTree] {
        for age in [1.0, 3.0, 5.0, 7.0, 9.0, inf] {
            add(format!("grow_{}_{age}", p.name()), preset(p), age);
        }
    }

    // Second set of extremes: shape, size, growth timing, floor.
    let extremes2: &[(&str, f32, Edit)] = &[
        ("extreme_notaper", inf, |p| p.trunk.taper = 1.0),
        ("extreme_needletrunk", inf, |p| p.trunk.taper = 0.0),
        ("extreme_flare", inf, |p| p.trunk.flare = 3.0),
        ("extreme_roots12", inf, |p| {
            p.trunk.root_flare_count = 12;
            p.trunk.root_flare_spread = 1.5;
            p.trunk.root_flare_height = 0.3;
        }),
        ("extreme_norecursion", inf, |p| p.branches.recursion = 0),
        ("extreme_longbranches", inf, |p| p.branches.length = 1.5),
        ("extreme_dense", inf, |p| p.branches.density = 8.0),
        ("extreme_bigleaves", inf, |p| p.foliage.leaf_size = 1.2),
        // Old flat per-card shading and colour, for comparing against the default look.
        ("extreme_flatleaves", inf, |p| {
            p.foliage.normal_bend = 0.0;
            p.foliage.color_variation = 0.0;
        }),
        ("extreme_leafvary", inf, |p| p.foliage.color_variation = 0.5),
        ("extreme_tiny", inf, |p| {
            p.trunk.height = 0.5;
            p.trunk.radius = 0.05;
        }),
        ("extreme_giant", inf, |p| {
            p.trunk.height = 40.0;
            p.trunk.radius = 3.0;
        }),
        ("extreme_floor2", inf, |p| {
            p.branches.floor_level = 2.0;
            p.branches.start = 0.1;
        }),
        ("extreme_lateleaves_6", 6.0, |p| p.growth.leaf_threshold = 1.0),
        ("extreme_lagthick_6", 6.0, |p| p.growth.thickening_lag = 5.0),
        ("extreme_fastgrow_2", 2.0, |p| {
            p.growth.trunk_fraction = 0.1;
            p.growth.branch_fraction = 0.1;
        }),
        ("extreme_smooth_split", inf, |p| {
            p.mesh.smooth_iterations = 3;
            p.split.enabled = true;
            p.split.probability = 1.0;
            p.split.radius_threshold = 0.0;
        }),
        ("extreme_droop_wobble", inf, |p| {
            p.trunk.randomness = 0.8;
            p.branches.gravity_strength = 1.5;
            p.branches.stiffness = 0.0;
        }),
    ];
    for (name, age, edit) in extremes2 {
        let mut params = preset(TreePreset::Oak);
        edit(&mut params);
        add(name.to_string(), params, *age);
    }

    // The season (`FloraSeason`, global): turning, every leaf turned, half fallen, bare, and bare
    // under snow. The pine is evergreen, so its first four must match its mature render; in the
    // snow its needles turn white on top.
    for p in [TreePreset::Maple, TreePreset::Oak, TreePreset::Birch, TreePreset::Willow, TreePreset::Pine, TreePreset::Cherry] {
        for (name, season) in [
            ("turning", FloraSeason { autumn: 0.5, ..FloraSeason::SUMMER }),
            ("autumn", FloraSeason::AUTUMN),
            ("falling", FloraSeason { leaf_fall: 0.5, ..FloraSeason::AUTUMN }),
            ("winter", FloraSeason::WINTER),
            ("snow", FloraSeason::SNOW),
        ] {
            list.push(Scenario { name: format!("season_{}_{name}", p.name()), params: preset(p), age: inf, season });
        }
    }
    // A flowering tree's spring: in full blossom on bare branches, flowering as half its leaves
    // are back, and the last flowers among the leaves.
    for (name, season) in [
        ("blossom", FloraSeason::BLOSSOM),
        ("flowering", FloraSeason { leaf_fall: 0.5, ..FloraSeason::BLOSSOM }),
        ("petals", FloraSeason { leaf_fall: 0.1, blossom: 0.3, ..FloraSeason::SUMMER }),
    ] {
        let p = TreePreset::Cherry;
        list.push(Scenario { name: format!("season_{}_{name}", p.name()), params: preset(p), age: inf, season });
    }

    // The stylised conifers: a faceted hull crown on a flat-shaded prism trunk (`CrownStyle::Hull`,
    // `TrunkStyle::Prism`), mature, young and under heavy snow. The fir is the spruce taller, with
    // short, flat branches. `analyze` flags a mature crown whose triangles are not tiers x sides x 2.
    let spruce = preset(TreePreset::Spruce);
    let mut fir = preset(TreePreset::Spruce);
    fir.trunk.height *= 1.15;
    fir.branches.length *= 0.7;
    fir.branches.angle = 70.0;
    for (name, params, tiers, sides) in [
        ("hull_spruce", spruce, 5, 7),
        ("hull_fir", fir, 6, 6),
        ("hull_cypress", preset(TreePreset::Cypress), 3, 5),
    ] {
        let mut params = params;
        params.crown = CrownStyle::Hull { tiers, sides, lift: 0.2, jitter: 1.0 };
        params.trunk_style = TrunkStyle::Prism { sides: 5 };
        for age in [inf, 2.0, 5.0] {
            list.push(Scenario { name: format!("{name}_{age}"), params: params.clone(), age, season: FloraSeason::SUMMER });
        }
        list.push(Scenario { name: format!("{name}_snow"), params, age: inf, season: FloraSeason::SNOW });
    }

    // The evergreen undergrowth presets, in summer and under snow (their needles and fronds stay
    // green in autumn and winter, so the snow is the only change).
    for p in [TreePreset::Juniper, TreePreset::Fern] {
        for (name, season) in [("summer", FloraSeason::SUMMER), ("snow", FloraSeason::SNOW)] {
            list.push(Scenario { name: format!("undergrowth_{}_{name}", p.name()), params: preset(p), age: inf, season });
        }
    }

    list
}

// ---------------------------------------------------------------------------
// Statistics and flags
// ---------------------------------------------------------------------------

struct Stats {
    nodes: usize,
    trunk_verts: usize,
    trunk_tris: usize,
    foliage_verts: usize,
    foliage_tris: usize,
    bbox: (Vec3, Vec3),
    flags: Vec<String>,
}

/// Bounding box of every vertex; a unit box when there is no geometry.
fn bounds(out: &TreeMeshes) -> (Vec3, Vec3) {
    let mut points = out.trunk.positions.iter().chain(&out.foliage.positions).chain(&out.flowers.positions);
    let Some(&first) = points.next() else {
        return (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5));
    };
    points.fold((first, first), |(min, max), &p| (min.min(p), max.max(p)))
}

/// Triangles whose geometric normal disagrees with the stored vertex normals
/// (same rule as `assert_faces_outward` in `src/mesh/rings.rs`).
fn inside_out(mesh: &MeshData) -> usize {
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|&&[a, b, c]| {
            let (a, b, c) = (a as usize, b as usize, c as usize);
            let geometric = (mesh.positions[b] - mesh.positions[a])
                .cross(mesh.positions[c] - mesh.positions[a]);
            if geometric.length_squared() < 1e-12 {
                return false;
            }
            geometric.dot(mesh.normals[a] + mesh.normals[b] + mesh.normals[c]) <= 0.0
        })
        .count()
}

fn analyze(params: &TreeParams, age: f32, out: &TreeMeshes) -> Stats {
    let nodes = out.skeleton.nodes.len();
    let mut flags = Vec::new();

    if out.trunk.is_empty() && age > 0.0 {
        flags.push("EMPTY_TRUNK".to_string());
    }
    let finite = |m: &MeshData| {
        m.positions.iter().all(|p| p.is_finite()) && m.normals.iter().all(|n| n.is_finite())
    };
    if !finite(&out.trunk) || !finite(&out.foliage) {
        flags.push("NONFINITE".to_string());
    }
    if age.is_infinite() && params.foliage.enabled && out.foliage.is_empty() {
        flags.push("NO_LEAVES_MATURE".to_string());
    }
    if nodes == 1 && age.is_infinite() {
        flags.push("NO_BRANCHES".to_string());
    }
    // A mature hull crown has exactly its stack's triangles; a young one has them or, when it has
    // too few leaves to wrap, none.
    if let CrownStyle::Hull { tiers, sides, .. } = params.crown {
        let expected = (tiers * sides * 2) as usize;
        let tris = out.foliage.indices.len() / 3;
        if tris != expected && (age.is_infinite() || tris != 0) {
            flags.push(format!("HULL_TRIS:{tris}!={expected}"));
        }
    }
    // The trunk starts at the ground and branches attached below a raised
    // floor may hang to their base, so the mesh check is against the lower of
    // the two (raised floors are covered by skeleton unit tests).
    if params.branches.floor_avoidance
        && out
            .trunk
            .positions
            .iter()
            .any(|p| p.y < params.branches.floor_level.min(0.0) - 0.01)
    {
        flags.push("BELOW_FLOOR".to_string());
    }
    // Foliage normals are bent toward the crown and deliberately disagree with
    // face winding; winding is covered by `leaf_meshes_face_outward`.
    let flipped = inside_out(&out.trunk);
    if flipped > 0 {
        flags.push(format!("INSIDE_OUT:{flipped}"));
    }

    Stats {
        nodes,
        trunk_verts: out.trunk.positions.len(),
        trunk_tris: out.trunk.indices.len() / 3,
        foliage_verts: out.foliage.positions.len() + out.flowers.positions.len(),
        foliage_tris: (out.foliage.indices.len() + out.flowers.indices.len()) / 3,
        bbox: bounds(out),
        flags,
    }
}

fn fmt_vec(v: Vec3) -> String {
    format!("{:.2},{:.2},{:.2}", v.x, v.y, v.z)
}

fn flags_text(flags: &[String]) -> String {
    if flags.is_empty() { "-".to_string() } else { flags.join(";") }
}

// ---------------------------------------------------------------------------
// Camera framing
// ---------------------------------------------------------------------------

fn look(target: Vec3, dir: Vec3, dist: f32) -> Transform {
    Transform::from_translation(target + dir * dist).looking_at(target, Vec3::Y)
}

/// Front, side, top and junction close-up transforms for one scenario.
fn view_transforms(params: &TreeParams, out: &TreeMeshes, bbox: (Vec3, Vec3)) -> [Transform; 4] {
    let (min, max) = bbox;
    let center = (min + max) * 0.5;
    let radius = (0.5 * (max - min).length()).max(0.5);
    let dist = radius / (PI / 8.0).sin() * 1.1;

    let elevated = |deg: f32, yaw: Vec3| {
        let e = deg.to_radians();
        Vec3::new(yaw.x * e.cos(), e.sin(), yaw.z * e.cos())
    };
    let front = look(center, elevated(10.0, Vec3::Z), dist);
    let side = look(center, elevated(10.0, Vec3::X), dist);
    let top = look(center, elevated(80.0, Vec3::Z), dist);

    // Aim at the lowest primary's attachment point (on the wobbling trunk
    // axis); without primaries, at the trunk top.
    let junction = out
        .skeleton
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Primary)
        .min_by(|a, b| a.start.y.total_cmp(&b.start.y))
        .map(|n| Vec3::new(n.anchor.x, n.start.y, n.anchor.z))
        .unwrap_or(Vec3::new(0.0, out.skeleton.nodes[0].length, 0.0));
    let close_dist = (params.trunk.radius * params.trunk.flare * 6.0).max(1.5);
    let (yaw, pitch) = (45f32.to_radians(), 15f32.to_radians());
    let close_dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    let closeup = look(junction, close_dir, close_dist);

    [front, side, top, closeup]
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Component)]
struct ViewCam(usize);

#[derive(Component)]
struct Overlay;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Warmup,
    Apply,
    Settle,
    Capture,
    Done,
}

/// TSV columns of the scenario in flight, written once its screenshot completes.
struct Pending {
    prefix: String,
    flags: Vec<String>,
}

#[derive(Resource)]
struct Gallery {
    scenarios: Vec<Scenario>,
    out: PathBuf,
    tsv: File,
    /// Render without shadow maps, to tell shadows from geometry defects.
    shadows: bool,
    /// Grow the tree on the GPU.
    gpu: bool,
    /// When the run started, and the least time to render before the first
    /// capture (see `--warmup-secs`).
    started: Instant,
    min_warmup: Duration,
    /// Offscreen image the four views render into; captures do not depend on
    /// the window being visible.
    target: Handle<Image>,
    /// Set by [`check_capture`] when the last capture was one flat color.
    blank_capture: bool,
    index: usize,
    phase: Phase,
    frames: u32,
    pending: Option<Pending>,
}

fn usage() -> ! {
    eprintln!("usage: gallery [--out <dir>] [--filter <substr>]... [--no-shadows] [--gpu] [--warmup-secs <s>]");
    std::process::exit(2);
}

fn main() {
    let mut out = PathBuf::from("target/gallery/latest");
    let mut filters: Vec<String> = Vec::new();
    let mut shadows = true;
    let mut gpu = false;
    let mut min_warmup = 0.0f32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().unwrap_or_else(|| usage()).into(),
            "--filter" => filters.push(args.next().unwrap_or_else(|| usage())),
            "--no-shadows" => shadows = false,
            "--gpu" => gpu = true,
            "--warmup-secs" => min_warmup = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            _ => usage(),
        }
    }

    let mut list = scenarios();
    list.retain(|s| filters.is_empty() || filters.iter().any(|f| s.name.contains(f.as_str())));
    if list.is_empty() {
        eprintln!("no scenarios match");
        std::process::exit(1);
    }

    fs::create_dir_all(&out).expect("create output directory");
    let mut tsv = File::create(out.join("stats.tsv")).expect("create stats.tsv");
    writeln!(tsv, "{TSV_HEADER}").expect("write stats.tsv header");

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(WINDOW.x, WINDOW.y)
                        .with_scale_factor_override(1.0),
                    title: "bevy_flora gallery".to_string(),
                    ..default()
                }),
                ..default()
            }),
            FloraPlugin,
        ))
        // Still air: the captures must repeat.
        .insert_resource(FloraWind { strength: 0.0, ..default() })
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.6, 0.6, 0.6),
            brightness: 300.0,
            ..default()
        })
        .insert_resource(Gallery {
            scenarios: list,
            out,
            tsv,
            shadows,
            gpu,
            started: Instant::now(),
            min_warmup: Duration::from_secs_f32(min_warmup),
            target: Handle::default(),
            blank_capture: false,
            index: 0,
            phase: Phase::Warmup,
            frames: 0,
            pending: None,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

fn setup(
    mut commands: Commands,
    mut gallery: ResMut<Gallery>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    gallery.target = images.add(Image::new_target_texture(
        WINDOW.x,
        WINDOW.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: gallery.shadows,
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.30),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));

    let first = &gallery.scenarios[0];
    let tree = commands.spawn((ProceduralTree(first.params.clone()), TreeAge(first.age))).id();
    if gallery.gpu {
        commands.entity(tree).insert(GpuGrowth);
    }

    let mut view0 = Entity::PLACEHOLDER;
    for i in 0..4usize {
        let position = UVec2::new((i as u32 % 2) * VIEW.x, (i as u32 / 2) * VIEW.y);
        let camera = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    order: i as isize,
                    viewport: Some(Viewport {
                        physical_position: position,
                        physical_size: VIEW,
                        ..default()
                    }),
                    ..default()
                },
                RenderTarget::Image(gallery.target.clone().into()),
                Transform::default(),
                ViewCam(i),
            ))
            .id();
        if i == 0 {
            view0 = camera;
        }
    }

    commands.spawn((
        Overlay,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::BLACK),
        Node {
            position_type: PositionType::Absolute,
            top: px(6),
            left: px(6),
            ..default()
        },
        UiTargetCamera(view0),
    ));
}

/// Flag a capture whose pixels are all one color: nothing was drawn.
fn check_capture(captured: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>) {
    let image = &captured.image;
    let pixel = image.texture_descriptor.format.pixel_size().unwrap_or(4);
    gallery.blank_capture = image.data.as_deref().is_none_or(|data| {
        let first = &data[..pixel.min(data.len())];
        data.chunks_exact(pixel).all(|px| px == first)
    });
}

/// Scenario state machine: Warmup → (Apply → Settle → Capture)* → exit.
#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut gallery: ResMut<Gallery>,
    mut tree: Single<(&mut ProceduralTree, &mut TreeAge)>,
    mut cameras: Query<(&ViewCam, &mut Transform)>,
    mut overlay: Single<&mut Text, With<Overlay>>,
    screenshots: Query<Entity, With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
    support: Res<GpuGrowthSupport>,
    mut season: ResMut<FloraSeason>,
) {
    let gallery = &mut *gallery;
    match gallery.phase {
        Phase::Warmup => {
            gallery.frames += 1;
            if gallery.frames >= WARMUP_FRAMES && gallery.started.elapsed() >= gallery.min_warmup {
                // Without GPU growth the trees would quietly grow on the CPU and
                // the "GPU" renders would prove nothing.
                if gallery.gpu && !support.0 {
                    eprintln!("gallery --gpu: this device cannot grow trees on the GPU (GpuGrowthSupport is false)");
                    exit.write(AppExit::error());
                    return;
                }
                gallery.phase = Phase::Apply;
            }
        }
        Phase::Apply => {
            let scenario = &gallery.scenarios[gallery.index];
            let (params, age) = (&scenario.params, scenario.age);

            let (tree_params, tree_age) = &mut *tree;
            tree_params.0 = params.clone();
            tree_params.set_changed();
            tree_age.0 = age;
            tree_age.set_changed();
            if *season != scenario.season {
                *season = scenario.season;
            }

            let out = build_tree(params, age);
            let stats = analyze(params, age, &out);
            let transforms = view_transforms(params, &out, stats.bbox);
            for (view, mut transform) in &mut cameras {
                *transform = transforms[view.0];
            }

            overlay.0 = format!(
                "{} ({}/{})\nnodes {}  trunk {}v/{}t  foliage {}v/{}t\nflags: {}",
                scenario.name,
                gallery.index + 1,
                gallery.scenarios.len(),
                stats.nodes,
                stats.trunk_verts,
                stats.trunk_tris,
                stats.foliage_verts,
                stats.foliage_tris,
                flags_text(&stats.flags),
            );
            gallery.pending = Some(Pending {
                prefix: format!(
                    "{}\t{age}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    scenario.name,
                    params.seed,
                    stats.nodes,
                    stats.trunk_verts,
                    stats.trunk_tris,
                    stats.foliage_verts,
                    stats.foliage_tris,
                    fmt_vec(stats.bbox.0),
                    fmt_vec(stats.bbox.1),
                ),
                flags: stats.flags,
            });
            gallery.frames = 0;
            gallery.phase = Phase::Settle;
        }
        Phase::Settle => {
            gallery.frames += 1;
            if gallery.frames >= SETTLE_FRAMES {
                let path = gallery
                    .out
                    .join(format!("{}.png", gallery.scenarios[gallery.index].name));
                gallery.blank_capture = false;
                commands
                    .spawn(Screenshot::image(gallery.target.clone()))
                    .observe(save_to_disk(path))
                    .observe(check_capture);
                gallery.frames = 0;
                gallery.phase = Phase::Capture;
            }
        }
        Phase::Capture => {
            gallery.frames += 1;
            let timed_out = gallery.frames >= CAPTURE_TIMEOUT_FRAMES;
            if !screenshots.is_empty() && !timed_out {
                return;
            }

            let mut pending = gallery.pending.take().expect("scenario in flight");
            if timed_out && !screenshots.is_empty() {
                pending.flags.push("CAPTURE_TIMEOUT".to_string());
                for screenshot in &screenshots {
                    commands.entity(screenshot).despawn();
                }
            }
            if gallery.blank_capture {
                pending.flags.push("BLANK_CAPTURE".to_string());
            }
            writeln!(gallery.tsv, "{}\t{}", pending.prefix, flags_text(&pending.flags))
                .expect("append stats.tsv row");
            gallery.tsv.flush().expect("flush stats.tsv");

            gallery.index += 1;
            gallery.frames = 0;
            if gallery.index == gallery.scenarios.len() {
                gallery.phase = Phase::Done;
                exit.write(AppExit::Success);
            } else {
                gallery.phase = Phase::Apply;
            }
        }
        Phase::Done => {}
    }
}
