//! Fire: trees that scorch, burn and stand charred ([`FloraFire`]).
//!
//! The fire is a grid of cells laid over the game's world, each with how charred it is and how hot
//! it burns. The shaders read the grid where each tree stands: its leaves scorch, glow at the
//! burning front and burn away one by one (in the shadow maps too), its wood blackens from the
//! ground up with embers glowing in the bark's cracks while it burns, and an impostor chars and
//! thins the same way. Every way a tree is drawn takes part, at no cost per tree: the grid rides in
//! the shared buffer the wind is in ([`FloraWindBuffer`]), written when it changes.
//!
//! The plugin spreads a fire from the cells it is lit in ([`FloraFire::ignite`]) to their
//! neighbours, faster downwind ([`FloraWind`]), through the cells within
//! [`FireSpread::reach`] of a tree (`ProceduralTree` or `PooledTree`); where no tree stands it
//! goes out. A cell burns until it is charred through, then its embers cool. The game reads the
//! grid back ([`FloraFire::char_at`], [`FloraFire::heat_at`]) to hurt what stands in the fire, hears
//! of each tree it burns through ([`TreeBurnt`]), and can put a fire out ([`FloraFire::extinguish`])
//! or lay a burnt patch down with no fire ([`FloraFire::scorch`]).
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraFire;
//!
//! // A 256 m square of forest around the origin, 2 m cells.
//! fn setup(mut commands: Commands) {
//!     commands.insert_resource(FloraFire::new(Vec2::ZERO, 256.0));
//! }
//!
//! // A lightning strike.
//! fn strike(mut fire: ResMut<FloraFire>) {
//!     fire.ignite(Vec3::new(10.0, 0.0, -4.0), 3.0);
//! }
//! ```
//!
//! While no `FloraFire` exists no shader carries the fire. Inserting one turns it on in every
//! flora material (the materials are re-prepared once, so insert it while the level loads, before
//! the first fire); removing it turns it off.

use bevy::asset::{Asset, AssetId, Assets};
use bevy::ecs::prelude::*;
use bevy::math::{Rect, Vec2, Vec3};
use bevy::reflect::Reflect;
use bevy::render::storage::ShaderBuffer;
use bevy::time::Time;
use bevy::transform::components::GlobalTransform;

use crate::gpu::GrowthMaterial;
use crate::impostor::ImpostorMaterial;
use crate::pool::PooledTree;
use crate::shading::{FloraMaterial, FloraWind, FloraWindBuffer, WIND_VECTORS};
use crate::units::FloraUnits;
use crate::ProceduralTree;

/// Where the fire's header sits in the shared buffer ([`FloraWindBuffer`]): after the weather, the
/// shakes and the see-through fade. `common.wesl`'s `FIRE` is this number. The cells follow it.
pub(crate) const FIRE_SLOT: usize = WIND_VECTORS;

/// How long one step of the spread is, seconds: the plugin steps the fire at this rate whatever
/// the frame rate, so a fire burns alike at any.
const STEP: f32 = 0.1;

/// A lit cell's heat (its glow; the char follows it) rises from a spark to 1 over this many
/// seconds, or a quarter of [`FireSpread::burn_time`] if that is less.
const KINDLE: f32 = 2.0;

/// The heat a cell is lit with.
const SPARK: f32 = 0.05;

/// A cell with nothing to burn goes out over this many seconds.
const NO_FUEL_COOLS: f32 = 1.0;

/// How strongly the wind drives the spread: a neighbour downwind is lit `exp(gain * wind)` times
/// as fast, one upwind as much slower, `wind` being [`FireSpread::wind`] times the wind's strength
/// along the way to it.
const WIND_GAIN: f32 = 2.0;

/// How far ahead of the burning front (in the leaf's moment) a leaf has already scorched brown.
/// `common.wesl` mirrors it as `LEAF_SCORCH`.
pub(crate) const LEAF_SCORCH: f32 = 0.35;

/// How close to the burning front (in the leaf's moment) a scorched leaf glows, burning, before it
/// is gone. `common.wesl` mirrors it as `LEAF_GLOW`.
pub(crate) const LEAF_GLOW: f32 = 0.08;

/// The burning front in leaf moments for the char `char` at a tree: a leaf whose moment is below it
/// has burnt away. No leaf scorches without char, every leaf has burnt at char 1. `common.wesl`'s
/// `leaf_burn_front` is the same.
pub(crate) fn leaf_burn_front(char: f32) -> f32 {
    let lead = LEAF_GLOW + LEAF_SCORCH + 0.02;
    char * (1.0 + lead) - lead
}

/// The burning front at a tree standing at `at`: the moment below which its leaves have burnt (no
/// leaf without a fire). The leaves the shaders no longer draw do not fall.
pub(crate) fn burnt_front(fire: Option<&FloraFire>, at: Vec3) -> f32 {
    match fire.map(|fire| fire.char_at(at)) {
        Some(char) if char > 0.0 => leaf_burn_front(char),
        _ => f32::NEG_INFINITY,
    }
}

/// How a fire spreads and burns; see [`FloraFire::spread`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireSpread {
    /// How fast the fire moves through a forest with no wind, metres a second (about; it moves by
    /// chance from cell to cell). Metres: [`FloraUnits`] scales it to the world.
    pub speed: f32,
    /// Seconds a cell burns at full heat before it is charred through (its trees' leaves gone, the
    /// wood black to the top).
    pub burn_time: f32,
    /// Seconds a charred cell's embers take to cool.
    pub cool_time: f32,
    /// How far from a tree's trunk the fire finds something to burn, metres ([`FloraUnits`]
    /// scales it to the world): crowns closer than twice this light each other, and a gap wider
    /// than that stops the fire.
    pub reach: f32,
    /// What burns where no tree stands, 0..1 (grass, litter): 0 keeps the fire to the trees.
    pub ground: f32,
    /// How much the wind ([`FloraWind`]) drives the fire: 0 not at all, 1 a fire runs downwind
    /// e² times as fast as against a wind of strength 1.
    pub wind: f32,
}

