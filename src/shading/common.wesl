// Shared by every flora shader: the wind, the flutter of a leaf, the surface of a leaf and the
// light that comes through it. The Rust side is `shading/mod.rs`.
//
// The wind follows Crysis' vegetation animation (GPU Gems 3, chapter 16): a main bend of the
// whole tree that grows with the square of the height and keeps every vertex's distance from the
// trunk, and per-leaf detail bending with smooth triangle waves of unrelated frequencies. The
// light through a leaf is Unreal's two-sided foliage model: the back side's wrapped N.L times a
// forward-scattering GGX lobe times the subsurface colour.
import bevy_pbr::render::{
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    pbr_lighting::D_GGX,
    pbr_ambient::ambient_light,
    shadows::fetch_directional_shadow,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    pbr_types::PbrInput,
};

struct FloraUniform {
    // 1 / trunk height, 0, 0, 0
    tree: vec4<f32>,
    // the species' sway, flutter, sway frequency, flutter frequency
    response: vec4<f32>,
    // translucency, roughness of the upper side, veins, underside
    leaf: vec4<f32>,
    // linear rgb of the light through a leaf, 0
    tint: vec4<f32>,
    // wood thinner than x metres is smooth, thicker than y metres wears the whole bark, 0, 0
    bark: vec4<f32>,
    // the bark scan's average linear rgb, 0
    bark_mean: vec4<f32>,
    // a turned leaf's colour multiplier (the autumn colour over the leaf colour), 1; zero for an
    // evergreen and for wood
    autumn: vec4<f32>,
    // a flower's colour multiplier (the blossom colour over the leaf colour), 1; zero for a
    // species without flowers and for wood
    blossom: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> flora: FloraUniform;

// The surface's detail textures (bark): colour, normal, and the sampler both share. Bound only
// when the material has them (`FLORA_DETAIL`, `FloraKey::detail`).
@if(FLORA_DETAIL)
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var detail_color: texture_2d<f32>;
@if(FLORA_DETAIL)
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var detail_normal: texture_2d<f32>;
@if(FLORA_DETAIL)
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var detail_sampler: sampler;

// The image of leaf veins (`FLORA_VENATION`, `FloraKey::venation`) and its own sampler.
@if(FLORA_VENATION)
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var venation: texture_2d<f32>;
@if(FLORA_VENATION)
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var venation_sampler: sampler;

// The weather, the same for every tree (`FloraWindBuffer`, `wind_vectors` in shading/mod.rs):
//   0: wind direction x, direction z, strength, speed
//   1: bend, flutter, gust strength, turbulence
//   2: gust frequency, flutter frequency, how wet the trees are (`FloraWetness`), 0
//   3: gust front origin x, origin z, direction x, direction z
//   4: gust front speed, width, strength, start time
//   5: the season (`FloraSeason`): how far the leaves have turned, the share fallen, snow, the
//      share of flowers out
//   6: the shakes (`TreeShakes`): the render time the last one ends (-1: none), 0, 0, 0
//   7 + 2i, 8 + 2i for i in 0..8: shake i's (at x, at z, radius, strength) and (direction x,
//      direction z, start time, duration); all zero for an empty slot
//   23..26: the see-through fade (`SeeThroughTrees`): the fading camera's position and the
//      clear radius at the target; the target and 1 (perspective) or 2 (orthographic) while a
//      camera has one; the camera's forward direction and the share kept; the feather's start,
//      where the cut ends before the target and the floor (all zero while none has)
//   27: the foliage pushers (`FoliagePusher`): see `PUSH`
//   60: the fire's header (`FloraFire`), then its grid: see `FIRE`
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage, read> flora_wind: array<vec4<f32>>;

// The see-through fade's slot (`see_through::SEE_THROUGH_SLOT`).
const SEE_THROUGH: u32 = 23u;

// The foliage pushers (`FoliagePusher`, push.rs), compiled in while one exists (`FLORA_PUSH`).
// Slot `PUSH` holds how many samples follow (x) and the circle across the ground everything they
// push lies in (centre x, z and radius: a vertex outside it reads no sample); each sample is two
// vectors: the middle of a body (x, y, z) and its radius, and half its height and its strength
// times the sample's weight. A body is sampled where it is and where it was over the last half
// second, ever weaker, so what it parted closes up behind it.
const PUSH: u32 = 27u;

// The share of a pusher's radius around its axis whose foliage spreads over its breadth: what is
// nearer still stays inside the body, which hides it, and a leaf across the axis stretches by
// at most 1.05 / `PUSH_CORE`.
const PUSH_CORE: f32 = 0.6;

// How far the vertex at `world` of a plant standing at `origin` gives way to the pushers: out of a
// body across the ground and a little down, as a pushed stem bends; `flex` of it (0 rigid wood to 1
// a leaf). What is nearer a body's axis than its radius goes out to between 1.05 and 1.25 radii,
// from its core (`PUSH_CORE` of the radius) out, keeping its order (no leaf folds over another),
// and the push eases off to nothing at 1.5 radii and past the body's ends. Where samples overlap
// their pushes are blended, the strongest most, so the push is continuous everywhere and a leaf
// bends rather than tears. A plant stays rooted: nothing in its lowest 15 cm moves, and the push
// grows over the next half metre.
fn push_offset(world: vec3<f32>, origin: vec3<f32>, flex: f32) -> vec3<f32> {
    let head = flora_wind[PUSH];
    let count = min(u32(head.x), 16u);
    if count == 0u || flex <= 0.0 || distance(world.xz, head.yz) > head.w {
        return vec3<f32>(0.0);
    }
    var total = vec3<f32>(0.0);
    var weights = 0.0;
    for (var i = 0u; i < count; i++) {
        let body = flora_wind[PUSH + 1u + 2u * i];
        let shape = flora_wind[PUSH + 2u + 2u * i];
        let r = body.w;
        let across = world.xz - body.xz;
        let d = length(across);
        if r <= 0.0 || d >= 1.5 * r {
            continue;
        }
        let beyond = abs(world.y - body.y) - shape.x;
        let along = 1.0 - smoothstep(0.0, 0.5 * r, beyond);
        // Where it goes: continuous through the axis and monotonic.
        var to = d + 0.5 * (1.5 * r - d);
        if d < PUSH_CORE * r {
            to = d * (1.05 / PUSH_CORE);
        } else if d < r {
            to = r * (1.05 + 0.2 * (d / r - PUSH_CORE) / (1.0 - PUSH_CORE));
        }
        let push = (to - d) * shape.y * along;
        if push > 0.0 {
            let way = select(vec2<f32>(1.0, 0.0), across / d, d > 1.0e-4);
            let weight = push * push * push;
            total += vec3<f32>(way.x, -0.3, way.y) * push * weight;
            weights += weight;
        }
    }
    if weights <= 0.0 {
        return vec3<f32>(0.0);
    }
    return total / weights * flex * smoothstep(0.15, 0.65, world.y - origin.y);
}

// How freely wood of radius `radius` (metres) gives way to a pusher: twigs fully, nothing from
// 5 cm up.
fn wood_flex(radius: f32) -> f32 {
    return 1.0 - smoothstep(0.012, 0.05, radius);
}

// Discard a tree's fragment at `world` (pixel `frag_coord`) when it stands between the fading
// camera and its target. Under a perspective camera the clear view is a cone around the line of
// sight, a third as wide at the camera as at the target; under an orthographic one (its rays are
// parallel) a cylinder along the camera's view through the target. Within `inner` of the radius
// the cut is whole, dithered over the rest, and `keep` of it stays; it ends before the target,
// over the last tenth of the way or `in_front` metres before it, so nothing behind the target is
// cut, and nothing below the floor is. Only the fading camera's own views cut: `view_position` is
// where the view being drawn looks from, so shadow maps and other cameras keep the whole tree.
// Compiled in only while a camera sees through (`FLORA_SEE_THROUGH`).
fn see_through(world: vec3<f32>, frag_coord: vec2<f32>, view_position: vec3<f32>) {
    let eye = flora_wind[SEE_THROUGH];
    // (`target` is a reserved word in WGSL.)
    let goal = flora_wind[SEE_THROUGH + 1u];
    if goal.w < 0.5 || any(abs(view_position - eye.xyz) > vec3<f32>(1.0e-3)) {
        return;
    }
    // The camera's forward direction and the share kept; the feather's start (a share of the
    // radius), where the cut ends before the target (< 0: over the last tenth of the way) and
    // the floor.
    let ahead = flora_wind[SEE_THROUGH + 2u];
    let shape = flora_wind[SEE_THROUGH + 3u];
    if world.y < shape.z {
        return;
    }
    var radius = eye.w;
    var off: f32;
    // How far before the target the fragment stands along the line of sight, and the camera's
    // distance to the target along it.
    var before: f32;
    var way: f32;
    if goal.w > 1.5 {
        let to = world - goal.xyz;
        let along = dot(to, ahead.xyz);
        before = -along;
        way = dot(goal.xyz - eye.xyz, ahead.xyz);
        off = length(to - ahead.xyz * along);
    } else {
        let line = goal.xyz - eye.xyz;
        let t = dot(world - eye.xyz, line) / max(dot(line, line), 1.0e-6);
        if t <= 0.0 {
            return;
        }
        way = length(line);
        before = (1.0 - t) * way;
        radius = eye.w * mix(1.0 / 3.0, 1.0, t);
        off = distance(world, eye.xyz + line * t);
    }
    if before <= 0.0 {
        return;
    }
    var close: f32;
    if shape.y < 0.0 {
        close = smoothstep(0.0, 0.1 * way, before);
    } else {
        close = smoothstep(shape.y, shape.y + 0.1 * eye.w, before);
    }
    let open = (1.0 - ahead.w) * (1.0 - smoothstep(shape.x * radius, radius, off)) * close;
    if open > ordered_dither(frag_coord) {
        discard;
    }
}

// A 4x4 ordered-dither threshold in (0, 1) for the pixel at `frag_coord`.
fn ordered_dither(frag_coord: vec2<f32>) -> f32 {
    var bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let p = vec2<u32>(frag_coord) % vec2<u32>(4u);
    return (bayer[p.y * 4u + p.x] + 0.5) / 16.0;
}

// A leaf's own moment in the season, 0..1, from its phase (`sway.w`, one per leaf): it turns
// around `autumn` = its moment and falls once `leaf fall` passes it, so the first to turn fall
// first. Spread so that the season's early part is a few leaves, not none.
fn leaf_season_moment(sway: vec4<f32>) -> f32 {
    return fract(sway.w * 7.31 + 0.137);
}

// Whether a deciduous tree's leaf has fallen this season: its vertex is then put outside the
// clip volume, so the leaf draws nothing, in the main pass and in the shadow maps alike.
fn leaf_has_fallen(sway: vec4<f32>) -> bool {
    return flora.autumn.w > 0.5 && leaf_season_moment(sway) < flora_wind[5].y;
}

// Where a fallen leaf's vertices go: past the right edge of the clip volume, so all three
// corners of each of its triangles are clipped.
const FALLEN_LEAF_CLIP: vec4<f32> = vec4<f32>(2.0, 2.0, 0.5, 1.0);

// What a leaf's vertex colour is multiplied by this season: 1 in summer, the autumn colour over
// the leaf colour (`flora.autumn.rgb`) once it has turned, a short blend while it turns.
fn leaf_autumn_tint(sway: vec4<f32>) -> vec3<f32> {
    let moment = leaf_season_moment(sway) * 0.8;
    let turn = smoothstep(moment, moment + 0.2, flora_wind[5].x) * flora.autumn.w;
    return mix(vec3<f32>(1.0), flora.autumn.rgb, turn);
}

// A flower card's vertex: its colour's alpha is 0, a fruit's 0.25, a leaf's 1
// (`FoliageParams::blossom`, `FoliageParams::fruit`).
fn is_flower(color: vec4<f32>) -> bool {
    return color.a < 0.125;
}

// A fruit's vertex (alpha 0.25, `FRUIT_ALPHA` in foliage.rs).
fn is_fruit(color: vec4<f32>) -> bool {
    return color.a >= 0.125 && color.a < 0.5;
}

// A flower's own moment in the blossom, 0..1, from its leaf's phase spread differently, so the
// flowers do not open in the order the leaves turn.
fn flower_moment(sway: vec4<f32>) -> f32 {
    return fract(sway.w * 3.71 + 0.53);
}

// How much of the leaf fall has passed when the last fruit drops: the fruit goes in its first two
// thirds, before the last leaves.
const FRUIT_FALL: f32 = 0.67;
// A fruit's skin: smooth, glossier than a leaf.
const FRUIT_ROUGHNESS: f32 = 0.35;

// Whether a leaf, a flower or a fruit draws nothing this season (it is then put outside the clip
// volume): a fallen leaf; a flower that is not out (flowers open once the blossom passes their
// moment and fall in the reverse order); a fruit while the blossom is out, once the leaf fall has
// passed its moment, on any tree (an evergreen's fruit drops too), or once picked (alpha 0.375,
// `TreeFruit::pick`).
fn leaf_hidden(sway: vec4<f32>, color: vec4<f32>) -> bool {
    if is_flower(color) {
        return flower_moment(sway) >= flora_wind[5].w;
    }
    if is_fruit(color) {
        return color.a > 0.3125 || flora_wind[5].w > 0.0 || leaf_season_moment(sway) * FRUIT_FALL < flora_wind[5].y;
    }
    return leaf_has_fallen(sway);
}

// What a leaf's, a flower's or a fruit's vertex colour is multiplied by this season: a flower's is
// the blossom colour over the leaf colour (`flora.blossom.rgb`), a fruit's 1 (its colour is in the
// vertex), a leaf's its autumn tint.
fn leaf_season_tint(sway: vec4<f32>, color: vec4<f32>) -> vec3<f32> {
    if is_flower(color) {
        return flora.blossom.rgb;
    }
    if is_fruit(color) {
        return vec3<f32>(1.0);
    }
    return leaf_autumn_tint(sway);
}

// The fire (`FloraFire`, fire.rs), compiled in while one exists (`FLORA_FIRE`). Slot `FIRE` holds
// the grid's lower corner x, z, 1 / the cell size and the cells per side (0 while no cell holds
// anything); `FIRE + 1` the world x, z rectangle (min x, min z, max x, max z) outside which every
// cell is empty. The cells follow two to a vector (char, heat, char, heat), row by row along x,
// rows along z.
const FIRE: u32 = 60u;

// The cell (x, z) of a grid `side` cells wide: (char, heat), nothing off the grid.
fn fire_cell(x: i32, z: i32, side: i32) -> vec2<f32> {
    if x < 0 || z < 0 || x >= side || z >= side {
        return vec2<f32>(0.0);
    }
    let i = u32(z * side + x);
    let pair = flora_wind[FIRE + 2u + (i >> 1u)];
    return select(pair.xy, pair.zw, (i & 1u) == 1u);
}

// How charred the trees at `world` are and how hot the fire there burns, 0..1 each: between the
// four nearest cell centres, nothing off the grid. `FloraFire::char_at` and `heat_at` are the same.
// A tree outside every touched cell reads two vectors and no cell.
fn fire_at(world: vec3<f32>) -> vec2<f32> {
    let head = flora_wind[FIRE];
    let side = i32(head.w);
    if side == 0 {
        return vec2<f32>(0.0);
    }
    let bounds = flora_wind[FIRE + 1u];
    if any(world.xz < bounds.xy) || any(world.xz > bounds.zw) {
        return vec2<f32>(0.0);
    }
    let g = (world.xz - head.xy) * head.z - 0.5;
    let base = floor(g);
    let f = g - base;
    let x = i32(base.x);
    let z = i32(base.y);
    let near = mix(fire_cell(x, z, side), fire_cell(x + 1, z, side), f.x);
    let far = mix(fire_cell(x, z + 1, side), fire_cell(x + 1, z + 1, side), f.x);
    return mix(near, far, f.y);
}

// How far ahead of the burning front (in the leaf's moment) a leaf has scorched brown, and how
// close to it a scorched leaf glows, burning (`fire::LEAF_SCORCH`, `fire::LEAF_GLOW`).
const LEAF_SCORCH: f32 = 0.35;
const LEAF_GLOW: f32 = 0.08;

// The burning front in leaf moments at a tree whose char is `burnt`: the leaves whose moment is
// below it have burnt away; none scorches without char, all have burnt at 1
// (`fire::leaf_burn_front`). A leaf burns at its moment in the season, so the fire takes the
// leaves in no order a viewer could follow.
fn leaf_burn_front(burnt: f32) -> f32 {
    let lead = LEAF_GLOW + LEAF_SCORCH + 0.02;
    return burnt * (1.0 + lead) - lead;
}

// Whether a leaf (or a flower) has burnt away in the fire `fire` at its tree: it then draws
// nothing, in the main pass and the shadow maps alike.
fn leaf_burnt(sway: vec4<f32>, fire: vec2<f32>) -> bool {
    return leaf_season_moment(sway) < leaf_burn_front(fire.x);
}

// The vertex alpha a leaf hands the fragment stage in a fire: 1 plus how scorched it is (0..1)
// plus how bright it burns (0..1, once fully scorched). All of a leaf's vertices share its moment,
// so the value is the same across it and survives interpolation. A flower or a fruit keeps its
// own (it burns away with no scorch).
fn leaf_fire_alpha(sway: vec4<f32>, color: vec4<f32>, fire: vec2<f32>) -> f32 {
    if color.a < 0.5 {
        return color.a;
    }
    let ahead = leaf_season_moment(sway) - leaf_burn_front(fire.x);
    let scorch = 1.0 - smoothstep(LEAF_GLOW, LEAF_GLOW + LEAF_SCORCH, ahead);
    let glow = (1.0 - smoothstep(0.0, LEAF_GLOW, ahead)) * fire.y;
    return color.a + scorch + glow;
}

// A scorched leaf's colour, and charcoal's (linear).
const SCORCHED: vec3<f32> = vec3<f32>(0.075, 0.036, 0.014);
const CHARCOAL: vec3<f32> = vec3<f32>(0.022, 0.019, 0.017);

// The light of embers and of a burning leaf: linear colour times luminance in nits, scaled by the
// view's exposure where it is added, so they glow faintly by day and brightly by night.
const EMBER: vec3<f32> = vec3<f32>(1.0, 0.24, 0.03);
const EMBER_NITS: f32 = 900.0;
const LEAF_FLAME: vec3<f32> = vec3<f32>(1.0, 0.27, 0.03);
const LEAF_FLAME_NITS: f32 = 1000.0;

// How bright embers glow at a heat (0..1): they dim fast as a fire cools.
fn ember_glow(heat: f32) -> f32 {
    return heat * heat;
}

// How charred a piece of wood `h` trunk heights up is (`h` from the vertex stage) where the char
// is `burnt`: from the ground up, the trunk by char 0.4, the crown's wood by 1.
fn wood_char(burnt: f32, h: f32) -> f32 {
    let reach = burnt * 2.6;
    return (1.0 - smoothstep(reach - 0.35, reach, h)) * smoothstep(0.0, 0.05, burnt);
}

// A flicker for embers at `world`, 0.35..1: slow and uneven, never the same in two places.
fn ember_flicker(world: vec3<f32>, time: f32) -> f32 {
    let a = sin(time * 3.1 + dot(world, vec3<f32>(2.7, 1.3, 1.9)));
    let b = sin(time * 7.7 + dot(world, vec3<f32>(-1.1, 4.3, 2.2)));
    return 0.35 + 0.65 * saturate(0.5 + 0.35 * a + 0.25 * b);
}

// A pseudo-random point in the unit square for the cell `p` of the char's crack pattern.
fn crack_point(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q) * 43758.547);
}

