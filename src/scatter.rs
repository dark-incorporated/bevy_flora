//! Where the trees of a forest stand: trunks never closer than a spacing and otherwise
//! irregular (Poisson-disc sampling, Bridson 2007), species growing in groves instead of salt and
//! pepper, and every tree its own yaw, size and pool variant. Pure CPU and deterministic: the
//! same [`Scatter`] gives the same forest on every run.
//!
//! ```
//! use bevy::math::{Rect, Vec2};
//! use bevy_flora::scatter::Scatter;
//!
//! // Three oaks to every birch, the birches in groves about 40 m across, trunks at least 7 m
//! // apart, over a 200 m square with a clearing 20 m around the middle.
//! let forest = Scatter {
//!     species: vec![3.0, 1.0],
//!     grove_size: 40.0,
//!     ..Scatter::new(Rect::new(-100.0, -100.0, 100.0, 100.0), 7.0)
//! };
//! let trees = forest.points_where(|at| if at.length() < 20.0 { 0.0 } else { 1.0 });
//! assert!(trees.iter().all(|tree| tree.position.length() >= 20.0));
//! // Each becomes a `PooledTree { pool: pools[tree.species].clone(), variant: tree.variant }`
//! // with `tree.transform(0.0)` on flat ground, or `tree.transform_on(&ground, &params)` on a
//! // `FloraGround` (terrain).
//! ```

use std::f32::consts::{SQRT_2, TAU};

use bevy::math::{Quat, Rect, Vec2, Vec3};
use bevy::reflect::Reflect;
use bevy::transform::components::Transform;

use crate::ground::FloraGround;
use crate::params::TreeParams;
use crate::rng::SeededRng;

/// Candidates Bridson's algorithm tries around a point before it retires it.
const CANDIDATES: u32 = 30;
/// Random streams: the trunks' disc, the groves' disc, the groves' species, and one stream per
/// tree from here on. The two warp fields take the seed with these salts.
const TRUNK_STREAM: u32 = 0;
const GROVE_STREAM: u32 = 1;
const GROVE_SPECIES_STREAM: u32 = 2;
const TREE_STREAMS: u32 = 16;
const WARP_X_SALT: i32 = 0x3A5C_29E1;
const WARP_Y_SALT: i32 = 0x0F1E_7B4D;
/// How far a grove's border wanders, as a share of the grove size, and how wide its bends are
/// (as a share too): borders are smooth curves, not the straight edges of nearest-centre cells.
const GROVE_RAGGEDNESS: f32 = 0.35;
const GROVE_BEND: f32 = 0.5;

/// How to spread trees over an area. Build one with [`Scatter::new`] and set what differs.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scatter {
    /// The ground to cover: world x along the rectangle's x, world z along its y.
    pub area: Rect,
    /// The least distance between two trunks, in world units. Trees fill the area as densely as that
    /// allows: about `0.7 * area / spacing²` of them.
    pub spacing: f32,
    /// The same seed gives the same forest; another gives another.
    pub seed: i32,
    /// The relative share of each species, by index (yours: an index into your pools). Empty
    /// means one species, `0`; a weight of 0 or less is never picked.
    pub species: Vec<f32>,
    /// How wide a grove of one species grows, in world units. At 0 every tree draws its own species;
    /// above it, the area is cut into groves about this wide (with ragged edges), each of one
    /// species drawn by the weights, so a forest reads as stands of birch among the oaks.
    pub grove_size: f32,
    /// The smallest and largest uniform scale a tree is drawn at.
    pub scale: (f32, f32),
}

impl Scatter {
    /// One species over `area`, trunks at least `spacing` world units apart, sizes from 0.85 to 1.15,
    /// seed 0.
    pub fn new(area: Rect, spacing: f32) -> Self {
        Self { area, spacing, seed: 0, species: Vec::new(), grove_size: 0.0, scale: (0.85, 1.15) }
    }

    /// Every tree of the forest.
    pub fn points(&self) -> Vec<ScatterPoint> {
        self.points_where(|_| 1.0)
    }

