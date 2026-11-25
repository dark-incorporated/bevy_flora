//! What a tree may be asked for: the range of every numeric [`TreeParams`] field, the catalogue a
//! parameter panel reads, and the size no tree exceeds.
//!
//! A [`TreeParams`] comes from callers and from `.tree.ron` files, and an editor hands every field to a
//! human, so any value can arrive: `NaN`, an infinity, a negative height, `1e30`, `u32::MAX`
//! branches. Without a range the generator's cost is unbounded (a trunk 1e6 m tall, a density of 1e6,
//! 255 sub-branches a branch) and its meshes can hold `NaN`. [`TreeParams::sanitized`] is the one
//! place that says what the generator accepts, and the generating functions ([`build_tree`],
//! [`build_lods`], [`generate_skeleton`], [`skeleton_mesh`]) and the plugin call it, so a tree is always
//! drawn from parameters inside the ranges below. [`MAX_NODES`] and [`MAX_LEAVES`] bound what ranges
//! that are each fine can multiply to.
//!
//! The ranges are one table, [`TreeParams::fields`]: each number's dotted path, kind, unit, range and
//! default. `sanitized` walks it, and a parameter panel reads it (and [`TreeParams::set_field`] refuses
//! what `sanitized` would have to change), so the range a person sees is the range the generator
//! applies: there is no second copy to drift.
//!
//! Pure CPU: this module reads only [`crate::params`].
//!
//! [`build_tree`]: crate::build_tree
//! [`build_lods`]: crate::build_lods
//! [`generate_skeleton`]: crate::generate_skeleton
//! [`skeleton_mesh`]: crate::mesh::branch::skeleton_mesh

use std::borrow::Cow;
use std::sync::LazyLock;

use bevy::color::Color;

use crate::params::{CrownStyle, FruitParams, TreeParams, TrunkStyle};

/// The most branches (nodes of the [`Skeleton`](crate::Skeleton): trunk, leader, primaries, splits,
/// sub-branches and strands) a tree has. Generation stops adding wood here, in its own order (the
/// leader, then each primary with its splits and sub-branches, then the strands), so a tree that asks for
/// more keeps its lower and earlier wood. The 20 presets have 5 to 588 (the largest gallery scenario 852),
/// so nothing the crate draws comes near it.
pub const MAX_NODES: usize = 5_000;

/// The most leaves a tree's skeleton can carry: when the leaves its nodes could bear
/// ([`FoliageParams`](crate::FoliageParams): terminal count, cluster size or density along the wood)
/// add up to more, the last nodes (the top of the crown) are dropped until they fit. The heaviest preset (the
/// willow, with its strands) has 4 630; the gallery's `extreme_giant` oak, 40 m tall, about 21 000.
pub const MAX_LEAVES: usize = 60_000;

/// The most leaves one node of a tree grown under
/// [`FoliagePlacement::AllBranches`](crate::FoliagePlacement::AllBranches) carries however long it is.
pub(crate) const MAX_LEAVES_PER_NODE: f32 = 1024.0;

/// `value` inside `lo..=hi`, or `default` (also inside it) when it is `NaN`. `f32::clamp` alone returns
/// `NaN` for `NaN`.
fn clean(value: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if value.is_nan() { default.clamp(lo, hi) } else { value.clamp(lo, hi) }
}

/// Whether `color` has four finite channels.
fn finite(color: Color) -> bool {
    // Every colour space converts a `NaN` channel to a `NaN`; sRGB is the one that costs nothing for
    // the colours of the presets.
    let c = color.to_srgba();
    c.red.is_finite() && c.green.is_finite() && c.blue.is_finite() && c.alpha.is_finite()
}

/// Applies the colour rules to a copy of the parameters and notes whether anything moved.
struct Limits {
    changed: bool,
}

impl Limits {
    /// A colour with a `NaN` or infinite channel reads as `default`.
    fn color(&mut self, field: &mut Color, default: Color) {
        if !finite(*field) {
            *field = default;
            self.changed = true;
        }
    }

    /// An optional colour with a `NaN` or infinite channel is `None`, the default of every one.
    fn optional_color(&mut self, field: &mut Option<Color>) {
        if field.is_some_and(|color| !finite(color)) {
            *field = None;
            self.changed = true;
        }
    }
}

static DEFAULTS: LazyLock<TreeParams> = LazyLock::new(TreeParams::default);

/// What a number of a [`TreeParams`] is measured in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamUnit {
    /// A share, a ratio, an exponent, a multiplier or a count: the field's own documentation says of what.
    None,
    /// Metres of the tree's own size (a trunk's height, a leaf's length). [`FloraUnits`](crate::FloraUnits)
    /// turns them into world units where they meet the world.
    Metres,
    /// Degrees.
    Degrees,
    /// Per metre of trunk or of branch (the two densities).
    PerMetre,
    /// Years of a tree's age ([`TreeAge`](crate::TreeAge)).
    Years,
}

impl ParamUnit {
    /// What a panel writes after the number: `" m"`, `"°"`, `" per metre"` or `" years"`, and nothing
    /// for [`ParamUnit::None`].
    pub const fn suffix(self) -> &'static str {
        match self {
            ParamUnit::None => "",
            ParamUnit::Metres => " m",
            ParamUnit::Degrees => "°",
            ParamUnit::PerMetre => " per metre",
            ParamUnit::Years => " years",
        }
    }
}

/// Whether a number of a [`TreeParams`] is a quantity or a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamKind {
    /// Any `f32` inside the range.
    Float,
    /// A whole number inside the range (a `u32` field): its `min`, `max` and `default` are whole.
    Count,
}

/// A number of a [`TreeParams`], where it can be written.
enum Place<'a> {
    Float(&'a mut f32),
    Count(&'a mut u32),
}

/// Where a number is written: a field, or `None` for a part the tree does not have.
type Write = for<'a> fn(&'a mut TreeParams) -> Option<Place<'a>>;

/// One number of a [`TreeParams`], as a panel needs to know it: where it is, what it is measured in, the
/// range the generator holds it in and the value it reads as when it is `NaN`.
///
/// The list is [`TreeParams::fields`], in the order of the parameters' own structs (trunk, branches, strands,
/// splits, mesh, foliage, wind, bark, growth, then the hull crown and prism trunk). The range is a hard
/// limit, wide on purpose (more than a factor of ten over every preset): it is what
/// [`TreeParams::sanitized`] enforces, not a sensible slider range.
#[derive(Clone, Copy)]
pub struct ParamField {
    /// The dotted path of the field, as the serde form of a [`TreeParams`] has it: `trunk.height`,
    /// `branches.strands.density`, `foliage.fruit.radius`, `crown.Hull.tiers`, `trunk_style.Prism.sides`.
    pub path: &'static str,
    /// A quantity or a count.
    pub kind: ParamKind,
    /// What it is measured in.
    pub unit: ParamUnit,
    /// The least value the generator accepts.
    pub min: f32,
    /// The most value the generator accepts.
    pub max: f32,
    /// What the field reads as when it is `NaN` (a [`TreeParams::default`] value), and what a panel
    /// resets it to. For the three parts a default tree does not have, a fruit, a hull crown and a prism
    /// trunk, it is the value a panel starts the part with.
    pub default: f32,
    read: fn(&TreeParams) -> Option<f32>,
    write: Write,
}