// The cracks of charred wood at the bark's `uv`: 1 on the seams between the blocks charcoal splits
// into, 0 inside them (the distance to the second-nearest point of a Voronoi pattern less the
// nearest's; blocks a little longer than wide).
fn char_cracks(uv: vec2<f32>) -> f32 {
    let p = uv * vec2<f32>(9.0, 5.0);
    let cell = floor(p);
    let f = p - cell;
    var nearest = 8.0;
    var second = 8.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let o = vec2<f32>(f32(i), f32(j));
            let d = length(o + crack_point(cell + o) - f);
            if d < nearest {
                second = nearest;
                nearest = d;
            } else if d < second {
                second = d;
            }
        }
    }
    return 1.0 - smoothstep(0.03, 0.12, second - nearest);
}

// Burn the surface of wood in place: `charred` (0..1) of it turns to charcoal, split into blocks
// whose seams (at the bark's `uv`) are darker still, and rougher. Returns how much of it is a
// seam, where embers glow.
fn char_wood(p: ptr<function, PbrInput>, charred: f32, uv: vec2<f32>) -> f32 {
    let crack = char_cracks(uv);
    let color = (*p).material.base_color.rgb;
    (*p).material.base_color = vec4<f32>(mix(color, CHARCOAL * (1.0 - 0.7 * crack), charred), (*p).material.base_color.a);
    (*p).material.perceptual_roughness = mix((*p).material.perceptual_roughness, 0.85, charred);
    // Charcoal is dull: little of the sky's or the sun's sheen.
    (*p).material.reflectance = (*p).material.reflectance * (1.0 - 0.75 * charred);
    return crack;
}

