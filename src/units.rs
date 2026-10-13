//! The world's unit: how many world units make a metre ([`FloraUnits`]).
//!
//! The crate builds trees in metres and a game places them with a `Transform` that scales them to
//! its own world, whatever its unit (a foot world scales its trees by about 3.28). Everything
//! the crate measures in the world itself, though, has to agree with that unit too: how fast a
//! tree falls, how far a leaf drifts, how wide a gust is, how far a fire reaches. Insert
//! [`FloraUnits`] with the number of world units to a metre and they follow.
//!
//! The rule, written on every field it touches: **a field that sizes or times the crate's own
//! simulation is in metres** (and scaled by [`FloraUnits::units_per_metre`] where it meets the
//! world: [`FellTree::cut_height`](crate::FellTree::cut_height), [`BuckTree`](crate::BuckTree),
//! [`FireSpread`](crate::FireSpread), [`FoliagePusher`](crate::FoliagePusher),
//! [`SeeThroughTrees`](crate::SeeThroughTrees), [`TreeShake::radius`](crate::TreeShake::radius),
//! [`GustFront`](crate::GustFront), [`LodDistances`](crate::LodDistances),
//! [`FallingLeaves::autumn_distance`](crate::FallingLeaves::autumn_distance)); **a position, or a
//! size that addresses the world, is in world units** (every `at`, `target`, `centre`, `origin`
//! and `feet`; [`FloraFire::new`](crate::FloraFire::new)'s size, the radius of
//! [`FloraFire::ignite`](crate::FloraFire::ignite); [`FloraIndex`](crate::FloraIndex)'s radii and
//! distances; [`Scatter`](crate::Scatter)'s area, spacing and grove size; [`FloraGround`](crate::FloraGround)'s
//! heights). A [`Transform`](bevy::transform::components::Transform) is always world units.
//!
//! The default, one world unit to a metre, changes nothing: a game in metres never needs this.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraUnits;
//!
//! // A world in feet.
//! fn setup(mut commands: Commands) {
//!     commands.insert_resource(FloraUnits { units_per_metre: 3.2808 });
//! }
//! ```
//!
//! Set it before planting: a pooled tree's bands of distance, for one, are made when it is
//! spawned. Changing it later moves the weather the shaders read and the simulations at once, not
//! what was already placed.

use bevy::ecs::prelude::*;
use bevy::reflect::Reflect;

/// How many world units make a metre: see the [module documentation](self).
///
/// Must be positive. A resource; [`FloraPlugin`](crate::FloraPlugin) adds the default (1.0, a
/// world in metres), and a game in another unit inserts its own.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct FloraUnits {
    /// World units to the metre: 1.0 for a world in metres, about 3.2808 for one in feet, 100.0 in
    /// centimetres. Every field of the crate documented as metres is multiplied by this where it
    /// meets the world; a field documented as world units is not.
    pub units_per_metre: f32,
}

impl Default for FloraUnits {
    fn default() -> Self {
        Self { units_per_metre: 1.0 }
    }
}

impl FloraUnits {
    /// `metres` in world units.
    pub fn world(&self, metres: f32) -> f32 {
        metres * self.units_per_metre
    }

    /// `world` units in metres.
    pub fn metres(&self, world: f32) -> f32 {
        world / self.units_per_metre
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_a_world_in_metres_and_conversions_round_trip() {
        let metres = FloraUnits::default();
        assert_eq!((metres.world(2.5), metres.metres(2.5)), (2.5, 2.5));
        let feet = FloraUnits { units_per_metre: 3.2808 };
        assert!((feet.world(10.0) - 32.808).abs() < 1.0e-4);
        assert!((feet.metres(feet.world(7.3)) - 7.3).abs() < 1.0e-5);
    }
}