impl Default for FireSpread {
    fn default() -> Self {
        Self { speed: 1.0, burn_time: 12.0, cool_time: 20.0, reach: 3.0, ground: 0.0, wind: 1.0 }
    }
}

/// One cell of the fire: how charred it is and how hot it burns, both 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireCell {
    /// 0 untouched, 1 burnt through.
    pub char: f32,
    /// 0 cold, 1 burning; a charred cell's embers cool back to 0.
    pub heat: f32,
}

/// The fire over the game's world: a square grid of [`FireCell`]s that the plugin spreads a fire
/// through and every flora shader reads (see the [module](crate::fire)). Insert it to have fires;
/// its size is fixed when it is made.
#[derive(Resource, Clone, Debug)]
pub struct FloraFire {
    /// How the fire spreads and burns.
    pub spread: FireSpread,
    /// The seed of the ragged front: the same fire, lit the same way on the same clock, burns the
    /// same way.
    pub seed: u64,
    corner: Vec2,
    cell: f32,
    side: u32,
    cells: Vec<FireCell>,
    fuel: Vec<f32>,
    /// When each cell was lit, on the fire's own clock (steps times `STEP`).
    lit: Vec<f32>,
    /// The tree count, reach, ground and world units the fuel was laid for.
    fueled_for: Option<(usize, f32, f32, f32)>,
    clock: f32,
    steps: u64,
    burning: usize,
    /// The cells holding any char or heat lie within these (lowest x, z, highest x, z, both
    /// included); `None` while none does.
    touched: Option<[usize; 4]>,
}

impl FloraFire {
    /// The most cells along a side: a grid of 512² is 4 MiB of buffer.
    pub const MAX_CELLS: u32 = 512;

    /// A fire over the square `size` world units wide centred on `centre` (world x, z), in 128
    /// cells along each side.
    pub fn new(centre: Vec2, size: f32) -> Self {
        Self::with_cells(centre, size, 128)
    }

    /// A fire over the square `size` world units wide centred on `centre` (world x, z), in `cells`
    /// cells along each side (2 to [`MAX_CELLS`](Self::MAX_CELLS)). Smaller cells burn along finer
    /// lines; the buffer the shaders read holds 8 bytes a cell and is written while anything
    /// burns, ten times a second.
    pub fn with_cells(centre: Vec2, size: f32, cells: u32) -> Self {
        let side = cells.clamp(2, Self::MAX_CELLS);
        let size = size.max(1.0e-3);
        let count = (side * side) as usize;
        Self {
            spread: FireSpread::default(),
            seed: 0,
            corner: centre - Vec2::splat(size * 0.5),
            cell: size / side as f32,
            side,
            cells: vec![FireCell::default(); count],
            fuel: vec![0.0; count],
            lit: vec![f32::INFINITY; count],
            fueled_for: None,
            clock: 0.0,
            steps: 0,
            burning: 0,
            touched: None,
        }
    }

    /// The same fire spreading by `spread`.
    pub fn with_spread(mut self, spread: FireSpread) -> Self {
        self.spread = spread;
        self
    }

    /// The edge of one cell, in world units.
    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    /// Cells along each side.
    pub fn cells_per_side(&self) -> u32 {
        self.side
    }

    /// The cells, row by row along x, rows along z from the grid's lower corner.
    pub fn cells(&self) -> &[FireCell] {
        &self.cells
    }

    /// Whether any cell burns or still glows.
    pub fn is_burning(&self) -> bool {
        self.burning > 0
    }

    /// The world x, z rectangle outside which every cell is untouched (no char, no heat): the
    /// cells holding any lie inside it, edge to edge. `None` before anything burnt or was
    /// scorched; a burnt area keeps it after its embers cooled, as its char stays.
    pub fn touched_bounds(&self) -> Option<Rect> {
        let [x0, z0, x1, z1] = self.touched?;
        let min = self.corner + Vec2::new(x0 as f32, z0 as f32) * self.cell;
        let max = self.corner + Vec2::new(x1 as f32 + 1.0, z1 as f32 + 1.0) * self.cell;
        Some(Rect::from_corners(min, max))
    }