// Scorch a leaf's surface in place: `scorch` (0..1) of it turns brown and dull.
fn scorch_leaf(p: ptr<function, PbrInput>, scorch: f32) {
    let color = (*p).material.base_color.rgb;
    let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
    let shade = clamp(dot(color, weights) / 0.06, 0.5, 1.5);
    (*p).material.base_color = vec4<f32>(mix(color, SCORCHED * shade, scorch), (*p).material.base_color.a);
    (*p).material.perceptual_roughness = mix((*p).material.perceptual_roughness, 0.9, scorch);
}

// Fresh snow's colour: bright and a little blue.
const SNOW_ALBEDO: vec3<f32> = vec3<f32>(0.80, 0.83, 0.88);

// The dusting of snow a sky-facing leaf or needle holds whichever way it faces, as a share of the
// full cover: a cold hint on a snowy conifer's green undersides. Tiny, because the mix is in
// linear light: on a spruce branch seen from below a share of 0.08 already lifts its mean colour
// from sRGB (39, 62, 50) to (67, 80, 70) and 0.2 to (89, 96, 87), a grey-olive that reads as
// paint, not snow. The snow itself lies on the sides that face up.
const LEAF_FROST: f32 = 0.03;

// How much snow lies on a surface whose world normal is `n` (unit): none until
// `FloraSeason::snow` rises, first on what faces straight up and, with more snow, on steeper
// faces too (all of it from `n.y` 0.45 at full snow). Not the normal the detail maps perturbed:
// the cover would break up into the bark's crevices and the leaf scans' veins.
fn snow_cover(n: vec3<f32>) -> f32 {
    let snow = flora_wind[5].z;
    let edge = 0.85 - 0.6 * snow;
    return snow * smoothstep(edge, edge + 0.2, n.y);
}

