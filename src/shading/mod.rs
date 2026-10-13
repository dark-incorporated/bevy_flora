//! Leaf shading and wind: the material trees are drawn with.
//!
//! [`FloraMaterial`] is a [`StandardMaterial`] with the shaders in this directory:
//!
//! - **Leaves** get an upper side that is darker and glossier than the paler, matte lower
//!   side, a midrib and side veins, a hue of their own and darker interiors of the crown (all
//!   baked into the leaf mesh by [`foliage_mesh`](crate::foliage::foliage_mesh)), and light
//!   that comes through a leaf lit from behind: the wrapped back-side `N·L` of Unreal's
//!   two-sided foliage model times a forward-scattering GGX lobe and the leaf colour, so the
//!   crown glows yellow-green toward the sun and stays dark where other leaves shade it. A
//!   leaf's gloss fades as the camera turns to face the sun: a flat card stands for a leaf or a
//!   cluster of needles, whose many surfaces would scatter the sun's glint, and reflecting all
//!   of it at once turned treetops white.
//! - **Wind** ([`FloraWind`]) bends a tree as a whole, more toward its top, and flutters each
//!   leaf (Crysis' vegetation animation, GPU Gems 3 chapter 16). A species answers it in its
//!   own way ([`WindResponse`](crate::WindResponse)), and a [`GustFront`] sweeps a band of
//!   stronger wind across the world. It runs in the vertex stage of the main pass and of the
//!   shadow maps alike, so shadows follow the motion, and costs the CPU nothing per frame:
//!   changing [`FloraWind`] writes one small shared buffer ([`FloraWindBuffer`]) and touches no
//!   material.
//!
//! The plugin uses it for every tree it draws ([`ProceduralTree`](crate::ProceduralTree),
//! [`GpuGrowth`](crate::GpuGrowth), [`TreePool`](crate::TreePool) and its impostors).

use bevy::app::App;
use bevy::asset::{Asset, AssetApp, AssetServer, Assets, Handle, embedded_asset, load_embedded_asset};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::log::warn;
use bevy::material::OpaqueRendererMethod;
use bevy::image::Image;
use bevy::math::{Vec2, Vec4};
use bevy::mesh::{Mesh, MeshVertexBufferLayoutRef};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin, MeshPipelineKey, PbrPlugin, StandardMaterial,
};
use bevy::reflect::Reflect;
use bevy::render::RenderApp;
use bevy::render::render_resource::{
    AsBindGroup, Face, FragmentState, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::{ShaderDefVal, ShaderRef};
use bevy::time::Time;

use crate::foliage::uses_scan;
use crate::impostor::ImpostorSupport;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::params::{CrownStyle, LeafStyle, RenderStyle, TreeParams};
use crate::push::{PUSH_SLOT, PUSH_VECTORS, PushSlots};
use crate::see_through::{SEE_THROUGH_SLOT, SEE_THROUGH_VECTORS, SeeThroughCapsule};
use crate::units::FloraUnits;

mod share;
pub use share::SharedFlora;

mod textures;
pub use textures::{BarkTexture, FloraTextures, LeafTexture, TextureLibrary};

/// A gust that sweeps across the world: a band of stronger wind, `width` metres deep, moving
/// along `direction` at `speed`. Launch it with [`FloraWind::launch_gust`]. `speed` and `width`
/// are metres, scaled to the world by [`FloraUnits`]; `origin` is a position,
/// in world units.
///
/// Trees lean along `direction` and flutter harder while the band is over them, most at its
/// centre. The band has no sides: it reaches every tree it passes, however far to the side of
/// `origin`.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct GustFront {
    /// Where the front is (world x, z) at `start`.
    pub origin: Vec2,
    /// Which way it travels and pushes the trees, world x and z; normalised when used. Zero
    /// turns it off.
    pub direction: Vec2,
    /// Metres per second ([`FloraUnits`] scales it to the world's unit).
    pub speed: f32,
    /// Depth of the band of strong wind, metres (at least 0.01; scaled by
    /// [`FloraUnits`]).
    pub width: f32,
    /// Wind strength added at the front's peak, on the scale of [`FloraWind::strength`].
    pub strength: f32,
    /// Render time ([`Time::elapsed_secs_wrapped`], the shaders' `globals.time`) when the front
    /// was at `origin`.
    pub start: f32,
}

impl Default for GustFront {
    fn default() -> Self {
        Self { origin: Vec2::ZERO, direction: Vec2::X, speed: 15.0, width: 30.0, strength: 1.0, start: 0.0 }
    }
}

/// The wind every tree drawn with [`FloraMaterial`] moves in.
///
/// Trees bend downwind as a whole, slowly and by more toward the top, and their leaves flutter
/// faster; how far and how fast each species does is [`TreeParams::wind`]. It is global: the
/// plugin writes it into one shared GPU buffer ([`FloraWindBuffer`]) whenever it changes, and
/// no material is touched, so a game may animate it every frame at the cost of one small buffer
/// write. Set `strength` to 0 and `gust` to `None` for a still scene (the gallery sets strength 0,
/// so its captures repeat).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FloraWind {
    /// Where the wind blows toward, on the ground: world x and z. Zero is still air.
    pub direction: Vec2,
    /// 0 is calm, 1 a strong wind; more bends further (the bend keeps every part's distance from
    /// the trunk). Negative is treated as 0.
    pub strength: f32,
    /// Multiplies time: 2 moves everything twice as fast.
    pub speed: f32,
    /// How much leaves flutter, relative to the default of 1.
    pub flutter: f32,
    /// How far trees bend for a given `strength`, relative to the default of 1.
    pub bend: f32,
    /// How much the slow gusts vary the wind, from 0 (steady) to 1 (from calm to full).
    pub gust_strength: f32,
    /// How often gusts come, relative to the default of 1.
    pub gust_frequency: f32,
    /// How fast leaves flutter, relative to the default of 1.
    pub flutter_frequency: f32,
    /// Sideways sway across the wind, as a share of the bend downwind.
    pub turbulence: f32,
    /// A gust travelling across the world, if any: see [`GustFront`] and
    /// [`launch_gust`](Self::launch_gust).
    pub gust: Option<GustFront>,
}

impl Default for FloraWind {
    fn default() -> Self {
        Self {
            direction: Vec2::new(0.8, 0.6),
            strength: 0.3,
            speed: 1.0,
            flutter: 1.0,
            bend: 1.0,
            gust_strength: 0.4,
            gust_frequency: 1.0,
            flutter_frequency: 1.0,
            turbulence: 0.2,
            gust: None,
        }
    }
}

impl FloraWind {
    /// Send `front` across the world, starting now: it was at `front.origin` when this was
    /// called. It replaces any gust still under way; there is one at a time.
    ///
    /// The shaders time the front with the render clock, [`Time::elapsed_secs_wrapped`], which
    /// starts over every `Time::wrap_period` (an hour by default); a front launched less than
    /// its crossing time before that wrap jumps.
    pub fn launch_gust(&mut self, mut front: GustFront, time: &Time) {
        front.start = time.elapsed_secs_wrapped();
        self.gust = Some(front);
    }
}

/// A knock that shakes the trees near a point: an axe or a sword on a trunk, a character pushing
/// through, a blast. Send it with [`TreeShakes::shake`].
///
/// Every tree whose trunk stands within `radius` of `at` swings, the nearest the most: first away
/// from `at` (or along `direction`), then back and forth, dying away over `duration`, while its
/// leaves rustle. It is the whole tree that sways, like a bend in the wind (each tree at its own
/// pace), on every way a tree is drawn: [`ProceduralTree`](crate::ProceduralTree),
/// [`GpuGrowth`](crate::GpuGrowth), pooled trees and their impostors. Nothing moves on the CPU:
/// [`TreeShape`](crate::TreeShape) colliders stay where they are. It also knocks leaves off the
/// trees it reaches ([`FallingLeaves`](crate::FallingLeaves)).
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct TreeShake {
    /// Where it happens, on the ground: world x and z.
    pub at: Vec2,
    /// Trees whose trunks stand closer to `at` than this many metres shake (at least 0.01; scaled
    /// to the world by [`FloraUnits`]): about a metre singles out the tree that was hit, tens of
    /// metres reach a whole grove.
    pub radius: f32,
    /// How far the trees swing, on the scale of [`FloraWind::strength`]: 1 leans a tree about as
    /// far as a strong wind does, before the swing dies away. Negative is treated as 0.
    pub strength: f32,
    /// Which way the trees swing first, world x and z; normalised when used. Zero pushes each tree
    /// away from `at` (a tree standing exactly there is pushed along +x).
    pub direction: Vec2,
    /// Seconds until the shaking has died away (at least 0.01).
    pub duration: f32,
    /// Render time ([`Time::elapsed_secs_wrapped`], the shaders' `globals.time`) when it began;
    /// set by [`TreeShakes::shake`].
    pub start: f32,
}