    /// Every cell burning now (heat above 0, cooling embers included) with its centre (world x,
    /// z): where to put the fire's lights, or what to keep the living away from. Only the
    /// [touched rectangle](Self::touched_bounds) is looked at, so a small fire on a large grid is
    /// cheap to read. The resource is marked changed only in a frame the fire stepped (ten times a
    /// second while anything burns) or the game changed it, so a system run on
    /// `resource_changed::<FloraFire>` reads it just when there is something new.
    pub fn burning(&self) -> impl Iterator<Item = (Vec2, FireCell)> + '_ {
        let side = self.side as usize;
        let [x0, z0, x1, z1] = self.touched.unwrap_or([1, 1, 0, 0]);
        (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z))).filter_map(move |(x, z)| {
            let cell = self.cells[z * side + x];
            let centre = self.corner + (Vec2::new(x as f32, z as f32) + 0.5) * self.cell;
            (cell.heat > 0.0).then_some((centre, cell))
        })
    }

    /// Light the cells within `radius` (world units) of `at` (world x, z; at least the cell `at` is in).
    /// They burn if a tree stands within [`FireSpread::reach`] of them (or the ground burns) and
    /// spread from there; a cell burnt through does not light again.
    pub fn ignite(&mut self, at: Vec3, radius: f32) {
        let now = self.steps as f32 * STEP;
        for i in self.cells_within(at, radius) {
            let cell = &mut self.cells[i];
            if cell.heat <= 0.0 && cell.char < 1.0 {
                cell.heat = SPARK;
                self.lit[i] = now;
                self.touch(i);
            }
        }
        self.settle();
    }

    /// Put the fire out within `radius` (world units) of `at` (rain, water, a firebreak): the cells keep
    /// their char and can be lit again.
    pub fn extinguish(&mut self, at: Vec3, radius: f32) {
        for i in self.cells_within(at, radius) {
            self.cells[i].heat = 0.0;
        }
        self.settle();
    }

    /// Set every cell to `cells` (as [`cells`](Self::cells) gave them, row by row), as when a
    /// save is loaded: the char as it was, and a cell still burning (heat above 0) burns on, its
    /// fire spreading again from now. `false`, and nothing changes, if `cells` is not one entry a
    /// cell.
    pub fn restore(&mut self, cells: &[FireCell]) -> bool {
        if cells.len() != self.cells.len() {
            return false;
        }
        let now = self.steps as f32 * STEP;
        for (i, cell) in cells.iter().enumerate() {
            let (char, heat) = (cell.char.clamp(0.0, 1.0), cell.heat.clamp(0.0, 1.0));
            self.cells[i] = FireCell { char, heat };
            self.lit[i] = if heat > 0.0 { now } else { f32::INFINITY };
        }
        self.touched = Some([0, 0, self.side as usize - 1, self.side as usize - 1]);
        self.settle();
        true
    }

    /// Char the cells within `radius` (world units) of `at` to at least `char` (0..1) with no fire: an old
    /// burn, laid down when a level loads.
    pub fn scorch(&mut self, at: Vec3, radius: f32, char: f32) {
        let char = char.clamp(0.0, 1.0);
        for i in self.cells_within(at, radius) {
            self.cells[i].char = self.cells[i].char.max(char);
            if char > 0.0 {
                self.touch(i);
            }
        }
    }

    /// How charred the trees at `at` are, 0..1, as the shaders see it: between the four nearest
    /// cell centres, 0 off the grid.
    pub fn char_at(&self, at: Vec3) -> f32 {
        self.sample(at, |cell| cell.char)
    }

    /// How hot the fire at `at` burns, 0..1, as the shaders see it.
    pub fn heat_at(&self, at: Vec3) -> f32 {
        self.sample(at, |cell| cell.heat)
    }

    /// `common.wesl`'s `fire_at`: bilinear between cell centres, cells off the grid 0.
    fn sample(&self, at: Vec3, value: impl Fn(&FireCell) -> f32) -> f32 {
        let g = (Vec2::new(at.x, at.z) - self.corner) / self.cell - 0.5;
        let base = g.floor();
        let f = g - base;
        let (x, z) = (base.x as i64, base.y as i64);
        let get = |x: i64, z: i64| self.index(x, z).map_or(0.0, |i| value(&self.cells[i]));
        let near = get(x, z) + (get(x + 1, z) - get(x, z)) * f.x;
        let far = get(x, z + 1) + (get(x + 1, z + 1) - get(x, z + 1)) * f.x;
        near + (far - near) * f.y
    }

    fn index(&self, x: i64, z: i64) -> Option<usize> {
        let side = self.side as i64;
        (0..side).contains(&x).then_some(())?;
        (0..side).contains(&z).then_some(())?;
        Some((z * side + x) as usize)
    }

    /// The cells whose centres lie within `radius` of `at`, and the cell `at` is in.
    fn cells_within(&self, at: Vec3, radius: f32) -> Vec<usize> {
        let at = Vec2::new(at.x, at.z);
        let radius = radius.max(0.0);
        let low = ((at - radius - self.corner) / self.cell).floor();
        let high = ((at + radius - self.corner) / self.cell).floor();
        let mut found = Vec::new();
        for z in low.y as i64..=high.y as i64 {
            for x in low.x as i64..=high.x as i64 {
                let Some(i) = self.index(x, z) else { continue };
                let centre = self.corner + (Vec2::new(x as f32, z as f32) + 0.5) * self.cell;
                if centre.distance(at) <= radius {
                    found.push(i);
                }
            }
        }
        let own = ((at - self.corner) / self.cell).floor();
        if let Some(i) = self.index(own.x as i64, own.y as i64)
            && !found.contains(&i)
        {
            found.push(i);
        }
        found
    }

    /// Lay the fuel down again from the trees standing at `roots`: within `reach` of a trunk a
    /// cell is full of fuel, fading to the ground's over the outer half. `units`: world units to a
    /// metre, which `reach` (metres) is scaled by.
    fn refuel(&mut self, roots: impl Iterator<Item = Vec3>, units: f32) {
        let ground = self.spread.ground.clamp(0.0, 1.0);
        self.fuel.fill(ground);
        let reach = (self.spread.reach * units).max(1.0e-3);
        for root in roots {
            for i in self.cells_within(root, reach) {
                let (x, z) = (i % self.side as usize, i / self.side as usize);
                let centre = self.corner + (Vec2::new(x as f32, z as f32) + 0.5) * self.cell;
                let d = centre.distance(Vec2::new(root.x, root.z));
                let near = 1.0 - ((d / reach - 0.5) * 2.0).clamp(0.0, 1.0);
                self.fuel[i] = (self.fuel[i] + near).min(1.0);
            }
        }
    }

    /// Grow the touched rectangle to hold the cell `i`.
    fn touch(&mut self, i: usize) {
        let side = self.side as usize;
        let (x, z) = (i % side, i / side);
        self.touched = Some(match self.touched {
            None => [x, z, x, z],
            Some([x0, z0, x1, z1]) => [x0.min(x), z0.min(z), x1.max(x), z1.max(z)],
        });
    }

    /// Count the burning cells and fit the touched rectangle to the cells holding any char or
    /// heat, looking only inside it.
    fn settle(&mut self) {
        let side = self.side as usize;
        let [x0, z0, x1, z1] = self.touched.unwrap_or([1, 1, 0, 0]);
        let (mut burning, mut fit) = (0, None::<[usize; 4]>);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let cell = self.cells[z * side + x];
                if cell.heat > 0.0 {
                    burning += 1;
                }
                if cell.heat > 0.0 || cell.char > 0.0 {
                    fit = Some(match fit {
                        None => [x, z, x, z],
                        Some([a, b, c, d]) => [a.min(x), b.min(z), c.max(x), d.max(z)],
                    });
                }
            }
        }
        self.burning = burning;
        self.touched = fit;
    }

    /// One step of `STEP` seconds: burning cells kindle and char, burnt ones cool, and every
    /// burning cell lights each neighbour once the fire has had the time to get there (`wind`: the
    /// wind's direction times its strength; `units`: world units to a metre, which `speed`, in
    /// metres a second, is scaled by).
    fn step(&mut self, wind: Vec2, units: f32) {
        let side = self.side as i64;
        let spread = self.spread;
        let now = self.steps as f32 * STEP;
        let mut lit: Vec<(usize, f32)> = Vec::new();
        // Only the touched cells can burn: the rest hold no heat.
        let [x0, z0, x1, z1] = self.touched.unwrap_or([1, 1, 0, 0]);
        let width = side as usize;
        for i in (z0..=z1).flat_map(|z| (x0..=x1).map(move |x| z * width + x)) {
            let fuel = self.fuel[i];
            let cell = &mut self.cells[i];
            if cell.heat <= 0.0 {
                continue;
            }
            if fuel <= 0.0 {
                cell.heat = (cell.heat - STEP / NO_FUEL_COOLS).max(0.0);
                continue;
            }
            if cell.char >= 1.0 {
                cell.heat = (cell.heat - STEP / spread.cool_time.max(STEP)).max(0.0);
                continue;
            }
            // The look: the heat rises over the kindling (at most a quarter of the burn, so a fast
            // fire flares at once), and the char with it.
            let kindle = KINDLE.min(0.25 * spread.burn_time).max(STEP);
            cell.heat = (cell.heat + STEP / kindle).min(1.0);
            cell.char = (cell.char + STEP / spread.burn_time.max(STEP) * cell.heat).min(1.0);
            // The spread: the fire reaches a neighbour `distance / speed` after it lit this cell
            // (sooner downwind, later where there is less to burn, a quarter more or less by the
            // neighbour), so the front moves at `speed` whatever the cells' size. Arrival times
            // are kept exact, not rounded to the step.
            let since = self.lit[i];
            let (x, z) = ((i as i64) % side, (i as i64) / side);
            for (dx, dz) in NEIGHBOURS {
                let Some(j) = self.index(x + dx, z + dz) else { continue };
                let (target, fuel) = (self.cells[j], self.fuel[j]);
                if fuel <= 0.0 || target.heat > 0.0 || target.char >= 1.0 {
                    continue;
                }
                let way = Vec2::new(dx as f32, dz as f32);
                let along = spread.wind * wind.dot(way.normalize());
                let pace = spread.speed.max(1.0e-6) * units * fuel * (WIND_GAIN * along).exp();
                let arrives = since + way.length() * self.cell / pace * jitter(self.seed, j);
                if arrives < now + STEP {
                    lit.push((j, arrives));
                }
            }
        }
        for (j, arrives) in lit {
            if self.cells[j].heat <= 0.0 {
                self.cells[j].heat = SPARK;
                self.lit[j] = arrives;
                self.touch(j);
            } else {
                self.lit[j] = self.lit[j].min(arrives);
            }
        }
        self.settle();
        self.steps += 1;
    }

    /// The shared buffer's fire part: two header vectors, then the cells two to a `Vec4` (char,
    /// heat, char, heat). The first header is the grid's corner x, z, 1 / cell size and cells per
    /// side, 0 while no cell holds any char or heat (the shaders then stop there); the second the
    /// world x, z rectangle outside which every cell is empty (grown by a cell, for the blend
    /// between cell centres), so trees outside it read no cells.
    fn vectors(&self) -> impl Iterator<Item = [f32; 4]> + '_ {
        let any = self.touched.is_some();
        let [x0, z0, x1, z1] = self.touched.unwrap_or_default();
        let (low, high) = ((x0, z0), (x1, z1));
        let header = [self.corner.x, self.corner.y, 1.0 / self.cell, if any { self.side as f32 } else { 0.0 }];
        let bounds = if any {
            let min = self.corner + (Vec2::new(low.0 as f32, low.1 as f32) - 1.0) * self.cell;
            let max = self.corner + (Vec2::new(high.0 as f32, high.1 as f32) + 2.0) * self.cell;
            [min.x, min.y, max.x, max.y]
        } else {
            [0.0; 4]
        };
        let pairs = self.cells.chunks(2).map(|pair| {
            let second = pair.get(1).copied().unwrap_or_default();
            [pair[0].char, pair[0].heat, second.char, second.heat]
        });
        [header, bounds].into_iter().chain(pairs)
    }

    /// `Vec4`s of the fire part of the shared buffer.
    fn vector_count(&self) -> usize {
        2 + self.cells.len().div_ceil(2)
    }
}