// How much of the falling snow reaches a leaf that sees `sky` of the sky (its baked crown
// visibility): none deep in the crown, all of it on the outside. Snow on an inner card is lit only
// by what little light gets in, and a white surface there renders charcoal, not white.
fn snow_shelter(sky: f32) -> f32 {
    return smoothstep(0.35, 0.85, sky);
}

// Lay this season's snow on a surface: `n` is the normal snow sees (the geometric normal for wood,
// `facing_normal` for a leaf card), `shelter` how much of the falling snow reaches it (a leaf's
// share of the sky; 1 for wood), `frost` the share of the snow that lies on it whichever way it
// faces (needles and leaves hold a dusting; wood 0). Its colour turns to snow's and it goes matte.
// Returns how much lies there, for what must fade under it (the light through a leaf). Without
// snow the surface is exactly as it was.
fn snow_on(p: ptr<function, PbrInput>, n: vec3<f32>, shelter: f32, frost: f32) -> f32 {
    let cover = max(snow_cover(n), frost * flora_wind[5].z) * shelter;
    let base = (*p).material.base_color;
    (*p).material.base_color = vec4<f32>(mix(base.rgb, SNOW_ALBEDO, cover), base.a);
    (*p).material.perceptual_roughness = mix((*p).material.perceptual_roughness, 0.9, cover);
    return cover;
}

