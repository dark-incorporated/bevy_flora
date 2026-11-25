//! Throwaway: habit numbers for a tree file (ZZ_PROBE_FILES=a.ron,b.ron). Deleted before commit.
use bevy::math::{Vec2, Vec3};
use bevy_flora::mesh::MeshData;
use bevy_flora::{build_tree, TreeParams};
use std::time::Instant;

const GRID: usize = 256;

fn coverage(meshes: &[&MeshData], lo: Vec3, hi: Vec3, band: (f32, f32), axis_u: usize) -> f32 {
    // Project onto (u, y); cover grid over the crown box's u extent and y band.
    let (u0, u1) = (lo[axis_u], hi[axis_u]);
    let y0 = lo.y + (hi.y - lo.y) * band.0;
    let y1 = lo.y + (hi.y - lo.y) * band.1;
    let mut grid = vec![false; GRID * GRID];
    let to = |p: Vec3| -> Vec2 {
        Vec2::new((p[axis_u] - u0) / (u1 - u0) * GRID as f32, (p.y - y0) / (y1 - y0) * GRID as f32)
    };
    for m in meshes {
        for t in m.indices.chunks_exact(3) {
            let a = to(m.positions[t[0] as usize]);
            let b = to(m.positions[t[1] as usize]);
            let c = to(m.positions[t[2] as usize]);
            let (minx, maxx) = (a.x.min(b.x).min(c.x).floor().max(0.0) as i32, a.x.max(b.x).max(c.x).ceil().min(GRID as f32) as i32);
            let (miny, maxy) = (a.y.min(b.y).min(c.y).floor().max(0.0) as i32, a.y.max(b.y).max(c.y).ceil().min(GRID as f32) as i32);
            for y in miny..maxy {
                for x in minx..maxx {
                    let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let d = |p1: Vec2, p2: Vec2, p3: Vec2| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
                    let (d1, d2, d3) = (d(p, a, b), d(p, b, c), d(p, c, a));
                    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                    if !(neg && pos) {
                        grid[y as usize * GRID + x as usize] = true;
                    }
                }
            }
        }
    }
    grid.iter().filter(|c| **c).count() as f32 / (GRID * GRID) as f32
}

fn main() {
    let files = std::env::var("ZZ_PROBE_FILES").unwrap();
    let seeds: Vec<u32> = std::env::var("ZZ_SEEDS").unwrap_or("42".into()).split(',').map(|s| s.parse().unwrap()).collect();
    for f in files.split(',') {
        let base = TreeParams::from_ron(&std::fs::read_to_string(f).unwrap()).unwrap();
        for &seed in &seeds {
            let mut p = base.clone();
            p.seed = seed as _;
            let t0 = Instant::now();
            let m = build_tree(&p, f32::INFINITY);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            let mut lo = Vec3::splat(f32::MAX);
            let mut hi = Vec3::splat(f32::MIN);
            for v in m.trunk.positions.iter().chain(m.foliage.positions.iter()) {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
            let ext = hi - lo;
            let width = ext.x.max(ext.z);
            let trunk_top = p.trunk.height;
            // highest foliage within 1.2 m (plan) of the axis
            let mut top_near = 0.0f32;
            for v in &m.foliage.positions {
                if Vec2::new(v.x, v.z).length() < 1.2 {
                    top_near = top_near.max(v.y);
                }
            }
            let mut wood_top_near = 0.0f32;
            for v in &m.trunk.positions {
                if Vec2::new(v.x, v.z).length() > 0.5 && Vec2::new(v.x, v.z).length() < 1.2 {
                    wood_top_near = wood_top_near.max(v.y);
                }
            }
            let ms_all = [&m.trunk, &m.foliage];
            let low = (coverage(&ms_all, lo, hi, (0.0, 0.34), 0), coverage(&ms_all, lo, hi, (0.0, 0.34), 2));
            let mid = (coverage(&ms_all, lo, hi, (0.34, 0.67), 0), coverage(&ms_all, lo, hi, (0.34, 0.67), 2));
            let top = (coverage(&ms_all, lo, hi, (0.67, 1.0), 0), coverage(&ms_all, lo, hi, (0.67, 1.0), 2));
            let mut ys: Vec<f32> = m.foliage.positions.iter().map(|v| v.y).collect();
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p99 = ys[(ys.len() as f32 * 0.99) as usize];
            let spike = hi.y - p99;
            let lowest_leaf = m.foliage.positions.iter().map(|v| v.y).fold(f32::MAX, f32::min);
            println!(
                "{f} seed {seed}: bbox {:.1}x{:.1}x{:.1} h/w {:.2} trunk {:.1}m ({:.0}% of h) bare-top {:.2} m (foliage near axis {:.2}) leaves-tris {} wood-tris {} nodes {} build {:.1}ms lowest-leaf {:.2} cover low {:.2}/{:.2} mid {:.2}/{:.2} top {:.2}/{:.2} spike {:.2} top-y {:.2}",
                ext.x, ext.y, ext.z,
                ext.y / width,
                trunk_top,
                100.0 * trunk_top / hi.y,
                (trunk_top - top_near).max(0.0),
                top_near,
                m.foliage.indices.len() / 3,
                m.trunk.indices.len() / 3,
                m.skeleton.nodes.len(),
                ms, lowest_leaf,
                low.0, low.1, mid.0, mid.1, top.0, top.1, spike, hi.y
            );
            let _ = wood_top_near;
        }
    }
}