/// The eight neighbours of a cell.
const NEIGHBOURS: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// A number in [0.75, 1.25) for the cell `cell` of the fire seeded `seed`: how much longer or
/// shorter than the rest the fire takes to reach it, so the front is ragged.
fn jitter(seed: u64, cell: usize) -> f32 {
    let mut x = seed ^ (cell as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // splitmix64's finaliser.
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    0.75 + 0.5 * ((x >> 40) as f32 / (1u64 << 24) as f32)
}

/// Run condition: a fire burns.
pub(crate) fn fire_burns(fire: Option<Res<FloraFire>>) -> bool {
    fire.is_some_and(|fire| fire.is_burning())
}

/// Run condition: the fire changed, or it went this frame.
pub(crate) fn fire_changed(fire: Option<Res<FloraFire>>, mut had: Local<bool>) -> bool {
    let now = fire.is_some();
    let changed = fire.is_some_and(|fire| fire.is_changed()) || (*had && !now);
    *had = now;
    changed
}

/// Run condition: the shared buffer was resized last frame.
pub(crate) fn fire_rebinds(rebind: Res<FireRebind>) -> bool {
    rebind.0
}

/// A tree the fire finds fuel around: drawn on the CPU or from a pool.
type AnyTree = Or<(With<ProceduralTree>, With<PooledTree>)>;

/// Step the fire by the time that passed, in steps of `STEP`, while it burns. The fuel is laid
/// down from where the trees stand first, and again whenever the number of trees changed (counted
/// by archetype, not by tree, so it costs nothing per tree). Runs after transform propagation, so
/// a tree planted this frame stands where it was put. Marks the fire changed (so the buffer is
/// written) only in a frame it steps.
pub(crate) fn burn(
    mut fire: ResMut<FloraFire>,
    time: Res<Time>,
    wind: Option<Res<FloraWind>>,
    units: Res<FloraUnits>,
    trees: Query<&GlobalTransform, AnyTree>,
) {
    let state = fire.bypass_change_detection();
    let units = units.units_per_metre;
    let laid = (trees.iter().len(), state.spread.reach, state.spread.ground, units);
    if state.fueled_for != Some(laid) {
        state.refuel(trees.iter().map(GlobalTransform::translation), units);
        state.fueled_for = Some(laid);
    }
    let wind = wind.map_or(Vec2::ZERO, |wind| wind.direction.normalize_or_zero() * wind.strength.max(0.0));
    state.clock += time.delta_secs();
    let mut stepped = false;
    while state.clock >= STEP && state.is_burning() {
        state.clock -= STEP;
        state.step(wind, units);
        stepped = true;
    }
    if !state.is_burning() {
        state.clock = 0.0;
    }
    if stepped {
        fire.set_changed();
    }
}

/// How charred the ground at a tree's trunk is when the fire has burnt the tree through: its wood
/// black, its leaves gone ([`TreeBurnt`]).
pub const BURNT: f32 = 0.95;

/// A tree the fire burnt through: the char where its trunk stands passed [`BURNT`]. The plugin
/// sends it once for each tree, the frame that happens, and marks the tree [`Burnt`]: the moment
/// to drop its loot, count it lost or put a dead tree in its place.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TreeBurnt {
    /// The tree: a [`ProceduralTree`] or a [`PooledTree`].
    pub tree: Entity,
    /// Where its trunk stands, in world space.
    pub at: Vec3,
}