// A soaked surface (`FloraWetness`). Bark, porous, darkens to about half (`WET_WOOD_ALBEDO`), its
// colour deepens (its linear colour raised to `1 + WET_WOOD_DEEPEN`: darker and more saturated, as
// water in a surface makes it) and it turns glossy, down to `WET_WOOD_ROUGHNESS`: the dark wet
// bark is what says rain. A leaf, waxy, keeps its brightness and its roughness and only its
// colour grows richer (`WET_LEAF_SATURATE` further from its own grey): a darker or glossier leaf
// leaves the sky's and the sun's white sheen a bigger share of what it shows, and the crown
// greys, reading as frost or dust, not rain.
const WET_WOOD_ALBEDO: f32 = 0.55;
const WET_WOOD_DEEPEN: f32 = 0.15;
const WET_WOOD_ROUGHNESS: f32 = 0.5;
const WET_LEAF_SATURATE: f32 = 0.25;

// Wet a surface by the rain (`FloraWetness`): `leaf` is 1 for a leaf, 0 for wood (between for an
// impostor's texel, by its baked translucency). Dry, the surface is exactly as it was. Before the
// snow, which covers it.
fn wet_on(p: ptr<function, PbrInput>, leaf: f32) {
    let wet = flora_wind[2].z;
    // Dry: nothing to do (and `pow(x, 1)` need not give back `x` to the last bit).
    if wet <= 0.0 {
        return;
    }
    let base = (*p).material.base_color;
    let rgb = max(base.rgb, vec3<f32>(0.0));
    let bark = pow(rgb, vec3<f32>(1.0 + WET_WOOD_DEEPEN * wet)) * mix(1.0, WET_WOOD_ALBEDO, wet);
    let grey = vec3<f32>(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
    let foliage = max(grey + (rgb - grey) * (1.0 + WET_LEAF_SATURATE * wet), vec3<f32>(0.0));
    (*p).material.base_color = vec4<f32>(mix(bark, foliage, leaf), base.a);
    let rough = (*p).material.perceptual_roughness;
    (*p).material.perceptual_roughness = mix(rough, min(rough, WET_WOOD_ROUGHNESS), wet * (1.0 - leaf));
}

// The world normal of the flat face being drawn, on the side the camera sees. Snow on a leaf card
// follows the card, not its crown-bent shading normal (which would whiten the top of the crown and
// nothing below it): a tier seen from above is white, seen from below green.
fn facing_normal(world_position: vec3<f32>, V: vec3<f32>) -> vec3<f32> {
    let n = normalize(cross(dpdx(world_position), dpdy(world_position)));
    return select(-n, n, dot(n, V) >= 0.0);
}

fn smooth_curve(x: vec4<f32>) -> vec4<f32> {
    return x * x * (3.0 - 2.0 * x);
}

fn triangle_wave(x: vec4<f32>) -> vec4<f32> {
    return abs(fract(x + 0.5) * 2.0 - 1.0);
}

fn smooth_triangle_wave(x: vec4<f32>) -> vec4<f32> {
    return smooth_curve(triangle_wave(x));
}

// How far the top of a tree leans at full strength, per metre of trunk height.
const MAIN_BEND: f32 = 0.04;
// How far a leaf's tip flutters at full strength, per metre of its length.
const FLUTTER: f32 = 0.25;

// A phase of its own for each tree, from where it stands, so a forest does not sway in step.
fn tree_phase(origin: vec3<f32>) -> f32 {
    return fract(dot(origin, vec3<f32>(0.0713, 0.0, 0.0531)));
}

// How strong a gust front is at `origin`: a bump of the front's strength, `width` metres deep,
// that travels along the front's direction. 0 where there is no front.
fn gust_front(origin: vec3<f32>, time: f32) -> f32 {
    let front = flora_wind[3];
    let front_shape = flora_wind[4];
    if front_shape.z <= 0.0 {
        return 0.0;
    }
    let along = dot(origin.xz - front.xy, front.zw) - front_shape.x * (time - front_shape.w);
    let u = along / front_shape.y;
    return front_shape.z * exp(-u * u);
}

// How fast a shaken tree swings, in swings a second for a tree 1 m tall: slower for taller trees
// (by the square root of the height, as a pendulum), 1.2 a second for a 6 m tree.
const SHAKE_SWINGS: f32 = 3.0;
// How hard a shake makes the leaves rustle, per unit of its strength, on the scale of the wind.
const SHAKE_RUSTLE: f32 = 0.8;

// What the shakes under way (`TreeShakes`) do to the tree standing at `origin`: (x, z) the lean
// they give it now, and how hard they make its leaves rustle. Each swings the tree first away
// from where it happened (or along its direction), then back and forth, dying away to nothing at
// its end; the nearer the tree, the harder. Nothing, and one comparison, once the last shake is
// over (the header in slot 6).
fn shake_at(origin: vec3<f32>, time: f32) -> vec3<f32> {
    if time > flora_wind[6].x {
        return vec3<f32>(0.0);
    }
    let swings = SHAKE_SWINGS * sqrt(max(flora.tree.x, 1.0e-4)) * (0.9 + 0.2 * tree_phase(origin));
    var lean = vec2<f32>(0.0);
    var rustle = 0.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let place = flora_wind[7u + 2u * i];
        let how = flora_wind[8u + 2u * i];
        let age = time - how.z;
        if place.w <= 0.0 || age < 0.0 || age > how.w {
            continue;
        }
        let off = origin.xz - place.xy;
        let d = length(off);
        let near = 1.0 - smoothstep(0.0, place.z, d);
        if near <= 0.0 {
            continue;
        }
        var dir = how.xy;
        if dot(dir, dir) == 0.0 {
            dir = select(vec2<f32>(1.0, 0.0), off / d, d > 1.0e-4);
        }
        let fade = 1.0 - age / how.w;
        let amount = place.w * near * fade * fade;
        lean = lean + dir * (amount * sin(6.2831853 * swings * age));
        rustle = rustle + amount;
    }
    return vec3<f32>(lean, rustle);
}

