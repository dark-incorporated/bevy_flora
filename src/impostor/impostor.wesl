// A far tree: one quad that turns about the trunk to face the camera and shows
// the baked view nearest to where the camera looks from, lit by the scene's own lights. The Rust
// side is `impostor/mod.rs` (the quad, the atlases) and `impostor/material.rs`.
//
// The quad is built in the tree's local space with the picture's frame: x across,
// y up, z toward the viewer. `world_from_local` carries the tree's position, yaw
// and size; only the position and the size are used to place it, the yaw to pick
// the view and to turn the baked normals back into the world.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{view, globals},
    pbr_types,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, calculate_view},
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
};
import package::shading::common::{
    flora, flora_wind, wind_bend, shake_at, oct_decode, transmitted_light, leaf_gloss_toward_light, toon_lighting, toon_rim,
    snow_on, snow_shelter, wet_on, LEAF_FROST,
};

@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@if(FLORA_SEE_THROUGH)
import package::shading::common::see_through;

@if(FLORA_FIRE)
import package::shading::common::{
    fire_at, leaf_burn_front, ember_flicker, ember_glow, LEAF_GLOW, LEAF_SCORCH, SCORCHED, CHARCOAL, EMBER, EMBER_NITS,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var albedo: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var surface: texture_2d<f32>;
// x: views in the atlas, y: the render style, w: 1 for an unlit look.
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<uniform> params: vec4<f32>;
struct ImpostorTint {
    wood: vec4<f32>,
    leaf: vec4<f32>,
}
// Twice the look's linear colour of the wood and of the leaves.
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var<uniform> look: ImpostorTint;

const TAU: f32 = 6.283185307179586;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let origin = world_from_local[3].xyz;
    let scale_x = length(world_from_local[0].xyz);
    let scale_y = length(world_from_local[1].xyz);
    let scale_z = length(world_from_local[2].xyz);

    // Toward the camera, along the ground.
    let to_camera = view.world_position - origin;
    var flat_to_camera = vec3<f32>(to_camera.x, 0.0, to_camera.z);
    if dot(flat_to_camera, flat_to_camera) < 1.0e-8 {
        flat_to_camera = vec3<f32>(0.0, 0.0, 1.0);
    }
    flat_to_camera = normalize(flat_to_camera);

    // The quad's right: up cross toward-the-camera, the camera's own right.
    let right = vec3<f32>(flat_to_camera.z, 0.0, -flat_to_camera.x);
    let world_position = origin
        + right * (vertex.position.x * scale_x)
        + vec3<f32>(0.0, 1.0, 0.0) * (vertex.position.y * scale_y);

    // The view: the yaw of the camera in the tree's own frame, to the nearest
    // of the baked ones (view k looks from the local +Z turned k / views of a
    // turn about Y).
    let local_x = dot(to_camera, world_from_local[0].xyz) / (scale_x * scale_x);
    let local_z = dot(to_camera, world_from_local[2].xyz) / (scale_z * scale_z);
    let views = params.x;
    let yaw = atan2(local_x, local_z);
    let view_index = floor(yaw / TAU * views + 0.5);
    let tile = view_index - views * floor(view_index / views);

    // The tree sways with the wind, and swings when shaken, like its meshes do.
    let bent = wind_bend(world_position, vertex.position.y, origin, scale_y, globals.time, shake_at(origin, globals.time));
    out.world_position = vec4<f32>(bent, 1.0);
    out.position = position_world_to_clip(bent);
    // The tree's own +Z in the world (its yaw): what the baked normals are relative to.
    out.world_normal = normalize(vec3<f32>(world_from_local[2].x, 0.0, world_from_local[2].z));
@if(VERTEX_UVS_A)
    out.uv = vec2<f32>((tile + vertex.uv.x) / views, vertex.uv.y);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);

    return out;
}

