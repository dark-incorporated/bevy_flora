//! Leaves that fall from the trees: knocked off by a shake ([`TreeShakes`]), shed by a felled tree
//! as it lands ([`TreeLanded`]), let go by the season ([`FloraSeason::leaf_fall`]), or dropped on
//! demand ([`DropLeaves`]).
//!
//! A drop copies some of the tree's own leaf cards into a burst ([`LeafBurst`]): one entity with
//! one small mesh, drawn with a copy of the tree's leaf material ([`FloraExtension::falling`]),
//! whose vertex shader moves every card on the GPU. A leaf hangs where it grew until its moment
//! (the leaves of a drop let go over half a second), falls, picking up speed and tumbling, swings
//! from side to side, drifts with the [`FloraWind`], turns flat as it reaches the ground
//! ([`FloraGround`], or the height the tree stands at), lies there for about
//! [`FallingLeaves::rest`] seconds and shrinks away. Nothing moves on the CPU; the burst is
//! despawned when its last leaf is gone. The leaves look like those still on the tree (the scan,
//! the tint, the season's colour, snow) because they are drawn by the same fragment shader. A
//! shake's or a drop's leaves are copies: the tree keeps its own, and a few falling copies are not
//! missed from a crown of thousands.
//!
//! In autumn the falling leaves are the tree's own. The shaders stop drawing a deciduous tree's
//! leaf once the season's leaf fall passes the leaf's moment; as it rises, every tenth of a second
//! the leaves it has passed meanwhile let go from where they hung, each from the moment it was
//! passed, on the trees within [`FallingLeaves::autumn_distance`] of a camera (one burst per look
//! of leaves, from many trees). A leaf fall that jumps by more than 0.1 in a frame (a season set,
//! not one passing) sheds those leaves without letting them fall, and one that goes down (spring)
//! drops nothing.
//!
//! [`FallingLeaves`] sets how many fall and how long they lie. Trees drawn on the CPU
//! ([`ProceduralTree`]) and from a pool ([`PooledTree`]) drop leaves; a tree grown on the GPU
//! ([`GpuGrowth`](crate::GpuGrowth)), one drawn without [`FloraMaterial`]s and a leafless one
//! drop none, and a deciduous tree drops none of those it has shed this season.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::prelude::*;
//!
//! // A bird lands in the oak.
//! fn bird_lands(mut drops: MessageWriter<DropLeaves>, oak: Single<Entity, With<ProceduralTree>>) {
//!     drops.write(DropLeaves { tree: *oak, count: 6 });
//! }
//! ```

use std::ops::Range;
use std::sync::RwLock;

use bevy::asset::{AssetEvent, AssetId, Assets, Handle, RenderAssetUsages};
use bevy::camera::Camera3d;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{
    Indices, Mesh, Mesh3d, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues,
    VertexFormat,
};
use bevy::pbr::MeshMaterial3d;
use bevy::platform::collections::HashMap;
use bevy::reflect::Reflect;
use bevy::render::render_resource::{RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::{Shader, ShaderDefVal};
use bevy::time::Time;
use bevy::transform::components::{GlobalTransform, Transform};

use crate::felling::{Felled, TreeLanded};
use crate::fire::{FloraFire, burnt_front};
use crate::ground::FloraGround;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::plugin::{Paint, ProceduralTree, TreeRender};
use crate::pool::{PoolMaterials, PooledLook, PooledTree, TreePool};
use crate::rng::SeededRng;
use crate::shading::{FloraExtension, FloraMaterial, FloraSeason, FloraWind, TreeShake, TreeShakes};
use crate::units::FloraUnits;

/// How many leaves fall and how long they lie. A resource; the plugin adds the default.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FallingLeaves {
    /// Leaves a shake of strength 1 knocks off a tree standing where it happens ([`TreeShakes`]);
    /// fewer the further the tree stands from it, none at its radius. 0 knocks none off.
    pub per_shake: f32,
    /// Leaves a felled tree sheds as it lands ([`TreeLanded`]). 0 sheds none.
    pub per_landing: u32,
    /// Deciduous trees standing within this many metres of a camera let the leaves the season
    /// sheds fall ([`FloraSeason::leaf_fall`]); farther ones lose them where they hang, as before.
    /// 0 lets none fall. Metres: [`FloraUnits`] scales it to the world.
    pub autumn_distance: f32,
    /// Seconds a fallen leaf lies on the ground before it shrinks away, each a quarter more or less.
    pub rest: f32,
    /// Most leaves falling or lying at once; a drop past it drops fewer, or none.
    pub max_leaves: u32,
}

impl Default for FallingLeaves {
    fn default() -> Self {
        Self { per_shake: 12.0, per_landing: 80, autumn_distance: 50.0, rest: 6.0, max_leaves: 5000 }
    }
}

/// Make `count` of `tree`'s leaves fall now, chosen at random (fewer if it has fewer, or if
/// [`FallingLeaves::max_leaves`] would be passed). Write it for a bird that lands, a branch that is
/// hit, a gust; the plugin writes it for shakes and landings. See the [module documentation](self).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropLeaves {
    /// The tree: a [`ProceduralTree`] or a [`PooledTree`].
    pub tree: Entity,
    /// How many leaves.
    pub count: u32,
}

/// A drop of falling leaves: one entity with one mesh, drawn with a copy of its trees' leaf
/// material that moves them ([`FloraExtension::falling`]), and despawned when the last of them is
/// gone.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LeafBurst {
    /// The tree they fell from, or `None` for the leaves the season sheds, which may come from
    /// several trees with one look.
    pub tree: Option<Entity>,
    /// How many leaves.
    pub leaves: u32,
    /// When the last of them is gone, in [`Time::elapsed_secs`].
    pub gone_at: f32,
}

/// Per falling-leaf vertex: where its card's centre hangs in the burst's space (xyz) and the render
/// time it lets go (w). See `falling_common.wesl`.
pub(crate) const ATTRIBUTE_FALL_ANCHOR: MeshVertexAttribute =
    MeshVertexAttribute::new("FallAnchor", 0x666C_6F72_6120, VertexFormat::Float32x4);
/// Per falling-leaf vertex: the ground's height under where it lands (burst space), its seed, how
/// long it lies on the ground and how fast it falls.
pub(crate) const ATTRIBUTE_FALL_PATH: MeshVertexAttribute =
    MeshVertexAttribute::new("FallPath", 0x666C_6F72_6121, VertexFormat::Float32x4);
/// Per falling-leaf vertex: its card's face normal in the burst's space (xyz; w unused).
pub(crate) const ATTRIBUTE_FALL_FACE: MeshVertexAttribute =
    MeshVertexAttribute::new("FallFace", 0x666C_6F72_6122, VertexFormat::Float32x4);

/// The vertex shaders of falling leaves, main pass and prepass (`falling.wesl`,
/// `falling_prepass.wesl`), kept once the shaders are embedded.
static SHADERS: RwLock<Option<(Handle<Shader>, Handle<Shader>)>> = RwLock::new(None);

/// Keep the handles of the falling leaves' vertex shaders for [`specialize_falling`].
pub(crate) fn set_shaders(main: Handle<Shader>, prepass: Handle<Shader>) {
    if let Ok(mut slot) = SHADERS.write() {
        *slot = Some((main, prepass));
    }
}

/// The pipeline of a falling-leaf material ([`FloraExtension::falling`]): its vertex stage moves the
/// cards of a burst (`falling.wesl`, or `falling_prepass.wesl` in a prepass or shadow pipeline, so
/// their shadows fall with them) and reads the fall's attributes after the leaf's sway (11).
pub(crate) fn specialize_falling(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
) -> Result<(), SpecializedMeshPipelineError> {
    let Some((main, prepass)) = SHADERS.read().ok().and_then(|shaders| shaders.clone()) else { return Ok(()) };
    let in_prepass = descriptor.vertex.shader_defs.contains(&ShaderDefVal::from("PREPASS_PIPELINE"));
    descriptor.vertex.shader = if in_prepass { prepass } else { main };
    let fall = layout.0.get_layout(&[
        ATTRIBUTE_FALL_ANCHOR.at_shader_location(12),
        ATTRIBUTE_FALL_PATH.at_shader_location(13),
        ATTRIBUTE_FALL_FACE.at_shader_location(14),
    ])?;
    descriptor.vertex.buffers[0].attributes.extend(fall.attributes);
    Ok(())
}

