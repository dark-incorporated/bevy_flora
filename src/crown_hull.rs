//! The faceted crown of a stylised conifer ([`CrownStyle::Hull`](crate::CrownStyle)): a stack of
//! cones wrapped round a tree's leaf cloud. Pure CPU, depends only on `bevy::math`.
//!
//! The cloud is cut into `tiers` bands from its base to its top. Each band gets a ring of `sides`
//! points at the 90th percentile of its leaves' distance from the trunk (never wider than the band
//! below, never a needle), and each ring is a cone: `sides` side faces up to an apex and `sides`
//! underside faces to a point just above the ring. The faces are flat and face outward, so the
//! crown reads as a few big planes. The numbers are those of the hull Sanctuary's forest drew
//! before the crate did.
//!
//! Everything that is random (each ring point's radius and height, each tier's turn) is drawn from
//! the tree's seed and the tier and side's index alone, so a hull never depends on age beyond the
//! cloud it wraps.

use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};

use crate::rng::SeededRng;

/// Fewer cloud points than this and a leaf cloud is too sparse to wrap a crown round.
pub const MIN_POINTS: usize = 8;

/// A crown shorter than this, in metres, is not worth drawing.
pub const MIN_CROWN_HEIGHT: f32 = 0.5;

/// Seed salt of the rings' jitter: its own random stream, so the hull moves no other draw.
const HULL_SALT: i32 = 0x4855_4C4C;

/// Seed salt of each tier's turn.
const TURN_SALT: i32 = 0x5455_524E;

/// How a hull is stacked: [`CrownStyle::Hull`](crate::CrownStyle)'s fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullSpec {
    /// Cones stacked from the crown's base to its top (at least 1).
    pub tiers: u32,
    /// Facets round each cone (at least 3).
    pub sides: u32,
    /// The crown starts at least this share of its top's height above the root.
    pub lift: f32,
    /// How far the rings are knocked about, 0 (regular cones) to 1.
    pub jitter: f32,
}

impl HullSpec {
    /// This hull with fewer facets, as a coarse level of detail draws it: `tiers` becomes
    /// `max(2, round(tiers * detail))` and `sides` `max(3, round(sides * detail))`.
    pub fn scaled(self, detail: f32) -> Self {
        let scale = |n: u32, least: u32| ((n as f32 * detail).round() as u32).max(least);
        Self { tiers: scale(self.tiers, 2), sides: scale(self.sides, 3), ..self }
    }
}

/// One flat triangle of a hull and where it sits in the stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facet {
    /// The corners, counter-clockwise seen from outside (from below for an underside face).
    pub triangle: [Vec3; 3],
    /// The tier it belongs to, 0 lowest.
    pub tier: u32,
    /// Which of the tier's `sides` faces it is (the side face and the underside face of one `side`
    /// stand on the same two ring points).
    pub side: u32,
    /// Whether it is an underside face rather than a side face.
    pub under: bool,
}

impl Facet {
    /// The unit normal of the face, out of the crown.
    pub fn normal(&self) -> Vec3 {
        let [a, b, c] = self.triangle;
        (b - a).cross(c - a).normalize_or(Vec3::Y)
    }
}

/// A faceted stacked-cone crown: per tier, `sides` side faces, then `sides` underside faces.
#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    /// Cones in the stack.
    pub tiers: u32,
    /// Facets round each cone.
    pub sides: u32,
    /// `tiers * sides * 2` faces.
    pub facets: Vec<Facet>,
}

impl Hull {
    /// Move every corner inside the box `lo..hi`, so a coarse level never reaches beyond the
    /// level it stands in for.
    pub fn clamp_to(&mut self, lo: Vec3, hi: Vec3) {
        for facet in &mut self.facets {
            for corner in &mut facet.triangle {
                *corner = corner.clamp(lo, hi);
            }
        }
    }
}