// How much of a leafy texel a crown's open flowers cover, on a bare tree: a blossoming crown is
// a little more open than a leafy one.
const BLOSSOM_COVER: f32 = 0.8;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
@if(FLORA_SEE_THROUGH)
    see_through(in.world_position.xyz, in.position.xy, view.world_position);

    var color = textureSample(albedo, atlas_sampler, in.uv);
    let baked = textureSample(surface, atlas_sampler, in.uv);
    // The picture is neutral grey (0.5): the look's colour, twice over, multiplies it into the
    // wood's or the leaves' (baked translucency 1; wood 0), before the season recolours the leaves.
    color = vec4<f32>(color.rgb * select(look.wood.rgb, look.leaf.rgb, baked.a > 0.5), color.a);
    // The season (`FloraSeason`): the baked picture is summer. A deciduous tree's leaves (baked
    // translucency 1, wood 0) turn their autumn colour in patches a few texels wide, each at its
    // own moment, and fallen leaves thin the crown's coverage, so a bare tree keeps its wood.
    let season = flora_wind[5];
    let leafy = saturate(baked.a) * flora.autumn.w;
    let spot = in.uv * vec2<f32>(textureDimensions(albedo)) / 3.0;
    let moment = fract(sin(dot(floor(spot), vec2<f32>(12.9898, 78.233))) * 43758.547) * 0.8;
    // Where one pixel spans several patches, one patch's moment would flicker as the view moves:
    // take the share of the patches that have turned (their moments are spread evenly) instead.
    let spread = saturate(max(fwidth(spot.x), fwidth(spot.y)) - 0.5);
    let turned = mix(smoothstep(moment, moment + 0.2, season.x), saturate((season.x - 0.1) / 0.8), spread);
    let turn = turned * leafy;
    let summer = color;
    color = vec4<f32>(color.rgb * mix(vec3<f32>(1.0), flora.autumn.rgb, turn), color.a * (1.0 - season.y * leafy));
    // A flowering tree (`FoliageParams::blossom`) wears its flowers in patches as they open, over
    // its leaves or on the bare crown in early spring; the picture was baked without them.
    let flower_spot = fract(sin(dot(floor(spot), vec2<f32>(39.346, 11.135))) * 24634.634);
    let bloomed = mix(step(flower_spot, season.w), season.w, spread) * saturate(baked.a) * flora.blossom.w;
    var flower_share = 0.0;
    if bloomed > 0.0 {
        let leaves = 1.0 - season.y * leafy;
        flower_share = bloomed / max(bloomed + leaves, 1.0e-3);
        color = vec4<f32>(
            mix(color.rgb, summer.rgb * flora.blossom.rgb, flower_share),
            max(color.a, summer.a * bloomed * BLOSSOM_COVER),
        );
    }
    // The fire (`FloraFire`), as on the meshes: the leaves scorch and burn away in patches (each at
    // its own moment, the share of them where a pixel spans several), the wood chars, and it glows
    // while it burns.
    var ember = vec3<f32>(0.0);
@if(FLORA_FIRE) {
    let fire = fire_at(in.world_position.xyz);
    if fire.x > 0.0 {
        let leaves = saturate(baked.a);
        let front = leaf_burn_front(fire.x);
        let ahead = fract(sin(dot(floor(spot), vec2<f32>(63.7264, 10.873))) * 43758.547) - front;
        let burnt = mix(step(ahead, 0.0), saturate(front), spread);
        let scorched = mix(
            1.0 - smoothstep(LEAF_GLOW, LEAF_GLOW + LEAF_SCORCH, ahead),
            saturate((front + LEAF_GLOW + LEAF_SCORCH) / (LEAF_GLOW + LEAF_SCORCH)),
            spread,
        );
        let charred = smoothstep(0.0, 0.4, fire.x) * (1.0 - leaves);
        let rgb = mix(color.rgb, SCORCHED, scorched * leaves);
        color = vec4<f32>(mix(rgb, CHARCOAL, charred), color.a * (1.0 - burnt * leaves));
        // The meshes glow in their char's seams, about a tenth of the wood: as much on average.
        let flicker = ember_flicker(in.world_position.xyz, globals.time);
        ember = EMBER * EMBER_NITS * 0.1 * ember_glow(fire.y) * charred * flicker;
    }
}
    // Alpha is the share of the texel the tree covers, averaged down the mipmaps. With MSAA it
    // becomes the share of the pixel's samples drawn (alpha to coverage), so a far crown is as
    // airy as the leaves it was baked from: the sky shows through its gaps in the same
    // proportion. It is not sharpened the way Bevy's materials sharpen it, which would turn a
    // far mip's partial coverage back into all or nothing. Without MSAA it is cut at one half.