impl std::fmt::Debug for ParamField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParamField")
            .field("path", &self.path)
            .field("kind", &self.kind)
            .field("unit", &self.unit)
            .field("min", &self.min)
            .field("max", &self.max)
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

impl ParamField {
    /// The number in `params`, or `None` when the tree has no such part (the radius of a fruit on a
    /// tree that bears none, the tiers of a crown that is not a hull). A count is returned as an `f32`.
    pub fn value(&self, params: &TreeParams) -> Option<f32> {
        (self.read)(params)
    }

    /// Whether `value` is inside the range. A `NaN` is not.
    pub fn holds(&self, value: f32) -> bool {
        value >= self.min && value <= self.max
    }

    /// Set the number in `params` to `value`, or refuse and change nothing: the part is not there, the value
    /// is not finite, a count is not whole, or the value is outside the range (the range is in the error).
    pub fn set(&self, params: &mut TreeParams, value: f32) -> Result<(), ParamError> {
        let Some(place) = (self.write)(params) else {
            return Err(ParamError::Absent { path: self.path });
        };
        if !value.is_finite() {
            return Err(ParamError::NotFinite { path: self.path, value });
        }
        if self.kind == ParamKind::Count && value.fract() != 0.0 {
            return Err(ParamError::NotWhole { path: self.path, value });
        }
        if !self.holds(value) {
            return Err(ParamError::OutOfRange { path: self.path, value, min: self.min, max: self.max, unit: self.unit });
        }
        match place {
            Place::Float(field) => *field = value,
            // A whole number inside `min..=max`, which is inside `u32`.
            Place::Count(field) => *field = value as u32,
        }
        Ok(())
    }

    /// Hold `place` in the range; whether it moved. A `NaN` reads as the default and anything else as the
    /// nearest end of the range. `-0.0` is left alone.
    fn hold(&self, place: Place<'_>) -> bool {
        match place {
            Place::Float(field) => {
                let new = clean(*field, self.default, self.min, self.max);
                // `NaN != x` is true, so a `NaN` always counts as moved; `-0.0 == 0.0`, so it never does.
                if new != *field {
                    *field = new;
                    return true;
                }
            }
            Place::Count(field) => {
                let new = (*field).clamp(self.min as u32, self.max as u32);
                if new != *field {
                    *field = new;
                    return true;
                }
            }
        }
        false
    }
}

/// Why [`ParamField::set`] and [`TreeParams::set_field`] refused a value. Its `Display` is a sentence an
/// editor can show as it is, naming the field and, for a range, the range and its unit.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamError {
    /// `path` is not a number of a tree.
    Unknown {
        /// What was asked for.
        path: String,
        /// The paths of [`TreeParams::fields`] it is close to (a typo, or the last word of a path), nearest
        /// first; empty when none is.
        nearest: Vec<&'static str>,
    },
    /// The tree has no such part: no fruit, a crown that is not a hull, a trunk that is not a prism. The part
    /// is set through the tree's other fields first.
    Absent {
        /// The [`ParamField::path`] of the number.
        path: &'static str,
    },
    /// The value is `NaN` or infinite.
    NotFinite {
        /// The [`ParamField::path`] of the number.
        path: &'static str,
        /// What was given.
        value: f32,
    },
    /// The field is a count and the value has a fraction.
    NotWhole {
        /// The [`ParamField::path`] of the number.
        path: &'static str,
        /// What was given.
        value: f32,
    },
    /// The value is outside the range the generator holds the field in.
    OutOfRange {
        /// The [`ParamField::path`] of the number.
        path: &'static str,
        /// What was given.
        value: f32,
        /// The least value the generator accepts.
        min: f32,
        /// The most value the generator accepts.
        max: f32,
        /// What the number is measured in.
        unit: ParamUnit,
    },
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamError::Unknown { path, nearest } => {
                write!(f, "`{path}` is not a number of a tree")?;
                if nearest.is_empty() {
                    return write!(f, "; `TreeParams::fields()` lists them");
                }
                f.write_str("; the nearest are ")?;
                for (i, near) in nearest.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "`{near}`")?;
                }
                Ok(())
            }
            ParamError::Absent { path } => {
                let part = if path.starts_with("foliage.fruit") {
                    "the tree bears no fruit (`foliage.fruit` is not set)"
                } else if path.starts_with("crown.Hull") {
                    "the crown is not a hull (`crown` is not `Hull`)"
                } else {
                    "the trunk is not a prism (`trunk_style` is not `Prism`)"
                };
                write!(f, "`{path}` is not there: {part}")
            }
            ParamError::NotFinite { path, value } => write!(f, "`{path}` was given {value}, which is not a finite number"),
            ParamError::NotWhole { path, value } => write!(f, "`{path}` counts whole things; {value} is not a whole number"),
            ParamError::OutOfRange { path, value, min, max, unit } => {
                let suffix = unit.suffix();
                write!(f, "`{path}` is held between {min}{suffix} and {max}{suffix}; {value} is out of range")
            }
        }
    }
}

impl std::error::Error for ParamError {}

/// How far (in single-character edits) a path may be from a typed one to be offered as what was meant.
const SUGGESTION_EDIT_DISTANCE: usize = 3;
/// The most paths an [`ParamError::Unknown`] names.
const SUGGESTIONS: usize = 4;

/// The number of single-character edits that turn `a` into `b`.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1).min(row[j] + 1).min(diagonal + usize::from(x != y));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The paths of the catalogue closest to `path`, nearest first: a few edits away, or ending in the same word.
fn nearest_paths(path: &str) -> Vec<&'static str> {
    let last = path.rsplit('.').next().unwrap_or(path);
    let mut near: Vec<(usize, usize, &'static str)> = FIELDS
        .iter()
        .enumerate()
        .filter_map(|(order, field)| {
            let distance = edit_distance(path, field.path);
            let same_word = field.path.rsplit('.').next() == Some(last);
            (distance <= SUGGESTION_EDIT_DISTANCE || same_word).then_some((distance, order, field.path))
        })
        .collect();
    near.sort_unstable();
    near.into_iter().take(SUGGESTIONS).map(|(_, _, path)| path).collect()
}

/// The hull a panel starts a crown with when it becomes a [`CrownStyle::Hull`], and the prism a trunk starts
/// with: the stylised conifer of the examples (`inspect --hull 5,7 --prism 5`). A design default; no
/// [`TreeParams`] has them.
const HULL_TIERS: u32 = 5;
const HULL_SIDES: u32 = 7;
const PRISM_SIDES: u32 = 5;

/// The catalogue: one row per number of a [`TreeParams`], in the order of the parameters' structs. This is the
/// only place that says what range a number is held in.
static FIELDS: LazyLock<Vec<ParamField>> = LazyLock::new(build_fields);