impl Default for TreeShake {
    fn default() -> Self {
        Self { at: Vec2::ZERO, radius: 1.5, strength: 1.0, direction: Vec2::ZERO, duration: 2.5, start: 0.0 }
    }
}

/// The trees being shaken (see [`TreeShake`]): up to [`TreeShakes::MAX`] at a time. Like
/// [`FloraWind`] it lives in the shared buffer, so shaking costs one small buffer write and
/// touches no material or mesh, and nothing at all while no shake is under way.
#[derive(Resource, Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct TreeShakes {
    shakes: Vec<TreeShake>,
}

impl TreeShakes {
    /// How many shakes can be under way at once.
    pub const MAX: usize = 8;

    /// Shake the trees near `shake.at`, starting now. With [`MAX`](Self::MAX) shakes already
    /// stored, it takes the place of the one that ends soonest, so a new knock always shows.
    ///
    /// The shaders time it with the render clock, [`Time::elapsed_secs_wrapped`], which starts
    /// over every `Time::wrap_period` (an hour by default); a shake under way at that wrap stops.
    pub fn shake(&mut self, mut shake: TreeShake, time: &Time) {
        shake.start = time.elapsed_secs_wrapped();
        if self.shakes.len() < Self::MAX {
            self.shakes.push(shake);
        } else if let Some(soonest) =
            self.shakes.iter_mut().min_by(|a, b| (a.start + a.duration).total_cmp(&(b.start + b.duration)))
        {
            *soonest = shake;
        }
    }

    /// Stop every shake at once.
    pub fn clear(&mut self) {
        self.shakes.clear();
    }

    /// The shakes under way: the plugin drops each one once it has died away.
    pub fn iter(&self) -> impl Iterator<Item = &TreeShake> {
        self.shakes.iter()
    }
}

/// What the flora shaders read from a material: the species' response to the wind and the look
/// of the leaves of the tree it belongs to. The wind itself is not here: it is the same for
/// every tree and lives in [`FloraWindBuffer`].
#[derive(ShaderType, Reflect, Clone, Copy, Debug, PartialEq, Default)]
pub struct FloraUniform {
    /// (1 / trunk height, 1 for a hull crown ([`CrownStyle::Hull`]) and 0 otherwise, 0, 0).
    pub tree: Vec4,
    /// (sway, flutter, sway frequency, flutter frequency) of the species: [`TreeParams::wind`].
    pub response: Vec4,
    /// (translucency, roughness of the upper side, veins, underside) of the leaves.
    pub leaf: Vec4,
    /// Linear rgb that light takes on passing through a leaf; w is 0.
    pub tint: Vec4,
    /// (radius below which wood is smooth, radius above which it shows the whole scan, 0, 0) in
    /// metres: [`BarkParams`](crate::BarkParams). Zero for materials without bark.
    pub bark: Vec4,
    /// The bark scan's average colour, linear rgb, 0: what smooth wood is multiplied by, since
    /// the material's base colour is the preset's over it. Zero for materials without bark.
    pub bark_mean: Vec4,
    /// What a turned leaf's colour is multiplied by (linear rgb: the autumn colour over the leaf
    /// colour, at most 1000 a channel) and 1, for a deciduous tree ([`FoliageParams::autumn`]
    /// is set); zero for an evergreen and for wood.
    ///
    /// [`FoliageParams::autumn`]: crate::FoliageParams::autumn
    pub autumn: Vec4,
    /// What a flower's colour is multiplied by (linear rgb: the blossom colour over the leaf
    /// colour, at most 1000 a channel) and 1, for a flowering tree ([`FoliageParams::blossom`]
    /// is set); zero otherwise and for wood.
    ///
    /// [`FoliageParams::blossom`]: crate::FoliageParams::blossom
    pub blossom: Vec4,
}

/// Most a channel of the leaf colour may be multiplied by to reach the autumn colour: enough for
/// any leaf channel above 0.1% (linear) to reach full brightness (the maple's dark green needs 47
/// in red), while a black leaf colour gives a finite number.
const AUTUMN_RATIO_MAX: f32 = 1000.0;

/// The season, the same for every tree drawn with [`FloraMaterial`] (and impostors): how far
/// deciduous trees ([`FoliageParams::autumn`](crate::FoliageParams::autumn)) have turned and
/// shed their leaves, and how much snow lies on every tree. Evergreens ignore the first two.
///
/// Like [`FloraWind`] it lives in the shared buffer, so changing it every frame (a day/night and
/// season cycle) costs one small buffer write and touches no material or mesh. Leaves turn and
/// fall one by one, each at its own moment, in the order they turned; leaf fall also takes the
/// leaves out of the shadows. For spring, set `autumn` to 0 while the trees are bare (nothing
/// shows it), then bring `leaf_fall` back to 0 and the leaves return green, as [`Self::of_year`]
/// does; flowering trees blossom with `blossom` before that. Snow settles on the surfaces that
/// face up, wood, needles and leaves alike, first on the flattest and with more of it on steeper
/// ones.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FloraSeason {
    /// How far the leaves have turned, 0 (summer green) to 1 (every leaf in its autumn colour).
    pub autumn: f32,
    /// Share of the leaves fallen, 0 (all on the tree) to 1 (bare). Raised gradually, the leaves
    /// it sheds from trees near a camera fall to the ground ([`FallingLeaves`](crate::FallingLeaves)).
    /// A fruiting tree's fruit ([`FoliageParams::fruit`](crate::FoliageParams::fruit)) drops in its
    /// first two thirds.
    pub leaf_fall: f32,
    /// How much snow lies on the trees, 0 (none) to 1 (every surface that faces up is white).
    pub snow: f32,
    /// Share of a flowering species' flowers out ([`FoliageParams::blossom`](crate::FoliageParams::blossom)),
    /// 0 (none) to 1 (in full blossom). They open one by one and fall in the reverse order. At 0
    /// the plugin does not draw a tree's flowers at all; above 0 no tree bears fruit.
    pub blossom: f32,
}

impl FloraSeason {
    /// Full summer: green, every leaf on the tree.
    pub const SUMMER: Self = Self { autumn: 0.0, leaf_fall: 0.0, snow: 0.0, blossom: 0.0 };
    /// Every leaf turned, none fallen.
    pub const AUTUMN: Self = Self { autumn: 1.0, leaf_fall: 0.0, snow: 0.0, blossom: 0.0 };
    /// Bare deciduous trees.
    pub const WINTER: Self = Self { autumn: 1.0, leaf_fall: 1.0, snow: 0.0, blossom: 0.0 };
    /// Bare deciduous trees, and snow on every tree.
    pub const SNOW: Self = Self { autumn: 1.0, leaf_fall: 1.0, snow: 1.0, blossom: 0.0 };
    /// Early spring: flowering trees in full blossom on bare branches, the other deciduous trees
    /// still bare.
    pub const BLOSSOM: Self = Self { autumn: 0.0, leaf_fall: 1.0, snow: 0.0, blossom: 1.0 };

    /// A season from the year's progress `t` (0..1, wraps): green until 0.55, turning to 0.75,
    /// falling to 0.9, snow settling from 0.88 to 0.92 and melting from 0.98 to the year's end,
    /// blossom opening on the bare branches from 0 to 0.015 and falling from 0.03 to 0.07, and
    /// the leaves coming back green from 0.02 to 0.07.
    pub fn of_year(t: f32) -> Self {
        let t = t.rem_euclid(1.0);
        let ramp = |a: f32, b: f32| ((t - a) / (b - a)).clamp(0.0, 1.0);
        if t < 0.07 {
            // Spring: the snow is gone; flowering trees blossom on bare branches, then the
            // leaves come back green as the flowers fall.
            Self {
                autumn: 0.0,
                leaf_fall: 1.0 - ramp(0.02, 0.07),
                snow: 0.0,
                blossom: ramp(0.0, 0.015).min(1.0 - ramp(0.03, 0.07)),
            }
        } else {
            Self {
                autumn: ramp(0.55, 0.75),
                leaf_fall: ramp(0.7, 0.9),
                snow: ramp(0.88, 0.92) * (1.0 - ramp(0.98, 1.0)),
                blossom: 0.0,
            }
        }
    }
}

