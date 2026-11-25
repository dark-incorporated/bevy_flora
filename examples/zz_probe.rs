//! THROWAWAY probe: the guide snippet, sanitized() old vs new, build_tree times.
use bevy::color::Color;
use bevy_flora::{CrownStyle, FruitParams, ParamError, TreeParams, TreePreset, TrunkStyle, build_tree};
use std::hint::black_box;
use std::time::Instant;

/// `value` inside `lo..=hi`, or `default` (also inside it) when it is `NaN`. `f32::clamp` alone returns
/// `NaN` for `NaN`.
fn clean(value: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if value.is_nan() { default.clamp(lo, hi) } else { value.clamp(lo, hi) }
}

/// Whether `color` has four finite channels.
fn finite(color: Color) -> bool {
    // Every colour space converts a `NaN` channel to a `NaN`; sRGB is the one that costs nothing for
    // the colours of the presets.
    let c = color.to_srgba();
    c.red.is_finite() && c.green.is_finite() && c.blue.is_finite() && c.alpha.is_finite()
}

/// Applies the table below to a copy of the parameters and notes whether anything moved.
struct Limits {
    changed: bool,
}

impl Limits {
    /// A float field: `NaN` reads as `default`, anything else is held in `lo..=hi`. `-0.0` is left alone.
    fn float(&mut self, field: &mut f32, default: f32, lo: f32, hi: f32) {
        let new = clean(*field, default, lo, hi);
        // `NaN != x` is true, so a `NaN` always counts as moved; `-0.0 == 0.0`, so it never does.
        if new != *field {
            *field = new;
            self.changed = true;
        }
    }

    /// A count: held in `lo..=hi`.
    fn count(&mut self, field: &mut u32, lo: u32, hi: u32) {
        let new = (*field).clamp(lo, hi);
        if new != *field {
            *field = new;
            self.changed = true;
        }
    }

    /// A colour with a `NaN` or infinite channel reads as `default`.
    fn color(&mut self, field: &mut Color, default: Color) {
        if !finite(*field) {
            *field = default;
            self.changed = true;
        }
    }