fn build_fields() -> Vec<ParamField> {
    let d = &*DEFAULTS;

    // A float field, by its path through a `TreeParams`: `NaN` reads as the default tree's value.
    macro_rules! float {
        ($($field:ident).+, $unit:ident, $lo:expr, $hi:expr) => {
            ParamField {
                path: stringify!($($field).+),
                kind: ParamKind::Float,
                unit: ParamUnit::$unit,
                min: $lo,
                max: $hi,
                default: d.$($field).+,
                read: |p| Some(p.$($field).+),
                write: |p| Some(Place::Float(&mut p.$($field).+)),
            }
        };
    }
    // A count field, by its path.
    macro_rules! count {
        ($($field:ident).+, $lo:expr, $hi:expr) => {
            ParamField {
                path: stringify!($($field).+),
                kind: ParamKind::Count,
                unit: ParamUnit::None,
                min: $lo as f32,
                max: $hi as f32,
                default: d.$($field).+ as f32,
                read: |p| Some(p.$($field).+ as f32),
                write: |p| Some(Place::Count(&mut p.$($field).+)),
            }
        };
    }

    let dfruit = FruitParams::default();
    vec![
        // Trunk: metres, fractions of its height, counts and degrees.
        float!(trunk.height, Metres, 0.0, 500.0),
        float!(trunk.radius, Metres, 0.0, 50.0),
        count!(trunk.radial_segments, 3, 64),
        count!(trunk.height_segments, 1, 256),
        float!(trunk.taper, None, 0.0, 4.0),
        // An exponent: `t^-1` is infinite at the base.
        float!(trunk.taper_curve, None, 0.0, 16.0),
        float!(trunk.flare, None, 0.0, 16.0),
        float!(trunk.randomness, None, 0.0, 8.0),
        count!(trunk.root_flare_count, 0, 64),
        float!(trunk.root_flare_spread, None, 0.0, 16.0),
        float!(trunk.root_flare_height, None, 0.0, 1.0),
        float!(trunk.leader_length, None, 0.0, 8.0),
        float!(trunk.leader_taper, None, 0.0, 4.0),
        float!(trunk.twist, Degrees, -7200.0, 7200.0),
        // Branches. The zone is a fraction of the trunk's height: below 0 is wood under the ground.
        float!(branches.start, None, 0.0, 1.0),
        float!(branches.end, None, 0.0, 1.0),
        // Per metre of trunk: the fern's 6 cm stem carries 300. The nodes are capped, not the density.
        float!(branches.density, PerMetre, 0.0, 1000.0),
        float!(branches.length, None, 0.0, 32.0),
        float!(branches.angle, Degrees, -360.0, 360.0),
        float!(branches.radius_ratio, None, 0.0, 4.0),
        // A tip radius is `base * (1 - taper)`: above 1 is a negative radius.
        float!(branches.taper, None, 0.0, 1.0),
        float!(branches.phyllotaxis_angle, Degrees, -3600.0, 3600.0),
        float!(branches.randomness, None, 0.0, 8.0),
        float!(branches.up_attraction, None, -8.0, 8.0),
        count!(branches.recursion, 0, 8),
        count!(branches.sub_branch_count, 0, 32),
        float!(branches.sub_branch_scale, None, 0.0, 4.0),
        // A length is scaled by `1 - variation` at the least.
        float!(branches.length_variation, None, 0.0, 1.0),
        float!(branches.sub_branch_position_bias, None, -1.0, 1.0),
        float!(branches.apical_dominance, None, 0.0, 4.0),
        float!(branches.flatness, None, 0.0, 1.0),
        float!(branches.angle_curve, None, -8.0, 8.0),
        float!(branches.crown_angle_variation, None, -8.0, 8.0),
        float!(branches.twist, Degrees, -7200.0, 7200.0),
        float!(branches.gravity_strength, None, -16.0, 16.0),
        // Above 1 reads as 1 (`bend = gravity * (1 - stiffness)`, floored at 0).
        float!(branches.stiffness, None, 0.0, 1.0),
        float!(branches.break_chance, None, 0.0, 1.0),
        float!(branches.crown_influence, None, 0.0, 4.0),
        float!(branches.floor_level, Metres, -1000.0, 1000.0),
        float!(branches.min_twig_radius, Metres, 0.0, 10.0),
        float!(branches.strands.density, PerMetre, 0.0, 32.0),
        float!(branches.strands.start, None, 0.0, 1.0),
        float!(branches.strands.length, None, 0.0, 16.0),
        float!(branches.strands.spread, Degrees, 0.0, 90.0),
        float!(branches.strands.clearance, Metres, -100.0, 100.0),
        float!(split.probability, None, 0.0, 1.0),
        float!(split.angle, Degrees, 0.0, 360.0),
        float!(split.position, None, 0.0, 1.0),
        float!(split.radius_threshold, Metres, 0.0, 100.0),
        // Mesh construction: the counts bound the vertex cost of the wood.
        count!(mesh.smooth_iterations, 0, 8),
        float!(mesh.smooth_factor, None, 0.0, 1.0),
        float!(mesh.resolution_scale, None, 0.0, 1024.0),
        // The two bounds may cross (the upper one wins), so each has the whole range.
        count!(mesh.min_radial_segments, 0, 32),
        count!(mesh.max_radial_segments, 0, 32),
        float!(mesh.pipe_radius_exponent, None, 0.5, 8.0),
        float!(mesh.pipe_radius_min, Metres, 0.0, 10.0),
        float!(mesh.collar_length, None, 0.0, 64.0),
        // Leaves. Density is per branch, or per metre of wood: with the height falloff at most 2x.
        float!(foliage.density, None, 0.0, 512.0),
        count!(foliage.cluster_size, 0, 64),
        float!(foliage.leaf_size, Metres, 0.0, 64.0),
        float!(foliage.leaf_size_variation, None, 0.0, 1.0),
        float!(foliage.radius_threshold, None, 0.0, 100.0),
        // The density at the crown's base is `1 - falloff` times its top's: 0 to 2.
        float!(foliage.height_falloff, None, -1.0, 1.0),
        float!(foliage.droop, None, -8.0, 8.0),
        float!(foliage.rotation_variation, None, 0.0, 1.0),
        float!(foliage.normal_bend, None, 0.0, 1.0),
        float!(foliage.color_variation, None, 0.0, 1.0),
        float!(foliage.hue_variation, None, 0.0, 1.0),
        float!(foliage.occlusion, None, 0.0, 1.0),
        float!(foliage.shading.translucency, None, 0.0, 16.0),
        float!(foliage.shading.roughness, None, 0.0, 1.0),
        float!(foliage.shading.veins, None, 0.0, 1.0),
        float!(foliage.shading.underside, None, 0.0, 1.0),
        // Fruit is a part of the tree a default one does not have: its numbers are there once `foliage.fruit` is set.
        ParamField {
            path: "foliage.fruit.radius",
            kind: ParamKind::Float,
            unit: ParamUnit::Metres,
            min: 0.0,
            max: 4.0,
            default: dfruit.radius,
            read: |p| p.foliage.fruit.as_ref().map(|fruit| fruit.radius),
            write: |p| p.foliage.fruit.as_mut().map(|fruit| Place::Float(&mut fruit.radius)),
        },
        ParamField {
            path: "foliage.fruit.share",
            kind: ParamKind::Float,
            unit: ParamUnit::None,
            min: 0.0,
            max: 1.0,
            default: dfruit.share,
            read: |p| p.foliage.fruit.as_ref().map(|fruit| fruit.share),
            write: |p| p.foliage.fruit.as_mut().map(|fruit| Place::Float(&mut fruit.share)),
        },
        float!(wind.sway, None, 0.0, 64.0),
        float!(wind.flutter, None, 0.0, 64.0),
        float!(wind.sway_frequency, None, 0.0, 64.0),
        float!(wind.flutter_frequency, None, 0.0, 64.0),
        float!(bark.smooth_below, Metres, 0.0, 10.0),
        float!(bark.fissured_above, Metres, 0.0, 10.0),
        float!(growth.maturity_age, Years, 0.0, 10_000.0),
        float!(growth.trunk_fraction, None, 0.0, 8.0),
        float!(growth.branch_fraction, None, 0.0, 8.0),
        float!(growth.child_duration_scale, None, 0.0, 8.0),
        float!(growth.thickening_lag, None, 0.0, 64.0),
        float!(growth.leaf_threshold, None, 0.0, 1.0),
        // The hull crown and the prism trunk exist only while the style is set.
        ParamField {
            path: "crown.Hull.tiers",
            kind: ParamKind::Count,
            unit: ParamUnit::None,
            min: 1.0,
            max: 64.0,
            default: HULL_TIERS as f32,
            read: |p| match &p.crown {
                CrownStyle::Hull { tiers, .. } => Some(*tiers as f32),
                _ => None,
            },
            write: |p| match &mut p.crown {
                CrownStyle::Hull { tiers, .. } => Some(Place::Count(tiers)),
                _ => None,
            },
        },
        ParamField {
            path: "crown.Hull.sides",
            kind: ParamKind::Count,
            unit: ParamUnit::None,
            min: 3.0,
            max: 64.0,
            default: HULL_SIDES as f32,
            read: |p| match &p.crown {
                CrownStyle::Hull { sides, .. } => Some(*sides as f32),
                _ => None,
            },
            write: |p| match &mut p.crown {
                CrownStyle::Hull { sides, .. } => Some(Place::Count(sides)),
                _ => None,
            },
        },
        ParamField {
            path: "crown.Hull.lift",
            kind: ParamKind::Float,
            unit: ParamUnit::None,
            min: 0.0,
            max: 1.0,
            default: 0.0,
            read: |p| match &p.crown {
                CrownStyle::Hull { lift, .. } => Some(*lift),
                _ => None,
            },
            write: |p| match &mut p.crown {
                CrownStyle::Hull { lift, .. } => Some(Place::Float(lift)),
                _ => None,
            },
        },
        ParamField {
            path: "crown.Hull.jitter",
            kind: ParamKind::Float,
            unit: ParamUnit::None,
            min: 0.0,
            max: 1.0,
            default: 0.0,
            read: |p| match &p.crown {
                CrownStyle::Hull { jitter, .. } => Some(*jitter),
                _ => None,
            },
            write: |p| match &mut p.crown {
                CrownStyle::Hull { jitter, .. } => Some(Place::Float(jitter)),
                _ => None,
            },
        },
        ParamField {
            path: "trunk_style.Prism.sides",
            kind: ParamKind::Count,
            unit: ParamUnit::None,
            min: 3.0,
            max: 64.0,
            default: PRISM_SIDES as f32,
            read: |p| match &p.trunk_style {
                TrunkStyle::Prism { sides } => Some(*sides as f32),
                _ => None,
            },
            write: |p| match &mut p.trunk_style {
                TrunkStyle::Prism { sides } => Some(Place::Count(sides)),
                _ => None,
            },
        },
    ]
}