/// How wet the trees are, 0 (dry, the default) to 1 (soaked): the game's rain, raised while it
/// rains and lowered as the trees dry. Wet bark darkens and turns glossy; a wet leaf keeps its
/// brightness and only its green grows richer; on every way a tree is drawn (pools and impostors
/// included), and under snow, snow wins. Changing it writes one value of the shared buffer, no
/// material, and costs nothing a frame.
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use bevy_flora::FloraWetness;
///
/// /// The game's weather.
/// #[derive(Resource)]
/// struct Raining(bool);
///
/// // A shower: wet in a minute, dry again in ten once it stops.
/// fn rain(raining: Res<Raining>, time: Res<Time>, mut wetness: ResMut<FloraWetness>) {
///     let rate = if raining.0 { 1.0 / 60.0 } else { -1.0 / 600.0 };
///     wetness.0 = (wetness.0 + rate * time.delta_secs()).clamp(0.0, 1.0);
/// }
/// ```
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FloraWetness(pub f32);

impl FloraWetness {
    /// Soaked.
    pub const SOAKED: Self = Self(1.0);

    /// The wetness the shaders use: clamped to 0..1, 0 for a NaN.
    pub fn amount(&self) -> f32 {
        if self.0.is_nan() { 0.0 } else { self.0.clamp(0.0, 1.0) }
    }
}

/// What `from` is multiplied by to become `to`, per linear channel (capped at
/// [`AUTUMN_RATIO_MAX`]), and 1: a turned leaf's colour, a flower's or a fruit's, from the leaf
/// colour, so a scan's own variation stays.
pub(crate) fn colour_ratio(to: bevy::color::Color, from: bevy::color::Color) -> Vec4 {
    let (to, from) = (to.to_linear(), from.to_linear());
    let ratio = |a: f32, b: f32| (a / b.max(1e-4)).clamp(0.0, AUTUMN_RATIO_MAX);
    Vec4::new(ratio(to.red, from.red), ratio(to.green, from.green), ratio(to.blue, from.blue), 1.0)
}

impl FloraUniform {
    pub(crate) fn new(params: &TreeParams) -> Self {
        let s = &params.foliage.shading;
        let r = &params.wind;
        // The veins are drawn on leaf-shaped cards; the other styles' UVs are not a leaf.
        let veined = !matches!(params.foliage.style, LeafStyle::NeedleCluster | LeafStyle::ClusterSphere);
        let tint = s.transmission_tint.to_linear();
        let hull = matches!(params.crown, CrownStyle::Hull { .. });
        Self {
            tree: Vec4::new(1.0 / params.trunk.height.max(0.01), if hull { 1.0 } else { 0.0 }, 0.0, 0.0),
            response: Vec4::new(
                r.sway.max(0.0),
                r.flutter.max(0.0),
                r.sway_frequency.max(0.0),
                r.flutter_frequency.max(0.0),
            ),
            // A hull crown ([`CrownStyle::Hull`]) lets no light through, has no veins and no paler
            // underside: its facets are matte (`tree.y`, which the shaders read as the hull flag).
            leaf: if hull {
                Vec4::new(0.0, s.roughness.clamp(0.05, 1.0), 0.0, 0.0)
            } else {
                Vec4::new(
                    s.translucency.max(0.0),
                    s.roughness.clamp(0.05, 1.0),
                    if veined { s.veins.clamp(0.0, 1.0) } else { 0.0 },
                    s.underside.clamp(0.0, 1.0),
                )
            },
            tint: Vec4::new(tint.red, tint.green, tint.blue, 0.0),
            bark: Vec4::ZERO,
            bark_mean: Vec4::ZERO,
            autumn: params.foliage.autumn.map_or(Vec4::ZERO, |autumn| colour_ratio(autumn, params.foliage.color)),
            blossom: params.foliage.blossom.map_or(Vec4::ZERO, |blossom| colour_ratio(blossom, params.foliage.color)),
        }
    }

    /// The same uniform for a tree's wood, which has no leaf look: the leaf fields are zeroed,
    /// so changing how leaves are shaded does not touch the wood's material. The wood bends like
    /// the species does.
    pub(crate) fn wood(self) -> Self {
        Self {
            leaf: Vec4::ZERO,
            tint: Vec4::ZERO,
            autumn: Vec4::ZERO,
            blossom: Vec4::ZERO,
            tree: self.tree.with_y(0.0),
            ..self
        }
    }
}

/// How many `Vec4`s the shared buffer holds before the fire's grid: see [`wind_vectors`], and after
/// them the see-through fade's (`see_through::SEE_THROUGH_SLOT`) and the pushers'
/// (`push::PUSH_SLOT`).
pub(crate) const WIND_VECTORS: usize = 7 + 2 * TreeShakes::MAX + SEE_THROUGH_VECTORS + PUSH_VECTORS;

/// The shared buffer's slot whose third lane holds [`FloraWetness`] (`sync_wind` writes it).
pub(crate) const WETNESS_SLOT: usize = 2;

/// The shared buffer's slot (the same one) whose fourth lane holds [`FloraUnits::units_per_metre`]
/// (`wind_vectors` packs it). `common.wesl`'s `UNITS` is this number.
pub(crate) const UNITS_SLOT: usize = 2;

/// The `Vec4`s of [`FloraWind`], [`FloraSeason`] and [`TreeShakes`] the shaders read, in the order
/// of the shared buffer: `wind` (direction x, direction z, strength, speed), `motion` (bend,
/// flutter, gust strength, turbulence), `rate` (gust frequency, flutter frequency, the
/// [`FloraWetness`] `sync_wind` puts there (0 here), `units`, the world units to a metre), `front`
/// (origin x, origin z, direction x, direction z), `front_shape` (speed, width, strength, start
/// time), `season` (autumn, leaf fall, snow, blossom), the shakes' header (the render time the last
/// of them ends, -1 without any, 0, 0) and two per shake slot, (at x, at z, radius, strength) and
/// (direction x, direction z, start, duration), all zero for an empty slot. `front` and
/// `front_shape` are zero without a [`GustFront`], or with one that has no direction. Directions
/// are normalised and every value clamped to what the shaders can use. The metres the weather is
/// given in (a gust's speed and width, a shake's radius) are packed in world units: times `units`.
pub(crate) fn wind_vectors(wind: &FloraWind, season: &FloraSeason, shakes: &TreeShakes, units: f32) -> [Vec4; WIND_VECTORS] {
    let d = wind.direction.normalize_or_zero();
    let (front, front_shape) = match wind.gust {
        Some(gust) if gust.direction.normalize_or_zero() != Vec2::ZERO => {
            let d = gust.direction.normalize_or_zero();
            (
                Vec4::new(gust.origin.x, gust.origin.y, d.x, d.y),
                Vec4::new(gust.speed.max(0.0) * units, gust.width.max(0.01) * units, gust.strength.max(0.0), gust.start),
            )
        }
        _ => (Vec4::ZERO, Vec4::ZERO),
    };
    let mut packed = [Vec4::ZERO; WIND_VECTORS];
    packed[..6].copy_from_slice(&[
        Vec4::new(d.x, d.y, wind.strength.max(0.0), wind.speed.max(0.0)),
        Vec4::new(wind.bend.max(0.0), wind.flutter.max(0.0), wind.gust_strength.clamp(0.0, 1.0), wind.turbulence.max(0.0)),
        Vec4::new(wind.gust_frequency.max(0.0), wind.flutter_frequency.max(0.0), 0.0, 0.0),
        front,
        front_shape,
        Vec4::new(
            season.autumn.clamp(0.0, 1.0),
            season.leaf_fall.clamp(0.0, 1.0),
            season.snow.clamp(0.0, 1.0),
            season.blossom.clamp(0.0, 1.0),
        ),
    ]);
    // Shakes that cannot move anything are left out, and the header says when the last one ends,
    // so the shaders skip them all, every vertex, once it has.
    let mut last_end = -1.0f32;
    let live = shakes.iter().filter(|s| s.strength > 0.0).take(TreeShakes::MAX);
    for (i, shake) in live.enumerate() {
        let duration = shake.duration.max(0.01);
        let dir = shake.direction.normalize_or_zero();
        packed[7 + 2 * i] = Vec4::new(shake.at.x, shake.at.y, shake.radius.max(0.01) * units, shake.strength);
        packed[8 + 2 * i] = Vec4::new(dir.x, dir.y, shake.start, duration);
        last_end = last_end.max(shake.start + duration);
    }
    packed[6] = Vec4::new(last_end, 0.0, 0.0, 0.0);
    packed[UNITS_SLOT].w = units;
    packed
}

