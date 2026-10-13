//! The ground trees stand on, for a world that is not flat.
//!
//! [`FloraGround`] is the height of the ground at every point (x, z) of the world: your
//! terrain's height function or a lookup into its heightmap. It plants a tree so that no side
//! of its trunk floats over a slope ([`FloraGround::footing`],
//! [`ScatterPoint::transform_on`](crate::ScatterPoint::transform_on)), and, inserted as a
//! resource, it is the ground a felled tree is cut above and lands on (see
//! [`felling`](crate::felling)). Without the resource the plugin takes the ground to be flat at
//! the height each tree stands at.
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_flora::{FloraGround, TreePreset};
//!
//! // Rolling hills, the same function the terrain mesh is built from.
//! let ground = FloraGround::new(|at: Vec2| 4.0 * (at.x * 0.05).sin() * (at.y * 0.04).cos());
//! let oak = TreePreset::Oak.params();
//! let at = Vec2::new(12.0, -30.0);
//! // Stand the oak where the lowest side of its trunk meets the ground.
//! let place = Transform::from_xyz(at.x, ground.footing(at, oak.base_radius()), at.y);
//! assert!(place.translation.y <= ground.height(at));
//! // No trees on ground steeper than 35°.
//! let plantable = ground.slope(at) < 35f32.to_radians();
//! # let _ = plantable;
//! ```

use std::f32::consts::TAU;
use std::fmt;
use std::sync::Arc;

use bevy::ecs::resource::Resource;
use bevy::math::{Vec2, Vec3};

/// How many points around a trunk's base [`FloraGround::footing`] looks at.
const FOOTING_SAMPLES: usize = 16;
/// Half the distance, in world units, over which [`FloraGround::normal`] measures the slope.
const SLOPE_STEP: f32 = 0.25;

/// The height of the ground at every point (x, z) of the world, in world units. See the [module
/// documentation](self).
///
/// Cloning is cheap (the function is shared). The function is called from the felling systems
/// and from your placement code, a few hundred to a few thousand times for each tree felled, so
/// it should be cheap: an analytic height or a heightmap lookup, not a ray cast.
#[derive(Resource, Clone)]
pub struct FloraGround {
    height: Arc<dyn Fn(Vec2) -> f32 + Send + Sync>,
}

impl FloraGround {
    /// The ground whose height at (x, z) is `height(Vec2::new(x, z))`.
    pub fn new(height: impl Fn(Vec2) -> f32 + Send + Sync + 'static) -> Self {
        Self { height: Arc::new(height) }
    }

    /// Flat ground at `height`.
    pub fn flat(height: f32) -> Self {
        Self::new(move |_| height)
    }

    /// The height of the ground at `at` (world x, z).
    pub fn height(&self, at: Vec2) -> f32 {
        (self.height)(at)
    }

    /// The ground's upward unit normal at `at`, from its heights 0.25 world units to either side.
    pub fn normal(&self, at: Vec2) -> Vec3 {
        let dx = self.height(at + Vec2::X * SLOPE_STEP) - self.height(at - Vec2::X * SLOPE_STEP);
        let dz = self.height(at + Vec2::Y * SLOPE_STEP) - self.height(at - Vec2::Y * SLOPE_STEP);
        Vec3::new(-dx, 2.0 * SLOPE_STEP, -dz).normalize_or(Vec3::Y)
    }

    /// How steep the ground is at `at`: the angle between it and the horizontal, in radians.
    pub fn slope(&self, at: Vec2) -> f32 {
        self.normal(at).y.clamp(-1.0, 1.0).acos()
    }

    /// The height to stand a trunk `radius` world units wide at `at`: the lowest ground under its
    /// base, so the trunk meets the ground on every side and does not float on the downhill
    /// one. On flat ground it is the ground's height. `radius` is
    /// [`TreeParams::base_radius`](crate::TreeParams::base_radius) times the tree's scale.
    pub fn footing(&self, at: Vec2, radius: f32) -> f32 {
        self.around(at, radius).fold(f32::INFINITY, f32::min)
    }

    /// The highest ground under a trunk `radius` world units wide at `at`: its uphill side.
    pub(crate) fn top(&self, at: Vec2, radius: f32) -> f32 {
        self.around(at, radius).fold(f32::NEG_INFINITY, f32::max)
    }

    /// The ground's heights at `at` and on a circle of `radius` around it.
    fn around(&self, at: Vec2, radius: f32) -> impl Iterator<Item = f32> + '_ {
        let ring = (0..FOOTING_SAMPLES).map(move |k| {
            let (sin, cos) = (k as f32 * TAU / FOOTING_SAMPLES as f32).sin_cos();
            self.height(at + Vec2::new(cos, sin) * radius)
        });
        std::iter::once(self.height(at)).chain(ring)
    }
}

impl Default for FloraGround {
    /// Flat ground at height 0.
    fn default() -> Self {
        Self::flat(0.0)
    }
}

impl fmt::Debug for FloraGround {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FloraGround").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trunk_on_a_slope_stands_where_its_downhill_side_meets_the_ground() {
        // Falling 0.3 m a metre toward +x: about 16.7°.
        let ground = FloraGround::new(|at: Vec2| 10.0 - 0.3 * at.x);
        let at = Vec2::new(4.0, -2.0);
        let radius = 0.5;
        let footing = ground.footing(at, radius);
        assert!((footing - (ground.height(at) - 0.3 * radius)).abs() < 1.0e-4, "footing {footing}");
        assert!((ground.top(at, radius) - (ground.height(at) + 0.3 * radius)).abs() < 1.0e-4);
        // No point of the trunk's base circle stands above the ground there.
        for k in 0..64 {
            let (sin, cos) = (k as f32 * TAU / 64.0).sin_cos();
            let edge = at + Vec2::new(cos, sin) * radius;
            assert!(footing <= ground.height(edge) + 1.0e-4);
        }
        assert!((ground.slope(at) - 0.3f32.atan()).abs() < 1.0e-4);
        let normal = ground.normal(at);
        assert!(normal.x > 0.0 && normal.z.abs() < 1.0e-6 && (normal.length() - 1.0).abs() < 1.0e-5);
        let flat = FloraGround::flat(2.0);
        assert_eq!((flat.footing(at, radius), flat.slope(at)), (2.0, 0.0));
    }
}
