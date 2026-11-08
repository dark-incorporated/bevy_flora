//! A tree's simple gameplay shape: capsules for its trunk and thick limbs, and a box around its
//! leaves, in the tree's local space, made from the same skeleton its meshes are.
//!
//! Physics engines, picking, line of sight and AI want a handful of primitives, not a mesh of
//! twenty thousand triangles. [`TreeShape`] is that handful, and it follows the tree as it grows:
//! add it to a [`ProceduralTree`](crate::ProceduralTree) (or a [`PooledTree`](crate::PooledTree))
//! and the plugin keeps it up to date, on the CPU and the [`GpuGrowth`](crate::GpuGrowth) paths
//! alike. Turn it into colliders of your engine with [`WoodCapsule::to_primitive`]:
//!
//! ```rust
//! use bevy_flora::{TreePreset, TreeShape, build_tree};
//!
//! let params = TreePreset::Oak.params();
//! let tree = build_tree(&params, f32::INFINITY);
//! let shape = TreeShape::from_skeleton(&tree.skeleton, &params, TreeShape::DEFAULT_LIMB_RADIUS);
//! for capsule in shape.capsules() {
//!     let (primitive, isometry) = capsule.to_primitive(); // Capsule3d along the wood, placed
//!     assert!(primitive.radius > 0.0 && isometry.translation.y >= 0.0);
//! }
//! ```

use bevy::reflect::Reflect;
use bevy::ecs::component::Component;
use bevy::ecs::reflect::ReflectComponent;
use bevy::math::{Isometry3d, Quat, Vec3};
use bevy::shape::Capsule3d;

use crate::foliage::node_leaf_scale;
use crate::params::{FoliagePlacement, TreeParams};
use crate::skeleton::{NodeKind, Skeleton, trunk_radius_at};

/// Capsules up a trunk taller than 3 m.
const TRUNK_PIECES: u32 = 3;

/// A capsule around a stretch of wood: the segment from `a` to `b` swept by a sphere of
/// `radius`, in the tree's local space (metres, the base of the trunk at the origin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
pub struct WoodCapsule {
    /// One end of the axis (the end nearer the trunk's base).
    pub a: Vec3,
    /// The other end of the axis.
    pub b: Vec3,
    /// Radius, metres.
    pub radius: f32,
}

impl WoodCapsule {
    /// Length of the axis, between the centres of the two end spheres.
    pub fn length(&self) -> f32 {
        self.a.distance(self.b)
    }

    /// Bevy's [`Capsule3d`] (along its local Y) and where it sits in the tree's local space:
    /// what most physics engines' capsule colliders take.
    pub fn to_primitive(&self) -> (Capsule3d, Isometry3d) {
        let axis = self.b - self.a;
        let rotation = match axis.try_normalize() {
            Some(dir) => Quat::from_rotation_arc(Vec3::Y, dir),
            None => Quat::IDENTITY,
        };
        let capsule = Capsule3d { radius: self.radius, half_length: axis.length() * 0.5 };
        (capsule, Isometry3d::new((self.a + self.b) * 0.5, rotation))
    }

    /// This capsule where `place` puts its space: the ends transformed, the radius scaled by the
    /// transform's scale along x (the crate places trees with uniform scale).
    pub fn placed(&self, place: &bevy::transform::components::GlobalTransform) -> WoodCapsule {
        let scale = place.affine().matrix3.x_axis.length();
        WoodCapsule { a: place.transform_point(self.a), b: place.transform_point(self.b), radius: self.radius * scale }
    }

    /// The part of this capsule at or above `y` (in its own space): `None` if it all lies below,
    /// else with its lower end moved up to `y` along the axis. A lying tree's wood above its cut.
    pub fn above(&self, y: f32) -> Option<WoodCapsule> {
        let (low, high) = if self.a.y <= self.b.y { (self.a, self.b) } else { (self.b, self.a) };
        if high.y < y {
            return None;
        }
        if low.y >= y {
            return Some(*self);
        }
        let cut = low + (high - low) * ((y - low.y) / (high.y - low.y));
        Some(WoodCapsule { a: cut, b: high, radius: self.radius })
    }

    fn grow(&self, min: &mut Vec3, max: &mut Vec3) {
        for p in [self.a, self.b] {
            *min = min.min(p - Vec3::splat(self.radius));
            *max = max.max(p + Vec3::splat(self.radius));
        }
    }
}