/// A tree the fire burnt through ([`TreeBurnt`]), marked by the plugin. A tree that has it is not
/// reported again: insert it on the burnt trees of a save, with the fire's grid
/// ([`FloraFire::restore`]), and loading reports none of them.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Burnt;

/// Report each tree the fire has burnt through ([`TreeBurnt`]), once: run when the fire changed
/// (it stepped, or the game set its cells).
pub(crate) fn report_burnt(
    mut commands: Commands,
    fire: Res<FloraFire>,
    trees: Query<(Entity, &GlobalTransform), (AnyTree, Without<Burnt>)>,
    mut burnt: MessageWriter<TreeBurnt>,
) {
    for (tree, at) in &trees {
        let at = at.translation();
        if fire.char_at(at) >= BURNT {
            commands.entity(tree).insert(Burnt);
            burnt.write(TreeBurnt { tree, at });
        }
    }
}

/// The shared buffer was resized: the flora materials must bind the new one. Set the frame it is
/// resized; acted on the next (`rebind_fire`), when the new buffer has been prepared.
#[derive(Resource, Default)]
pub(crate) struct FireRebind(bool);

/// Write the fire into the shared buffer when it changed: in place, or grown to hold its grid (and
/// shrunk back when the fire is gone), after which the materials are re-prepared.
pub(crate) fn sync_fire(
    fire: Option<Res<FloraFire>>,
    buffer: Option<Res<FloraWindBuffer>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
    mut rebind: ResMut<FireRebind>,
) {
    let (Some(buffer), Some(mut buffers)) = (buffer, buffers) else { return };
    let length = buffers.get(&buffer.0).and_then(|b| b.cast_slice::<[f32; 4]>()).map_or(0, <[_]>::len);
    let wanted = FIRE_SLOT + fire.as_ref().map_or(1, |fire| fire.vector_count());
    let changed = fire.as_ref().is_some_and(|fire| fire.is_changed());
    if length == wanted && !changed {
        return;
    }
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    let Some(slice) = shader_buffer.cast_slice_mut::<[f32; 4]>() else { return };
    if slice.len() == wanted {
        if let Some(fire) = &fire {
            for (slot, vector) in slice[FIRE_SLOT..].iter_mut().zip(fire.vectors()) {
                *slot = vector;
            }
        }
        return;
    }
    let mut all: Vec<[f32; 4]> = slice.iter().take(FIRE_SLOT).copied().collect();
    all.resize(FIRE_SLOT, [0.0; 4]);
    match &fire {
        Some(fire) => all.extend(fire.vectors()),
        None => all.push([0.0; 4]),
    }
    *shader_buffer = ShaderBuffer::from(all);
    rebind.0 = true;
}