/// What a [`FloraExtension`] changes about the pipeline it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloraKey {
    /// Leaves: see [`FloraExtension::leaf`].
    pub leaf: bool,
    /// Detail textures (bark): see [`FloraExtension::detail_color`]. True when both maps are set.
    pub detail: bool,
    /// Leaf veins from an image: see [`FloraExtension::venation`].
    pub venation: bool,
    /// The look: see [`FloraExtension::style`].
    pub style: RenderStyle,
    /// The see-through fade: see [`FloraExtension::see_through`].
    pub see_through: bool,
    /// Falling leaves: see [`FloraExtension::falling`].
    pub falling: bool,
    /// The fire: see [`FloraExtension::fire`].
    pub fire: bool,
    /// The foliage pushers: see [`FloraExtension::push`].
    pub push: bool,
}

/// The part of [`FloraMaterial`] that bends and shades.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
#[bind_group_data(FloraKey)]
pub struct FloraExtension {
    /// The look of the tree's leaves and its response to the wind.
    // Binding slots below 100 belong to the base material; 100 is GPU growth's node buffer,
    // which shares this extension's fragment shader.
    #[uniform(101)]
    pub flora: FloraUniform,
    /// The weather, shared by every flora material: [`FloraWindBuffer`].
    #[storage(102, read_only)]
    pub wind: Handle<ShaderBuffer>,
    /// Leaves get the leaf surface, the light through the leaf and the flutter (their mesh must
    /// carry the attributes [`foliage_mesh`](crate::foliage::foliage_mesh) bakes); wood only bends.
    pub leaf: bool,
    /// The colour map of the surface's detail (bark), multiplied into the base colour, or `None`
    /// for none: see [`BarkParams`](crate::BarkParams). Bound with the normal map and
    /// [`FloraUniform::bark`].
    #[texture(103)]
    #[sampler(105)]
    pub detail_color: Option<Handle<Image>>,
    /// The normal map of the surface's detail (tangent space, OpenGL convention).
    #[texture(104)]
    pub detail_normal: Option<Handle<Image>>,
    /// The image of leaf veins (`R8`, the card's UVs address it): the veins of a leaf without a
    /// scan, drawn by the leaf shader, or `None` for none. See
    /// [`FloraTextures::venation`].
    #[texture(106)]
    #[sampler(107)]
    pub venation: Option<Handle<Image>>,
    /// How the surface is lit: [`RenderStyle::Toon`] replaces the physically based lighting of
    /// the fragment stage with bands and an ink rim; the other styles are lit as ever (the
    /// painterly look is in the normals of the mesh).
    pub style: RenderStyle,
    /// Whether the see-through fade ([`SeeThroughTrees`](crate::SeeThroughTrees)) cuts this
    /// material. The plugin sets it on every flora material while a camera has one and clears it
    /// when none has, so that otherwise no pipeline carries the fade's discard.
    pub see_through: bool,
    /// Falling leaves: the mesh is a burst of leaf cards that carries the fall's attributes, and
    /// the vertex stages move them instead of bending a tree (see [`falling`](crate::falling)). The
    /// plugin makes such a copy of a tree's leaf material for the leaves that fall from it.
    pub falling: bool,
    /// Whether the fire ([`FloraFire`](crate::FloraFire)) burns this material's trees. The plugin
    /// sets it on every flora material while a `FloraFire` exists and clears it when none does,
    /// so that otherwise no pipeline carries the fire.
    pub fire: bool,
    /// Whether foliage pushers ([`FoliagePusher`](crate::FoliagePusher)) bend this material's
    /// leaves and thin stems aside. The plugin sets it on every flora material while an entity has
    /// one and clears it when none has, so that otherwise no pipeline carries the push.
    pub push: bool,
}

impl From<&FloraExtension> for FloraKey {
    fn from(extension: &FloraExtension) -> Self {
        Self {
            leaf: extension.leaf,
            detail: extension.detail_color.is_some() && extension.detail_normal.is_some(),
            venation: extension.venation.is_some(),
            style: extension.style,
            see_through: extension.see_through,
            falling: extension.falling,
            fire: extension.fire,
            push: extension.push,
        }
    }
}

/// [`StandardMaterial`] with leaf shading and wind. The wood and leaves of trees use it.
pub type FloraMaterial = ExtendedMaterial<StandardMaterial, FloraExtension>;

impl MaterialExtension for FloraExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/vertex.wesl".into()
    }

    // Shadow maps run through the prepass pipeline, so this is what makes shadows move too.
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/prepass.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/fragment.wesl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        specialize_flora(descriptor, layout, key.bind_group_data, key.mesh_key)
    }
}

/// Make the pipeline of a leaf material read the leaf mesh's flutter attribute and turn on the
/// leaf branches of the shaders, and that of a material with detail textures sample them. Plain
/// wood needs nothing. Detail needs the mesh's two texture coordinate sets (bark is laid out in
/// the first, fades in with the second); a mesh without them is drawn without it.
pub(crate) fn specialize_flora(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    key: FloraKey,
    mesh_key: MeshPipelineKey,
) -> Result<(), SpecializedMeshPipelineError> {
    // The foliage pushers (`FoliagePusher`): leaves and thin stems bend out of a body's way, in the
    // vertex stages (the main pass and the prepasses alike, so shadows follow).
    if key.push {
        descriptor.vertex.shader_defs.push("FLORA_PUSH".into());
    }
    // The fire (`FloraFire`): leaves scorch and burn away and wood chars in the vertex stages, in
    // the main pass and the prepasses alike (so shadows lose the burnt leaves), and the fragment
    // stage blackens and lights the embers.
    if key.fire {
        descriptor.vertex.shader_defs.push("FLORA_FIRE".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("FLORA_FIRE".into());
        }
    }
    if key.see_through {
        add_see_through(descriptor, mesh_key);
    }
    if key.venation
        && layout.0.contains(Mesh::ATTRIBUTE_UV_0.id)
        && let Some(fragment) = descriptor.fragment.as_mut()
    {
        fragment.shader_defs.push("FLORA_VENATION".into());
    }
    if key.detail
        && layout.0.contains(Mesh::ATTRIBUTE_UV_0.id)
        && layout.0.contains(Mesh::ATTRIBUTE_UV_1.id)
        && let Some(fragment) = descriptor.fragment.as_mut()
    {
        fragment.shader_defs.push("FLORA_DETAIL".into());
    }
    // Toon shading replaces the lighting of the whole surface, wood and leaves alike. The painterly
    // style differs from the natural one in the mesh (its normals), not in the shaders.
    if key.style == RenderStyle::Toon
        && let Some(fragment) = descriptor.fragment.as_mut()
    {
        fragment.shader_defs.push("FLORA_TOON".into());
    }
    if !key.leaf {
        return Ok(());
    }
    descriptor.vertex.shader_defs.push("FLORA_LEAF".into());
    if let Some(fragment) = descriptor.fragment.as_mut() {
        fragment.shader_defs.push("FLORA_LEAF".into());
    }
    // The base material chose its attributes and their locations (0-7); growth's are 8-10.
    let sway = layout.0.get_layout(&[ATTRIBUTE_LEAF_SWAY.at_shader_location(11)])?;
    descriptor.vertex.buffers[0].attributes.extend(sway.attributes);
    if key.falling {
        crate::falling::specialize_falling(descriptor, layout)?;
    }
    Ok(())
}

/// The see-through fade's part of a pipeline ([`SeeThroughTrees`](crate::SeeThroughTrees)): in
/// the main pass, the fragment stage's discard; in a camera's prepasses, a fragment stage that
/// discards the same fragments (`see_through_prepass.wesl`: Bevy's prepass fragment with the
/// discard first, made where a depth-only prepass has none), so a depth prepass keeps no depth
/// where the main pass cut the tree. A directional light's shadow cascades are left as they are;
/// a point or spot light's shadow pipeline cannot be told from a depth-only camera prepass here,
/// so the shaders cut only in the view drawn from the fading camera's position.
fn add_see_through(descriptor: &mut RenderPipelineDescriptor, mesh_key: MeshPipelineKey) {
    let prepass = descriptor.vertex.shader_defs.contains(&ShaderDefVal::from("PREPASS_PIPELINE"));
    if !prepass {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("FLORA_SEE_THROUGH".into());
        }
        return;
    }
    if mesh_key.intersects(MeshPipelineKey::UNCLIPPED_DEPTH_ORTHO | MeshPipelineKey::DEFERRED_PREPASS) {
        return;
    }
    let Some(shader) = crate::see_through::prepass_fragment() else { return };
    match descriptor.fragment.as_mut() {
        Some(fragment) => {
            fragment.shader = shader;
            fragment.shader_defs.push("FLORA_SEE_THROUGH".into());
        }
        None => {
            let mut shader_defs = descriptor.vertex.shader_defs.clone();
            shader_defs.push("FLORA_SEE_THROUGH".into());
            descriptor.fragment = Some(FragmentState {
                shader,
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: Vec::new(),
                ..Default::default()
            });
        }
    }
}