impl TreeParams {
    /// Every number of a tree with its range, unit and default, for a parameter panel: one slider per entry,
    /// labelled with its unit, bounded by its range. The list is also what [`TreeParams::sanitized`] enforces, so
    /// a range read here is never out of step with the generator's. Colours, enums, flags, the scans and the seed
    /// are not numbers in a range and are not listed.
    ///
    /// ```
    /// use bevy_flora::{ParamKind, ParamUnit, TreeParams, TreePreset};
    ///
    /// let oak = TreePreset::Oak.params();
    /// let height = TreeParams::fields().iter().find(|f| f.path == "trunk.height").expect("a trunk has a height");
    /// assert_eq!((height.kind, height.unit), (ParamKind::Float, ParamUnit::Metres));
    /// assert_eq!((height.min, height.max), (0.0, 500.0));
    /// // Its value in a tree, to put the slider where the tree is.
    /// assert_eq!(height.value(&oak), Some(oak.trunk.height));
    /// // A part the tree does not have has no value to show: the oak bears no fruit.
    /// let fruit = TreeParams::fields().iter().find(|f| f.path == "foliage.fruit.radius").expect("fruit has a radius");
    /// assert_eq!(fruit.value(&oak), None);
    /// ```
    pub fn fields() -> &'static [ParamField] {
        &FIELDS
    }

    /// The number at `path` (one of [`ParamField::path`]), or `None` for a path that is not a number of a tree
    /// and for a part this tree does not have (a fruit it does not bear). A count is an `f32` here.
    pub fn field(&self, path: &str) -> Option<f32> {
        FIELDS.iter().find(|field| field.path == path)?.value(self)
    }

    /// Set the number at `path` to `value` as an editor's field would: the value is taken only if
    /// [`TreeParams::sanitized`] would leave it as it is, otherwise the edit is refused with the reason and the
    /// parameters are unchanged. `sanitized` still clamps what arrives some other way (a file, a script); this is
    /// the door that says no first.
    ///
    /// ```
    /// use bevy_flora::{ParamError, TreePreset};
    ///
    /// let mut oak = TreePreset::Oak.params();
    /// // A value outside the range is refused with the range, before anything changes.
    /// let refused = oak.set_field("trunk.height", 1e9).expect_err("1e9 m is not a tree");
    /// assert!(matches!(refused, ParamError::OutOfRange { .. }));
    /// assert_eq!(refused.to_string(), "`trunk.height` is held between 0 m and 500 m; 1000000000 is out of range");
    /// assert_eq!(oak, TreePreset::Oak.params());
    /// // A typo names the paths it is near.
    /// let typo = oak.set_field("trunk.hieght", 12.0).expect_err("a typo");
    /// assert!(typo.to_string().contains("`trunk.height`"));
    /// // A value in range is taken.
    /// oak.set_field("trunk.height", 12.0).expect("12 m is a tree");
    /// assert_eq!(oak.field("trunk.height"), Some(12.0));
    /// ```
    pub fn set_field(&mut self, path: &str, value: f32) -> Result<(), ParamError> {
        match FIELDS.iter().find(|field| field.path == path) {
            Some(field) => field.set(self, value),
            None => Err(ParamError::Unknown { path: path.to_string(), nearest: nearest_paths(path) }),
        }
    }
}