// The fall's timing and distances, as in `falling_common.wesl`. The distances are metres; the
// world's units to a metre ([`FloraUnits`]) scale them where they are used.
/// Seconds a leaf takes to pick up most of its speed.
const PICK_UP: f32 = 0.35;
/// How fast the wind carries a leaf across, metres per second at strength 1.
const DRIFT: f32 = 1.5;
/// Seconds a leaf takes to shrink away.
const SHRINK: f32 = 1.0;
/// Most a leaf swings to one side, metres.
const SWING: f32 = 0.4;

/// Seconds over which the leaves of one drop let go.
const LET_GO: f32 = 0.5;
/// How fast a leaf falls once it has picked up speed, metres per second: each its own in this range.
const SPEED: Range<f32> = 0.9..1.5;
/// A leaf that hangs less than this above the ground, metres (a felled tree's crown lies on it),
/// stays put.
const LOWEST: f32 = 0.05;
/// Leaf meshes whose cards [`FallCaches`] keeps, at most: a pool's few variants, the trees that
/// drop leaves often.
const CACHED_MESHES: usize = 64;
/// Seconds between the season's sheddings: the leaves whose moment the leaf fall passed meanwhile
/// let go together, each from the time it passed, so a leaf shows a few centimetres down at most,
/// a tenth of a second after its tree stopped drawing it.
const SHED_EVERY: f32 = 0.1;
/// A rise of the leaf fall bigger than this in one frame is a season set, not one passing: those
/// leaves are gone at once and none falls.
const SEASON_JUMP: f32 = 0.1;
/// The render clock (`globals.time`, [`Time::elapsed_secs_wrapped`]) wraps every hour.
const WRAP: f32 = 3600.0;

/// What the falling leaves keep between frames: the falling-leaf material made from each leaf
/// material, and the cards of each leaf mesh read.
#[derive(Resource, Default)]
pub(crate) struct FallCaches {
    materials: HashMap<AssetId<FloraMaterial>, Handle<FloraMaterial>>,
    cards: HashMap<AssetId<Mesh>, MeshCards>,
}

impl FallCaches {
    /// The cards of `source` (the mesh `id`; a hull crown's, with the tree's seed in `hull`),
    /// read now unless they were read from it as it is.
    fn cards(&mut self, id: AssetId<Mesh>, source: &Mesh, hull: Option<i32>) -> &MeshCards {
        if self.cards.len() >= CACHED_MESHES && !self.cards.contains_key(&id) {
            self.cards.clear();
        }
        // Read again if the mesh is not the one read last time (its change not seen yet).
        let size = (source.count_vertices(), source.indices().map_or(0, Indices::len));
        let read = self.cards.entry(id).or_insert_with(|| MeshCards::read(source, size, hull));
        if read.size != size {
            *read = MeshCards::read(source, size, hull);
        }
        read
    }

    /// The material falling leaves of the leaf material `leaves` are drawn with: a copy with
    /// [`FloraExtension::falling`] set, made once per look and again when the look changes.
    fn material(&mut self, leaves: &Handle<FloraMaterial>, flora: &mut Assets<FloraMaterial>) -> Option<Handle<FloraMaterial>> {
        let source = flora.get(leaves)?;
        let look = FloraMaterial {
            base: source.base.clone(),
            extension: FloraExtension { falling: true, ..source.extension.clone() },
        };
        // Made again when the leaves' look changed: their uniform, or their colour or lighting.
        if let Some(made) = self.materials.get(&leaves.id())
            && flora.get(made).is_some_and(|made| {
                made.extension == look.extension
                    && made.base.base_color == look.base.base_color
                    && made.base.unlit == look.base.unlit
            })
        {
            return Some(made.clone());
        }
        let made = flora.add(look);
        self.materials.insert(leaves.id(), made.clone());
        Some(made)
    }
}

/// The cards of a leaf mesh, its vertex and index counts when they were read, and its leaves in the
/// order the season sheds them.
struct MeshCards {
    size: (usize, usize),
    cards: Vec<Card>,
    /// The leaves (not the flowers), by their moment in the season.
    by_moment: Vec<u32>,
    /// The chips of a hull crown, which the cards are of.
    chips: Option<Mesh>,
}

impl MeshCards {
    /// The cards of `mesh`, or, for a hull crown's mesh (`hull`: the tree's seed), of the chips
    /// it sheds ([`hull_chips`]), which are then the mesh the cards are of ([`MeshCards::mesh`]).
    fn read(mesh: &Mesh, size: (usize, usize), hull: Option<i32>) -> Self {
        let chips = hull.and_then(|seed| hull_chips(mesh, seed));
        let cards = leaf_cards(chips.as_ref().unwrap_or(mesh));
        let mut by_moment: Vec<u32> = (0..cards.len() as u32).filter(|&k| !cards[k as usize].flower).collect();
        by_moment.sort_by(|a, b| cards[*a as usize].moment.total_cmp(&cards[*b as usize].moment));
        Self { size, cards, by_moment, chips }
    }

    /// The mesh the cards are runs of: the chips of a hull crown, else `source`, the leaf mesh.
    fn mesh<'a>(&'a self, source: &'a Mesh) -> &'a Mesh {
        self.chips.as_ref().unwrap_or(source)
    }

    /// The leaves whose moment the leaf fall passes as it rises from `from` to `to`, in that order.
    fn shed_between(&self, from: f32, to: f32) -> impl Iterator<Item = &Card> {
        let moment = |k: &u32| self.cards[*k as usize].moment;
        let start = self.by_moment.partition_point(|k| moment(k) < from);
        let end = self.by_moment.partition_point(|k| moment(k) < to);
        self.by_moment[start..end].iter().map(|&k| &self.cards[k as usize])
    }
}

/// Seconds a leaf that lets go `height` metres above the ground takes to reach it.
pub(crate) fn landing_time(height: f32, speed: f32) -> f32 {
    if height <= 0.0 {
        return 0.0;
    }
    let mut t = height / speed + PICK_UP;
    for _ in 0..4 {
        let e = (-t / PICK_UP).exp();
        let f = speed * (t - PICK_UP * (1.0 - e)) - height;
        t -= f / (speed * (1.0 - e)).max(1.0e-3);
    }
    t.max(0.0)
}

/// `leaf_season_moment` in `common.wesl`: a leaf's moment in the season, from its phase.
pub(crate) fn leaf_season_moment(phase: f32) -> f32 {
    let x = phase * 7.31 + 0.137;
    x - x.floor()
}

/// One card of a tree's leaf mesh: its vertices and its triangles' indices (runs of the mesh's
/// buffers), its moment in the season and whether it is a flower.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Card {
    pub vertices: Range<usize>,
    pub indices: Range<usize>,
    /// `leaf_season_moment` of its phase: a deciduous tree sheds it once the leaf fall passes it.
    pub moment: f32,
    /// A flower or a fruit (vertex alpha below 0.5), not a leaf: it never drops as a leaf.
    pub flower: bool,
}

impl Card {
    /// Whether it is a leaf still on the tree, given the season's leaf fall of a deciduous tree and
    /// the fire's burning front at the tree (`fire::burnt_front`).
    pub fn on_tree(&self, shed: Option<f32>, burnt: f32) -> bool {
        !self.flower && self.moment >= burnt && shed.is_none_or(|shed| self.moment >= shed)
    }
}