    /// The trees, thinned by `density`: a tree at `p` (world x and z) stays with probability
    /// `density(p)` (clamped to 0..1). Give 0 for clearings, roads, water or buildings and a
    /// fraction for sparse woodland; the trees that stay stand exactly where they stand in
    /// [`Scatter::points`], so a mask only ever removes trees.
    pub fn points_where(&self, density: impl FnMut(Vec2) -> f32) -> Vec<ScatterPoint> {
        self.points_with(density, |_, _| 1.0)
    }

    /// The trees, thinned by `density` as in [`points_where`](Self::points_where), each species'
    /// share also weighted by where the tree stands: `weight(p, species)` multiplies the share
    /// of `species` (an index into [`species`](Self::species)) at `p`. Stands the game places:
    /// weight only the snags inside a dead patch, or the saplings along the forest's edge. A weight
    /// of 0 shuts a species out there; where every weight is 1 the trees are those of
    /// `points_where`, species and all. With groves, a tree joins the nearest grove whose
    /// species is weighted, distances divided by the weights, so inside a stand of two species
    /// the groves still meet along bent borders, and a weight fading out over a rim bends the
    /// stand's edge with them.
    pub fn points_with(
        &self,
        mut density: impl FnMut(Vec2) -> f32,
        mut weight: impl FnMut(Vec2, usize) -> f32,
    ) -> Vec<ScatterPoint> {
        let trunks = poisson_disc_stream(self.area, self.spacing, self.seed, TRUNK_STREAM);
        let groves = Groves::new(self);
        let (small, large) = self.scale;
        // The global shares (one species when none are given), and each tree's weighted ones.
        let global = if self.species.is_empty() { vec![1.0] } else { self.species.clone() };
        let mut shares = vec![0.0; global.len()];
        trunks
            .into_iter()
            .enumerate()
            .filter_map(|(i, position)| {
                // Each tree draws from its own stream, in a fixed order, so no draw of one tree
                // shifts another's.
                let mut rng = SeededRng::from_parts(self.seed, TREE_STREAMS.wrapping_add(i as u32));
                let keep = rng.next_f32();
                let yaw = rng.range(0.0, TAU);
                let scale = rng.range(small, large);
                let variant = (rng.next_u64() >> 32) as u32;
                let pick = rng.next_f32();
                // (Groves take their species from where the tree stands, not from its stream.)
                if keep >= density(position).clamp(0.0, 1.0) {
                    return None;
                }
                // `max` drops a NaN weight to 0.
                for (species, share) in shares.iter_mut().enumerate() {
                    *share = weight(position, species).max(0.0);
                }
                let grove = groves.as_ref().and_then(|groves| groves.species_at(position, &shares));
                let species = grove.unwrap_or_else(|| {
                    for (share, global) in shares.iter_mut().zip(&global) {
                        *share *= global;
                    }
                    weighted(&shares, pick)
                });
                Some(ScatterPoint { position, yaw, scale, species, variant })
            })
            .collect()
    }
}

/// One tree of a [`Scatter`].
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScatterPoint {
    /// Where the trunk stands: world x and z.
    pub position: Vec2,
    /// Turn about the vertical, radians.
    pub yaw: f32,
    /// Uniform scale, within [`Scatter::scale`].
    pub scale: f32,
    /// Which species, an index into [`Scatter::species`] (0 when it is empty).
    pub species: usize,
    /// A random draw for the pool variant: [`PooledTree`](crate::PooledTree) takes it modulo the
    /// pool's variant count.
    pub variant: u32,
}

impl ScatterPoint {
    /// Where the tree goes, standing at height `ground`: its position, yaw and scale. On
    /// sloping ground use [`transform_on`](Self::transform_on), which keeps the trunk's downhill
    /// side from floating.
    pub fn transform(&self, ground: f32) -> Transform {
        Transform::from_xyz(self.position.x, ground, self.position.y)
            .with_rotation(Quat::from_rotation_y(self.yaw))
            .with_scale(Vec3::splat(self.scale))
    }