// The leaf's movement in the tree's local space. `sway`: the leaf's direction scaled by how far
// the vertex is from where the leaf is attached (xyz) and the leaf's phase (w); `shake`:
// `shake_at(origin, time)`, which the caller works out once for both this and `wind_bend`.
fn flutter_offset(sway: vec4<f32>, origin: vec3<f32>, time: f32, shake: vec3<f32>) -> vec3<f32> {
    let wind = flora_wind[0];
    let motion = flora_wind[1];
    let rate = flora_wind[2];
    let t = time * wind.w * 2.0 * rate.y * flora.response.w + sway.w + tree_phase(origin);
    let w = smooth_triangle_wave(vec4<f32>(t * 1.975, t * 0.793, 0.0, 0.0));
    let strength = wind.z + gust_front(origin, time) + SHAKE_RUSTLE * shake.z;
    return sway.xyz * (FLUTTER * motion.y * flora.response.y * strength * (w.x + w.y - 1.0));
}

// `world` bent downwind with the tree. `local_y`: the vertex's height in the tree's own units,
// `scale_y`: the tree's scale along y, `shake`: `shake_at(origin, time)`.
fn wind_bend(world: vec3<f32>, local_y: f32, origin: vec3<f32>, scale_y: f32, time: f32, shake: vec3<f32>) -> vec3<f32> {
    let wind = flora_wind[0];
    let motion = flora_wind[1];
    let rate = flora_wind[2];
    let front = flora_wind[3];
    let bump = gust_front(origin, time);
    if wind.z <= 0.0 && bump <= 0.0 && shake.z <= 0.0 {
        return world;
    }
    let t = time * wind.w * rate.x * flora.response.z;
    let phase = tree_phase(origin);
    let g = smooth_triangle_wave(vec4<f32>(t * 0.193 + phase, t * 0.375 + 1.7 * phase, t * 0.27 + 2.3 * phase, 0.0));
    // 0.6 to 1.0 of the wind at the default gust strength of 0.4.
    let gust = 1.0 - motion.z + motion.z * 0.5 * (g.x + g.y);
    let down = wind.xy * (wind.z * gust);
    let across = vec2<f32>(-wind.y, wind.x) * (wind.z * motion.w * (2.0 * g.z - 1.0));
    let push = down + across + front.zw * bump;
    let h = max(local_y, 0.0) * flora.tree.x;
    // How far this vertex moves per unit of lean: the species' sway, more toward the top.
    let lean = MAIN_BEND * flora.response.x * (scale_y / max(flora.tree.x, 1.0e-4)) * h * h;
    let reach = lean * motion.x;
    let rel = world - origin;
    // A shake leans the tree whatever `FloraWind::bend` says: a still day has knocks too.
    let moved = rel + vec3<f32>(push.x, 0.0, push.y) * reach + vec3<f32>(shake.x, 0.0, shake.y) * lean;
    // Bend, do not stretch.
    return origin + moved * (length(rel) / max(length(moved), 1.0e-6));
}