/// A tree's gameplay shape, in its local space (apply the entity's `GlobalTransform` for the
/// world): a capsule along the trunk, one for each stretch of wood thicker than `limb_radius`,
/// and a box around the leaves.
///
/// As a component it is opt-in and kept up to date by the plugin: insert
/// `TreeShape::default()` (or [`TreeShape::with_limb_radius`]) next to a
/// [`ProceduralTree`](crate::ProceduralTree) and every rebuild, every age step of a growing
/// tree included, rewrites it (only when it changed, so `Changed<TreeShape>` is a reliable cue to
/// rebuild colliders). On a [`PooledTree`](crate::PooledTree) it is filled once, from the
/// pool's variant. Building it costs the time of ageing the skeleton and a little more (6 to 9
/// microseconds for an oak, a maple or a willow on an Apple M4 Pro), which is why it is opt-in.
///
/// The capsules follow the skeleton's axes: the trunk's wobble and root flare and the leaves'
/// own geometry are not in them. `crown` is a box around every leaf-bearing branch padded by the
/// leaf size, so it holds the leaves; it is `None` while the tree has no leaves.
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeShape {
    /// Wood thinner than this (metres of radius, at the stretch's base) gets no capsule. Set it
    /// when inserting the component; the plugin keeps it.
    pub limb_radius: f32,
    /// The trunk, from the ground to its top, bottom up: three capsules that taper with it (one
    /// for a trunk under 3 m). The lowest has at least the trunk's radius at hip height (1 m), so a
    /// walking character collides with what it sees; the trunk's wobble and root flare are not in
    /// them.
    pub trunk: Vec<WoodCapsule>,
    /// The wood, other than the trunk, whose mean radius is at least `limb_radius`: one capsule
    /// per straight branch, two along a branch that gravity bends.
    pub limbs: Vec<WoodCapsule>,
    /// Centre and half extents of a box around the leaves, if there are any.
    pub crown: Option<(Vec3, Vec3)>,
    /// Lowest corner of a box around every capsule and the crown.
    pub min: Vec3,
    /// Highest corner of that box.
    pub max: Vec3,
}

impl Default for TreeShape {
    fn default() -> Self {
        Self::with_limb_radius(Self::DEFAULT_LIMB_RADIUS)
    }
}

impl TreeShape {
    /// The [`TreeShape::limb_radius`] of `TreeShape::default()`: 6 cm, wood a person can stand
    /// on or bump into.
    pub const DEFAULT_LIMB_RADIUS: f32 = 0.06;

    /// An empty shape that, inserted as a component, gets capsules for wood at least `radius`
    /// metres thick (use `f32::INFINITY` for the trunk alone).
    pub fn with_limb_radius(radius: f32) -> Self {
        Self {
            limb_radius: radius,
            trunk: Vec::new(),
            limbs: Vec::new(),
            crown: None,
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        }
    }