@if(ALPHA_TO_COVERAGE)
    if color.a < 1.0 / 255.0 {
        discard;
    }
@else
    if color.a < 0.5 {
        discard;
    }

    // The baked normal is in the tree's frame: x across (its local +X in the world), y up,
    // z along its local +Z.
    let n = oct_decode(baked.rg * 2.0 - 1.0);
    let z = normalize(in.world_normal);
    let x = vec3<f32>(z.z, 0.0, -z.x);
    let N = normalize(x * n.x + vec3<f32>(0.0, 1.0, 0.0) * n.y + z * n.z);

    var p = pbr_types::pbr_input_new();
    p.material.base_color = vec4<f32>(color.rgb, 1.0);
    p.material.perceptual_roughness = 0.9;
    p.material.flags = p.material.flags | pbr_types::STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT;
    p.frag_coord = in.position;
    p.world_position = in.world_position;
    p.world_normal = N;
    p.N = N;
    p.is_orthographic = view.clip_from_view[3].w == 1.0;
    p.V = calculate_view(in.world_position, p.is_orthographic);
    // Leaves (baked translucency 1; wood 0) lose their gloss facing the sun as the meshes' do.
    p.material.reflectance = p.material.reflectance * mix(1.0, leaf_gloss_toward_light(p.V), saturate(baked.a));
    p.flags = MESH_FLAGS_SHADOW_RECEIVER_BIT;
    // What the crown's own leaves keep from the sky and the sun.
    p.diffuse_occlusion = vec3<f32>(baked.b);
    p.directional_shadow_factor = baked.b;
    // The rain, wood or leaf by the texel's baked translucency (wood 0; a leaf's own translucency,
    // below 1, so anything above a trace is a leaf), then the season's snow on what faces up, by
    // the baked (crown-bent) normal, as much as the texel sees of the sky (wood: 1), as the meshes
    // take them.
    wet_on(&p, saturate(baked.a * 20.0));
    let snowed = snow_on(&p, N, snow_shelter(baked.b), LEAF_FROST * saturate(baked.a));

    var out: FragmentOutput;
    if params.w > 0.5 {
        // An unlit look (`PoolColors::unlit`): the colour as the weather and the fire left it.
        out.color = vec4<f32>(p.material.base_color.rgb, color.a);
    } else {
        // The material's `params.y` is the render style: 2 is toon.
        let toon = params.y > 1.5;
        if toon {
            out.color = toon_lighting(p);
        } else {
            out.color = apply_pbr_lighting(p);
        }
        // Light through a flower takes its own colour, as on the meshes (`leaf_transmission`).
        let tint = mix(flora.tint.rgb, vec3<f32>(1.0), flower_share);
        let through = color.rgb * tint * flora.leaf.x * baked.a * (1.0 - snowed);
        out.color = vec4<f32>(
            out.color.rgb + view.exposure * transmitted_light(
                p.world_position, N, N, p.V, p.frag_coord, p.flags, baked.b, through, baked.b
            ),
            color.a,
        );
        if toon {
            out.color = vec4<f32>(out.color.rgb * toon_rim(N, p.V), out.color.a);
        }
    }
@if(FLORA_FIRE)
    out.color = vec4<f32>(out.color.rgb + view.exposure * ember, out.color.a);
    out.color = main_pass_post_lighting_processing(p, out.color);
    return out;
}