/// The base of a flora material: the leaf shader lights the surface itself, which only the
/// forward pass can do.
pub(crate) fn flora_base(color: Color, unlit: bool) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 1.0,
        cull_mode: Some(Face::Back),
        opaque_render_method: OpaqueRendererMethod::Forward,
        unlit,
        ..Default::default()
    }
}

/// What a flora material is made of, besides the wind: its base colour, the uniform the shaders
/// read, whether it is for leaves, and the textures it wears (bark, a leaf scan's atlas, or the
/// image of leaf veins). Two trees with equal surfaces share a material ([`SharedFlora`]).
#[derive(Clone, Debug)]
pub(crate) struct Surface {
    /// The material's base colour: for scanned wood or leaves the preset colour over the
    /// scan's average.
    pub color: Color,
    /// What the flora shaders read.
    pub uniform: FloraUniform,
    /// Leaves rather than wood.
    pub leaf: bool,
    /// The bark scan wood wears, with its images.
    pub bark: Option<BarkTexture>,
    /// The leaf scan leaves wear, with its atlas.
    pub leaf_scan: Option<LeafTexture>,
    /// The image of leaf veins a leaf without a scan wears.
    pub venation: Option<Handle<Image>>,
    /// How the surface is lit: see [`RenderStyle`].
    pub style: RenderStyle,
    /// Drawn as its colour, with no lighting: [`PoolColors::unlit`](crate::PoolColors::unlit).
    pub unlit: bool,
}

impl Surface {
    /// The wood of a tree with `params`, wearing `bark` when it is given.
    pub(crate) fn wood(params: &TreeParams, bark: Option<BarkTexture>) -> Self {
        let mut uniform = FloraUniform::new(params).wood();
        if let Some(bark) = &bark {
            uniform.bark = Vec4::new(params.bark.smooth_below, params.bark.fissured_above, 0.0, 0.0);
            uniform.bark_mean = Vec4::new(bark.mean.red, bark.mean.green, bark.mean.blue, 0.0);
        }
        let color = bark.as_ref().map_or(params.trunk_color, |bark| bark.base_color(params.trunk_color));
        Self { color, uniform, leaf: false, style: params.style, bark, leaf_scan: None, venation: None, unlit: false }
    }

    /// The leaves of a tree with `params`, wearing `leaf_scan`'s atlas, or `venation`'s veins.
    /// Cards cut to a scan's outline have the real leaf's veins in the atlas, so the uniform
    /// draws none of its own on them, whether or not the atlas could be loaded.
    pub(crate) fn leaves(
        params: &TreeParams,
        leaf_scan: Option<LeafTexture>,
        venation: Option<Handle<Image>>,
    ) -> Self {
        let mut uniform = FloraUniform::new(params);
        if uses_scan(&params.foliage) {
            uniform.leaf.z = 0.0;
        }
        let color = leaf_scan.as_ref().map_or(params.foliage.color, |scan| scan.base_color(params.foliage.color));
        // Flowers wear no scan: their colour is the material's base colour times this ratio.
        if let Some(blossom) = params.foliage.blossom {
            uniform.blossom = colour_ratio(blossom, color);
        }
        Self { color, uniform, leaf: true, style: params.style, bark: None, leaf_scan, venation, unlit: false }
    }

    /// The same surface, lit as ever or drawn as its colour alone.
    pub(crate) fn with_unlit(mut self, unlit: bool) -> Self {
        self.unlit = unlit;
        self
    }

    /// The colour and normal maps the material binds, if it has them.
    pub(crate) fn detail(&self) -> (Option<Handle<Image>>, Option<Handle<Image>>) {
        if let Some(bark) = &self.bark {
            (Some(bark.color.clone()), Some(bark.normal.clone()))
        } else if let Some(scan) = &self.leaf_scan {
            (Some(scan.color.clone()), Some(scan.normal.clone()))
        } else {
            (None, None)
        }
    }
}

pub(crate) fn flora_material(surface: &Surface, wind: &FloraWindBuffer) -> FloraMaterial {
    let (detail_color, detail_normal) = surface.detail();
    FloraMaterial {
        base: flora_base(surface.color, surface.unlit),
        extension: FloraExtension {
            flora: surface.uniform,
            wind: wind.0.clone(),
            leaf: surface.leaf,
            detail_color,
            detail_normal,
            venation: surface.venation.clone(),
            style: surface.style,
            see_through: false,
            falling: false,
            fire: false,
            push: false,
        },
    }
}

/// Whether [`FloraMaterial`] can be drawn: the plugin was added after the renderer's plugins
/// (which registers the material) and the device has storage buffers in the vertex stage (the
/// wind lives in one; everything but WebGL2 does). Decided at runtime, once the renderer
/// exists; until then no tree is built. Without it, trees are drawn with plain materials and
/// pools without impostors. Insert it yourself only where there is no renderer (a headless
/// test) and the asset type was registered by hand.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloraMaterialSupport;

/// The wind the flora shaders read: one storage buffer of `Vec4`s (direction and strength,
/// motion, rates and the [`GustFront`], packed from [`FloraWind`], the [`FloraSeason`] and the
/// [`TreeShakes`]), shared by every flora material (binding 102), so changing any of them writes
/// this buffer in place and touches no material. Hand it to the `wind` field of a
/// [`FloraExtension`], [`GrowthExtension`](crate::GrowthExtension) or
/// [`ImpostorMaterial`](crate::ImpostorMaterial) you build yourself.
#[derive(Resource, Clone, Debug)]
pub struct FloraWindBuffer(pub Handle<ShaderBuffer>);

impl FloraWindBuffer {
    fn new(
        buffers: &mut Assets<ShaderBuffer>,
        wind: &FloraWind,
        season: &FloraSeason,
        shakes: &TreeShakes,
        wetness: &FloraWetness,
        units: &FloraUnits,
    ) -> Self {
        let mut packed = wind_vectors(wind, season, shakes, units.units_per_metre).map(|v| v.to_array());
        packed[WETNESS_SLOT][2] = wetness.amount();
        Self(buffers.add(ShaderBuffer::from(shared_vectors(&packed))))
    }
}

/// The renderer's plugins came before this one, so [`FloraMaterial`] is registered, but whether
/// the device can draw it is not known until it exists.
#[derive(Resource)]
pub(crate) struct FloraSupportPending;

/// Run condition: whether it is known how trees are drawn.
pub(crate) fn flora_decided(pending: Option<Res<FloraSupportPending>>) -> bool {
    pending.is_none()
}

/// Once the renderer's device exists, decide whether flora materials can be drawn on it; if
/// not, trees are drawn with plain materials and pools without impostors.
pub(crate) fn detect_flora_support(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    pending: Option<Res<FloraSupportPending>>,
) {
    let (Some(device), Some(_)) = (device, pending) else { return };
    commands.remove_resource::<FloraSupportPending>();
    if bevy::render::storage_buffers_are_unsupported(&device.limits()) {
        warn!(
            "bevy_flora: this device has no storage buffers in the vertex stage (WebGL2?), so trees \
             are drawn without leaf shading and wind, and pools without impostors"
        );
        commands.remove_resource::<ImpostorSupport>();
    } else {
        commands.insert_resource(FloraMaterialSupport);
    }
}