    /// Where a tree of `params` goes on `ground`: [`transform`](Self::transform) at the
    /// [footing](FloraGround::footing) of its trunk, the lowest ground under its base at this
    /// point's scale, so the trunk meets the ground all round on a slope.
    pub fn transform_on(&self, ground: &FloraGround, params: &TreeParams) -> Transform {
        self.transform(ground.footing(self.position, params.base_radius() * self.scale))
    }
}

/// Points over `area` no two closer than `spacing`, and every spot of the area within about
/// twice `spacing` of one (Bridson's fast Poisson-disc sampling, 30 candidates a point):
/// blue noise, the way trees that compete for light stand. Empty if the area or the spacing is
/// not positive.
///
/// # Panics
///
/// If the area holds more than about four billion cells of `spacing / √2`.
pub fn poisson_disc(area: Rect, spacing: f32, seed: i32) -> Vec<Vec2> {
    poisson_disc_stream(area, spacing, seed, TRUNK_STREAM)
}

fn poisson_disc_stream(area: Rect, spacing: f32, seed: i32, stream: u32) -> Vec<Vec2> {
    let size = area.size();
    if !(spacing > 0.0 && size.x > 0.0 && size.y > 0.0 && size.is_finite() && spacing.is_finite()) {
        return Vec::new();
    }
    let mut rng = SeededRng::from_parts(seed, stream);
    // A cell's diagonal is the spacing, so a cell holds at most one point.
    let mut grid = Grid::new(area, spacing / SQRT_2);
    let mut points = Vec::new();
    let mut active = Vec::new();
    let first = area.min + Vec2::new(rng.next_f32(), rng.next_f32()) * size;
    grid.insert(first, 0);
    points.push(first);
    active.push(0usize);
    let spacing_sq = spacing * spacing;
    while !active.is_empty() {
        let slot = (rng.next_f32() * active.len() as f32) as usize % active.len();
        let centre = points[active[slot]];
        let mut placed = false;
        for _ in 0..CANDIDATES {
            // Uniform over the annulus from one to two spacings.
            let angle = rng.range(0.0, TAU);
            let radius = spacing * (1.0 + 3.0 * rng.next_f32()).sqrt();
            let candidate = centre + Vec2::from_angle(angle) * radius;
            if !area.contains(candidate) {
                continue;
            }
            let crowded = grid.around(candidate, 2).any(|i| points[i].distance_squared(candidate) < spacing_sq);
            if !crowded {
                grid.insert(candidate, points.len());
                active.push(points.len());
                points.push(candidate);
                placed = true;
                break;
            }
        }
        if !placed {
            active.swap_remove(slot);
        }
    }
    points
}

/// A square grid over an area, each cell holding the index of at most one point.
struct Grid {
    min: Vec2,
    cell: f32,
    width: usize,
    height: usize,
    cells: Vec<u32>,
}

impl Grid {
    const EMPTY: u32 = u32::MAX;

    fn new(area: Rect, cell: f32) -> Self {
        let size = area.size();
        let width = (size.x / cell).ceil().max(1.0) as usize;
        let height = (size.y / cell).ceil().max(1.0) as usize;
        let count = width.checked_mul(height).filter(|&n| n < Self::EMPTY as usize);
        let count = count.expect("scatter: the area is too large for its spacing");
        Self { min: area.min, cell, width, height, cells: vec![Self::EMPTY; count] }
    }

    /// The cell of `p`, clamped into the grid.
    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let c = ((p - self.min) / self.cell).floor();
        (
            (c.x.max(0.0) as usize).min(self.width - 1),
            (c.y.max(0.0) as usize).min(self.height - 1),
        )
    }

    fn insert(&mut self, p: Vec2, index: usize) {
        let (x, y) = self.cell_of(p);
        self.cells[y * self.width + x] = index as u32;
    }

    /// The points in the cells within `reach` cells of `p`'s (a square of side `2 reach + 1`).
    fn around(&self, p: Vec2, reach: usize) -> impl Iterator<Item = usize> + '_ {
        let (cx, cy) = self.cell_of(p);
        let xs = cx.saturating_sub(reach)..=(cx + reach).min(self.width - 1);
        let ys = cy.saturating_sub(reach)..=(cy + reach).min(self.height - 1);
        ys.flat_map(move |y| xs.clone().map(move |x| self.cells[y * self.width + x]))
            .filter(|&i| i != Self::EMPTY)
            .map(|i| i as usize)
    }
}