fn sign_not_zero(v: vec2<f32>) -> vec2<f32> {
    return select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

// A unit vector as two numbers in -1..1: the octahedral mapping.
fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        return (1.0 - abs(p.yx)) * sign_not_zero(p);
    }
    return p;
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var v = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if v.z < 0.0 {
        let folded = (1.0 - abs(v.yx)) * sign_not_zero(v.xy);
        v = vec3<f32>(folded.x, folded.y, v.z);
    }
    return normalize(v);
}

// The light a surface lets through from the far side of `N`: every directional light that shines
// on the back, scattered toward the viewer. `through`: the colour the light takes (the leaf's
// colour, its tint and its translucency). `visibility`: how much of the sky the leaf sees, for the
// ambient light that comes through. Point and spot lights do not shine through.
fn transmitted_light(
    P: vec4<f32>,
    world_normal: vec3<f32>,
    N: vec3<f32>,
    V: vec3<f32>,
    frag_coord: vec4<f32>,
    mesh_flags: u32,
    shadow_factor: f32,
    through: vec3<f32>,
    visibility: f32,
) -> vec3<f32> {
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), P);

    var sum = vec3<f32>(0.0);
    let n_directional_lights = view_bindings::lights.n_directional_lights;
    for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        let L = (*light).direction_to_light;
        let wrap = saturate((-dot(N, L) + 0.5) / 2.25);
        let scatter = D_GGX(0.6, saturate(-dot(V, L)));
        var shadow = 1.0;
        if (mesh_flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            // The shadow of what is in front of the light, seen from the back of the leaf.
            shadow = fetch_directional_shadow(i, P, -world_normal, view_z, frag_coord.xy) * shadow_factor;
        }
        sum = sum + (*light).color.rgb * (wrap * scatter * shadow);
    }
    return sum * through
        + ambient_light(P, -N, -V, 1.0, through, vec3<f32>(0.0), 1.0, vec3<f32>(visibility));
}

// How much of a leaf's specular reflection to keep, seen along `V` (from the surface toward the
// camera): all of it unless the camera looks toward a directional light, none when it looks
// straight into it. A leaf card stands for a leaf, or a cluster of needles, whose many small
// surfaces would scatter the sun's glint; a flat card seen against the sun reflects all of it
// into the camera at once, and the crown's top turns white, as if it were snow or dead. Facing
// the sun, a crown glows green through its leaves (`transmitted_light`) instead. Lit from the
// side or from behind the camera nothing changes.
fn leaf_gloss_toward_light(V: vec3<f32>) -> f32 {
    var toward = 0.0;
    for (var i: u32 = 0u; i < view_bindings::lights.n_directional_lights; i = i + 1u) {
        toward = max(toward, -dot(V, view_bindings::lights.directional_lights[i].direction_to_light));
    }
    return 1.0 - smoothstep(0.4, 0.8, toward);
}

// The surface of a leaf card, in place: veins, hue of the underside, gloss, crown occlusion.
// `uv`: the card's own (0..1 across and from base to tip), `shade`: x is 1 on the upper side,
// y is how much of the sky the leaf sees. The veins are the image of `FLORA_VENATION` (grown by
// space colonisation, `venation.rs`), as strong as `flora.leaf.z` says; a card cut to a scanned
// leaf has its veins in the scan, and a material with no image draws none.
fn leaf_surface(p: ptr<function, PbrInput>, uv: vec2<f32>, shade: vec2<f32>) {
    var color = (*p).material.base_color.rgb;
@if(FLORA_VENATION)
    let vein = textureSample(venation, venation_sampler, uv).r * flora.leaf.z;
@if(FLORA_VENATION)
    color = mix(color, color * vec3<f32>(1.35, 1.3, 0.75), vein) * mix(0.92, 1.05, uv.y);

    // The lower side is paler and less saturated.
    let upper = shade.x > 0.5;
    if !upper {
        let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        color = mix(color, vec3<f32>(luminance), 0.25) * (1.0 + flora.leaf.w);
    }

    (*p).material.base_color = vec4<f32>(color, (*p).material.base_color.a);
    (*p).material.perceptual_roughness = select(min(flora.leaf.y + 0.3, 1.0), flora.leaf.y, upper);
    (*p).material.reflectance = (*p).material.reflectance * leaf_gloss_toward_light((*p).V);
    (*p).diffuse_occlusion = (*p).diffuse_occlusion * shade.y;
    (*p).specular_occlusion = (*p).specular_occlusion * shade.y;
}

// How rough a hull crown's facets are (`CrownStyle::Hull`, `flora.tree.y`): matte, as flat paper.
const HULL_ROUGHNESS: f32 = 0.85;

// The surface of a facet of a hull crown, in place, instead of `leaf_surface`: matte, with no veins,
// no paler underside and no gloss to flare against the sun, since a facet is a plane of the
// painted crown, not a leaf. `shade` is the facet's `uv1` (y: how much of the sky it sees).
fn hull_surface(p: ptr<function, PbrInput>, shade: vec2<f32>) {
    (*p).material.perceptual_roughness = HULL_ROUGHNESS;
    (*p).diffuse_occlusion = (*p).diffuse_occlusion * shade.y;
    (*p).specular_occlusion = (*p).specular_occlusion * shade.y;
}