/// Write [`FloraWind`], [`FloraSeason`], [`FloraWetness`], [`TreeShakes`] and [`FloraUnits`] into
/// the shared buffer, in place. Runs when any of them changed. Only their slots: the fire's grid
/// after them ([`FloraFire`](crate::FloraFire)) is `fire::sync_fire`'s.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_wind(
    wind: Res<FloraWind>,
    season: Res<FloraSeason>,
    wetness: Res<FloraWetness>,
    shakes: Res<TreeShakes>,
    units: Res<FloraUnits>,
    capsule: Res<SeeThroughCapsule>,
    push: Res<PushSlots>,
    buffer: Res<FloraWindBuffer>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let mut packed = wind_vectors(&wind, &season, &shakes, units.units_per_metre).map(|v| v.to_array());
    packed[WETNESS_SLOT][2] = wetness.amount();
    // The see-through fade's slots are written by `sync_see_through`; keep them.
    for (i, vector) in capsule.0.iter().enumerate() {
        packed[SEE_THROUGH_SLOT + i] = vector.to_array();
    }
    // So are the pushers', by `push::sync_pushers`.
    packed[PUSH_SLOT..PUSH_SLOT + PUSH_VECTORS].copy_from_slice(&push.0);
    // Look before borrowing mutably: a mutable borrow marks the buffer changed.
    let current = buffers.get(&buffer.0).and_then(|buffer| buffer.cast_slice::<[f32; 4]>());
    if current.and_then(|slice| slice.get(..WIND_VECTORS)) == Some(&packed[..]) {
        return;
    }
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    match shader_buffer.cast_slice_mut::<[f32; 4]>() {
        Some(slice) if slice.len() > WIND_VECTORS => slice[..WIND_VECTORS].copy_from_slice(&packed),
        _ => *shader_buffer = ShaderBuffer::from(shared_vectors(&packed)),
    }
}

/// The shared buffer's contents for the weather `packed`: it, then the fire's header saying there
/// is no fire (`fire::sync_fire` grows the buffer by the grid when there is one).
fn shared_vectors(packed: &[[f32; 4]; WIND_VECTORS]) -> Vec<[f32; 4]> {
    let mut all = packed.to_vec();
    all.push([0.0; 4]);
    all
}

/// Drop the shakes that have died away, and those the render clock's wrap left in the future, so
/// the shaders skip the shakes altogether once none is under way. Touches the resource (and so
/// rewrites the buffer) only in a frame when one goes.
pub(crate) fn expire_shakes(mut shakes: ResMut<TreeShakes>, time: Res<Time>) {
    let now = time.elapsed_secs_wrapped();
    let under_way = |shake: &TreeShake| shake.start <= now && now <= shake.start + shake.duration.max(0.01);
    // Look before borrowing mutably: a mutable borrow marks the resource changed.
    if !shakes.shakes.iter().all(under_way) {
        shakes.shakes.retain(under_way);
    }
}

/// Register the wind, and, when the renderer's plugins have already been added, the material
/// and its shaders.
///
/// Plugins may be added in any order. Added ahead of the renderer's (or of the asset plugin),
/// this registers nothing that needs them; [`finish`] covers the asset types, and trees are drawn
/// with plain materials.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<FloraWind>()
        .init_resource::<FloraSeason>()
        .init_resource::<FloraWetness>()
        .init_resource::<TreeShakes>()
        .init_resource::<SeeThroughCapsule>()
        .init_resource::<SharedFlora>()
        .init_resource::<FloraTextures>()
        .register_type::<FloraWind>()
        .register_type::<FloraSeason>()
        .register_type::<FloraWetness>()
        .register_type::<TreeShakes>()
        .register_type::<GustFront>();
    if app.get_sub_app(RenderApp).is_some() && app.is_plugin_added::<PbrPlugin>() {
        add_shaders(app);
        app.add_plugins(MaterialPlugin::<FloraMaterial>::default()).insert_resource(FloraSupportPending);
    } else {
        add_asset_types(app);
    }
    add_wind_buffer(app);
}

/// Once every plugin is built, make sure the asset types the systems take exist even if the
/// plugin was added too early to register them.
pub(crate) fn finish(app: &mut App) {
    add_asset_types(app);
    add_wind_buffer(app);
    if app.get_sub_app(RenderApp).is_some() && !app.world().contains_resource::<FloraSupportPending>() {
        warn!(
            "bevy_flora: FloraPlugin was added before the renderer's plugins, so trees are drawn \
             without leaf shading and wind; add it after DefaultPlugins"
        );
    }
}

/// Register [`FloraMaterial`] and [`ShaderBuffer`] as assets where the asset plugin is present
/// and nobody has yet (the renderer's plugins do it when they are).
fn add_asset_types(app: &mut App) {
    if app.world().contains_resource::<AssetServer>() {
        if !app.world().contains_resource::<Assets<FloraMaterial>>() {
            app.init_asset::<FloraMaterial>();
        }
        if !app.world().contains_resource::<Assets<ShaderBuffer>>() {
            app.init_asset::<ShaderBuffer>();
        }
    }
}

/// Make the shared wind buffer, once there are buffer assets to put it in.
fn add_wind_buffer(app: &mut App) {
    if app.world().contains_resource::<FloraWindBuffer>() || !app.world().contains_resource::<Assets<ShaderBuffer>>() {
        return;
    }
    let wind = *app.world().resource::<FloraWind>();
    let season = *app.world().resource::<FloraSeason>();
    let shakes = app.world().resource::<TreeShakes>().clone();
    let wetness = *app.world().resource::<FloraWetness>();
    let units = *app.world().resource::<FloraUnits>();
    let buffer = FloraWindBuffer::new(
        &mut app.world_mut().resource_mut::<Assets<ShaderBuffer>>(),
        &wind,
        &season,
        &shakes,
        &wetness,
        &units,
    );
    app.insert_resource(buffer);
}

/// Embed the shaders.
fn add_shaders(app: &mut App) {
    bevy::shader::load_shader_library!(app, "common.wesl");
    embedded_asset!(app, "vertex.wesl");
    embedded_asset!(app, "prepass.wesl");
    embedded_asset!(app, "fragment.wesl");
    // Not named by any material: `specialize_flora` puts it in a camera's prepasses while a
    // camera sees through the trees.
    embedded_asset!(app, "see_through_prepass.wesl");
    crate::see_through::set_prepass_fragment(load_embedded_asset!(app, "see_through_prepass.wesl"));
    // Falling leaves (`falling.rs`): vertex stages `specialize_flora` puts in a falling material's
    // pipelines.
    bevy::shader::load_shader_library!(app, "falling_common.wesl");
    embedded_asset!(app, "falling.wesl");
    embedded_asset!(app, "falling_prepass.wesl");
    crate::falling::set_shaders(
        load_embedded_asset!(app, "falling.wesl"),
        load_embedded_asset!(app, "falling_prepass.wesl"),
    );
}

#[cfg(test)]
mod tests {
    /// `common.wesl` spells the buffer's slots as literals: they must be the Rust constants.
    #[test]
    fn the_shaders_slots_are_the_buffers_slots() {
        let source = include_str!("common.wesl");
        let literal = |name: &str| -> usize {
            let at = source.find(&format!("const {name}: u32 = ")).unwrap_or_else(|| panic!("no {name}"));
            let rest = &source[at + format!("const {name}: u32 = ").len()..];
            rest[..rest.find("u;").expect("a u32 literal")].parse().expect("a number")
        };
        assert_eq!(literal("UNITS"), UNITS_SLOT);
        assert_eq!(literal("SEE_THROUGH"), SEE_THROUGH_SLOT);
        assert_eq!(literal("PUSH"), PUSH_SLOT);
        assert_eq!(literal("FIRE"), crate::fire::FIRE_SLOT);
    }

    use std::time::Duration;

    use bevy::math::Vec3;

    use super::*;
    use crate::presets::TreePreset;

    #[test]
    fn veins_are_drawn_only_on_leaf_shaped_cards() {
        let veins = |style| {
            let mut params = TreePreset::Oak.params();
            params.foliage.style = style;
            params.foliage.shading.veins = 0.7;
            FloraUniform::new(&params).leaf.z
        };
        assert_eq!(veins(LeafStyle::CrossedPlanes), 0.7);
        assert_eq!(veins(LeafStyle::SingleQuad), 0.7);
        assert_eq!(veins(LeafStyle::NeedleCluster), 0.0);
        assert_eq!(veins(LeafStyle::ClusterSphere), 0.0);
    }

    #[test]
    fn the_uniform_holds_the_tree_and_nothing_of_the_weather() {
        let uniform = FloraUniform::new(&TreePreset::Willow.params());
        assert_eq!(uniform.response, Vec4::new(1.4, 1.6, 0.7, 0.8));
        let wood = uniform.wood();
        assert_eq!((wood.tree, wood.response), (uniform.tree, uniform.response));
        assert_eq!((wood.leaf, wood.tint), (Vec4::ZERO, Vec4::ZERO));
    }