/// The groves of a scatter: centres spread like trunks, `grove_size` apart, each with a species,
/// and a smooth warp of the ground so their borders bend.
struct Groves {
    centres: Vec<Vec2>,
    species: Vec<usize>,
    grid: Grid,
    size: f32,
    seed: i32,
}

impl Groves {
    /// `None` when every tree draws its own species: no grove size, or one species or none.
    fn new(scatter: &Scatter) -> Option<Self> {
        let size = scatter.grove_size;
        if !(size > 0.0 && size.is_finite()) || scatter.species.len() < 2 {
            return None;
        }
        // Past the edges too, so a tree near one has its grove's centre nearby.
        let area = Rect::from_corners(scatter.area.min - size, scatter.area.max + size);
        let centres = poisson_disc_stream(area, size, scatter.seed, GROVE_STREAM);
        let mut rng = SeededRng::from_parts(scatter.seed, GROVE_SPECIES_STREAM);
        let species = centres.iter().map(|_| weighted(&scatter.species, rng.next_f32())).collect();
        let mut grid = Grid::new(area, size / SQRT_2);
        for (i, &centre) in centres.iter().enumerate() {
            grid.insert(centre, i);
        }
        Some(Self { centres, species, grid, size, seed: scatter.seed })
    }

    /// The species of the grove whose centre is nearest `p` once the ground is warped, each
    /// centre's distance divided by its species' share at `p` (`shares`, by species): a share of 0
    /// shuts a species out, a larger one widens its groves there, and equal shares change
    /// nothing. `None` when no grove's species has a share.
    fn species_at(&self, p: Vec2, shares: &[f32]) -> Option<usize> {
        let most = shares.iter().copied().fold(0.0, f32::max);
        if most <= 0.0 {
            return None;
        }
        let q = p / (GROVE_BEND * self.size);
        let warp = Vec2::new(value_noise(q, self.seed ^ WARP_X_SALT), value_noise(q, self.seed ^ WARP_Y_SALT));
        let p = p + warp * (GROVE_RAGGEDNESS * self.size);
        let mut best = (f32::INFINITY, None);
        // Widen the square until nothing outside it can be nearer than the best so far.
        for reach in 1..=self.grid.width.max(self.grid.height) {
            for i in self.grid.around(p, reach) {
                let share = shares.get(self.species[i]).copied().unwrap_or(0.0);
                if share <= 0.0 {
                    continue;
                }
                let d = self.centres[i].distance(p) / share;
                if d < best.0 {
                    best = (d, Some(self.species[i]));
                }
            }
            if best.0 <= reach as f32 * self.grid.cell / most {
                break;
            }
        }
        best.1
    }
}

/// Smooth value noise in -1..1 with features about one unit wide: random values on the integer
/// lattice, blended with smoothstep.
fn value_noise(p: Vec2, seed: i32) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (x, y) = (cell.x as i32, cell.y as i32);
    let at = |dx: i32, dy: i32| lattice(x.wrapping_add(dx), y.wrapping_add(dy), seed);
    let bottom = at(0, 0) + (at(1, 0) - at(0, 0)) * s.x;
    let top = at(0, 1) + (at(1, 1) - at(0, 1)) * s.x;
    bottom + (top - bottom) * s.y
}