/// The cards of `mesh`, a tree's leaf mesh, each with its triangles. The foliage mesher keeps each
/// leaf's vertices and triangles together and gives the leaf one phase (`sway.w`), which is what
/// tells one card from the next.
pub(crate) fn leaf_cards(mesh: &Mesh) -> Vec<Card> {
    let Some(VertexAttributeValues::Float32x4(sway)) = mesh.attribute(ATTRIBUTE_LEAF_SWAY) else {
        return Vec::new();
    };
    let Some(indices) = mesh.indices() else { return Vec::new() };
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(colors)) => Some(colors),
        _ => None,
    };
    // Runs of vertices with one phase.
    let mut cards: Vec<Card> = Vec::new();
    let mut card_of = Vec::with_capacity(sway.len());
    for (v, s) in sway.iter().enumerate() {
        match cards.last_mut() {
            Some(card) if sway[card.vertices.start][3].to_bits() == s[3].to_bits() => card.vertices.end = v + 1,
            _ => cards.push(Card {
                vertices: v..v + 1,
                indices: 0..0,
                moment: leaf_season_moment(s[3]),
                flower: colors.is_some_and(|colors| colors[v][3] < 0.5),
            }),
        }
        card_of.push(cards.len() - 1);
    }
    // Each card's triangles follow one another, in the order of the cards.
    let indices: Vec<usize> = indices.iter().collect();
    for (k, triangle) in indices.as_chunks::<3>().0.iter().enumerate() {
        let card = &mut cards[card_of[triangle[0]]];
        if card.indices.is_empty() {
            card.indices = 3 * k..3 * k;
        }
        card.indices.end = 3 * k + 3;
    }
    cards.retain(|card| !card.indices.is_empty());
    cards
}

/// How many chips fall from a hull crown, whatever its size: one per `CHIPS` random points of
/// its surface, so a crown sheds a handful, as a leaf-carded tree sheds a few of its thousands.
const CHIPS: usize = 96;
/// The length of a chip, metres: an isosceles triangle a little longer than it is wide.
const CHIP_LENGTH: f32 = 0.12;
/// A chip's base as a share of its length.
const CHIP_WIDTH: f32 = 0.6;
/// Separate random stream of the chips, so they move no other draw.
const CHIP_SALT: i32 = 0x4348_4950;

/// The chips a hull crown sheds, as a leaf mesh: `hull`, a hull crown's leaf mesh (flat facets of
/// vertex alpha 1, three vertices each), gives [`CHIPS`] isosceles triangles [`CHIP_LENGTH`] long
/// lying in the plane of a facet chosen by area, at a random point of it and a random heading,
/// in the facet's colour with its normal. Each chip is a card of its own (one phase each), and its
/// moment in the season is that of the facet it came from, so a burnt or shed facet sheds no
/// chips. `seed` is the tree's: the same crown always sheds the same chips. `None` for a mesh
/// with no facets.
pub(crate) fn hull_chips(hull: &Mesh, seed: i32) -> Option<Mesh> {
    let (Some(VertexAttributeValues::Float32x3(positions)), Some(VertexAttributeValues::Float32x4(colors))) =
        (hull.attribute(Mesh::ATTRIBUTE_POSITION), hull.attribute(Mesh::ATTRIBUTE_COLOR))
    else {
        return None;
    };
    let sway = match hull.attribute(ATTRIBUTE_LEAF_SWAY) {
        Some(VertexAttributeValues::Float32x4(sway)) => sway,
        _ => return None,
    };
    let indices: Vec<usize> = hull.indices()?.iter().collect();
    // The facets: triangles of leaf alpha (a fruit's or a flower's is below 0.5), with their areas.
    let at = |v: usize| Vec3::from_array(positions[v]);
    let mut facets: Vec<([usize; 3], f32)> = Vec::new();
    for tri in indices.as_chunks::<3>().0 {
        if colors[tri[0]][3] < 0.5 {
            continue;
        }
        let area = 0.5 * (at(tri[1]) - at(tri[0])).cross(at(tri[2]) - at(tri[0])).length();
        if area > 1.0e-8 {
            facets.push((*tri, area));
        }
    }
    let total: f32 = facets.iter().map(|(_, area)| area).sum();
    if facets.is_empty() || total <= 0.0 {
        return None;
    }
    let mut out = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD);
    let (mut p, mut n, mut uv, mut uv1, mut col, mut sw) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..CHIPS {
        let mut rng = SeededRng::from_parts(seed ^ CHIP_SALT, k as u32);
        // A facet by area, then a point of it (uniform: the square-root fold of two draws).
        let mut pick = rng.next_f32() * total;
        let facet = facets
            .iter()
            .find(|(_, area)| {
                pick -= area;
                pick < 0.0
            })
            .unwrap_or(&facets[facets.len() - 1]);
        let tri = facet.0;
        let (u, v) = (rng.next_f32(), rng.next_f32());
        let (a, b) = if u + v > 1.0 { (1.0 - u, 1.0 - v) } else { (u, v) };
        let centre = at(tri[0]) + (at(tri[1]) - at(tri[0])) * a + (at(tri[2]) - at(tri[0])) * b;
        let normal = (at(tri[1]) - at(tri[0])).cross(at(tri[2]) - at(tri[0])).normalize_or(Vec3::Y);
        let heading = normal.any_orthogonal_vector().normalize();
        let (sin, cos) = (rng.next_f32() * std::f32::consts::TAU).sin_cos();
        let along = (heading * cos + normal.cross(heading) * sin) * CHIP_LENGTH;
        let across = normal.cross(along) * CHIP_WIDTH * 0.5;
        let (tip, left, right) = (centre + along * 0.5, centre - along * 0.5 - across, centre - along * 0.5 + across);
        // Counter-clockwise seen from outside, along the facet's normal.
        let (left, right) = if (left - tip).cross(right - tip).dot(normal) < 0.0 { (right, left) } else { (left, right) };
        // The facet's colour, and its phase nudged by the chip's own so no two neighbours share one
        // (a run of one phase is one card); the nudge moves the moment by under a thousandth.
        let phase = sway[tri[0]][3] + k as f32 * 1.0e-6;
        for corner in [tip, left, right] {
            p.push(corner.to_array());
            n.push(normal.to_array());
            uv.push([0.5_f32, 0.5]);
            uv1.push([1.0_f32, 1.0]);
            col.push(colors[tri[0]]);
            sw.push([0.0_f32, 0.0, 0.0, phase]);
        }
    }
    let count = p.len() as u32;
    out.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    out.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    out.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    out.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1);
    out.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    out.insert_attribute(ATTRIBUTE_LEAF_SWAY, sw);
    out.insert_indices(Indices::U32((0..count).collect()));
    Some(out)
}

/// How a drop falls: when it begins, how long its leaves lie and how the wind carries them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fall {
    /// The render time ([`Time::elapsed_secs_wrapped`]) leaves' release times count from.
    pub now: f32,
    /// [`FallingLeaves::rest`].
    pub rest: f32,
    /// The wind's drift, world units per second on the ground.
    pub drift: Vec2,
    /// World units to a metre ([`FloraUnits`]): what scales the speeds and the margins below.
    pub units: f32,
}

impl Fall {
    /// The fall of leaves that let go now, under `wind`, in a world of `units`.
    fn now(time: &Time, settings: &FallingLeaves, wind: &FloraWind, units: &FloraUnits) -> Self {
        let units = units.units_per_metre;
        Self {
            now: time.elapsed_secs_wrapped(),
            rest: settings.rest,
            drift: wind.direction.normalize_or_zero() * (wind.strength.max(0.0) * DRIFT * units),
            units,
        }
    }
}

/// What a burst is made of: its mesh and bounds in the space of the burst standing at its origin,
/// how many leaves it holds, and how many seconds until the last is gone.
pub(crate) struct Burst {
    pub mesh: Mesh,
    pub aabb: Aabb,
    pub leaves: u32,
    pub life: f32,
}