/// Re-prepare every flora material the frame after the shared buffer was resized, so they bind the
/// new buffer (Bevy prepares materials in no fixed order with the buffers they bind).
pub(crate) fn rebind_fire(
    mut rebind: ResMut<FireRebind>,
    flora: Option<ResMut<Assets<FloraMaterial>>>,
    growth: Option<ResMut<Assets<GrowthMaterial>>>,
    impostors: Option<ResMut<Assets<ImpostorMaterial>>>,
) {
    if !rebind.0 {
        return;
    }
    rebind.0 = false;
    if let Some(mut assets) = flora {
        touch(&mut assets);
    }
    if let Some(mut assets) = growth {
        touch(&mut assets);
    }
    if let Some(mut assets) = impostors {
        touch(&mut assets);
    }
}

/// Borrow every asset mutably, which has it prepared again.
fn touch<A: Asset>(assets: &mut Assets<A>) {
    let ids: Vec<AssetId<A>> = assets.ids().collect();
    for id in ids {
        // Only a mutable dereference marks it modified.
        if let Some(mut asset) = assets.get_mut(id) {
            let _: &mut A = &mut asset;
        }
    }
}

/// Turn the fire on in every flora material while a [`FloraFire`] exists, and off when none does:
/// the flag is part of the materials' pipeline keys, so their shaders carry the fire only then.
/// Materials made while it is on are turned on the frame after.
pub(crate) fn arm_fire(
    fire: Option<Res<FloraFire>>,
    flora: Option<ResMut<Assets<FloraMaterial>>>,
    growth: Option<ResMut<Assets<GrowthMaterial>>>,
    impostors: Option<ResMut<Assets<ImpostorMaterial>>>,
) {
    let on = fire.is_some();
    if let Some(mut assets) = flora {
        crate::see_through::arm(&mut assets, on, |m| m.extension.fire, |m, on| m.extension.fire = on);
    }
    if let Some(mut assets) = growth {
        crate::see_through::arm(&mut assets, on, |m| m.extension.fire, |m, on| m.extension.fire = on);
    }
    if let Some(mut assets) = impostors {
        crate::see_through::arm(&mut assets, on, |m| m.fire, |m, on| m.fire = on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fire over 64 m in 2 m cells with a tree every 4 m along z = 0 from x = -24 to 24, in a
    /// world of `units` world units to the metre (the grid and the trees' places scaled by it).
    fn row_of_trees_in(units: f32) -> FloraFire {
        let mut fire = FloraFire::with_cells(Vec2::ZERO, 64.0 * units, 32);
        fire.refuel((-6..=6).map(|i| Vec3::new(i as f32 * 4.0 * units, 0.0, 0.0)), units);
        fire
    }

    /// [`row_of_trees_in`] a world in metres.
    fn row_of_trees() -> FloraFire {
        row_of_trees_in(1.0)
    }

    fn run_in(fire: &mut FloraFire, seconds: f32, wind: Vec2, units: f32) {
        for _ in 0..(seconds / STEP).round() as usize {
            fire.step(wind, units);
        }
    }

    fn run(fire: &mut FloraFire, seconds: f32, wind: Vec2) {
        run_in(fire, seconds, wind, 1.0);
    }

    #[test]
    fn each_tree_the_fire_burns_through_is_reported_once_and_marked() {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::transform::components::Transform;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<bevy::mesh::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        let mut oak = |x: f32| {
            let params = crate::presets::TreePreset::Oak.params();
            app.world_mut().spawn((ProceduralTree(params), Transform::from_xyz(x, 0.0, 0.0))).id()
        };
        let (burning, restored, far) = (oak(0.0), oak(2.0), oak(40.0));
        // Burnt before a save: not reported again.
        app.world_mut().entity_mut(restored).insert(Burnt);
        app.insert_resource(FloraFire::with_cells(Vec2::ZERO, 32.0, 32));
        app.update();
        let mut cursor = app.world().resource::<Messages<TreeBurnt>>().get_cursor_current();
        app.world_mut().resource_mut::<FloraFire>().scorch(Vec3::ZERO, 4.0, 1.0);
        app.update();
        let reported: Vec<Entity> = cursor.read(app.world().resource::<Messages<TreeBurnt>>()).map(|b| b.tree).collect();
        assert_eq!(reported, vec![burning]);
        assert!(app.world().get::<Burnt>(burning).is_some());
        assert!(app.world().get::<Burnt>(far).is_none());
        // Charred again: nothing new.
        app.world_mut().resource_mut::<FloraFire>().scorch(Vec3::ZERO, 4.0, 1.0);
        app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<TreeBurnt>>()).count(), 0);
    }

    #[test]
    fn a_fire_restored_from_its_cells_burns_on_from_where_it_was() {
        let mut fire = row_of_trees();
        fire.ignite(Vec3::new(-24.0, 0.0, 0.0), 1.0);
        run(&mut fire, 8.0, Vec2::ZERO);
        assert!(fire.is_burning());
        let saved = fire.cells().to_vec();
        let mut loaded = row_of_trees();
        assert!(!loaded.restore(&saved[1..]), "a grid of another size was restored");
        assert!(loaded.restore(&saved));
        assert_eq!(loaded.cells(), &saved[..]);
        assert!(loaded.is_burning());
        // It burns on along the row to its end.
        run(&mut loaded, 120.0, Vec2::ZERO);
        for i in -6..=6 {
            assert!(loaded.char_at(Vec3::new(i as f32 * 4.0, 0.0, 0.0)) > 0.95, "tree {i}");
        }
    }

    #[test]
    fn a_fire_runs_along_the_trees_and_stops_where_they_end() {
        let mut fire = row_of_trees();
        fire.ignite(Vec3::new(-24.0, 0.0, 0.0), 1.0);
        run(&mut fire, 120.0, Vec2::ZERO);
        // Every tree burnt, the cells far from any tree never lit.
        for i in -6..=6 {
            assert!(fire.char_at(Vec3::new(i as f32 * 4.0, 0.0, 0.0)) > 0.95, "tree {i}");
        }
        assert_eq!(fire.char_at(Vec3::new(0.0, 0.0, 20.0)), 0.0);
        assert_eq!(fire.char_at(Vec3::new(30.0, 0.0, 0.0)), 0.0);
        // It has burnt out and is cooling or cold, never lighting anything again.
        let lit_again = fire.cells().iter().filter(|cell| cell.heat > 0.0 && cell.char < 1.0).count();
        assert_eq!(lit_again, 0);
    }

    #[test]
    fn the_fire_moves_at_its_speed_whatever_the_size_of_the_cells() {
        // Trees every 3 m along x, lit at x = 0; when does the fire reach the tree 24 m on?
        let arrival = |cells: u32| {
            let mut fire = FloraFire::with_cells(Vec2::ZERO, 64.0, cells);
            fire.refuel((0..=10).map(|i| Vec3::new(i as f32 * 3.0, 0.0, 0.0)), 1.0);
            fire.ignite(Vec3::ZERO, 0.5);
            let target = Vec3::new(24.0, 0.0, 0.0);
            (1..=600).find(|_| {
                fire.step(Vec2::ZERO, 1.0);
                fire.heat_at(target) > 0.0
            })
            .map(|steps| steps as f32 * STEP)
        };
        let (fine, coarse) = (arrival(128).unwrap(), arrival(32).unwrap());
        // At 1 m/s, some 24 s: by chance and over eight neighbours, a little sooner.
        for (cells, seconds) in [("0.5 m", fine), ("2 m", coarse)] {
            assert!((10.0..36.0).contains(&seconds), "{cells} cells: {seconds} s");
        }
        assert!(fine.max(coarse) / fine.min(coarse) < 1.6, "0.5 m cells {fine} s, 2 m cells {coarse} s");
    }

    #[test]
    fn a_fire_runs_faster_downwind_and_burns_alike_twice() {
        let reach = |wind: Vec2| {
            let mut fire = row_of_trees();
            fire.ignite(Vec3::ZERO, 1.0);
            run(&mut fire, 12.0, wind);
            let east = (1..=6).filter(|&i| fire.char_at(Vec3::new(i as f32 * 4.0, 0.0, 0.0)) > 0.0).count();
            let west = (1..=6).filter(|&i| fire.char_at(Vec3::new(i as f32 * -4.0, 0.0, 0.0)) > 0.0).count();
            (east, west, fire.cells().to_vec())
        };
        let (east, west, cells) = reach(Vec2::new(1.0, 0.0));
        assert!(east > west, "downwind {east} trees, upwind {west}");
        assert_eq!(cells, reach(Vec2::new(1.0, 0.0)).2, "the same fire twice");
    }

    #[test]
    fn a_fire_in_a_world_in_feet_spreads_as_many_cells_a_second_as_one_in_metres() {
        const FEET: f32 = 3.2808;
        // The same row of trees, in a world and a grid 3.2808 times as large, lit in the same place
        // (inside a cell, not on the corner of four, where a rounding error picks the cell) with the
        // same wind: `speed` and `reach` are metres, so it is the same fire.
        let lit = |world: f32, units: f32, seconds: f32| {
            let mut fire = row_of_trees_in(world);
            fire.ignite(Vec3::new(-24.5 * world, 0.0, 0.3 * world), 0.5 * world);
            run_in(&mut fire, seconds, Vec2::ZERO, units);
            fire.cells().iter().filter(|cell| cell.heat > 0.0 || cell.char > 0.0).count()
        };
        let mut last = 0;
        for seconds in [4.0, 8.0, 12.0, 16.0] {
            let metres = lit(1.0, 1.0, seconds);
            let feet = lit(FEET, FEET, seconds);
            assert!(metres > last, "{metres} cells alight after {seconds} s, {last} before");
            assert!(metres.abs_diff(feet) <= 1, "{metres} cells alight after {seconds} s in metres, {feet} in feet");
            last = metres;
        }
        // Told nothing of the unit, the fire is 3.2808 times too slow and its reach too short.
        let unaware = lit(FEET, 1.0, 16.0);
        assert!(unaware < last / 2, "an unaware fire lit {unaware} cells against {last}");
    }

    #[test]
    fn a_cell_without_fuel_goes_out_and_extinguish_keeps_the_char() {
        let mut fire = row_of_trees();
        // Off the row: nothing to burn.
        fire.ignite(Vec3::new(0.0, 0.0, 20.0), 0.5);
        assert!(fire.is_burning());
        run(&mut fire, 2.0, Vec2::ZERO);
        assert!(!fire.is_burning());
        assert_eq!(fire.char_at(Vec3::new(0.0, 0.0, 20.0)), 0.0);
        // On a tree: put out half-burnt, the char stays.
        fire.ignite(Vec3::ZERO, 0.5);
        run(&mut fire, 6.0, Vec2::ZERO);
        fire.extinguish(Vec3::ZERO, 30.0);
        let char = fire.char_at(Vec3::ZERO);
        assert!(!fire.is_burning() && char > 0.1 && char < 1.0, "char {char}");
        run(&mut fire, 5.0, Vec2::ZERO);
        assert_eq!(fire.char_at(Vec3::ZERO), char);
    }

    #[test]
    fn reading_back_is_bilinear_between_cell_centres_and_zero_off_the_grid() {
        let mut fire = FloraFire::with_cells(Vec2::ZERO, 4.0, 2);
        // Cells are 2 m; the lower corner is (-2, -2); centres at -1 and 1.
        fire.scorch(Vec3::new(-1.0, 0.0, -1.0), 0.1, 1.0);
        assert_eq!(fire.char_at(Vec3::new(-1.0, 0.0, -1.0)), 1.0);
        assert!((fire.char_at(Vec3::new(0.0, 0.0, -1.0)) - 0.5).abs() < 1.0e-6);
        assert!((fire.char_at(Vec3::new(0.0, 0.0, 0.0)) - 0.25).abs() < 1.0e-6);
        assert_eq!(fire.char_at(Vec3::new(1.0, 0.0, 1.0)), 0.0);
        // Half a cell past the edge, halfway to the zero outside.
        assert!((fire.char_at(Vec3::new(-2.0, 0.0, -1.0)) - 0.5).abs() < 1.0e-6);
        assert_eq!(fire.char_at(Vec3::new(-50.0, 0.0, -1.0)), 0.0);
    }

    #[test]
    fn the_burning_front_scorches_nothing_without_char_and_burns_every_leaf_at_char_one() {
        // A leaf at moment 0 is just out of reach of the glow and the scorch at char 0 ...
        assert!(0.0 - leaf_burn_front(0.0) > LEAF_GLOW + LEAF_SCORCH);
        // ... and the last leaf (moment just under 1) has burnt at char 1.
        assert!(leaf_burn_front(1.0) >= 1.0);
    }

    #[test]
    fn the_burning_cells_and_the_touched_bounds_follow_the_fire_from_spark_to_cold_embers() {
        // 512 half-metre cells a side, a 15 m stand of trees in one corner.
        let mut fire = FloraFire::with_cells(Vec2::ZERO, 256.0, 512);
        let stand: Vec<Vec3> =
            (0..36).map(|i| Vec3::new(-120.0 + (i % 6) as f32 * 2.5, 0.0, -120.0 + (i / 6) as f32 * 2.5)).collect();
        fire.refuel(stand.iter().copied(), 1.0);
        // What a full scan of the grid finds: the burning cells with their centres, and the
        // lowest and highest touched cell.
        let scan = |fire: &FloraFire| {
            let side = fire.cells_per_side() as usize;
            let (mut burning, mut touched) = (Vec::new(), None::<((usize, usize), (usize, usize))>);
            for (i, cell) in fire.cells().iter().enumerate() {
                let (x, z) = (i % side, i / side);
                if cell.heat > 0.0 {
                    let centre = Vec2::splat(-128.0) + (Vec2::new(x as f32, z as f32) + 0.5) * 0.5;
                    burning.push((centre, *cell));
                }
                if cell.heat > 0.0 || cell.char > 0.0 {
                    touched = Some(touched.map_or(((x, z), (x, z)), |(l, h): ((usize, usize), (usize, usize))| {
                        ((l.0.min(x), l.1.min(z)), (h.0.max(x), h.1.max(z)))
                    }));
                }
            }
            (burning, touched)
        };
        let check = |fire: &FloraFire| {
            let (burning, touched) = scan(fire);
            assert_eq!(fire.burning().collect::<Vec<_>>(), burning);
            let corner = |x: usize, z: usize| Vec2::splat(-128.0) + Vec2::new(x as f32, z as f32) * 0.5;
            match (fire.touched_bounds(), touched) {
                (None, None) => {}
                (Some(bounds), Some((low, high))) => {
                    // Edge to edge round the touched cells, not a cell more.
                    assert!(bounds.min.distance(corner(low.0, low.1)) < 1.0e-3, "{bounds:?} {low:?}");
                    assert!(bounds.max.distance(corner(high.0 + 1, high.1 + 1)) < 1.0e-3, "{bounds:?} {high:?}");
                    // The shaders' rectangle is the same, grown by a cell.
                    let shader = fire.vectors().nth(1).unwrap();
                    let grown = [bounds.min.x - 0.5, bounds.min.y - 0.5, bounds.max.x + 0.5, bounds.max.y + 0.5];
                    assert!(shader.iter().zip(grown).all(|(a, b)| (a - b).abs() < 1.0e-3), "{shader:?}");
                }
                other => panic!("the bounds and the cells disagree: {other:?}"),
            }
        };
        assert!(fire.touched_bounds().is_none() && fire.burning().next().is_none(), "nothing before the spark");
        check(&fire);

        fire.ignite(stand[0], 1.0);
        check(&fire);
        let (mut steps, mut most) = (0, 0);
        while fire.is_burning() {
            fire.step(Vec2::ZERO, 1.0);
            steps += 1;
            most = most.max(fire.burning().count());
            if steps % 25 == 0 {
                check(&fire);
            }
            assert!(steps < 3000, "the fire never went out");
        }
        check(&fire);
        assert!(most > 50, "the fire spread through the stand: {most} cells at most");
        // Cold: nothing burns, and the char keeps the bounds, round the stand alone.
        assert_eq!(fire.burning().count(), 0);
        let burnt = fire.touched_bounds().expect("the char stays");
        assert!(burnt.min.cmpge(Vec2::splat(-128.0)).all() && burnt.max.cmple(Vec2::splat(-100.0)).all(), "{burnt:?}");

        // Put out, restored, scorched: the bounds follow.
        fire.ignite(Vec3::new(100.0, 0.0, 100.0), 0.2);
        assert!(fire.touched_bounds().unwrap().max.x > 100.0);
        fire.extinguish(Vec3::new(100.0, 0.0, 100.0), 1.0);
        assert_eq!(fire.touched_bounds(), Some(burnt), "a spark put out leaves no char");
        fire.restore(&vec![FireCell::default(); 512 * 512]);
        assert!(fire.touched_bounds().is_none());
        fire.scorch(Vec3::new(10.0, 0.0, -10.0), 1.0, 0.5);
        check(&fire);
    }
}