/// A random value in -1..1 for a lattice point.
fn lattice(x: i32, y: i32, seed: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (seed as u32).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

/// The index `u` (0..1) falls on when the positive weights are laid end to end; 0 if there are
/// none.
fn weighted(weights: &[f32], u: f32) -> usize {
    // `max` drops a NaN weight to 0, so the total is never NaN.
    let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut left = u * total;
    for (i, &w) in weights.iter().enumerate() {
        let w = w.max(0.0);
        if w > 0.0 && left < w {
            return i;
        }
        left -= w;
    }
    // Rounding at the very end: the last species with weight.
    weights.iter().rposition(|&w| w > 0.0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(half: f32) -> Rect {
        Rect::new(-half, -half, half, half)
    }

    #[test]
    fn trunks_keep_their_spacing_stay_inside_and_leave_no_holes() {
        let area = Rect::new(-60.0, -40.0, 60.0, 40.0);
        let spacing = 5.0;
        let points = poisson_disc(area, spacing, 7);
        for (i, a) in points.iter().enumerate() {
            assert!(area.contains(*a), "{a} outside");
            for b in &points[i + 1..] {
                assert!(a.distance(*b) >= spacing - 1e-4, "{a} and {b} closer than {spacing}");
            }
        }
        // Every spot of the ground is near a trunk: no gaps a tree would have fitted in.
        let mut worst: f32 = 0.0;
        for x in 0..=120 {
            for y in 0..=80 {
                let spot = Vec2::new(-60.0 + x as f32, -40.0 + y as f32);
                let nearest = points.iter().map(|p| p.distance(spot)).fold(f32::INFINITY, f32::min);
                worst = worst.max(nearest);
            }
        }
        assert!(worst <= 2.0 * spacing, "a hole: a spot {worst} m from the nearest trunk");
        // As dense as the spacing allows.
        let per_area = points.len() as f32 * spacing * spacing / (120.0 * 80.0);
        assert!((0.55..0.8).contains(&per_area), "{per_area} trunks per spacing²");
    }

    #[test]
    fn a_seed_is_a_forest() {
        let a = Scatter::new(square(50.0), 6.0);
        assert_eq!(a.points(), a.points());
        assert_ne!(a.points(), Scatter { seed: 1, ..a.clone() }.points());
    }

    #[test]
    fn nothing_to_cover_gives_no_trees() {
        assert!(poisson_disc(Rect::new(0.0, 0.0, 0.0, 10.0), 1.0, 0).is_empty());
        assert!(poisson_disc(square(10.0), 0.0, 0).is_empty());
        assert!(poisson_disc(square(10.0), f32::NAN, 0).is_empty());
        // An area smaller than the spacing still takes one tree.
        assert_eq!(poisson_disc(square(1.0), 5.0, 0).len(), 1);
    }

    #[test]
    fn a_density_mask_only_removes_trees() {
        let forest = Scatter { species: vec![1.0, 1.0], grove_size: 30.0, ..Scatter::new(square(80.0), 5.0) };
        let all = forest.points();
        let cleared = forest.points_where(|p| if p.x < 0.0 { 0.0 } else { 1.0 });
        let east: Vec<_> = all.iter().filter(|t| t.position.x >= 0.0).copied().collect();
        assert_eq!(cleared, east, "the trees east of the clearing are untouched, species and all");
        let sparse = forest.points_where(|_| 0.25);
        let share = sparse.len() as f32 / all.len() as f32;
        assert!((0.18..0.32).contains(&share), "a density of 0.25 kept {share}");
        assert!(sparse.iter().all(|t| all.contains(t)));
    }

    #[test]
    fn species_follow_their_weights_and_grow_in_groves() {
        let area = square(300.0);
        // The share of trees whose nearest neighbour is of the same species.
        let together = |trees: &[ScatterPoint]| {
            let same = trees
                .iter()
                .filter(|a| {
                    let nearest = trees
                        .iter()
                        .filter(|b| b.position != a.position)
                        .min_by(|b, c| {
                            a.position.distance_squared(b.position).total_cmp(&a.position.distance_squared(c.position))
                        })
                        .unwrap();
                    nearest.species == a.species
                })
                .count();
            same as f32 / trees.len() as f32
        };
        let share = |trees: &[ScatterPoint]| trees.iter().filter(|t| t.species == 1).count() as f32 / trees.len() as f32;

        let mixed = Scatter { species: vec![3.0, 1.0, 0.0], ..Scatter::new(area, 8.0) }.points();
        assert!((0.22..0.28).contains(&share(&mixed)), "{}", share(&mixed));
        assert!(mixed.iter().all(|t| t.species != 2), "a species of weight 0 grew");
        // Independent draws: neighbours agree as often as chance says (0.75² + 0.25²).
        assert!((0.57..0.68).contains(&together(&mixed)), "{}", together(&mixed));

        let groves = Scatter { species: vec![3.0, 1.0, 0.0], grove_size: 60.0, ..Scatter::new(area, 8.0) }.points();
        assert!((0.12..0.38).contains(&share(&groves)), "{}", share(&groves));
        assert!(groves.iter().all(|t| t.species != 2), "a species of weight 0 grew");
        assert!(together(&groves) > 0.85, "groves: {}", together(&groves));
    }

    #[test]
    fn a_stand_picks_its_species_inside_and_leaves_the_forest_outside_as_it_was() {
        let centre = Vec2::new(20.0, -10.0);
        let inside = |p: Vec2| p.distance(centre) < 50.0;
        let stand = |p: Vec2, species: usize| if inside(p) { (species == 3) as u8 as f32 } else { 1.0 };
        let density = |p: Vec2| if p.x > 100.0 { 0.0 } else { 1.0 };
        let groves = Scatter { species: vec![1.0, 2.0, 1.0, 1.0], grove_size: 30.0, ..Scatter::new(square(150.0), 5.0) };
        for forest in [groves.clone(), Scatter { grove_size: 0.0, ..groves }] {
            let plain = forest.points_where(density);
            let stood = forest.points_with(density, stand);
            assert_eq!(stood, forest.points_with(density, stand), "a seed is a forest");
            assert_eq!(plain.len(), stood.len(), "the weights move no tree");
            let mut within = 0;
            for (before, after) in plain.iter().zip(&stood) {
                if inside(after.position) {
                    assert_eq!((after.position, after.species), (before.position, 3));
                    within += 1;
                } else {
                    assert_eq!(after, before, "a tree outside the stand changed");
                }
            }
            assert!(within > 200, "{within} trees in the stand");
        }

        // A stand fading out over a 30 m rim, its species nowhere else: inside the rim it is the
        // stand's alone, and over the rim its groves and the forest's meet along a bent border,
        // not a circle.
        let forest = Scatter { species: vec![1.0, 2.0, 1.0, 1.0], grove_size: 30.0, ..Scatter::new(square(150.0), 4.0) };
        let rim = |p: Vec2, species: usize| {
            let out = ((p.distance(centre) - 30.0) / 30.0).clamp(0.0, 1.0);
            if species == 3 { 1.0 - out } else { out }
        };
        let trees = forest.points_with(|_| 1.0, rim);
        let at = |t: &ScatterPoint| t.position.distance(centre);
        assert!(trees.iter().filter(|t| at(t) < 30.0).all(|t| t.species == 3));
        assert!(trees.iter().filter(|t| at(t) > 60.0).all(|t| t.species != 3));
        let stand_reaches = trees.iter().filter(|t| t.species == 3).map(at).fold(0.0, f32::max);
        let forest_reaches = trees.iter().filter(|t| t.species != 3).map(at).fold(f32::INFINITY, f32::min);
        assert!(stand_reaches - forest_reaches > 8.0, "a circular edge: the stand to {stand_reaches} m, the forest from {forest_reaches} m");
    }

    #[test]
    fn a_tree_stands_where_it_was_scattered() {
        let tree = ScatterPoint { position: Vec2::new(3.0, -4.0), yaw: 1.0, scale: 1.2, species: 0, variant: 9 };
        let t = tree.transform(2.5);
        assert_eq!(t.translation, Vec3::new(3.0, 2.5, -4.0));
        assert_eq!(t.scale, Vec3::splat(1.2));
        assert!((t.rotation.to_euler(bevy::math::EulerRot::YXZ).0 - 1.0).abs() < 1e-5);
    }
}