/// The `p`-quantile (0..=1) of `values`, by nearest rank; 0 for an empty slice.
pub fn percentile(values: &mut [f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let last = values.len() - 1;
    values[((last as f32 * p).round() as usize).min(last)]
}

/// The vertical extent `(base, top)` a crown covers: `top` is the highest point of the cloud or
/// the trunk's top, `base` the 5th percentile of the points' heights (a stray low twig does not
/// drag the crown to the ground), lifted to at least `lift` of `top`. `None` for a sparse or flat
/// cloud.
pub fn crown_extent(cloud: &[Vec3], trunk_top: f32, lift: f32) -> Option<(f32, f32)> {
    if cloud.len() < MIN_POINTS {
        return None;
    }
    let top = cloud.iter().fold(trunk_top, |top, p| top.max(p.y));
    let mut heights: Vec<f32> = cloud.iter().map(|p| p.y).collect();
    let base = percentile(&mut heights, 0.05).max(lift * top);
    (top - base >= MIN_CROWN_HEIGHT).then_some((base, top))
}

/// One band of the cloud between two heights.
#[derive(Default)]
struct Band {
    /// Horizontal distance of each point from the trunk axis.
    radii: Vec<f32>,
    /// Sum of the points' `(x, z)`.
    sum: Vec2,
}

/// The hull round `cloud` (tree metres, y up, the trunk on the y axis), or `None` for a sparse
/// cloud or one under [`MIN_CROWN_HEIGHT`] tall. `trunk_top` is the trunk's top height. Points below
/// the lifted base count toward the lowest tier, so the skirt keeps the width of the tree's
/// lowest branches; no ring dips below `lift` of the top. `seed` is the tree's.
pub fn crown_triangles(cloud: &[Vec3], trunk_top: f32, spec: HullSpec, seed: i32) -> Option<Hull> {
    let lift = spec.lift.clamp(0.0, 0.9);
    let jitter = spec.jitter.clamp(0.0, 1.0);
    let (base, top) = crown_extent(cloud, trunk_top, lift)?;
    let tiers = spec.tiers.max(1) as usize;
    let sides = spec.sides.max(3) as usize;
    let band_height = (top - base) / tiers as f32;

    let mut bands: Vec<Band> = (0..tiers).map(|_| Band::default()).collect();
    let mut all_radii = Vec::with_capacity(cloud.len());
    for p in cloud {
        let radius = p.x.hypot(p.z);
        all_radii.push(radius);
        let i = (((p.y - base) / band_height).floor().max(0.0) as usize).min(tiers - 1);
        bands[i].radii.push(radius);
        bands[i].sum += Vec2::new(p.x, p.z);
    }
    let whole_cloud_radius = percentile(&mut all_radii, 0.9);

    // Tier radius: the 90th percentile of the band's points, never wider than the tier below and
    // never a needle. A tier with almost no points keeps the shape of the one under it.
    let mut radii: Vec<f32> = Vec::with_capacity(tiers);
    let mut centres: Vec<Vec2> = Vec::with_capacity(tiers);
    for band in &mut bands {
        let below = radii.last().copied();
        let mut radius = if band.radii.len() >= 3 {
            percentile(&mut band.radii, 0.9)
        } else {
            below.map_or(whole_cloud_radius, |below| 0.85 * below)
        };
        if let Some(below) = below {
            radius = radius.min(below);
        }
        radii.push(radius.max(0.06 * (top - base)));
        centres.push(if band.radii.is_empty() { Vec2::ZERO } else { 0.5 * band.sum / band.radii.len() as f32 });
    }

    let mut facets = Vec::with_capacity(tiers * sides * 2);
    for i in 0..tiers {
        let band_bottom = base + i as f32 * band_height;
        let ring_height = band_bottom - 0.25 * band_height;
        let centre = centres[i];
        let turn = SeededRng::from_parts(seed ^ TURN_SALT, i as u32).next_f32() * jitter;
        let phase = turn * TAU / sides as f32;
        let ring: Vec<Vec3> = (0..sides)
            .map(|j| {
                let mut rng = SeededRng::from_parts(seed ^ HULL_SALT, (i * 64 + j) as u32);
                let angle = TAU * j as f32 / sides as f32 + phase;
                let radius = 1.08 * radii[i] * (1.0 + 0.16 * jitter * (2.0 * rng.next_f32() - 1.0));
                let y = (ring_height + 0.12 * jitter * band_height * (2.0 * rng.next_f32() - 1.0)).max(lift * top);
                Vec3::new(centre.x + radius * angle.cos(), y, centre.y + radius * angle.sin())
            })
            .collect();
        let apex_y = if i + 1 == tiers { top } else { band_bottom + band_height + 0.55 * band_height };
        let apex = Vec3::new(centre.x, apex_y, centre.y);
        let under = Vec3::new(centre.x, ring_height + 0.35 * band_height, centre.y);
        // With x = cos and z = sin, `(next, this, apex)` winds counter-clockwise seen from
        // outside, and `(this, next, under)` seen from below.
        for j in 0..sides {
            facets.push(Facet {
                triangle: [ring[(j + 1) % sides], ring[j], apex],
                tier: i as u32,
                side: j as u32,
                under: false,
            });
        }
        for j in 0..sides {
            facets.push(Facet {
                triangle: [ring[j], ring[(j + 1) % sides], under],
                tier: i as u32,
                side: j as u32,
                under: true,
            });
        }
    }
    Some(Hull { tiers: tiers as u32, sides: sides as u32, facets })
}

/// The spruce preset as a stylised conifer, for the tests of every path a hull crown and a prism
/// trunk go through: a 5 x 7 hull lifted 0.2, a five-sided trunk.
#[cfg(test)]
pub(crate) fn hull_spruce() -> crate::params::TreeParams {
    use crate::params::{CrownStyle, TrunkStyle};
    let mut params = crate::presets::TreePreset::Spruce.params();
    params.crown = CrownStyle::Hull { tiers: 5, sides: 7, lift: 0.2, jitter: 1.0 };
    params.trunk_style = TrunkStyle::Prism { sides: 5 };
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: HullSpec = HullSpec { tiers: 5, sides: 7, lift: 0.2, jitter: 1.0 };

    /// `count` points on a cone of base radius 3 m and height 10 m, from `from` up.
    fn cone_cloud(count: u32, from: f32, stream: u32) -> Vec<Vec3> {
        let mut rng = SeededRng::from_parts(1000, stream);
        (0..count)
            .map(|_| {
                let y = from + (10.0 - from) * rng.next_f32();
                let angle = TAU * rng.next_f32();
                let radius = 3.0 * (1.0 - y / 10.0);
                Vec3::new(radius * angle.cos(), y, radius * angle.sin())
            })
            .collect()
    }

    fn hull(cloud: &[Vec3], spec: HullSpec, seed: i32) -> Hull {
        crown_triangles(cloud, 0.0, spec, seed).expect("a 400-point cone gives a hull")
    }

    #[test]
    fn a_hull_has_tiers_times_sides_times_two_triangles() {
        for (tiers, sides) in [(5, 7), (6, 6), (3, 5), (1, 3), (2, 12)] {
            let spec = HullSpec { tiers, sides, ..SPEC };
            let h = hull(&cone_cloud(400, 1.0, 1), spec, 3);
            assert_eq!(h.facets.len() as u32, tiers * sides * 2, "{tiers}x{sides}");
            assert_eq!((h.tiers, h.sides), (tiers, sides));
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_hull_and_another_seed_a_different_one() {
        let cloud = cone_cloud(400, 1.0, 2);
        assert_eq!(hull(&cloud, SPEC, 7), hull(&cloud, SPEC, 7));
        assert_ne!(hull(&cloud, SPEC, 7), hull(&cloud, SPEC, 8));
        // No jitter, no seed.
        let plain = HullSpec { jitter: 0.0, ..SPEC };
        assert_eq!(hull(&cloud, plain, 7), hull(&cloud, plain, 8));
    }

    #[test]
    fn a_sparse_cloud_gets_no_hull() {
        let few: Vec<Vec3> = cone_cloud(400, 1.0, 3).into_iter().take(MIN_POINTS - 1).collect();
        assert!(crown_triangles(&few, 0.0, SPEC, 1).is_none());
        assert!(crown_triangles(&[], 0.0, SPEC, 1).is_none());
        // Plenty of points, but a crown under half a metre tall.
        let flat: Vec<Vec3> = cone_cloud(400, 1.0, 3).into_iter().map(|p| Vec3::new(p.x, 1.0 + 0.04 * p.y, p.z)).collect();
        assert!(crown_triangles(&flat, 0.0, SPEC, 1).is_none());
    }

    #[test]
    fn facets_face_out_and_the_top_is_the_clouds() {
        let cloud = cone_cloud(400, 1.0, 4);
        let h = hull(&cloud, SPEC, 7);
        for (index, facet) in h.facets.iter().enumerate() {
            let normal = facet.normal();
            let centroid = facet.triangle.iter().sum::<Vec3>() / 3.0;
            if facet.under {
                assert!(normal.y < 0.2, "underside face {index} faces up: {normal}");
            } else {
                // The tier centres sit within a few decimetres of the cone's axis, well inside a
                // facet's reach.
                let outward = normal.x * centroid.x + normal.z * centroid.z;
                assert!(outward > 0.0, "side face {index} faces inward: normal {normal}, centroid {centroid}");
            }
            assert_eq!(facet.under, index % (2 * h.sides as usize) >= h.sides as usize, "{index}");
        }
        let cloud_top = cloud.iter().fold(0.0_f32, |top, p| top.max(p.y));
        let crown_top = h.facets.iter().flat_map(|f| f.triangle).fold(0.0_f32, |top, p| top.max(p.y));
        assert!((crown_top - cloud_top).abs() <= 0.01 * cloud_top, "{crown_top} vs {cloud_top}");
    }

    #[test]
    fn tiers_never_widen_upward() {
        let spec = HullSpec { jitter: 0.0, ..SPEC };
        let h = hull(&cone_cloud(600, 1.0, 5), spec, 1);
        let mut below = f32::MAX;
        for tier in 0..h.tiers {
            // The ring of a tier is the first two corners of its side faces.
            let ring: Vec<Vec3> =
                h.facets.iter().filter(|f| f.tier == tier && !f.under).map(|f| f.triangle[1]).collect();
            let centre = ring.iter().sum::<Vec3>() / ring.len() as f32;
            let radius = ring.iter().map(|p| Vec2::new(p.x - centre.x, p.z - centre.z).length()).sum::<f32>() / ring.len() as f32;
            assert!(radius <= below + 1e-4, "tier {tier} is {radius} wide over one {below} wide");
            below = radius;
        }
    }

    #[test]
    fn a_hull_stays_clear_of_the_ground_but_keeps_its_width() {
        // Points down to the root: the crown still starts at `lift` of the top, and its skirt is
        // as wide as the lowest branches (3 m), not the narrower cloud at the lift height.
        let h = hull(&cone_cloud(400, 0.0, 6), HullSpec { tiers: 5, sides: 7, lift: 0.2, jitter: 1.0 }, 3);
        let vertices: Vec<Vec3> = h.facets.iter().flat_map(|f| f.triangle).collect();
        let top = vertices.iter().fold(0.0_f32, |top, p| top.max(p.y));
        let lowest = vertices.iter().fold(f32::MAX, |low, p| low.min(p.y));
        assert!(lowest >= 0.2 * top - 1e-4, "crown reaches down to {lowest} of {top}");
        let widest = vertices.iter().fold(0.0_f32, |wide, p| wide.max(p.x.hypot(p.z)));
        assert!(widest >= 2.5, "skirt only {widest} m wide");
    }

    #[test]
    fn a_coarse_hull_has_fewer_facets_and_clamps_into_a_box() {
        let spec = SPEC.scaled(0.5);
        assert_eq!((spec.tiers, spec.sides), (3, 4));
        // Never fewer than 2 tiers and 3 sides.
        let tiny = HullSpec { tiers: 3, sides: 4, ..SPEC }.scaled(0.1);
        assert_eq!((tiny.tiers, tiny.sides), (2, 3));
        let mut h = hull(&cone_cloud(400, 1.0, 7), SPEC, 3);
        let (lo, hi) = (Vec3::new(-1.0, 0.5, -1.0), Vec3::new(1.0, 6.0, 1.0));
        h.clamp_to(lo, hi);
        assert!(h.facets.iter().flat_map(|f| f.triangle).all(|p| p.cmpge(lo).all() && p.cmple(hi).all()));
    }
}