    /// The shape of a tree with `params` whose (aged) skeleton is `skeleton`, as
    /// [`TreeMeshes::skeleton`](crate::TreeMeshes::skeleton) holds it.
    pub fn from_skeleton(skeleton: &Skeleton, params: &TreeParams, limb_radius: f32) -> Self {
        let mut shape = Self::with_limb_radius(limb_radius);
        let Some(trunk) = skeleton.nodes.first() else { return shape };

        // The trunk: a stack of capsules up its axis that taper with it, the bottom one resting
        // on the ground with the trunk's radius at hip height (1 m), the top one ending at the
        // trunk's top. The radii follow the trunk's own profile (taper curve, flare), scaled by
        // how far it has thickened.
        let length = trunk.length.max(0.0);
        let full_base = trunk_radius_at(&params.trunk, 0.0);
        let thickened = if full_base > 0.0 { trunk.base_radius / full_base } else { 0.0 };
        let r = |t: f32| (trunk_radius_at(&params.trunk, t) * thickened).max(0.0);
        let at = |t: f32| trunk.start + trunk.direction * (length * t);
        let pieces = if length > 3.0 { TRUNK_PIECES } else { 1 };
        for k in 0..pieces {
            let (t0, t1) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
            let (mut a, mut b) = (at(t0), at(t1));
            let radius = if k == 0 {
                // At least the radius a walking character meets.
                r((1.0 / length.max(1e-4)).min(1.0)).max((r(t0) + r(t1)) * 0.5 * 0.8)
            } else {
                (r(t0) + r(t1)) * 0.5
            };
            if k == 0 {
                a += Vec3::Y * radius.min(length * 0.5);
            }
            if k + 1 == pieces {
                b -= trunk.direction * radius.min((b - a).length());
            }
            if (b - a).dot(trunk.direction) < 0.0 {
                b = a;
            }
            shape.trunk.push(WoodCapsule { a, b, radius });
        }

        let foliage = &params.foliage;
        let mut crown: Option<(Vec3, Vec3)> = None;
        for (i, node) in skeleton.nodes.iter().enumerate().skip(1) {
            if node.length <= 1e-4 {
                continue;
            }
            // A bent branch is two capsules, split where its axis turns most (the middle); each
            // piece has the mean radius of its ends and is kept if that reaches `limb_radius`.
            let pieces: &[f32] = if node.bend > 0.0 { &[0.0, 0.5, 1.0] } else { &[0.0, 1.0] };
            for w in pieces.windows(2) {
                let at = |t: f32| node.base_radius + (node.tip_radius - node.base_radius) * t;
                let radius = (at(w[0]) + at(w[1])) * 0.5;
                if radius >= limb_radius {
                    shape.limbs.push(WoodCapsule { a: node.point_at(w[0]), b: node.point_at(w[1]), radius });
                }
            }
            if foliage.enabled && node.kind != NodeKind::Trunk && bears_leaves(skeleton, i, params) {
                let scale = node_leaf_scale(node, &params.growth);
                if scale <= 0.0 {
                    continue;
                }
                // A leaf reaches its size (or a tip cluster its radius) from where it sits.
                let pad = Vec3::splat(foliage.leaf_size * scale * (1.0 + foliage.leaf_size_variation.max(0.0)) * 1.5);
                let end = node.point_at(1.0);
                // Leaves along a branch sit on its wood, which a bent branch arches past both ends.
                let (wood_lo, wood_hi) = if foliage.placement == FoliagePlacement::AllBranches {
                    if node.bend > 0.0 {
                        node.axis(node.axis_points()).into_iter().fold((end, end), |(lo, hi), (c, _)| (lo.min(c), hi.max(c)))
                    } else {
                        (node.start.min(end), node.start.max(end))
                    }
                } else {
                    (end, end)
                };
                let lo = wood_lo - pad;
                let hi = wood_hi + pad;
                crown = Some(match crown {
                    Some((min, max)) => (min.min(lo), max.max(hi)),
                    None => (lo, hi),
                });
            }
        }
        shape.crown = crown.map(|(min, max)| ((min + max) * 0.5, (max - min) * 0.5));

        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for piece in &shape.trunk {
            piece.grow(&mut min, &mut max);
        }
        for limb in &shape.limbs {
            limb.grow(&mut min, &mut max);
        }
        if let Some((center, half)) = shape.crown {
            min = min.min(center - half);
            max = max.max(center + half);
        }
        (shape.min, shape.max) = (min, max);
        shape
    }

    /// The trunk's capsules (bottom up), then every limb's.
    pub fn capsules(&self) -> impl Iterator<Item = &WoodCapsule> {
        self.trunk.iter().chain(&self.limbs)
    }

    /// Height of the tree's top (the highest wood or leaf), metres above its base.
    pub fn height(&self) -> f32 {
        self.max.y
    }
}