// The light a leaf lets through, tinted by the species' leaves (`flora.tint`); a flower's petals
// let through their own colour.
fn leaf_transmission(p: PbrInput, visibility: f32, flower: bool) -> vec3<f32> {
    let tint = select(flora.tint.rgb, vec3<f32>(1.0), flower);
    return transmitted_light(
        p.world_position,
        p.world_normal,
        p.N,
        p.V,
        p.frag_coord,
        p.flags,
        p.directional_shadow_factor,
        p.material.base_color.rgb * tint * flora.leaf.x,
        visibility,
    );
}

// Toon lighting (`RenderStyle::Toon`, `FLORA_TOON` or the impostor's `params.y`), in place of
// `apply_pbr_lighting`: each directional light's N.L is cut into three bands (0.15 shaded, 0.55
// middle from N.L > 0, 1.0 lit from N.L > 0.5), times its colour, its shadow and the surface's
// colour, plus the ambient light, all on the surface's own (geometric, painterly) normal. Point and
// spot lights do not light a toon. Returns the exposed colour `apply_pbr_lighting` would.
const TOON_PI: f32 = 3.14159265358979;

fn toon_lighting(p: PbrInput) -> vec4<f32> {
    // The geometric normal, not the one the detail maps perturbed: the bands would break up into
    // the leaf scan's veins and the bark's crevices.
    let n = p.world_normal;
    let base = p.material.base_color;
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), p.world_position);

    var sum = vec3<f32>(0.0);
    let n_directional_lights = view_bindings::lights.n_directional_lights;
    for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        let n_dot_l = dot(n, (*light).direction_to_light);
        let band = select(select(0.15, 0.55, n_dot_l > 0.0), 1.0, n_dot_l > 0.5);
        var shadow = p.directional_shadow_factor;
        if (p.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadow * fetch_directional_shadow(i, p.world_position, p.world_normal, view_z, p.frag_coord.xy);
        }
        sum = sum + (*light).color.rgb * (band * shadow);
    }
    let ambient = ambient_light(
        p.world_position, n, p.V, saturate(dot(n, p.V)), base.rgb, vec3<f32>(0.0), 1.0, p.diffuse_occlusion
    );
    return vec4<f32>(view_bindings::view.exposure * (base.rgb * sum / TOON_PI + ambient), base.a);
}

// The toon's ink: 1 where the surface faces the camera, darker (0.35) toward its silhouette.
fn toon_rim(N: vec3<f32>, V: vec3<f32>) -> f32 {
    return mix(0.35, 1.0, smoothstep(0.05, 0.3, abs(dot(N, V))));
}

// Schüler's cotangent frame ("Followup: Normal Mapping Without Precomputed Tangents", 2006): the
// tangent and bitangent of `uv` at `p` from screen-space derivatives (the meshes carry no
// tangents), scaled to the same length. Columns: T (along u), B (along v), N.
@if(FLORA_DETAIL)
fn cotangent_frame(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>) -> mat3x3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let inv_max = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1.0e-30));
    return mat3x3<f32>(t * inv_max, b * inv_max, n);
}

// A scan's colour and normal map on a surface, in place, faded in by `fade` (0..1). The colour
// is a ratio to the scan's average, which the material's base colour has divided out so the
// surface keeps its colour: with no fade the surface is `plain` (the scan's average for bark)
// times the base colour, with all of it the scan's own colour. `uv`: the mesh's, in tiles of
// the scan (bark) or the atlas coordinates of a scanned leaf.
@if(FLORA_DETAIL)
fn detail_surface(p: ptr<function, PbrInput>, uv: vec2<f32>, fade: f32, plain: vec3<f32>) {
    // Derivatives and samples before any branch.
    let n = (*p).N;
    let frame = cotangent_frame(n, (*p).world_position.xyz, uv);
    let albedo = textureSample(detail_color, detail_sampler, uv).rgb;
    let tangent_normal = textureSample(detail_normal, detail_sampler, uv).xyz * 2.0 - 1.0;
    // The map is OpenGL's (green points up the image) and v runs down it, so B is flipped.
    let bumped = frame * vec3<f32>(tangent_normal.x, -tangent_normal.y, tangent_normal.z);
    let bumped_length = length(bumped);
    let perturbed = select(n, bumped / max(bumped_length, 1.0e-6), bumped_length > 1.0e-4);
    // The relief fades out toward the silhouette: there the bumped normal turns away from the
    // camera and the light, and the crevices turn to black speckle.
    let facing = smoothstep(0.05, 0.4, dot(n, (*p).V));
    (*p).N = normalize(mix(n, perturbed, fade * facing));
    let base = (*p).material.base_color;
    (*p).material.base_color = vec4<f32>(base.rgb * mix(plain, albedo, fade), base.a);
}

// The bark of a piece of wood, in place, faded in with the wood's radius: smooth below
// `flora.bark.x` metres, the whole scan above `flora.bark.y`. `uv`: the mesh's, in tiles of the
// scan; `radius`: the vertex's, in metres.
@if(FLORA_DETAIL)
fn bark_surface(p: ptr<function, PbrInput>, uv: vec2<f32>, radius: f32) {
    let fade = smoothstep(flora.bark.x, max(flora.bark.y, flora.bark.x + 1.0e-4), radius);
    detail_surface(p, uv, fade, flora.bark_mean.rgb);
}

// A leaf card cut to a scanned leaf wears the scan whole.
@if(FLORA_DETAIL)
fn leaf_detail(p: ptr<function, PbrInput>, uv: vec2<f32>) {
    detail_surface(p, uv, 1.0, vec3<f32>(1.0));
}
