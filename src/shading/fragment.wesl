// Fragment stage of a tree drawn with `FloraMaterial` or `GrowthMaterial` (main pass, forward
// only): Bevy's `pbr.wesl` with the leaf's surface and the season's snow before lighting and the
// light through the leaf after it. Wood (no FLORA_LEAF) is StandardMaterial's, under the snow.
import bevy_pbr::{
    render::{
        pbr_functions::alpha_discard,
        pbr_fragment::pbr_input_from_standard_material,
        forward_io::{VertexOutput, FragmentOutput},
        pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
        mesh_view_bindings::view,
    },
    decal::clustered::apply_decals,
};

@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@if(FLORA_DETAIL && !FLORA_LEAF)
import package::shading::common::bark_surface;

@if(FLORA_DETAIL && FLORA_LEAF)
import package::shading::common::leaf_detail;

@if(FLORA_TOON)
import package::shading::common::{toon_lighting, toon_rim};

@if(FLORA_LEAF)
import package::shading::common::{leaf_surface, hull_surface, leaf_transmission, FRUIT_ROUGHNESS, flora};

import package::shading::common::{snow_on, snow_shelter, wet_on, facing_normal, LEAF_FROST};

@if(FLORA_SEE_THROUGH)
import package::shading::common::see_through;

@if(FLORA_FIRE && !FLORA_LEAF)
import package::shading::common::{fire_at, wood_char, char_wood, ember_flicker, ember_glow, EMBER, EMBER_NITS};

@if(FLORA_FIRE && !FLORA_LEAF)
import bevy_pbr::render::mesh_view_bindings::globals;

@if(FLORA_FIRE && FLORA_LEAF)
import package::shading::common::{scorch_leaf, LEAF_FLAME, LEAF_FLAME_NITS};

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;

    // In the crossfade section of a visibility range, discard according to the dither pattern.
@if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);

    // A camera sees through the trees in front of what it follows (`SeeThroughTrees`).
@if(FLORA_SEE_THROUGH)
    see_through(in.world_position.xyz, in.position.xy, view.world_position);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );

@if(FLORA_DETAIL && !FLORA_LEAF)
    bark_surface(&pbr_input, in.uv, in.uv_b.x);

    // A flower (vertex alpha 0, `FoliageParams::blossom`) is a plain petal and a fruit (0.25,
    // `FoliageParams::fruit`) a plain round skin: no leaf scan, veins or leaf gloss. A fruit's skin
    // is smooth and lets no light through.
@if(FLORA_LEAF && VERTEX_COLORS)
    let flower = in.color.a < 0.5;
@if(FLORA_LEAF && VERTEX_COLORS)
    let fruit = in.color.a >= 0.125 && in.color.a < 0.5;
@if(FLORA_LEAF && !VERTEX_COLORS)
    let flower = false;
@if(FLORA_LEAF && !VERTEX_COLORS)
    let fruit = false;

@if(FLORA_DETAIL && FLORA_LEAF)
    if !flower {
        leaf_detail(&pbr_input, in.uv);
    }

@if(FLORA_LEAF)
    if !flower {
        if flora.tree.y > 0.5 {
            hull_surface(&pbr_input, in.uv_b);
        } else {
            leaf_surface(&pbr_input, in.uv, in.uv_b);
        }
    } else if fruit {
        pbr_input.material.perceptual_roughness = FRUIT_ROUGHNESS;
    }

    // The fire (`FloraFire`): wood chars from the ground up as the fire where it stands chars, and
    // its cracks glow while it burns; a leaf scorches brown and glows as it burns away (both from
    // the vertex stage, in its colour's alpha).
    var ember = vec3<f32>(0.0);
@if(FLORA_FIRE && !FLORA_LEAF) {
    let fire = fire_at(in.world_position.xyz);
    if fire.x > 0.0 {
        var h = 0.0;
@if(VERTEX_UVS_B)
        h = in.uv_b.y;
        var uv = vec2<f32>(0.0);
@if(VERTEX_UVS_A)
        uv = in.uv;
        let charred = wood_char(fire.x, h);
        let crack = char_wood(&pbr_input, charred, uv);
        let flicker = ember_flicker(in.world_position.xyz, globals.time);
        ember = EMBER * EMBER_NITS * ember_glow(fire.y) * charred * crack * flicker;
    }
}
@if(FLORA_FIRE && FLORA_LEAF && VERTEX_COLORS) {
    if !flower {
        scorch_leaf(&pbr_input, saturate(in.color.a - 1.0));
        ember = LEAF_FLAME * LEAF_FLAME_NITS * max(in.color.a - 2.0, 0.0);
    }
}

    // The rain (none when dry, which leaves the surface as it was): wet bark darkens and turns
    // glossy, a wet leaf's colour grows richer. Then the season's snow over it (none in summer): on
    // the side of a leaf card the camera sees, if that side faces up, and a dusting on every leaf,
    // as much as the leaf sees of the sky; on the wood, by its own normal. A leaf under snow lets
    // no light through.
@if(FLORA_LEAF)
    wet_on(&pbr_input, 1.0);
@else
    wet_on(&pbr_input, 0.0);
@if(FLORA_LEAF)
    let snowed = snow_on(
        &pbr_input,
        facing_normal(pbr_input.world_position.xyz, pbr_input.V),
        snow_shelter(in.uv_b.y),
        LEAF_FROST,
    );
@else
    let snowed = snow_on(&pbr_input, pbr_input.world_normal, 1.0, 0.0);

    apply_decals(&pbr_input);

    var out: FragmentOutput;
@if(FLORA_TOON)
    out.color = toon_lighting(pbr_input);
@else
    out.color = apply_pbr_lighting(pbr_input);
@if(FLORA_LEAF)
    // A hull crown's facets let no light through (`hull_surface`): only a leaf card does.
    if flora.tree.y < 0.5 {
        out.color = vec4<f32>(
            out.color.rgb + view.exposure * leaf_transmission(pbr_input, in.uv_b.y, flower) * select(1.0 - snowed, 0.0, fruit),
            out.color.a
        );
    }
@if(FLORA_TOON)
    out.color = vec4<f32>(out.color.rgb * toon_rim(pbr_input.world_normal, pbr_input.V), out.color.a);
    // Embers and burning leaves give light of their own, after the toon rim, which would darken it.
@if(FLORA_FIRE)
    out.color = vec4<f32>(out.color.rgb + view.exposure * ember, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