    /// An optional colour with a `NaN` or infinite channel is `None`, the default of every one.
    fn optional_color(&mut self, field: &mut Option<Color>) {
        if field.is_some_and(|color| !finite(color)) {
            *field = None;
            self.changed = true;
        }
    }
}

    fn old_apply(p: &mut TreeParams) -> bool {
        let default = TreeParams::default();
        let d = &default;
        let mut l = Limits { changed: false };

        // Trunk: metres, fractions of its height, counts and degrees.
        l.float(&mut p.trunk.height, d.trunk.height, 0.0, 500.0);
        l.float(&mut p.trunk.radius, d.trunk.radius, 0.0, 50.0);
        l.count(&mut p.trunk.radial_segments, 3, 64);
        l.count(&mut p.trunk.height_segments, 1, 256);
        l.float(&mut p.trunk.taper, d.trunk.taper, 0.0, 4.0);
        // An exponent: `t^-1` is infinite at the base.
        l.float(&mut p.trunk.taper_curve, d.trunk.taper_curve, 0.0, 16.0);
        l.float(&mut p.trunk.flare, d.trunk.flare, 0.0, 16.0);
        l.float(&mut p.trunk.randomness, d.trunk.randomness, 0.0, 8.0);
        l.count(&mut p.trunk.root_flare_count, 0, 64);
        l.float(&mut p.trunk.root_flare_spread, d.trunk.root_flare_spread, 0.0, 16.0);
        l.float(&mut p.trunk.root_flare_height, d.trunk.root_flare_height, 0.0, 1.0);
        l.float(&mut p.trunk.leader_length, d.trunk.leader_length, 0.0, 8.0);
        l.float(&mut p.trunk.leader_taper, d.trunk.leader_taper, 0.0, 4.0);
        l.float(&mut p.trunk.twist, d.trunk.twist, -7200.0, 7200.0);

        // Branches. The zone is a fraction of the trunk's height: below 0 is wood under the ground.
        let b = &mut p.branches;
        let db = &d.branches;
        l.float(&mut b.start, db.start, 0.0, 1.0);
        l.float(&mut b.end, db.end, 0.0, 1.0);
        // Per metre of trunk: the fern's 6 cm stem carries 300. The nodes are capped, not the density.
        l.float(&mut b.density, db.density, 0.0, 1000.0);
        l.float(&mut b.length, db.length, 0.0, 32.0);
        l.float(&mut b.angle, db.angle, -360.0, 360.0);
        l.float(&mut b.radius_ratio, db.radius_ratio, 0.0, 4.0);
        // A tip radius is `base * (1 - taper)`: above 1 is a negative radius.
        l.float(&mut b.taper, db.taper, 0.0, 1.0);
        l.float(&mut b.phyllotaxis_angle, db.phyllotaxis_angle, -3600.0, 3600.0);
        l.float(&mut b.randomness, db.randomness, 0.0, 8.0);
        l.float(&mut b.up_attraction, db.up_attraction, -8.0, 8.0);
        l.count(&mut b.recursion, 0, 8);
        l.count(&mut b.sub_branch_count, 0, 32);
        l.float(&mut b.sub_branch_scale, db.sub_branch_scale, 0.0, 4.0);
        // A length is scaled by `1 - variation` at the least.
        l.float(&mut b.length_variation, db.length_variation, 0.0, 1.0);
        l.float(&mut b.sub_branch_position_bias, db.sub_branch_position_bias, -1.0, 1.0);
        l.float(&mut b.apical_dominance, db.apical_dominance, 0.0, 4.0);
        l.float(&mut b.flatness, db.flatness, 0.0, 1.0);
        l.float(&mut b.angle_curve, db.angle_curve, -8.0, 8.0);
        l.float(&mut b.crown_angle_variation, db.crown_angle_variation, -8.0, 8.0);
        l.float(&mut b.twist, db.twist, -7200.0, 7200.0);
        l.float(&mut b.gravity_strength, db.gravity_strength, -16.0, 16.0);
        // Above 1 reads as 1 (`bend = gravity * (1 - stiffness)`, floored at 0).
        l.float(&mut b.stiffness, db.stiffness, 0.0, 1.0);
        l.float(&mut b.break_chance, db.break_chance, 0.0, 1.0);
        l.float(&mut b.crown_influence, db.crown_influence, 0.0, 4.0);
        l.float(&mut b.floor_level, db.floor_level, -1000.0, 1000.0);
        l.float(&mut b.min_twig_radius, db.min_twig_radius, 0.0, 10.0);
        let s = &mut b.strands;
        let ds = &db.strands;
        l.float(&mut s.density, ds.density, 0.0, 32.0);
        l.float(&mut s.start, ds.start, 0.0, 1.0);
        l.float(&mut s.length, ds.length, 0.0, 16.0);
        l.float(&mut s.spread, ds.spread, 0.0, 90.0);
        l.float(&mut s.clearance, ds.clearance, -100.0, 100.0);

        let s = &mut p.split;
        let ds = &d.split;
        l.float(&mut s.probability, ds.probability, 0.0, 1.0);
        l.float(&mut s.angle, ds.angle, 0.0, 360.0);
        l.float(&mut s.position, ds.position, 0.0, 1.0);
        l.float(&mut s.radius_threshold, ds.radius_threshold, 0.0, 100.0);

        // Mesh construction: the counts bound the vertex cost of the wood.
        let m = &mut p.mesh;
        let dm = &d.mesh;
        l.count(&mut m.smooth_iterations, 0, 8);
        l.float(&mut m.smooth_factor, dm.smooth_factor, 0.0, 1.0);
        l.float(&mut m.resolution_scale, dm.resolution_scale, 0.0, 1024.0);
        // The two bounds may cross (the upper one wins), so each has the whole range.
        l.count(&mut m.min_radial_segments, 0, 32);
        l.count(&mut m.max_radial_segments, 0, 32);
        l.float(&mut m.pipe_radius_exponent, dm.pipe_radius_exponent, 0.5, 8.0);
        l.float(&mut m.pipe_radius_min, dm.pipe_radius_min, 0.0, 10.0);
        l.float(&mut m.collar_length, dm.collar_length, 0.0, 64.0);

        // Leaves. Density is per branch, or per metre of wood: with the height falloff at most 2x.
        let f = &mut p.foliage;
        let df = &d.foliage;
        l.float(&mut f.density, df.density, 0.0, 512.0);
        l.count(&mut f.cluster_size, 0, 64);
        l.float(&mut f.leaf_size, df.leaf_size, 0.0, 64.0);
        l.float(&mut f.leaf_size_variation, df.leaf_size_variation, 0.0, 1.0);
        l.float(&mut f.radius_threshold, df.radius_threshold, 0.0, 100.0);
        // The density at the crown's base is `1 - falloff` times its top's: 0 to 2.
        l.float(&mut f.height_falloff, df.height_falloff, -1.0, 1.0);
        l.float(&mut f.droop, df.droop, -8.0, 8.0);
        l.float(&mut f.rotation_variation, df.rotation_variation, 0.0, 1.0);
        l.float(&mut f.normal_bend, df.normal_bend, 0.0, 1.0);
        l.float(&mut f.color_variation, df.color_variation, 0.0, 1.0);
        l.float(&mut f.hue_variation, df.hue_variation, 0.0, 1.0);
        l.float(&mut f.occlusion, df.occlusion, 0.0, 1.0);
        let v = &mut f.shading;
        let dv = &df.shading;
        l.float(&mut v.translucency, dv.translucency, 0.0, 16.0);
        l.float(&mut v.roughness, dv.roughness, 0.0, 1.0);
        l.float(&mut v.veins, dv.veins, 0.0, 1.0);
        l.float(&mut v.underside, dv.underside, 0.0, 1.0);
        l.color(&mut v.transmission_tint, dv.transmission_tint);
        l.color(&mut f.color, df.color);
        l.optional_color(&mut f.autumn);
        l.optional_color(&mut f.blossom);
        if let Some(fruit) = &mut f.fruit {
            let dfruit = FruitParams::default();
            l.float(&mut fruit.radius, dfruit.radius, 0.0, 4.0);
            l.float(&mut fruit.share, dfruit.share, 0.0, 1.0);
            l.color(&mut fruit.color, dfruit.color);
        }

        let w = &mut p.wind;
        let dw = &d.wind;
        l.float(&mut w.sway, dw.sway, 0.0, 64.0);
        l.float(&mut w.flutter, dw.flutter, 0.0, 64.0);
        l.float(&mut w.sway_frequency, dw.sway_frequency, 0.0, 64.0);
        l.float(&mut w.flutter_frequency, dw.flutter_frequency, 0.0, 64.0);

        l.float(&mut p.bark.smooth_below, d.bark.smooth_below, 0.0, 10.0);
        l.float(&mut p.bark.fissured_above, d.bark.fissured_above, 0.0, 10.0);
        l.color(&mut p.trunk_color, d.trunk_color);

        let g = &mut p.growth;
        let dg = &d.growth;
        l.float(&mut g.maturity_age, dg.maturity_age, 0.0, 10_000.0);
        l.float(&mut g.trunk_fraction, dg.trunk_fraction, 0.0, 8.0);
        l.float(&mut g.branch_fraction, dg.branch_fraction, 0.0, 8.0);
        l.float(&mut g.child_duration_scale, dg.child_duration_scale, 0.0, 8.0);
        l.float(&mut g.thickening_lag, dg.thickening_lag, 0.0, 64.0);
        l.float(&mut g.leaf_threshold, dg.leaf_threshold, 0.0, 1.0);

        if let CrownStyle::Hull { tiers, sides, lift, jitter, .. } = &mut p.crown {
            l.count(tiers, 1, 64);
            l.count(sides, 3, 64);
            l.float(lift, 0.0, 0.0, 1.0);
            l.float(jitter, 0.0, 0.0, 1.0);
        }
        if let TrunkStyle::Prism { sides } = &mut p.trunk_style {
            l.count(sides, 3, 64);
        }
        l.changed
    }