    #[test]
    fn a_hull_crown_is_flagged_in_the_uniform_lets_no_light_through_and_wood_is_never_flagged() {
        let leaves = FloraUniform::new(&TreePreset::Spruce.params());
        assert_eq!(leaves.tree.y, 0.0, "leaf cards are not a hull");
        assert!(leaves.leaf.x > 0.0, "a leaf is translucent");
        let hull = FloraUniform::new(&crate::crown_hull::hull_spruce());
        assert_eq!(hull.tree, Vec4::new(leaves.tree.x, 1.0, 0.0, 0.0), "the hull flag, and the same 1 / height");
        assert_eq!((hull.leaf.x, hull.leaf.z, hull.leaf.w), (0.0, 0.0, 0.0), "no light through, no veins, no underside");
        assert_eq!(hull.leaf.y, leaves.leaf.y, "the roughness is the species'");
        assert_ne!(hull, leaves, "a hull's material is not a leaf card's");
        // The wood of a hull tree is wood: it keeps its 1 / height for the wind and is no hull.
        assert_eq!(hull.wood().tree, leaves.tree);
    }

    #[test]
    fn a_turned_leaf_takes_the_autumn_colour_and_evergreens_and_wood_never_turn() {
        let maple = TreePreset::Maple.params();
        let uniform = FloraUniform::new(&maple);
        assert_eq!(uniform.autumn.w, 1.0, "a deciduous preset turns");
        // The shader multiplies the leaf colour by the ratio: that must give the autumn colour.
        let (leaf, autumn) = (maple.foliage.color.to_linear(), maple.foliage.autumn.unwrap().to_linear());
        let turned = Vec3::new(leaf.red, leaf.green, leaf.blue) * uniform.autumn.truncate();
        assert!(turned.abs_diff_eq(Vec3::new(autumn.red, autumn.green, autumn.blue), 1e-5), "{turned}");
        assert_eq!(uniform.wood().autumn, Vec4::ZERO, "wood never turns");
        assert_eq!(FloraUniform::new(&TreePreset::Pine.params()).autumn, Vec4::ZERO, "a pine stays green");
        // A black summer leaf cannot be multiplied into a colour: the ratio is capped, not infinite.
        let mut black = maple.clone();
        black.foliage.color = Color::BLACK;
        assert!(FloraUniform::new(&black).autumn.truncate().cmple(Vec3::splat(AUTUMN_RATIO_MAX)).all());
    }

    #[test]
    fn over_a_year_leaves_turn_before_they_fall_snow_lies_on_bare_trees_and_leaves_come_back_green() {
        assert_eq!(FloraSeason::of_year(0.3), FloraSeason::SUMMER);
        assert_eq!(FloraSeason::of_year(0.95), FloraSeason::SNOW);
        assert_eq!(FloraSeason::of_year(1.3), FloraSeason::of_year(0.3), "the year wraps");
        assert_eq!(FloraSeason::of_year(-0.05), FloraSeason::of_year(0.95), "the year wraps");
        let steps = 2000;
        let mut last = FloraSeason::of_year(0.0);
        let mut snowed = false;
        let mut blossomed = 0.0f32;
        for i in 1..=steps {
            let now = FloraSeason::of_year(i as f32 / steps as f32);
            // The colour only goes back to green while the trees are bare, so no turned leaf
            // turns green on the tree; leaves only come back once the colour is green, so none
            // sprouts in its autumn colour; and a leaf only falls once most leaves have turned.
            if now.autumn < last.autumn {
                assert_eq!(last.leaf_fall, 1.0, "colour reset on a tree in leaf at step {i}");
            }
            if now.leaf_fall < last.leaf_fall {
                assert_eq!(now.autumn, 0.0, "leaves came back turned at step {i}");
                assert_eq!(now.snow, 0.0, "leaves came back under snow at step {i}");
            }
            if now.leaf_fall > 0.0 && now.leaf_fall > last.leaf_fall {
                assert!(now.autumn >= 0.75, "leaves fell green at step {i}: {now:?}");
            }
            // Snow lies only on trees that have (nearly) shed their leaves, and comes and goes
            // smoothly, with the year's wrap in the snowless spring.
            if now.snow > 0.0 {
                assert!(now.leaf_fall >= 0.9, "snow on a tree in leaf at step {i}: {now:?}");
                snowed = true;
            }
            assert!((now.snow - last.snow).abs() < 0.05, "snow jumped at step {i}");
            // Flowering trees blossom on bare branches once the snow has gone, and the flowers
            // open and fall over many steps, not at once.
            if now.blossom > 0.0 {
                assert!(now.snow == 0.0 && now.autumn == 0.0, "blossom in snow or autumn at step {i}: {now:?}");
                blossomed = blossomed.max(now.blossom);
            }
            if now.blossom == 1.0 {
                assert!(now.leaf_fall > 0.5, "full blossom on a tree in leaf at step {i}: {now:?}");
            }
            assert!((now.blossom - last.blossom).abs() < 0.05, "blossom jumped at step {i}");
            last = now;
        }
        assert!(snowed, "a year without snow");
        assert_eq!(blossomed, 1.0, "a year without full blossom");
    }

    #[test]
    fn the_wind_is_packed_normalised_and_clamped() {
        let gale = FloraWind {
            direction: Vec2::new(0.0, -5.0),
            strength: 7.0,
            speed: -1.0,
            flutter: 2.0,
            bend: 3.0,
            gust_strength: 2.0,
            gust_frequency: 0.5,
            flutter_frequency: 4.0,
            turbulence: -1.0,
            gust: None,
        };
        let [wind, motion, rate, front, front_shape, season, ..] =
            wind_vectors(&gale, &FloraSeason { autumn: 2.0, leaf_fall: -1.0, snow: 3.0, blossom: 4.0 }, &TreeShakes::default(), 1.0);
        // Direction normalised, a negative speed stopped, strength not capped, the rest clamped.
        assert_eq!(wind, Vec4::new(0.0, -1.0, 7.0, 0.0));
        assert_eq!(motion, Vec4::new(3.0, 2.0, 1.0, 0.0));
        assert_eq!(rate, Vec4::new(0.5, 4.0, 0.0, 1.0), "the units are the fourth lane");
        assert_eq!((front, front_shape), (Vec4::ZERO, Vec4::ZERO));
        assert_eq!(season, Vec4::new(1.0, 0.0, 1.0, 1.0), "the season is clamped to 0..1");
        let (calm, still) = (FloraSeason::default(), TreeShakes::default());
        assert_ne!(
            wind_vectors(&FloraWind { strength: 0.0, ..gale }, &calm, &still, 1.0),
            wind_vectors(&gale, &calm, &still, 1.0)
        );
    }

    #[test]
    fn gust_front_is_packed_normalised_and_off_when_absent() {
        let pack = |gust: Option<GustFront>| {
            let [_, _, _, front, front_shape, ..] =
                wind_vectors(&FloraWind { gust, ..Default::default() }, &FloraSeason::default(), &TreeShakes::default(), 1.0);
            (front, front_shape)
        };

        assert_eq!(pack(None), (Vec4::ZERO, Vec4::ZERO));

        let front = GustFront {
            origin: Vec2::new(4.0, -2.0),
            direction: Vec2::new(0.0, 3.0),
            speed: 12.0,
            width: 0.0,
            strength: 2.0,
            start: 7.5,
        };
        assert_eq!(pack(Some(front)), (Vec4::new(4.0, -2.0, 0.0, 1.0), Vec4::new(12.0, 0.01, 2.0, 7.5)));

        // No direction, no front.
        assert_eq!(pack(Some(GustFront { direction: Vec2::ZERO, ..front })), (Vec4::ZERO, Vec4::ZERO));
    }

    /// The surfaces of `params`'s wood and leaves, resolved through the library in a bare world
    /// that has image assets.
    fn surfaces(params: &TreeParams, images: bool) -> (Surface, Surface) {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        world.init_resource::<FloraTextures>();
        if images {
            world.init_resource::<Assets<Image>>();
        }
        let mut state = SystemState::<TextureLibrary>::new(&mut world);
        let mut library = state.get_mut(&mut world).expect("the library's resources exist");
        (library.wood(params), library.leaves(params))
    }

