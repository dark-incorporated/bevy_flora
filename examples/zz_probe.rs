//! Throwaway probe (iteration 16): build cost and counts per preset; the willow's strands and leaves.
use bevy_flora::{NodeKind, TreePreset, build_lods, build_tree, LodSettings};
use std::time::Instant;

fn main() {
    let only: Option<String> = std::env::args().nth(1);
    for preset in TreePreset::ALL {
        let name = format!("{preset:?}").to_lowercase();
        if let Some(o) = &only {
            if !name.contains(o.as_str()) {
                continue;
            }
        }
        let params = preset.params();
        for age in [4.0_f32, f32::INFINITY] {
            // best of 5
            let mut best = f64::MAX;
            let mut meshes = None;
            for _ in 0..5 {
                let t = Instant::now();
                let m = build_tree(&params, age);
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
                meshes = Some(m);
            }
            let m = meshes.unwrap();
            let strands = m.skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Strand).count();
            println!(
                "{name:14} age {:>4} build {best:7.2} ms nodes {:4} strands {strands:4} trunk {:6}v/{:6}t foliage {:6}v/{:6}t",
                if age.is_finite() { format!("{age}") } else { "inf".into() },
                m.skeleton.nodes.len(),
                m.trunk.positions.len(),
                m.trunk.indices.len() / 3,
                m.foliage.positions.len(),
                m.foliage.indices.len() / 3,
            );
        }
        if only.is_some() {
            let sk = bevy_flora::generate_skeleton(&params);
            let (mut strand_leaves, mut other_leaves) = (0usize, 0usize);
            let mut strand_len = 0.0f32;
            let mut v = Vec::new();
            for i in 0..sk.nodes.len() {
                v.clear();
                bevy_flora::foliage::leaf_points_for_node(&sk, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut v);
                if sk.nodes[i].kind == NodeKind::Strand {
                    strand_leaves += v.len();
                    strand_len += sk.nodes[i].length;
                } else {
                    other_leaves += v.len();
                }
            }
            println!("   leaves on strands {strand_leaves} (total strand length {strand_len:.1} m), elsewhere {other_leaves}");
            let lods = build_lods(&params, f32::INFINITY, &LodSettings::default());
            for (i, l) in lods.iter().enumerate() {
                println!(
                    "   level {i}: trunk {}v/{}t foliage {}v/{}t",
                    l.trunk.positions.len(),
                    l.trunk.indices.len() / 3,
                    l.foliage.positions.len(),
                    l.foliage.indices.len() / 3
                );
            }
        }
    }
}