/// A burst under construction: cards of one tree or of several, every vertex in world orientation
/// and scale relative to `origin`, with the attributes `falling_common.wesl` reads.
pub(crate) struct BurstBuilder {
    pub origin: Vec3,
    out: BurstBuffers,
    lo: Vec3,
    hi: Vec3,
    life: f32,
    leaves: u32,
}

impl BurstBuilder {
    /// An empty burst standing at `origin` (world).
    pub fn new(origin: Vec3) -> Self {
        Self {
            origin,
            out: BurstBuffers::default(),
            lo: Vec3::splat(f32::INFINITY),
            hi: Vec3::splat(f32::NEG_INFINITY),
            life: 0.0,
            leaves: 0,
        }
    }

    /// Add `cards` of `source` (a tree's leaf mesh) for the tree placed at `tree`, each letting go
    /// `release(card, rng)` seconds after `fall.now` (before it when negative). `ground` is the
    /// ground's height at a world (x, z); a card hanging less than 5 cm above it stays. Returns how
    /// many it added.
    #[allow(clippy::too_many_arguments)]
    pub fn add<'a>(
        &mut self,
        source: &Mesh,
        cards: impl IntoIterator<Item = &'a Card>,
        tree: &GlobalTransform,
        ground: impl Fn(Vec2) -> f32,
        fall: Fall,
        rng: &mut SeededRng,
        mut release: impl FnMut(&Card, &mut SeededRng) -> f32,
    ) -> u32 {
        let floats3 = |id| match source.attribute(id) {
            Some(VertexAttributeValues::Float32x3(v)) => Some(v),
            _ => None,
        };
        let floats4 = |id| match source.attribute(id) {
            Some(VertexAttributeValues::Float32x4(v)) => Some(v),
            _ => None,
        };
        let floats2 = |id| match source.attribute(id) {
            Some(VertexAttributeValues::Float32x2(v)) => Some(v),
            _ => None,
        };
        let (Some(positions), Some(normals), Some(uv0), Some(sway), Some(source_indices)) = (
            floats3(Mesh::ATTRIBUTE_POSITION),
            floats3(Mesh::ATTRIBUTE_NORMAL),
            floats2(Mesh::ATTRIBUTE_UV_0),
            floats4(ATTRIBUTE_LEAF_SWAY),
            source.indices(),
        ) else {
            return 0;
        };
        let uv1 = floats2(Mesh::ATTRIBUTE_UV_1);
        let colors = floats4(Mesh::ATTRIBUTE_COLOR);
        let source_indices: Vec<usize> = source_indices.iter().collect();

        let origin = self.origin;
        let affine = tree.affine();
        let out = &mut self.out;
        let mut added = 0;
        for card in cards {
            let world: Vec<Vec3> =
                card.vertices.clone().map(|v| affine.transform_point3(Vec3::from_array(positions[v])) - origin).collect();
            let anchor = world.iter().sum::<Vec3>() / world.len() as f32;
            let reach = world.iter().map(|p| p.distance(anchor)).fold(0.0, f32::max);
            // The ground under where it will land: under where it hangs, then where the wind takes it.
            let speed = rng.range(SPEED.start, SPEED.end) * fall.units;
            let at = anchor + origin;
            let mut landing = at.xz();
            let mut floor = ground(landing);
            for _ in 0..2 {
                landing = at.xz() + fall.drift * landing_time(at.y - floor, speed);
                floor = ground(landing);
            }
            let height = at.y - floor;
            if height < LOWEST * fall.units {
                continue;
            }
            let land = landing_time(height, speed);
            let delay = release(card, rng);
            let rest = fall.rest.max(0.0) * rng.range(0.75, 1.25);
            let seed = rng.next_f32();
            self.life = self.life.max(delay + land + rest + SHRINK);
            added += 1;

            // The card's face, from its first triangle.
            let first = &source_indices[card.indices.start..card.indices.start + 3];
            let corner = |k: usize| affine.transform_point3(Vec3::from_array(positions[first[k]]));
            let face = (corner(1) - corner(0)).cross(corner(2) - corner(0)).normalize_or(Vec3::Y);

            let base = out.positions.len() as u32;
            let local_floor = floor - origin.y;
            for (k, v) in card.vertices.clone().enumerate() {
                out.positions.push(world[k].to_array());
                out.normals.push(affine.transform_vector3(Vec3::from_array(normals[v])).normalize_or(Vec3::Y).to_array());
                out.uv0.push(uv0[v]);
                if let Some(uv1) = uv1 {
                    out.uv1.push(uv1[v]);
                }
                if let Some(colors) = colors {
                    out.colors.push(colors[v]);
                }
                out.sway.push(sway[v]);
                out.anchor.push(anchor.extend(fall.now + delay).to_array());
                out.path.push([local_floor, seed, rest, speed]);
                out.face.push(face.extend(0.0).to_array());
            }
            let start = card.vertices.start;
            out.indices.extend(source_indices[card.indices.clone()].iter().map(|&i| base + (i - start) as u32));

            // Bounds: where it hangs and where it lands, each with the card's reach and the swing.
            let landed = Vec3::new(landing.x - origin.x, local_floor, landing.y - origin.z);
            let margin = Vec3::splat(reach + SWING * fall.units);
            self.lo = self.lo.min(anchor - margin).min(landed - margin);
            self.hi = self.hi.max(anchor + margin).max(landed + margin);
        }
        self.leaves += added;
        added
    }

    /// The burst, or `None` when no card was added.
    pub fn finish(self) -> Option<Burst> {
        (self.leaves > 0).then(|| Burst {
            mesh: self.out.into_mesh(),
            aabb: Aabb::from_min_max(self.lo, self.hi),
            leaves: self.leaves,
            life: self.life,
        })
    }
}

/// The vertex buffers of a burst under construction.
#[derive(Default)]
struct BurstBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv0: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    sway: Vec<[f32; 4]>,
    anchor: Vec<[f32; 4]>,
    path: Vec<[f32; 4]>,
    face: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl BurstBuffers {
    fn into_mesh(self) -> Mesh {
        // Only the GPU reads it.
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv0)
            .with_inserted_attribute(ATTRIBUTE_LEAF_SWAY, self.sway)
            .with_inserted_attribute(ATTRIBUTE_FALL_ANCHOR, self.anchor)
            .with_inserted_attribute(ATTRIBUTE_FALL_PATH, self.path)
            .with_inserted_attribute(ATTRIBUTE_FALL_FACE, self.face)
            .with_inserted_indices(Indices::U32(self.indices));
        if !self.uv1.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, self.uv1);
        }
        if !self.colors.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }
        mesh
    }
}