impl TreeParams {
    /// The parameters a tree is actually built from: these, with every numeric field inside the range the
    /// generator is defined over. Borrowed when nothing needs to move (every preset, and every value
    /// the gallery draws); otherwise a copy where a `NaN` reads as the field's [`Default`] and any other
    /// value that is out of range, an infinity included, is the nearest end of its range.
    ///
    /// [`build_tree`](crate::build_tree), [`build_lods`](crate::build_lods),
    /// [`generate_skeleton`](crate::generate_skeleton), [`skeleton_mesh`](crate::mesh::branch::skeleton_mesh)
    /// and [`TreeParams::mature_at`] call it on what they are given, and [`FloraPlugin`](crate::FloraPlugin)
    /// writes it back into a [`ProceduralTree`](crate::ProceduralTree) that is added or changed, so a
    /// tree is built in bounded time from finite values whatever a `.tree.ron` file or an editor's number
    /// field held. An editor that shows the value in force asks for it here.
    ///
    /// The ranges are wide, and every one has more than a factor of ten of room over the presets and the
    /// gallery's extreme scenarios: heights and lengths in metres up to 500 and 32, fractions of a height
    /// from 0 to 1, angles to a few turns, `branches.recursion` to 8, `sub_branch_count` to 32,
    /// `radial_segments` to 64, `smooth_iterations` to 8, leaf `density` to 512. Enums, flags and the
    /// seed are not limited, and a colour with a `NaN` channel reads as its default. A tree that
    /// the ranges allow can still ask for more wood or leaves than any tree has: [`MAX_NODES`] and
    /// [`MAX_LEAVES`] cut it.
    ///
    /// ```
    /// use bevy_flora::{TreeParams, TreePreset, build_tree};
    ///
    /// // A preset is already inside every range.
    /// assert!(matches!(TreePreset::Oak.params().sanitized(), std::borrow::Cow::Borrowed(_)));
    ///
    /// // A number field that held `NaN`, and one that held 1e9 metres, draw the default height and the tallest.
    /// let mut params = TreeParams::default();
    /// params.trunk.height = f32::NAN;
    /// assert_eq!(params.sanitized().trunk.height, TreeParams::default().trunk.height);
    /// params.trunk.height = 1e9;
    /// assert_eq!(params.sanitized().trunk.height, 500.0);
    /// assert!(build_tree(&params, f32::INFINITY).trunk.positions.iter().all(|p| p.is_finite()));
    /// ```
    pub fn sanitized(&self) -> Cow<'_, TreeParams> {
        let mut copy = self.clone();
        if copy.apply_limits() { Cow::Owned(copy) } else { Cow::Borrowed(self) }
    }

    /// Hold every numeric field in its range; whether any moved.
    fn apply_limits(&mut self) -> bool {
        let mut moved = false;
        for field in FIELDS.iter() {
            // Reading is a plain field access, so only a number that is out of range is written (and a
            // part the tree does not have, a fruit it does not bear, has nothing to hold).
            if field.value(self).is_none_or(|value| field.holds(value)) {
                continue;
            }
            if let Some(place) = (field.write)(self) {
                moved |= field.hold(place);
            }
        }
        // The colours are not numbers in a range: a channel that is not finite is the only way they are wrong.
        moved | self.hold_colours()
    }

    /// A colour with a `NaN` or infinite channel reads as its default (an optional one as `None`); whether any moved.
    fn hold_colours(&mut self) -> bool {
        let d = &*DEFAULTS;
        let mut l = Limits { changed: false };
        l.color(&mut self.foliage.shading.transmission_tint, d.foliage.shading.transmission_tint);
        l.color(&mut self.foliage.color, d.foliage.color);
        l.optional_color(&mut self.foliage.autumn);
        l.optional_color(&mut self.foliage.blossom);
        if let Some(fruit) = &mut self.foliage.fruit {
            l.color(&mut fruit.color, FruitParams::default().color);
        }
        l.color(&mut self.trunk_color, d.trunk_color);
        l.changed
    }
}

#[cfg(test)]
pub(crate) mod fields {
    //! Every numeric field of a [`TreeParams`](crate::TreeParams), found by reflection, so a test that
    //! sets each one covers a field added later without anyone remembering it.

    use bevy::reflect::{PartialReflect, ReflectMut};