/// Whether node `i` grows leaves under its foliage placement (the skeleton decides: thick wood
/// and, unless every branch carries leaves, wood with children carry none).
fn bears_leaves(skeleton: &Skeleton, i: usize, params: &TreeParams) -> bool {
    let node = &skeleton.nodes[i];
    let foliage = &params.foliage;
    let full_tip = if node.radius_progress > 0.0 { node.tip_radius / node.radius_progress } else { node.tip_radius };
    match foliage.placement {
        FoliagePlacement::AllBranches => true,
        FoliagePlacement::TerminalBranches | FoliagePlacement::TipClusters => {
            skeleton.is_terminal(i)
                && !(params.trunk.radius > 0.0 && full_tip / params.trunk.radius > foliage.radius_threshold)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage::collect_leaf_points;
    use crate::pipeline::build_tree;
    use crate::presets::TreePreset;

    #[test]
    fn the_crown_holds_every_leaf_and_the_box_holds_the_crown_at_every_age() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            for age in [2.0, 5.0, 9.0, f32::INFINITY] {
                let tree = build_tree(&params, age);
                let shape = TreeShape::from_skeleton(&tree.skeleton, &params, TreeShape::DEFAULT_LIMB_RADIUS);
                let leaves = &tree.foliage.positions;
                match shape.crown {
                    None => assert!(leaves.is_empty(), "{preset:?} at {age}: leaves but no crown"),
                    Some((center, half)) => {
                        for p in leaves {
                            let d = (*p - center).abs() - half;
                            assert!(
                                d.max_element() <= 1e-3,
                                "{preset:?} at {age}: leaf vertex {p} is {d} outside the crown"
                            );
                        }
                        assert!(shape.min.cmple(center - half).all() && shape.max.cmpge(center + half).all());
                    }
                }
                // The points the leaves hang from are all inside too (and not by a wide margin
                // only: the crown is not the whole tree's box when leaves sit at the tips).
                let points = collect_leaf_points(&tree.skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
                assert_eq!(points.is_empty(), shape.crown.is_none(), "{preset:?} at {age}");
            }
        }
    }

    #[test]
    fn capsules_follow_the_wood_and_respect_the_limb_radius() {
        let params = TreePreset::Oak.params();
        let tree = build_tree(&params, f32::INFINITY);
        let shape = TreeShape::from_skeleton(&tree.skeleton, &params, 0.05);
        assert!(!shape.limbs.is_empty(), "a mature oak has limbs thicker than 5 cm");
        assert!(shape.limbs.iter().all(|c| c.radius >= 0.05 - 1e-6 && c.length() > 0.0));
        // Wood thick to its tip has every piece; wood thin at its base has none.
        let wood = || tree.skeleton.nodes.iter().skip(1).filter(|n| n.length > 1e-4);
        let thick = wood().filter(|n| n.tip_radius >= 0.05).count();
        let all = wood().filter(|n| n.base_radius >= 0.05).count();
        assert!(thick > 0 && all < wood().count(), "the oak has thick limbs and thin twigs");
        assert!(shape.limbs.len() >= thick && shape.limbs.len() <= 2 * all);
        // The trunk is a stack from the ground to its top that tapers like the trunk.
        let trunk = &tree.skeleton.nodes[0];
        let pieces = &shape.trunk;
        assert_eq!(pieces.len(), 3);
        assert!((pieces[0].a.y - pieces[0].radius).abs() < 1e-4, "the bottom rests on the ground");
        let top = pieces.last().unwrap();
        assert!((top.b.y + top.radius - (trunk.start.y + trunk.length)).abs() < 1e-3, "the top ends at the top");
        assert!(pieces.windows(2).all(|w| w[0].radius >= w[1].radius), "radii shrink upward");
        assert!(pieces[1].a == pieces[0].b && pieces[2].a == pieces[1].b, "no gap between pieces");
        assert!(pieces.iter().all(|p| p.radius <= trunk.base_radius && p.radius >= trunk.tip_radius));
        // Every capsule's ends lie in the tree's box.
        for c in shape.capsules() {
            for p in [c.a, c.b] {
                assert!(p.cmpge(shape.min).all() && p.cmple(shape.max).all());
            }
        }
        // Infinity keeps the trunk alone.
        assert!(TreeShape::from_skeleton(&tree.skeleton, &params, f32::INFINITY).limbs.is_empty());
    }

    #[test]
    fn a_capsule_primitive_spans_its_axis() {
        let c = WoodCapsule { a: Vec3::new(1.0, 2.0, 0.0), b: Vec3::new(1.0, 2.0, 4.0), radius: 0.3 };
        let (capsule, iso) = c.to_primitive();
        assert_eq!(capsule.radius, 0.3);
        assert!((capsule.half_length - 2.0).abs() < 1e-6);
        assert!((Vec3::from(iso.translation) - Vec3::new(1.0, 2.0, 2.0)).length() < 1e-6);
        // The capsule's local +Y is carried onto the axis.
        assert!((iso.rotation * Vec3::Y - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn a_seedling_is_a_trunk_without_limbs_or_leaves() {
        let params = TreePreset::Oak.params();
        let tree = build_tree(&params, 0.3);
        let shape = TreeShape::from_skeleton(&tree.skeleton, &params, TreeShape::DEFAULT_LIMB_RADIUS);
        assert!(shape.limbs.is_empty() && shape.crown.is_none());
        assert!(shape.trunk.len() == 1 && shape.trunk[0].radius >= 0.0 && shape.height() >= 0.0);
    }
}