fn best_ns(mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..20 {
        let t = Instant::now();
        for _ in 0..50_000 { f(); }
        best = best.min(t.elapsed().as_nanos() as f64 / 50_000.0);
    }
    best
}

fn main() {
    // The guide's snippet.
    let mut oak = TreePreset::Oak.params();
    for field in TreeParams::fields().iter().take(3) {
        let now = field.value(&oak).expect("the first numbers are a trunk's");
        println!("{}: {now}{} in {}..={}", field.path, field.unit.suffix(), field.min, field.max);
    }
    let refused = oak.set_field("trunk.height", 1e9).expect_err("1e9 m is not a tree");
    assert!(matches!(refused, ParamError::OutOfRange { .. }));
    println!("{refused}");
    oak.set_field("trunk.height", 12.0).expect("12 m is a tree");
    assert_eq!(oak.field("trunk.height"), Some(12.0));
    println!("fields: {}", TreeParams::fields().len());

    // sanitized(): new, in range (Borrowed), against a copy-and-old-apply.
    let mut every = TreePreset::Birch.params();
    every.foliage.fruit = Some(FruitParams::default());
    every.crown = CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.2, trunk_only: false };
    every.trunk_style = TrunkStyle::Prism { sides: 5 };
    for (name, params) in [("oak", TreePreset::Oak.params()), ("every part", every)] {
        let new = best_ns(|| { black_box(black_box(&params).sanitized()); });
        let old = best_ns(|| { let mut c = black_box(&params).clone(); black_box(old_apply(&mut c)); black_box(c); });
        println!("sanitized {name}: new {new:.0} ns, old (clone + old apply) {old:.0} ns");
    }

    // build_tree at maturity and age 4, every preset: best of 7.
    for preset in TreePreset::ALL {
        let params = preset.params();
        let mut line = format!("{:<14}", preset.name());
        for age in [f32::INFINITY, 4.0] {
            let mut best = f64::MAX;
            for _ in 0..7 {
                let t = Instant::now();
                let tree = build_tree(black_box(&params), age);
                best = best.min(t.elapsed().as_secs_f64() * 1e3);
                black_box(tree);
            }
            let tree = build_tree(&params, age);
            line += &format!("  age {:>4}: {best:7.3} ms, {} trunk v, {} foliage v", if age.is_infinite() { "max".to_string() } else { age.to_string() }, tree.trunk.positions.len(), tree.foliage.positions.len());
        }
        println!("{line}");
    }
    let _ = Color::NONE;
}