    /// The name of the child of `value` at `index`, for a path: a struct's field name, an enum's variant
    /// field name, or the position of a tuple field.
    fn child<'a>(value: &'a mut dyn PartialReflect, name: &str) -> Option<&'a mut dyn PartialReflect> {
        match value.reflect_mut() {
            ReflectMut::Struct(s) => s.field_mut(name),
            ReflectMut::Enum(e) => {
                let index = (0..e.field_len()).find(|&i| e.name_at(i).map_or_else(|| i.to_string(), str::to_string) == name)?;
                e.field_at_mut(index)
            }
            ReflectMut::Tuple(t) => t.field_mut(name.parse().ok()?),
            _ => None,
        }
    }

    /// Paths (`.trunk.height`, `.crown.tiers`: an enum's fields are named without its variant, an optional part
    /// is read through its tuple field `.foliage.fruit.0.radius`) of every `f32` and `u32` under `value`.
    pub(crate) fn numeric(value: &mut dyn PartialReflect) -> Vec<String> {
        fn walk(value: &mut dyn PartialReflect, path: &str, out: &mut Vec<String>) {
            let mut names = Vec::new();
            match value.reflect_mut() {
                ReflectMut::Struct(s) => names.extend((0..s.field_len()).filter_map(|i| s.name_at(i).map(str::to_string))),
                ReflectMut::Enum(e) => names.extend((0..e.field_len()).map(|i| e.name_at(i).map_or_else(|| i.to_string(), str::to_string))),
                ReflectMut::Tuple(t) => names.extend((0..t.field_len()).map(|i| i.to_string())),
                ReflectMut::Opaque(_) => {
                    if value.try_downcast_ref::<f32>().is_some() || value.try_downcast_ref::<u32>().is_some() {
                        out.push(path.to_string());
                    }
                    return;
                }
                _ => return,
            }
            for name in names {
                if let Some(field) = child(value, &name) {
                    walk(field, &format!("{path}.{name}"), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(value, "", &mut out);
        out
    }

    fn at<'a>(root: &'a mut dyn PartialReflect, path: &str) -> Option<&'a mut dyn PartialReflect> {
        let mut cursor = root;
        for name in path.split('.').skip(1) {
            cursor = child(cursor, name)?;
        }
        Some(cursor)
    }

    /// Set the number at `path` to `value`; a count takes `value` rounded into `0..=u32::MAX` (a `NaN` is 0).
    pub(crate) fn set(root: &mut dyn PartialReflect, path: &str, value: f64) {
        let Some(field) = at(root, path) else { return };
        if let Some(f) = field.try_downcast_mut::<f32>() {
            *f = value as f32;
        } else if let Some(u) = field.try_downcast_mut::<u32>() {
            *u = if value.is_nan() { 0 } else { value.clamp(0.0, u32::MAX as f64) as u32 };
        }
    }

    /// The number at `path`, or `NaN` if there is none.
    pub(crate) fn get(root: &mut dyn PartialReflect, path: &str) -> f64 {
        let Some(field) = at(root, path) else { return f64::NAN };
        if let Some(f) = field.try_downcast_ref::<f32>() {
            return f64::from(*f);
        }
        field.try_downcast_ref::<u32>().map_or(f64::NAN, |u| f64::from(*u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;

    /// The fields where `a` and `b` differ, for a message.
    fn moved(a: &TreeParams, b: &TreeParams) -> Vec<String> {
        let (mut a, mut b) = (a.clone(), b.clone());
        let paths = fields::numeric(&mut a);
        paths.into_iter().filter(|path| fields::get(&mut a, path) != fields::get(&mut b, path)).collect()
    }

    #[test]
    fn every_preset_and_the_default_are_inside_every_range() {
        assert!(matches!(TreeParams::default().sanitized(), Cow::Borrowed(_)));
        for preset in TreePreset::ALL {
            let params = preset.params();
            let clean = params.sanitized();
            assert!(
                matches!(clean, Cow::Borrowed(_)),
                "{} has fields outside their ranges: {:?}",
                preset.name(),
                moved(&params, &clean)
            );
        }
    }

    #[test]
    fn a_nan_reads_as_the_default_and_the_rest_as_the_nearest_end() {
        let mut params = TreePreset::Oak.params();
        params.trunk.height = f32::NAN;
        params.branches.density = f32::INFINITY;
        params.branches.start = -3.0;
        params.trunk.radial_segments = u32::MAX;
        params.foliage.autumn = Some(Color::srgb(f32::NAN, 0.0, 0.0));
        let clean = params.sanitized();
        assert_eq!(clean.trunk.height, TreeParams::default().trunk.height);
        assert_eq!(clean.branches.density, 1000.0);
        assert_eq!(clean.branches.start, 0.0);
        assert_eq!(clean.trunk.radial_segments, 64);
        assert_eq!(clean.foliage.autumn, None);
        // What was in range is as it was, and the result is in range.
        assert_eq!(clean.trunk.radius, params.trunk.radius);
        assert!(matches!(clean.sanitized(), Cow::Borrowed(_)));
    }

    #[test]
    fn every_numeric_field_is_limited() {
        // A table that forgot a field leaves it at the hostile value.
        let mut base = TreePreset::Apple.params();
        base.crown = CrownStyle::Hull { tiers: 3, sides: 5, lift: 0.1, jitter: 0.1, trunk_only: false };
        base.trunk_style = TrunkStyle::Prism { sides: 5 };
        let paths = fields::numeric(&mut base);
        assert!(paths.len() > 90, "found {} numeric fields", paths.len());
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e30, -1e30] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                let mut clean = params.sanitized().into_owned();
                // Whatever is left (a `NaN` colour is gone, as `None`) is finite, and a size is no bigger
                // than any range: a colour's channel may be any finite number.
                for left in fields::numeric(&mut clean) {
                    let value = fields::get(&mut clean, &left);
                    let is_colour = ["color", "tint", "autumn", "blossom"].iter().any(|word| left.contains(word));
                    assert!(value.is_finite(), "{path} = {hostile} left {left} = {value}");
                    assert!(is_colour || value.abs() <= 1.0e4, "{path} = {hostile} left {left} = {value}");
                }
            }
        }
    }

    /// A mesh a renderer can take: finite, unit normals, indices inside the vertices, whole triangles.
    fn assert_sound(mesh: &crate::mesh::MeshData, what: &str) {
        assert!(mesh.positions.iter().all(|p| p.is_finite()), "{what}: a position is not finite");
        assert!(mesh.normals.iter().all(|n| n.is_finite()), "{what}: a normal is not finite");
        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()), "{what}: an index is out of range");
        assert_eq!(mesh.indices.len() % 3, 0, "{what}: not whole triangles");
    }

    fn assert_tree_sound(tree: &crate::TreeMeshes, what: &str) {
        assert_sound(&tree.trunk, what);
        assert_sound(&tree.foliage, what);
        assert_sound(&tree.flowers, what);
        assert!(!tree.skeleton.nodes.is_empty() && tree.skeleton.nodes.len() <= MAX_NODES, "{what}: {} nodes", tree.skeleton.nodes.len());
    }

    /// A birch, the cheapest tree with leaves, that also has every optional part set.
    fn birch_with_everything() -> TreeParams {
        let mut params = TreePreset::Birch.params();
        params.foliage.fruit = Some(FruitParams::default());
        params.foliage.autumn = Some(Color::srgb(0.8, 0.3, 0.1));
        params.foliage.blossom = Some(Color::srgb(1.0, 0.8, 0.9));
        params
    }

    #[test]
    fn a_hostile_value_in_any_field_still_builds_a_sound_tree() {
        use crate::{LodSettings, build_lods, build_tree};
        let base = birch_with_everything();
        let mut probe = base.clone();
        let paths = fields::numeric(&mut probe);
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 1e30, f64::from(u32::MAX)] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                let what = format!("{path} = {hostile}");
                assert_tree_sound(&build_tree(&params, f32::INFINITY), &what);
                // Young as well (the aged skeleton), with the levels of detail.
                for level in &build_lods(&params, 3.0, &LodSettings::default()) {
                    assert_tree_sound(level, &what);
                }
            }
        }
    }

    #[test]
    fn a_hull_and_a_prism_with_hostile_numbers_build_too() {
        use crate::build_tree;
        let mut base = TreePreset::Spruce.params();
        base.crown = CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.2, trunk_only: false };
        base.trunk_style = TrunkStyle::Prism { sides: 5 };
        let mut probe = base.clone();
        let paths: Vec<String> = fields::numeric(&mut probe).into_iter().filter(|p| p.starts_with(".crown") || p.starts_with(".trunk_style")).collect();
        assert_eq!(paths.len(), 5, "{paths:?}");
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, -1.0, f64::from(u32::MAX)] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                assert_tree_sound(&build_tree(&params, f32::INFINITY), &format!("{path} = {hostile}"));
            }
        }
    }

    /// `base` with everything that makes a tree large at the top of its range: a 500 m trunk 50 m
    /// thick, a thousand branches a metre along it, each 32 trunk-heights long, 8 levels of 32 sub-branches,
    /// strands, every ring 64 sides, smoothing, and 512 leaves a metre.
    fn everything_at_its_largest(base: &TreeParams) -> TreeParams {
        let mut params = base.clone();
        for path in [
            ".trunk.height",
            ".trunk.radius",
            ".trunk.radial_segments",
            ".trunk.height_segments",
            ".branches.density",
            ".branches.length",
            ".branches.recursion",
            ".branches.sub_branch_count",
            ".branches.gravity_strength",
            ".branches.strands.density",
            ".branches.strands.length",
            ".branches.strands.spread",
            ".mesh.smooth_iterations",
            ".mesh.min_radial_segments",
            ".mesh.max_radial_segments",
            ".mesh.resolution_scale",
            ".mesh.collar_length",
            ".foliage.density",
            ".foliage.cluster_size",
            ".foliage.leaf_size",
        ] {
            fields::set(&mut params, path, 1e30);
        }
        params.branches.stiffness = 0.0;
        params.mesh.adaptive_resolution = true;
        params
    }

    #[test]
    fn the_leaves_the_ranges_allow_are_cut_at_the_leaf_budget() {
        use crate::build_tree;
        use crate::foliage::collect_leaf_points;
        let mut base = birch_with_everything();
        base.foliage.placement = crate::FoliagePlacement::AllBranches;
        let params = everything_at_its_largest(&base);
        let clean = params.sanitized();
        assert_eq!((clean.trunk.height, clean.branches.sub_branch_count, clean.foliage.density), (500.0, 32, 512.0));
        let tree = build_tree(&params, f32::INFINITY);
        assert_tree_sound(&tree, "every size at its largest");
        let leaves = collect_leaf_points(&tree.skeleton, &clean.foliage, &clean.growth, &clean.trunk, clean.seed).len();
        assert!(leaves <= MAX_LEAVES && leaves > MAX_LEAVES / 2, "{leaves} leaves for a budget of {MAX_LEAVES}");
    }

    #[test]
    fn the_wood_the_ranges_allow_is_cut_at_the_node_budget() {
        use crate::build_tree;
        let mut base = birch_with_everything();
        base.foliage.enabled = false;
        let params = everything_at_its_largest(&base);
        let tree = build_tree(&params, f32::INFINITY);
        assert_tree_sound(&tree, "every size at its largest, no leaves");
        assert!(tree.skeleton.nodes.len() > MAX_NODES * 9 / 10, "{} nodes", tree.skeleton.nodes.len());
    }

    #[test]
    fn an_age_of_nan_is_an_age_of_zero() {
        use crate::build_tree;
        let params = TreePreset::Oak.params();
        let nan = build_tree(&params, f32::NAN);
        let zero = build_tree(&params, 0.0);
        assert_eq!(nan.trunk.positions, zero.trunk.positions);
        assert_eq!(nan.foliage.positions, zero.foliage.positions);
    }

    #[test]
    fn no_tree_the_crate_draws_comes_near_the_budget() {
        use crate::foliage::leaf_bound;
        // The presets, and the gallery's largest scenario (`extreme_giant`: an oak 40 m tall, 3 m thick).
        let mut giant = TreePreset::Oak.params();
        giant.trunk.height = 40.0;
        giant.trunk.radius = 3.0;
        let trees = TreePreset::ALL.map(|preset| (preset.name(), preset.params())).into_iter().chain([("giant oak", giant)]);
        for (name, params) in trees {
            let skeleton = crate::generate_skeleton(&params);
            let bound: usize = skeleton.nodes.iter().map(|n| leaf_bound(&params.foliage, n.length, n.height_ratio)).sum();
            assert!(skeleton.nodes.len() * 4 <= MAX_NODES, "{name}: {} nodes", skeleton.nodes.len());
            assert!(bound * 2 <= MAX_LEAVES, "{name}: could bear {bound} leaves");
        }
    }

    #[test]
    fn a_tube_never_has_more_sides_than_the_range_says() {
        // `skeleton_mesh` is public and takes parameters from a caller: 4 billion sides is 64.
        let mut params = TreePreset::Oak.params();
        params.trunk.radial_segments = u32::MAX;
        params.trunk.height_segments = u32::MAX;
        params.mesh.smooth_iterations = u32::MAX;
        params.mesh.adaptive_resolution = true;
        params.mesh.min_radial_segments = u32::MAX;
        params.mesh.max_radial_segments = u32::MAX;
        let skeleton = crate::generate_skeleton(&TreePreset::Oak.params());
        assert_sound(&crate::mesh::branch::skeleton_mesh(&skeleton, &params), "skeleton_mesh");
    }

    /// A birch with every optional part a tree can have, so each number of the catalogue is there to read.
    fn with_every_part() -> TreeParams {
        let mut params = birch_with_everything();
        params.crown = CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.2, trunk_only: false };
        params.trunk_style = TrunkStyle::Prism { sides: 5 };
        params
    }

    /// The reflection path (`fields::get`) of a catalogue path. Reflection names the fields of an enum's active
    /// variant without the variant (`crown.tiers`) and reads an optional part through its tuple field
    /// (`fruit.0.radius`).
    fn reflected(path: &str) -> String {
        let path = path.replace("crown.Hull.", "crown.").replace("trunk_style.Prism.", "trunk_style.").replace("foliage.fruit.", "foliage.fruit.0.");
        format!(".{path}")
    }

    /// The catalogue path of a reflection path: the inverse of [`reflected`].
    fn catalogued(path: &str) -> String {
        let path = path.trim_start_matches('.');
        if let Some(rest) = path.strip_prefix("crown.") {
            return format!("crown.Hull.{rest}");
        }
        if let Some(rest) = path.strip_prefix("trunk_style.") {
            return format!("trunk_style.Prism.{rest}");
        }
        path.replace("foliage.fruit.0.", "foliage.fruit.")
    }

    #[test]
    fn the_catalogue_lists_every_number_of_a_tree_and_only_those() {
        // Found by reflection, which shares nothing with the table: a number a later field adds is missing
        // from the table and fails here. A colour is an enum of `f32` channels, not a number in a range; the
        // six colour fields of a `TreeParams` are named, so a seventh fails here until it is decided.
        const COLOURS: [&str; 6] = ["trunk_color.", "foliage.color.", "foliage.shading.transmission_tint.", "foliage.autumn.", "foliage.blossom.", "foliage.fruit.0.color."];
        let mut params = with_every_part();
        let mut found: Vec<String> = fields::numeric(&mut params)
            .into_iter()
            .map(|path| path.trim_start_matches('.').to_string())
            .filter(|path| !COLOURS.iter().any(|colour| path.starts_with(colour)))
            .map(|path| catalogued(&path))
            .collect();
        let mut listed: Vec<String> = TreeParams::fields().iter().map(|field| field.path.to_string()).collect();
        assert!(listed.len() > 90, "{} numbers listed", listed.len());
        found.sort();
        listed.sort();
        assert_eq!(listed, found);
        listed.dedup();
        assert_eq!(listed.len(), TreeParams::fields().len(), "a path is listed twice");
    }

    #[test]
    fn every_row_reads_and_writes_the_field_its_path_names() {
        // Reading: the number the row reads is the one reflection finds at its path, in every preset (the
        // apple's fruit is there, the other 19 presets have none) and in a tree with every part.
        let mut trees: Vec<TreeParams> = TreePreset::ALL.map(|preset| preset.params()).into();
        trees.push(with_every_part());
        for params in trees {
            let mut probe = params.clone();
            for field in TreeParams::fields() {
                let found = fields::get(&mut probe, &reflected(field.path));
                match field.value(&params) {
                    Some(value) => assert_eq!(f64::from(value), found, "{}", field.path),
                    None => assert!(found.is_nan(), "{} reads nothing but reflection finds {found}", field.path),
                }
            }
        }
        // Writing: a row changes its own number and no other.
        for field in TreeParams::fields() {
            let before = with_every_part();
            let mut after = before.clone();
            let value = if field.value(&before) == Some(field.min) { field.max } else { field.min };
            field.set(&mut after, value).unwrap_or_else(|refused| panic!("{}: {refused}", field.path));
            assert_eq!(field.value(&after), Some(value), "{}", field.path);
            assert_eq!(moved(&before, &after), vec![reflected(field.path)], "{}", field.path);
        }
    }

    #[test]
    fn a_default_is_the_default_trees_number_and_a_range_holds_it() {
        let default = TreeParams::default();
        for field in TreeParams::fields() {
            assert!(field.min < field.max, "{}", field.path);
            assert!(field.holds(field.default), "{}: default {} is outside {}..={}", field.path, field.default, field.min, field.max);
            if let Some(value) = field.value(&default) {
                assert_eq!(field.default, value, "{} is not the default tree's number", field.path);
            }
            if field.kind == ParamKind::Count {
                assert!([field.min, field.max, field.default].iter().all(|n| n.fract() == 0.0), "{}", field.path);
            }
        }
        // The presets are inside every range, so a panel's slider can always show them.
        for preset in TreePreset::ALL {
            let params = preset.params();
            for field in TreeParams::fields() {
                if let Some(value) = field.value(&params) {
                    assert!(field.holds(value), "{} {} = {value}", preset.name(), field.path);
                }
            }
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_path_is_the_dotted_path_of_the_serde_form() {
        // What an editor's `{"trunk.radius": 0.6}` map walks: each path finds the same number in the JSON.
        let params = with_every_part();
        let json = serde_json::to_value(&params).expect("a tree is JSON");
        for field in TreeParams::fields() {
            let mut at = &json;
            for part in field.path.split('.') {
                at = at.get(part).unwrap_or_else(|| panic!("{}: the JSON has no `{part}`", field.path));
            }
            let number = at.as_f64().unwrap_or_else(|| panic!("{}: {at} is not a number", field.path));
            assert_eq!(Some(number as f32), field.value(&params), "{}", field.path);
        }
    }

    #[test]
    fn a_value_the_door_takes_is_one_sanitized_leaves_alone() {
        for field in TreeParams::fields() {
            for value in [field.min, field.max, field.default] {
                let mut params = with_every_part();
                field.set(&mut params, value).unwrap_or_else(|refused| panic!("{} = {value}: {refused}", field.path));
                assert_eq!(field.value(&params), Some(value), "{}", field.path);
                assert!(matches!(params.sanitized(), Cow::Borrowed(_)), "{} = {value} is taken and then changed", field.path);
            }
        }
    }

    #[test]
    fn a_value_past_the_range_is_the_nearest_end_to_the_generator() {
        for field in TreeParams::fields() {
            // A `u32` cannot go below 0, so a count whose range starts there has no value to be past it.
            let past_min = (field.min - 1.0 - field.min.abs(), field.min);
            let past_max = (field.max + 1.0 + field.max.abs(), field.max);
            let below = !(field.kind == ParamKind::Count && field.min == 0.0);
            for (past, end) in [past_max].into_iter().chain(below.then_some(past_min)) {
                let mut params = with_every_part();
                fields::set(&mut params, &reflected(field.path), f64::from(past));
                assert!(!field.holds(field.value(&params).expect("the part is there")), "{}", field.path);
                assert_eq!(field.value(&params.sanitized()), Some(end), "{} = {past}", field.path);
            }
        }
    }

    #[test]
    fn a_refused_edit_says_why_and_changes_nothing() {
        let mut params = with_every_part();
        let before = params.clone();
        let height = params.set_field("trunk.height", 1e9).expect_err("1e9 m");
        assert_eq!(height, ParamError::OutOfRange { path: "trunk.height", value: 1e9, min: 0.0, max: 500.0, unit: ParamUnit::Metres });
        assert_eq!(height.to_string(), "`trunk.height` is held between 0 m and 500 m; 1000000000 is out of range");
        let below = params.set_field("branches.angle", -361.0).expect_err("-361 degrees");
        assert_eq!(below.to_string(), "`branches.angle` is held between -360° and 360°; -361 is out of range");
        let nan = params.set_field("trunk.radius", f32::NAN).expect_err("NaN");
        assert!(matches!(nan, ParamError::NotFinite { path: "trunk.radius", .. }), "{nan}");
        let inf = params.set_field("trunk.radius", f32::INFINITY).expect_err("inf");
        assert!(matches!(inf, ParamError::NotFinite { .. }), "{inf}");
        let fraction = params.set_field("trunk.radial_segments", 8.5).expect_err("8.5 sides");
        assert_eq!(fraction.to_string(), "`trunk.radial_segments` counts whole things; 8.5 is not a whole number");
        let few = params.set_field("trunk.radial_segments", 2.0).expect_err("2 sides");
        assert_eq!(few.to_string(), "`trunk.radial_segments` is held between 3 and 64; 2 is out of range");
        let negative = params.set_field("branches.recursion", -1.0).expect_err("-1 levels");
        assert!(matches!(negative, ParamError::OutOfRange { path: "branches.recursion", .. }), "{negative}");
        assert_eq!(params, before, "a refusal changed the tree");
    }

    #[test]
    fn an_unknown_path_names_the_paths_it_is_near() {
        let mut params = TreePreset::Oak.params();
        let typo = params.set_field("trunk.hieght", 4.0).expect_err("a typo");
        let ParamError::Unknown { path, nearest } = &typo else { panic!("{typo}") };
        assert_eq!(path, "trunk.hieght");
        assert_eq!(nearest.first(), Some(&"trunk.height"), "{nearest:?}");
        assert!(nearest.len() <= SUGGESTIONS);
        // The last word alone finds its fields wherever they are.
        let word = params.set_field("radius", 1.0).expect_err("no such path");
        let ParamError::Unknown { nearest, .. } = &word else { panic!("{word}") };
        assert!(nearest.contains(&"trunk.radius"), "{nearest:?}");
        // Nothing like it: the message says where the list is.
        let lost = params.set_field("zzzzzzzzzzzzzzzzzzzz", 1.0).expect_err("nothing");
        assert_eq!(lost.to_string(), "`zzzzzzzzzzzzzzzzzzzz` is not a number of a tree; `TreeParams::fields()` lists them");
        assert_eq!(params.field("trunk.hieght"), None);
    }

    #[test]
    fn a_part_the_tree_does_not_have_has_no_number_to_set() {
        let mut oak = TreePreset::Oak.params();
        assert!(oak.foliage.fruit.is_none() && oak.crown == CrownStyle::Leaves && oak.trunk_style == TrunkStyle::Round);
        for (path, reason) in [
            ("foliage.fruit.radius", "the tree bears no fruit"),
            ("crown.Hull.tiers", "the crown is not a hull"),
            ("trunk_style.Prism.sides", "the trunk is not a prism"),
        ] {
            let refused = oak.set_field(path, 4.0).expect_err("the part is not there");
            assert!(matches!(refused, ParamError::Absent { path: p } if p == path), "{refused}");
            assert!(refused.to_string().contains(reason), "{refused}");
            assert_eq!(oak.field(path), None);
        }
        assert_eq!(oak, TreePreset::Oak.params());
        // Once the part is set, its numbers are there.
        oak.foliage.fruit = Some(FruitParams::default());
        oak.set_field("foliage.fruit.share", 0.5).expect("the oak bears fruit now");
        assert_eq!(oak.field("foliage.fruit.share"), Some(0.5));
    }
}
