//! Throwaway probe (iteration 16): build cost and counts per preset; the willow's strands and leaves.
use bevy_flora::{NodeKind, TreePreset, build_lods, build_tree, LodSettings};
use std::time::Instant;

/// Share of the grid cells of `[x0,x1] x [y0,y1]` whose centre lies under a triangle of `meshes`
/// projected along `axis` (0: look along x, so the plane is (z, y); 2: look along z, plane (x, y)).
fn coverage(meshes: &[&bevy_flora::MeshData], axis: usize, x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    const N: usize = 256;
    let mut grid = vec![false; N * N];
    let h = if axis == 0 { 2 } else { 0 };
    let project = |p: bevy::math::Vec3| (p[h], p.y);
    for m in meshes {
        for t in m.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| project(m.positions[i as usize]));
            let (minx, maxx) = (a.0.min(b.0).min(c.0), a.0.max(b.0).max(c.0));
            let (miny, maxy) = (a.1.min(b.1).min(c.1), a.1.max(b.1).max(c.1));
            let cx = |v: f32| ((v - x0) / (x1 - x0) * N as f32);
            let cy = |v: f32| ((v - y0) / (y1 - y0) * N as f32);
            let (i0, i1) = ((cx(minx).floor().max(0.0)) as usize, (cx(maxx).ceil().min(N as f32)) as usize);
            let (j0, j1) = ((cy(miny).floor().max(0.0)) as usize, (cy(maxy).ceil().min(N as f32)) as usize);
            let den = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
            if den.abs() < 1e-12 {
                continue;
            }
            for j in j0..j1 {
                for i in i0..i1 {
                    let px = x0 + (i as f32 + 0.5) / N as f32 * (x1 - x0);
                    let py = y0 + (j as f32 + 0.5) / N as f32 * (y1 - y0);
                    let l1 = ((b.1 - c.1) * (px - c.0) + (c.0 - b.0) * (py - c.1)) / den;
                    let l2 = ((c.1 - a.1) * (px - c.0) + (a.0 - c.0) * (py - c.1)) / den;
                    if l1 >= 0.0 && l2 >= 0.0 && l1 + l2 <= 1.0 {
                        grid[j * N + i] = true;
                    }
                }
            }
        }
    }
    grid.iter().filter(|&&g| g).count() as f32 / (N * N) as f32
}

fn main() {
    let only: Option<String> = std::env::args().nth(1);
    for preset in TreePreset::ALL {
        let name = format!("{preset:?}").to_lowercase();
        if let Some(o) = &only {
            if !name.contains(o.as_str()) {
                continue;
            }
        }
        let mut params = preset.params();
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
        if let Some(v) = env("WSD") { params.foliage.strand_density = v; }
        if let Some(v) = env("WSS") { params.foliage.strand_size = v; }
        if let Some(v) = env("WSA") { params.foliage.strand_angle = v; }
        if let Some(v) = env("WSN") { params.branches.strands.density = v; }
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
            // the crown's box, from level 0
            let (mut lo, mut hi) = (bevy::math::Vec3::splat(f32::MAX), bevy::math::Vec3::splat(f32::MIN));
            for p in lods[0].trunk.positions.iter().chain(&lods[0].foliage.positions) {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            let height = hi.y;
            println!("   crown box {lo:?} .. {hi:?}");
            for (i, l) in lods.iter().enumerate() {
                for (name, band) in [("lower third", (0.0, height / 3.0)), ("middle third", (height / 3.0, 2.0 * height / 3.0))] {
                    let front = coverage(&[&l.trunk, &l.foliage], 2, lo.x, hi.x, band.0, band.1);
                    let side = coverage(&[&l.trunk, &l.foliage], 0, lo.z, hi.z, band.0, band.1);
                    println!("   level {i} {name}: covered front {front:.3} side {side:.3}");
                }
            }
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