    #[test]
    fn leaves_wear_a_scan_or_the_vein_image_never_both_and_veins_only_where_a_card_is_a_leaf() {
        // The oak's cards are cut to a scanned leaf: its real veins are in the atlas, so none
        // are drawn, whether or not the atlas loaded (without the `textures` feature the card
        // is procedural again and draws its veins).
        let oak = TreePreset::Oak.params();
        let (_, leaves) = surfaces(&oak, true);
        if cfg!(feature = "textures") {
            assert!(leaves.leaf_scan.is_some() && leaves.venation.is_none());
            assert_eq!(leaves.uniform.leaf.z, 0.0);
            assert_ne!(leaves.color, oak.foliage.color, "the scan's mean is divided out of the colour");
            assert!(leaves.detail().0.is_some() && leaves.detail().1.is_some());
        } else {
            assert!(leaves.leaf_scan.is_none() && leaves.venation.is_some());
            assert!(leaves.uniform.leaf.z > 0.0);
        }

        // A leaf card without a scan has veins grown by space colonisation, in one image all
        // trees share.
        let dragon = TreePreset::DragonTree.params();
        assert_eq!(dragon.foliage.scan, None);
        let (_, a) = surfaces(&dragon, true);
        assert!(a.leaf_scan.is_none() && a.venation.is_some() && a.uniform.leaf.z > 0.0);
        assert_eq!(a.color, dragon.foliage.color);

        // Needles and cluster spheres are not leaf-shaped: no veins, no image.
        let (_, needles) = surfaces(&TreePreset::Pine.params(), true);
        assert!(needles.venation.is_none() && needles.leaf_scan.is_none() && needles.uniform.leaf.z == 0.0);

        // Without veins to draw there is no image either.
        let mut plain = dragon.clone();
        plain.foliage.shading.veins = 0.0;
        assert!(surfaces(&plain, true).1.venation.is_none());

        // Nothing is made without image assets.
        let (wood, leaves) = surfaces(&oak, false);
        assert!(wood.bark.is_none() && leaves.leaf_scan.is_none() && leaves.venation.is_none());
        assert_eq!(wood.color, oak.trunk_color);
    }

    #[test]
    fn trees_of_other_seeds_share_their_materials_and_other_scans_do_not() {
        use crate::shading::SharedFlora;
        let mut shared = SharedFlora::default();
        let mut materials = Assets::<FloraMaterial>::default();
        let wind = FloraWindBuffer(Handle::default());
        let mut resolve = |surface: &Surface| shared.resolve(&mut materials, surface, &wind);
        let (mut oak, other) = (TreePreset::Oak.params(), TreePreset::Maple.params());
        let (wood, leaves) = surfaces(&oak, true);
        oak.seed += 1;
        let (wood_again, leaves_again) = surfaces(&oak, true);
        // A different seed is the same look.
        assert_eq!(resolve(&wood), resolve(&wood_again));
        assert_eq!(resolve(&leaves), resolve(&leaves_again));
        // A different species is not.
        let (other_wood, other_leaves) = surfaces(&other, true);
        assert_ne!(resolve(&wood), resolve(&other_wood));
        assert_ne!(resolve(&leaves), resolve(&other_leaves));
    }

    #[test]
    fn every_render_style_has_its_own_material_and_pipeline_key() {
        use crate::shading::SharedFlora;
        let mut shared = SharedFlora::default();
        let mut materials = Assets::<FloraMaterial>::default();
        let wind = FloraWindBuffer(Handle::default());
        let mut params = TreePreset::Oak.params();
        let mut resolved = Vec::new();
        for style in [RenderStyle::Natural, RenderStyle::Painterly, RenderStyle::Toon] {
            params.style = style;
            let (wood, leaves) = surfaces(&params, true);
            for surface in [&wood, &leaves] {
                assert_eq!(FloraKey::from(&flora_material(surface, &wind).extension).style, style);
                resolved.push(shared.resolve(&mut materials, surface, &wind));
            }
        }
        let distinct: std::collections::HashSet<_> = resolved.iter().collect();
        assert_eq!(distinct.len(), resolved.len(), "a style shares its material with another");
        // The impostor shader reads the style as a number.
        assert_eq!([RenderStyle::Natural, RenderStyle::Painterly, RenderStyle::Toon].map(|s| s as u32), [0, 1, 2]);
    }

    #[test]
    fn launching_a_gust_stamps_the_render_clock() {
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(5.0));
        let mut wind = FloraWind::default();
        wind.launch_gust(GustFront { start: -1.0, ..Default::default() }, &time);
        let front = wind.gust.expect("the gust was launched");
        assert_eq!(front.start, time.elapsed_secs_wrapped());
        assert_eq!(front.start, 5.0);
        let [_, _, _, _, front_shape, ..] = wind_vectors(&wind, &FloraSeason::default(), &TreeShakes::default(), 1.0);
        assert_eq!(front_shape.w, 5.0);
    }

    #[test]
    fn a_shake_is_packed_and_a_new_one_always_gets_a_slot() {
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(10.0));
        let calm = (FloraWind::default(), FloraSeason::default());
        let pack = |shakes: &TreeShakes| wind_vectors(&calm.0, &calm.1, shakes, 1.0);

        // Nothing shaking: the header says so, and every slot is empty.
        let none = pack(&TreeShakes::default());
        assert_eq!(none[6].x, -1.0);
        assert!(none[7..].iter().all(|v| *v == Vec4::ZERO));

        let mut shakes = TreeShakes::default();
        let hit = TreeShake { at: Vec2::new(3.0, -4.0), radius: 0.0, strength: 2.0, direction: Vec2::new(0.0, 5.0), duration: 3.0, start: -1.0 };
        shakes.shake(hit, &time);
        assert_eq!(shakes.iter().next().map(|s| s.start), Some(10.0), "stamped with the render clock");
        let packed = pack(&shakes);
        assert_eq!(packed[7], Vec4::new(3.0, -4.0, 0.01, 2.0), "radius kept above 0");
        assert_eq!(packed[8], Vec4::new(0.0, 1.0, 10.0, 3.0), "direction normalised");
        assert_eq!(packed[6].x, 13.0, "the header says when the last shake ends");

        // A shake that cannot move anything takes no slot.
        let mut idle = TreeShakes::default();
        idle.shake(TreeShake { strength: 0.0, ..hit }, &time);
        assert_eq!(pack(&idle)[6].x, -1.0);

        // Full: a new shake replaces the one that ends soonest, so it always shows.
        let mut full = TreeShakes::default();
        for i in 0..TreeShakes::MAX {
            full.shake(TreeShake { duration: 10.0 + i as f32, ..TreeShake::default() }, &time);
        }
        let newest = TreeShake { at: Vec2::new(99.0, 0.0), duration: 1.0, ..TreeShake::default() };
        time.advance_by(Duration::from_secs_f32(1.0));
        full.shake(newest, &time);
        let durations: Vec<f32> = full.iter().map(|s| s.duration).collect();
        assert_eq!(full.iter().count(), TreeShakes::MAX);
        assert!(!durations.contains(&10.0), "the shortest-lived shake should have made room: {durations:?}");
        assert!(full.iter().any(|s| s.at.x == 99.0 && s.start == 11.0));
        assert_eq!(pack(&full)[6].x, 10.0 + 17.0, "the longest still ends last");
    }

    #[test]
    fn a_world_in_other_units_packs_the_weather_in_world_units() {
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(2.0));
        let gust = GustFront { origin: Vec2::new(5.0, 6.0), direction: Vec2::X, speed: 15.0, width: 30.0, ..Default::default() };
        let wind = FloraWind { gust: Some(gust), ..Default::default() };
        let mut shakes = TreeShakes::default();
        shakes.shake(TreeShake { at: Vec2::new(1.0, 2.0), radius: 3.0, ..Default::default() }, &time);
        let pack = |units: f32| wind_vectors(&wind, &FloraSeason::default(), &shakes, units);
        let (metres, feet) = (pack(1.0), pack(3.2808));
        // Metres become world units: a gust's speed and width, a shake's radius, and the unit
        // itself for the shaders to scale what they size (a pusher's reach, a falling leaf's drift).
        assert!((feet[4].x - 15.0 * 3.2808).abs() < 1.0e-3 && (feet[4].y - 30.0 * 3.2808).abs() < 1.0e-3);
        assert!((feet[7].z - 3.0 * 3.2808).abs() < 1.0e-4);
        assert_eq!((metres[2].w, feet[2].w), (1.0, 3.2808));
        // Where things are is already in world units, and so is the rest: nothing else moves.
        assert_eq!((feet[3], feet[7].x, feet[7].y, feet[7].w), (metres[3], 1.0, 2.0, 1.0));
        for slot in [0, 1, 5, 6, 8] {
            assert_eq!(feet[slot], metres[slot], "slot {slot}");
        }
    }
}
