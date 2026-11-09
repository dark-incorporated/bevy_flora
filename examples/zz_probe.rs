//! Throwaway build probe (iteration 2): `build_tree` time per preset, mature and at age 4.
use std::time::Instant;

use bevy_flora::{build_tree, TreeParams, TreePreset};

fn time(params: &TreeParams, age: f32) -> (f64, usize, usize, usize, usize) {
    let mut times = Vec::new();
    let mut counts = (0, 0, 0, 0);
    for _ in 0..15 {
        let start = Instant::now();
        let tree = build_tree(params, age);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        counts = (tree.trunk.positions.len(), tree.trunk.indices.len() / 3, tree.foliage.positions.len(), tree.foliage.indices.len() / 3);
        std::hint::black_box(&tree);
    }
    times.sort_by(f64::total_cmp);
    (times[times.len() / 2], counts.0, counts.1, counts.2, counts.3)
}

fn main() {
    let mut rows: Vec<(String, TreeParams)> = TreePreset::ALL.iter().map(|p| (p.name().to_string(), p.params())).collect();
    let mut adaptive = TreePreset::Oak.params();
    adaptive.mesh.adaptive_resolution = true;
    rows.push(("oak_adaptive".to_string(), adaptive));
    println!("name\tage\tmedian_ms\ttrunk_verts\ttrunk_tris\tfoliage_verts\tfoliage_tris");
    for (name, params) in &rows {
        for age in [f32::INFINITY, 4.0] {
            let (ms, tv, tt, fv, ft) = time(params, age);
            println!("{name}\t{age}\t{ms:.3}\t{tv}\t{tt}\t{fv}\t{ft}");
        }
    }
}