/// `smoothstep(0, edge, x)`, as in WGSL.
fn smoothstep(edge: f32, x: f32) -> f32 {
    let t = (x / edge.max(1.0e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Turn each new shake into drops from the standing trees it reaches, as many as it shakes them
/// (`shake_at` in `common.wesl`: all of its strength where it happens, none at its radius).
#[allow(clippy::type_complexity)]
pub(crate) fn shake_leaves_loose(
    shakes: Res<TreeShakes>,
    settings: Res<FallingLeaves>,
    units: Res<FloraUnits>,
    mut seen: Local<Vec<TreeShake>>,
    trees: Query<(Entity, &GlobalTransform), (Or<(With<TreeRender>, With<PooledTree>)>, Without<Felled>)>,
    mut drops: MessageWriter<DropLeaves>,
) {
    if !shakes.is_changed() {
        return;
    }
    let new: Vec<TreeShake> = shakes.iter().filter(|shake| !seen.contains(shake)).copied().collect();
    seen.clear();
    seen.extend(shakes.iter().copied());
    if settings.per_shake <= 0.0 {
        return;
    }
    for shake in new {
        // The shake's radius is metres, the distance to a tree world units.
        let radius = shake.radius.max(0.01) * units.units_per_metre;
        for (tree, at) in &trees {
            let near = 1.0 - smoothstep(radius, at.translation().xz().distance(shake.at));
            let count = (settings.per_shake * shake.strength.max(0.0) * near).round() as u32;
            if count > 0 {
                drops.write(DropLeaves { tree, count });
            }
        }
    }
}

/// A felled tree sheds leaves as it lands.
pub(crate) fn shed_landed_leaves(
    mut landed: MessageReader<TreeLanded>,
    settings: Res<FallingLeaves>,
    mut drops: MessageWriter<DropLeaves>,
) {
    for landing in landed.read() {
        if settings.per_landing > 0 {
            drops.write(DropLeaves { tree: landing.tree, count: settings.per_landing });
        }
    }
}

/// What a tree's leaves fall as: its level-0 leaf mesh, its leaf material, and whether it sheds its
/// leaves in autumn.
struct LeafSource {
    mesh: Handle<Mesh>,
    material: Handle<FloraMaterial>,
    deciduous: bool,
    /// The seed chips are drawn from when the tree has a hull crown ([`CrownStyle::Hull`]): what
    /// falls from a hull is chips of its facets ([`hull_chips`]), not its facets.
    hull: Option<i32>,
}

/// The leaves of a tree drawn on the CPU with [`FloraMaterial`]s, or from a pool's: `None` for any
/// other tree, and a tree with no leaves.
fn leaf_source(
    render: Option<&TreeRender>,
    tree: Option<&ProceduralTree>,
    pooled: Option<&PooledTree>,
    look: Option<&PooledLook>,
    pools: Option<&Assets<TreePool>>,
) -> Option<LeafSource> {
    let hull = |params: &crate::params::TreeParams, variant: i32| {
        matches!(params.crown, crate::params::CrownStyle::Hull { .. }).then_some(params.seed.wrapping_add(variant))
    };
    match (render, tree, pooled) {
        (Some(render), Some(tree), _) => {
            let (Paint::Flora { foliage, .. }, Some((_, mesh))) = (&render.paint, &render.foliage) else { return None };
            Some(LeafSource {
                mesh: mesh.clone(),
                material: foliage.clone(),
                deciduous: tree.0.foliage.autumn.is_some(),
                hull: hull(&tree.0, 0),
            })
        }
        (_, _, Some(pooled)) => {
            let pool = pools?.get(&pooled.pool)?;
            let look = look.map_or(0, |look| look.0 as usize);
            let Some(PoolMaterials::Flora { leaves, .. }) = pool.materials.get(look % pool.materials.len().max(1)) else {
                return None;
            };
            let index = pooled.variant as usize % pool.variants.len();
            Some(LeafSource {
                mesh: pool.variants[index].leaves[0].clone()?,
                material: leaves.clone(),
                deciduous: pool.params.foliage.autumn.is_some(),
                hull: hull(&pool.params, index as i32),
            })
        }
        _ => None,
    }
}

/// Spawn `burst`, standing at `origin` and drawn with `material`.
fn spawn_burst(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    burst: Burst,
    origin: Vec3,
    material: Handle<FloraMaterial>,
    tree: Option<Entity>,
    time: &Time,
) {
    commands.spawn((
        Mesh3d(meshes.add(burst.mesh)),
        MeshMaterial3d(material),
        // Spawned after this frame's transform propagation: its global transform is set here.
        Transform::from_translation(origin),
        GlobalTransform::from_translation(origin),
        Visibility::default(),
        burst.aabb,
        LeafBurst { tree, leaves: burst.leaves, gone_at: time.elapsed_secs() + burst.life },
    ));
}

/// Forget the cards of leaf meshes that changed (a growing tree's, every frame) or went.
pub(crate) fn forget_changed_meshes(mut caches: ResMut<FallCaches>, mut changed: MessageReader<AssetEvent<Mesh>>) {
    for event in changed.read() {
        if let AssetEvent::Modified { id } | AssetEvent::Removed { id } | AssetEvent::Unused { id } = event
            && caches.cards.contains_key(id)
        {
            caches.cards.remove(id);
        }
    }
}

/// Make the bursts the drops ask for.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn drop_leaves(
    mut commands: Commands,
    mut drops: MessageReader<DropLeaves>,
    settings: Res<FallingLeaves>,
    time: Res<Time>,
    wind: Res<FloraWind>,
    season: Res<FloraSeason>,
    ground: Option<Res<FloraGround>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    pools: Option<Res<Assets<TreePool>>>,
    trees: Query<(
        &GlobalTransform,
        Option<&TreeRender>,
        Option<&ProceduralTree>,
        Option<&PooledTree>,
        Option<&PooledLook>,
        Option<&Felled>,
    )>,
    stumps: Query<&GlobalTransform>,
    bursts: Query<&LeafBurst>,
    mut caches: ResMut<FallCaches>,
    fire: Option<Res<FloraFire>>,
    units: Res<FloraUnits>,
) {
    let mut live: u32 = bursts.iter().map(|burst| burst.leaves).sum();
    let fall = Fall::now(&time, &settings, &wind, &units);
    for (k, drop) in drops.read().enumerate() {
        let budget = settings.max_leaves.saturating_sub(live).min(drop.count) as usize;
        if budget == 0 {
            continue;
        }
        let Ok((at, render, tree, pooled, look, felled)) = trees.get(drop.tree) else { continue };
        let Some(leaves) = leaf_source(render, tree, pooled, look, pools.as_deref()) else { continue };
        let Some(source) = meshes.get(&leaves.mesh) else { continue };
        let shed = leaves.deciduous.then_some(season.leaf_fall);
        let burnt = burnt_front(fire.as_deref(), at.translation());
        let read = caches.cards(leaves.mesh.id(), source, leaves.hull);
        let mut cards: Vec<&Card> = read.cards.iter().filter(|card| card.on_tree(shed, burnt)).collect();
        if cards.is_empty() {
            continue;
        }
        // Which leaves: a few at random, by a partial shuffle.
        let mut rng = SeededRng::from_parts(drop.tree.to_bits() as i32 ^ fall.now.to_bits() as i32, k as u32);
        let (chosen, last) = (budget.min(cards.len()), cards.len() - 1);
        for i in 0..chosen {
            let j = i + (rng.next_f32() * (cards.len() - i) as f32) as usize;
            cards.swap(i, j.min(last));
        }
        // The ground: the terrain, or the height the tree stands at (a felled tree's: its stump's).
        let stood = felled.and_then(|felled| stumps.get(felled.stump).ok()).unwrap_or(at).translation().y;
        let height = |p: Vec2| ground.as_deref().map_or(stood, |ground| ground.height(p));
        let mut burst = BurstBuilder::new(at.translation());
        burst.add(read.mesh(source), cards[..chosen].iter().copied(), at, height, fall, &mut rng, |_, rng| rng.range(0.0, LET_GO));
        let Some(burst) = burst.finish() else { continue };
        let Some(material) = caches.material(&leaves.material, &mut flora) else { continue };
        live += burst.leaves;
        spawn_burst(&mut commands, &mut meshes, burst, at.translation(), material, Some(drop.tree), &time);
    }
}

/// Where the season's shedding stands: the leaf fall shed up to and the render time it was, and
/// the leaf fall of the last frame.
#[derive(Resource, Default)]
pub(crate) struct SeasonShed {
    shed: Option<(f32, f32)>,
    last: f32,
}

/// Whether [`shed_with_season`] has anything to do: the season changed, or leaves it passed are
/// still to let go.
pub(crate) fn season_sheds(season: Res<FloraSeason>, shed: Res<SeasonShed>) -> bool {
    season.is_changed() || shed.shed.is_some_and(|(from, _)| from < shed.last)
}

/// Let the leaves the season sheds fall. Every [`SHED_EVERY`] seconds while the leaf fall rises,
/// the leaves of the deciduous trees within [`FallingLeaves::autumn_distance`] of a camera whose
/// moment it has passed since (the very leaves the shaders stop drawing) let go, each at the time
/// the leaf fall passed it, in one burst per look of leaves.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn shed_with_season(
    mut commands: Commands,
    settings: Res<FallingLeaves>,
    season: Res<FloraSeason>,
    time: Res<Time>,
    wind: Res<FloraWind>,
    ground: Option<Res<FloraGround>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    pools: Option<Res<Assets<TreePool>>>,
    mut caches: ResMut<FallCaches>,
    mut state: ResMut<SeasonShed>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    trees: Query<
        (&GlobalTransform, Option<&TreeRender>, Option<&ProceduralTree>, Option<&PooledTree>, Option<&PooledLook>),
        Without<Felled>,
    >,
    bursts: Query<&LeafBurst>,
    fire: Option<Res<FloraFire>>,
    units: Res<FloraUnits>,
) {
    let to = season.leaf_fall.clamp(0.0, 1.0);
    let now = time.elapsed_secs_wrapped();
    let last = std::mem::replace(&mut state.last, to);
    let Some((from, since)) = state.shed else {
        state.shed = Some((to, now));
        return;
    };
    // Spring (the leaves come back) or a season set at once (they are gone at once): none falls.
    if to < from || to - last > SEASON_JUMP {
        state.shed = Some((to, now));
        return;
    }
    let mut elapsed = now - since;
    if elapsed < -0.5 * WRAP {
        elapsed += WRAP;
    }
    if to == from {
        state.shed = Some((to, now));
        return;
    }
    if elapsed < SHED_EVERY {
        return;
    }
    state.shed = Some((to, now));

    let eyes: Vec<Vec3> = cameras.iter().map(GlobalTransform::translation).collect();
    let distance = units.world(settings.autumn_distance);
    let reach = distance * distance;
    if eyes.is_empty() || reach <= 0.0 {
        return;
    }
    let mut live: u32 = bursts.iter().map(|burst| burst.leaves).sum();
    let fall = Fall::now(&time, &settings, &wind, &units);
    let mut rng = SeededRng::from_parts(now.to_bits() as i32, 1);
    let span = to - from;
    let mut looks: HashMap<AssetId<FloraMaterial>, (Handle<FloraMaterial>, BurstBuilder)> = HashMap::default();
    for (at, render, tree, pooled, look) in &trees {
        let budget = settings.max_leaves.saturating_sub(live) as usize;
        if budget == 0 {
            break;
        }
        let position = at.translation();
        if !eyes.iter().any(|eye| eye.distance_squared(position) <= reach) {
            continue;
        }
        let Some(leaves) = leaf_source(render, tree, pooled, look, pools.as_deref()).filter(|leaves| leaves.deciduous) else {
            continue;
        };
        let Some(source) = meshes.get(&leaves.mesh) else { continue };
        let burnt = burnt_front(fire.as_deref(), position);
        let read = caches.cards(leaves.mesh.id(), source, leaves.hull);
        let falling: Vec<&Card> =
            read.shed_between(from, to).filter(|card| card.moment >= burnt).take(budget).collect();
        if falling.is_empty() {
            continue;
        }
        let height = |p: Vec2| ground.as_deref().map_or(position.y, |ground| ground.height(p));
        let (_, burst) = looks
            .entry(leaves.material.id())
            .or_insert_with(|| (leaves.material.clone(), BurstBuilder::new(position)));
        // Each lets go when the leaf fall passed its moment, assuming it rose evenly meanwhile.
        live += burst.add(read.mesh(source), falling, at, height, fall, &mut rng, |card, _| {
            ((card.moment - from) / span - 1.0) * elapsed
        });
    }
    for (_, (leaf_material, burst)) in looks {
        let origin = burst.origin;
        let (Some(burst), Some(material)) = (burst.finish(), caches.material(&leaf_material, &mut flora)) else { continue };
        spawn_burst(&mut commands, &mut meshes, burst, origin, material, None, &time);
    }
}

/// Despawn the bursts whose last leaf is gone.
pub(crate) fn expire_bursts(mut commands: Commands, time: Res<Time>, bursts: Query<(Entity, &LeafBurst)>) {
    let now = time.elapsed_secs();
    for (entity, burst) in &bursts {
        if now >= burst.gone_at {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::TreeAge;
    use bevy::pbr::StandardMaterial;
    use crate::pipeline::build_tree;
    use crate::presets::TreePreset;

    fn leaf_mesh(preset: TreePreset) -> Mesh {
        build_tree(&preset.params(), f32::INFINITY).foliage.into_bevy_mesh()
    }

    /// How far a leaf has fallen `t` seconds after letting go, at full speed `speed` (`fallen` in
    /// `falling_common.wesl`).
    fn fallen(t: f32, speed: f32) -> f32 {
        speed * (t - PICK_UP * (1.0 - (-t / PICK_UP).exp()))
    }

    #[test]
    fn a_leaf_lands_when_it_has_fallen_its_height() {
        for (height, speed) in [(0.3, 0.9), (2.0, 1.2), (8.0, 1.5), (25.0, 1.0)] {
            let t = landing_time(height, speed);
            assert!((fallen(t, speed) - height).abs() < 1.0e-3, "{height} m at {speed} m/s: {t} s");
        }
        assert_eq!(landing_time(-1.0, 1.0), 0.0);
    }

    #[test]
    fn every_leaf_is_one_card_and_a_card_holds_only_its_own_triangles() {
        for preset in [TreePreset::Oak, TreePreset::Pine, TreePreset::Maple, TreePreset::Willow] {
            let params = preset.params();
            let built = build_tree(&params, f32::INFINITY);
            let points = crate::foliage::collect_leaf_points(
                &built.skeleton,
                &params.foliage,
                &params.growth,
                &params.trunk,
                params.seed,
            );
            let mesh = built.foliage.into_bevy_mesh();
            let cards = leaf_cards(&mesh);
            assert_eq!(cards.len(), points.len(), "{preset:?}");
            let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();
            for card in &cards {
                assert!(indices[card.indices.clone()].iter().all(|i| card.vertices.contains(i)), "{preset:?}");
            }
        }
    }

    #[test]
    fn a_tree_drops_neither_flowers_fruit_nor_the_leaves_it_has_shed() {
        // A pool's leaf mesh holds the flowers after the leaves.
        let built = build_tree(&TreePreset::Cherry.params(), f32::INFINITY);
        let leaves = on_tree(&built.foliage.clone().into_bevy_mesh(), None);
        let mut merged = built.foliage;
        merged.extend(&built.flowers);
        assert_eq!(on_tree(&merged.into_bevy_mesh(), None), leaves);
        // Each fruit is a card of its own in the leaf mesh, never a leaf's.
        let mut apple = TreePreset::Apple.params();
        let fruiting = build_tree(&apple, f32::INFINITY).foliage.into_bevy_mesh();
        apple.foliage.fruit = None;
        let bare = build_tree(&apple, f32::INFINITY).foliage.into_bevy_mesh();
        assert_eq!(on_tree(&fruiting, None), on_tree(&bare, None));
        assert!(leaf_cards(&fruiting).len() > leaf_cards(&bare).len(), "the fruit went into its leaves' cards");

        let oak = leaf_mesh(TreePreset::Oak);
        let all = on_tree(&oak, None);
        let half = on_tree(&oak, Some(0.5));
        assert!((all as f32 * 0.4..all as f32 * 0.6).contains(&(half as f32)), "{half} of {all} left at half the leaf fall");
        assert_eq!(on_tree(&oak, Some(1.0)), 0);
    }

    /// How many of the cards of `mesh` are leaves on the tree at the season's leaf fall `shed`.
    fn on_tree(mesh: &Mesh, shed: Option<f32>) -> usize {
        leaf_cards(mesh).iter().filter(|card| card.on_tree(shed, f32::NEG_INFINITY)).count()
    }

    #[test]
    fn a_hull_sheds_chips_not_facets_and_a_burnt_one_none_it_has_lost() {
        let hull = build_tree(&crate::crown_hull::hull_spruce(), f32::INFINITY).foliage.into_bevy_mesh();
        let facets = leaf_cards(&hull);
        assert_eq!(facets.len(), 70);

        // Through the cache, as the systems read it: the cards are of the chips, not of the hull.
        let mut caches = FallCaches::default();
        let read = caches.cards(AssetId::default(), &hull, Some(7));
        let chips = read.mesh(&hull).clone();
        let cards = read.cards.clone();
        assert_eq!((chips.count_vertices(), cards.len()), (CHIPS * 3, CHIPS), "a chip is a card of three vertices");
        assert_ne!(chips.count_vertices(), hull.count_vertices(), "the cards are of the chips, not the hull");
        assert_eq!(hull_chips(&hull, 7).unwrap().attribute(Mesh::ATTRIBUTE_POSITION), chips.attribute(Mesh::ATTRIBUTE_POSITION));
        assert_ne!(hull_chips(&hull, 8).unwrap().attribute(Mesh::ATTRIBUTE_POSITION), chips.attribute(Mesh::ATTRIBUTE_POSITION));
        // A mesh that is not a hull is read as it is.
        let mut plain = FallCaches::default();
        let oak = leaf_mesh(TreePreset::Oak);
        assert_eq!(plain.cards(AssetId::default(), &oak, None).cards.len(), leaf_cards(&oak).len());

        // Every chip lies in the plane of a facet, inside the hull's box, and is a chip long.
        let (Some(VertexAttributeValues::Float32x3(positions)), Some(VertexAttributeValues::Float32x3(hull_positions))) =
            (chips.attribute(Mesh::ATTRIBUTE_POSITION), hull.attribute(Mesh::ATTRIBUTE_POSITION))
        else {
            panic!("no positions")
        };
        let hull_points: Vec<Vec3> = hull_positions.iter().map(|p| Vec3::from_array(*p)).collect();
        let (lo, hi) = hull_points.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        for tri in positions.as_chunks::<3>().0 {
            let [tip, left, right] = tri.map(Vec3::from_array);
            assert!(tip.cmpge(lo - 0.01).all() && tip.cmple(hi + 0.01).all(), "a chip off the hull at {tip}");
            let length = tip.distance((left + right) / 2.0);
            assert!((length - CHIP_LENGTH).abs() < 1.0e-3, "a chip {length} long");
        }

        // A chip falls only while its facet is on the tree: it has its facet's moment, so the fire
        // that burnt a facet away (`on_tree`'s front) burns away its chips, and one that burnt all
        // of them leaves none.
        let alive = |front: f32| facets.iter().map(|f| f.moment).filter(|&m| m >= front).collect::<Vec<f32>>();
        for front in [f32::NEG_INFINITY, 0.3, 0.6, 0.95, 1.0] {
            let alive = alive(front);
            for chip in cards.iter().filter(|c| c.on_tree(None, front)) {
                assert!(
                    alive.iter().any(|m| (m - chip.moment).abs() < 1.0e-3),
                    "front {front}: a chip of moment {} whose facet is burnt",
                    chip.moment
                );
            }
        }
        assert_eq!(cards.iter().filter(|c| c.on_tree(None, 1.0)).count(), 0, "a burnt crown sheds nothing");
        let some = cards.iter().filter(|c| c.on_tree(None, 0.5)).count();
        assert!((CHIPS / 4..CHIPS * 3 / 4).contains(&some), "{some} of {CHIPS} chips are on a half-burnt crown");
    }

    #[test]
    fn a_burst_holds_the_chosen_cards_falling_to_the_ground_from_where_they_hang() {
        let oak = leaf_mesh(TreePreset::Oak);
        let cards = leaf_cards(&oak);
        let chosen: Vec<&Card> = cards.iter().step_by(97).collect();
        let tree = GlobalTransform::from(Transform::from_xyz(4.0, 1.0, -3.0).with_scale(Vec3::splat(1.5)));
        let fall = Fall { now: 10.0, rest: 6.0, drift: Vec2::new(0.5, 0.0), units: 1.0 };
        let mut rng = SeededRng::new(3);
        let mut burst = BurstBuilder::new(tree.translation());
        burst.add(&oak, chosen.iter().copied(), &tree, |_| 1.0, fall, &mut rng, |_, rng| rng.range(0.0, LET_GO));
        let burst = burst.finish().expect("leaves above the ground");
        assert_eq!(burst.leaves as usize, chosen.len());
        let vertices: usize = chosen.iter().map(|card| card.vertices.len()).sum();
        assert_eq!(burst.mesh.count_vertices(), vertices);
        let Some(VertexAttributeValues::Float32x4(anchor)) = burst.mesh.attribute(ATTRIBUTE_FALL_ANCHOR) else {
            panic!("no anchors")
        };
        let Some(VertexAttributeValues::Float32x4(path)) = burst.mesh.attribute(ATTRIBUTE_FALL_PATH) else {
            panic!("no paths")
        };
        let Some(VertexAttributeValues::Float32x3(positions)) = burst.mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("no positions")
        };
        // Each card is where the tree holds it, relative to the tree; it lets go within the drop's
        // half second, lands on the ground (1 m, the tree's own height in burst space: 0) and stays
        // inside the bounds whether it hangs or lies there.
        let first = Vec3::from_array(positions[0]);
        let source = match oak.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => Vec3::from_array(p[chosen[0].vertices.start]),
            _ => unreachable!(),
        };
        assert!((first - (tree.transform_point(source) - tree.translation())).length() < 1.0e-4);
        for (a, p) in anchor.iter().zip(path) {
            assert!((10.0..10.0 + LET_GO).contains(&a[3]), "let go at {}", a[3]);
            assert_eq!(p[0], 0.0);
            assert!(SPEED.contains(&p[3]));
            let hangs = Vec3::new(a[0], a[1], a[2]);
            assert!(burst.aabb.min().cmple(hangs.into()).all() && burst.aabb.max().cmpge(hangs.into()).all());
        }
        assert!(burst.aabb.min().y <= 0.0, "the bounds reach the ground");
        let tallest = anchor.iter().map(|a| a[1]).fold(0.0, f32::max);
        assert!(burst.life > landing_time(tallest, SPEED.end) + 6.0 * 0.75);
        assert!(burst.life < LET_GO + landing_time(tallest, SPEED.start) + 6.0 * 1.25 + SHRINK + 1.0e-3);
    }

    #[test]
    fn leaves_fall_for_as_long_and_as_fast_in_a_world_of_another_unit() {
        let oak = leaf_mesh(TreePreset::Oak);
        let cards = leaf_cards(&oak);
        let chosen: Vec<&Card> = cards.iter().step_by(97).collect();
        // The same oak and the same leaves, in metres and in a world 3.2808 times as large (the
        // tree, the ground and the wind's drift scaled; the speeds come from the units).
        let burst_in = |k: f32| {
            let tree = GlobalTransform::from(Transform::from_xyz(4.0 * k, k, -3.0 * k).with_scale(Vec3::splat(1.5 * k)));
            let fall = Fall { now: 10.0, rest: 6.0, drift: Vec2::new(0.5 * k, 0.0), units: k };
            let mut rng = SeededRng::new(3);
            let mut burst = BurstBuilder::new(tree.translation());
            burst.add(&oak, chosen.iter().copied(), &tree, |_| k, fall, &mut rng, |_, rng| rng.range(0.0, LET_GO));
            burst.finish().expect("leaves above the ground")
        };
        let (metres, feet) = (burst_in(1.0), burst_in(3.2808));
        assert_eq!(metres.leaves, feet.leaves, "a leaf too close to the ground to fall in one world and not the other");
        assert!((metres.life - feet.life).abs() < 1.0e-3, "falls for {} s in metres, {} s in feet", metres.life, feet.life);
        let speeds = |burst: &Burst| match burst.mesh.attribute(ATTRIBUTE_FALL_PATH) {
            Some(VertexAttributeValues::Float32x4(path)) => path.iter().map(|p| p[3]).collect::<Vec<f32>>(),
            _ => panic!("no paths"),
        };
        for (m, f) in speeds(&metres).into_iter().zip(speeds(&feet)) {
            assert!(SPEED.contains(&m) && (f / m - 3.2808).abs() < 1.0e-4, "{m} m/s fell at {f} ft/s");
        }
    }

    fn app() -> bevy::app::App {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::time::TimeUpdateStrategy;
        let mut app = bevy::app::App::new();
        // `MinimalPlugins` leaves out transform propagation, which says where the trees stand.
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(crate::FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app
    }

    fn bursts(app: &mut bevy::app::App) -> Vec<LeafBurst> {
        app.world_mut().query::<&LeafBurst>().iter(app.world()).copied().collect()
    }

    #[test]
    fn a_shake_or_a_drop_makes_a_burst_from_the_trees_it_reaches_until_its_leaves_are_gone() {
        let mut app = app();
        let oak = TreePreset::Oak.params();
        let near = app.world_mut().spawn((ProceduralTree(oak.clone()), Transform::default())).id();
        let far = app.world_mut().spawn((ProceduralTree(oak), TreeAge(10.0), Transform::from_xyz(30.0, 0.0, 0.0))).id();
        app.update();
        app.update();
        assert!(bursts(&mut app).is_empty());

        // An axe on the near oak: the default dozen leaves, from it alone, and once.
        let time = *app.world().resource::<Time>();
        app.world_mut().resource_mut::<TreeShakes>().shake(TreeShake { at: Vec2::ZERO, ..Default::default() }, &time);
        app.update();
        app.update();
        let shaken = bursts(&mut app);
        assert_eq!(shaken.len(), 1);
        assert_eq!((shaken[0].tree, shaken[0].leaves), (Some(near), 12));

        // A drop takes what is left under the cap.
        app.world_mut().resource_mut::<FallingLeaves>().max_leaves = 20;
        app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree: far, count: 50 });
        app.update();
        let dropped: Vec<LeafBurst> = bursts(&mut app).into_iter().filter(|burst| burst.tree == Some(far)).collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].leaves, 8);

        // A tree whose leaf mesh changed in the frame it drops (it is younger now: fewer leaves)
        // drops from its new leaves, not from the cards of the old mesh.
        app.world_mut().resource_mut::<FallingLeaves>().max_leaves = 5000;
        app.world_mut().get_mut::<TreeAge>(far).unwrap().0 = 5.0;
        app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree: far, count: 100_000 });
        app.update();
        let young = {
            let world = app.world();
            let render = world.get::<TreeRender>(far).unwrap();
            let mesh = world.resource::<Assets<Mesh>>().get(&render.foliage.as_ref().unwrap().1).unwrap();
            on_tree(mesh, None)
        };
        let regrown: Vec<LeafBurst> = bursts(&mut app).into_iter().filter(|burst| burst.tree == Some(far)).collect();
        assert_eq!(regrown.len(), 2);
        assert_eq!(regrown.iter().map(|burst| burst.leaves).max(), Some(young as u32));

        // The bursts go once their leaves have fallen, lain and shrunk away.
        let gone_at = bursts(&mut app).iter().map(|burst| burst.gone_at).fold(0.0, f32::max);
        let now = app.world().resource::<Time>().elapsed_secs();
        assert!(gone_at - now > 6.0 * 0.75, "gone {:.1} s after the drop", gone_at - now);
        while app.world().resource::<Time>().elapsed_secs() < gone_at + 0.2 {
            app.update();
        }
        assert!(bursts(&mut app).is_empty());
    }

    #[test]
    fn a_burnt_tree_drops_none_of_the_leaves_the_fire_took() {
        use crate::fire::{FloraFire, burnt_front};
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::default())).id();
        let mut fire = FloraFire::with_cells(Vec2::ZERO, 32.0, 16);
        fire.scorch(Vec3::ZERO, 3.0, 0.8);
        app.insert_resource(fire);
        app.update();
        app.update();
        let (all, left) = {
            let world = app.world();
            let render = world.get::<TreeRender>(oak).unwrap();
            let mesh = world.resource::<Assets<Mesh>>().get(&render.foliage.as_ref().unwrap().1).unwrap();
            let burnt = burnt_front(Some(world.resource::<FloraFire>()), Vec3::ZERO);
            let cards = leaf_cards(mesh);
            (cards.iter().filter(|card| !card.flower).count(), cards.iter().filter(|card| card.on_tree(None, burnt)).count())
        };
        assert!(left > 0 && left < all / 2, "{left} of {all} leaves left at char 0.8");
        let drop_all = |app: &mut bevy::app::App| {
            app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree: oak, count: 100_000 });
            app.update();
            bursts(app).iter().map(|burst| burst.leaves).sum::<u32>()
        };
        assert_eq!(drop_all(&mut app), left as u32, "the leaves still on the tree, and only those");

        // Burnt through: nothing more falls.
        app.world_mut().resource_mut::<FloraFire>().scorch(Vec3::ZERO, 3.0, 1.0);
        assert_eq!(drop_all(&mut app), left as u32);
    }

    #[test]
    fn a_pooled_tree_drops_leaves_from_its_variant() {
        use crate::lod::LodSettings;
        use crate::pool::build_pool;
        let mut app = app();
        let pool = {
            let world = app.world_mut();
            let built = world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                let _ = world;
                build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut meshes)
            });
            world.resource_mut::<Assets<TreePool>>().add(built)
        };
        let tree = app.world_mut().spawn((PooledTree { pool, variant: 1 }, Transform::from_xyz(5.0, 0.0, 0.0))).id();
        for _ in 0..5 {
            app.update();
        }
        app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree, count: 15 });
        app.update();
        let dropped = bursts(&mut app);
        assert_eq!(dropped.len(), 1);
        assert_eq!((dropped[0].tree, dropped[0].leaves), (Some(tree), 15));
    }

    #[test]
    fn the_leaves_the_season_sheds_near_a_camera_fall_and_none_when_it_is_set_at_once() {
        let mut app = app();
        let oak = TreePreset::Oak.params();
        let near = app.world_mut().spawn((ProceduralTree(oak.clone()), Transform::default())).id();
        app.world_mut().spawn((ProceduralTree(oak), Transform::from_xyz(200.0, 0.0, 0.0)));
        app.world_mut().spawn((Camera3d::default(), Transform::from_xyz(0.0, 2.0, 12.0)));
        *app.world_mut().resource_mut::<FloraSeason>() = FloraSeason { autumn: 1.0, leaf_fall: 0.3, ..FloraSeason::SUMMER };
        app.update();
        app.update();
        let fallen = |app: &mut bevy::app::App| bursts(app).iter().map(|burst| burst.leaves).sum::<u32>();
        assert_eq!(fallen(&mut app), 0);

        // Leaf fall rises 0.3 -> 0.4 over a second: the near oak's leaves whose moment it passes
        // fall (the very ones the shaders stop drawing), from no one tree; the far oak's do not.
        for step in 1..=10 {
            app.world_mut().resource_mut::<FloraSeason>().leaf_fall = 0.3 + 0.01 * step as f32;
            app.update();
        }
        app.update();
        let expected = {
            let world = app.world();
            let render = world.get::<TreeRender>(near).unwrap();
            let mesh = world.resource::<Assets<Mesh>>().get(&render.foliage.as_ref().unwrap().1).unwrap();
            let to = app.world().resource::<FloraSeason>().leaf_fall;
            leaf_cards(mesh).iter().filter(|card| !card.flower && (0.3..to).contains(&card.moment)).count() as u32
        };
        assert!(expected > 20, "only {expected} leaves in a tenth of the season");
        assert_eq!(fallen(&mut app), expected);
        assert!(bursts(&mut app).iter().all(|burst| burst.tree.is_none()));

        // A season set at once (to bare) and spring (back to green) drop none.
        for leaf_fall in [0.9, 0.2] {
            app.world_mut().resource_mut::<FloraSeason>().leaf_fall = leaf_fall;
            app.update();
            app.update();
            assert_eq!(fallen(&mut app), expected, "at {leaf_fall}");
        }
    }
}
