//! Throwaway (iteration 15): what `PointGrid::near` costs against the list it replaces.
use bevy::math::{Rect, Vec2};
use bevy_flora::{PointGrid, Scatter, SeededRng};
use std::time::Instant;

fn main() {
    for (count, spacing) in [(4_000usize, 3.0f32), (40_000, 3.0), (200_000, 3.0)] {
        let extent = (count as f32 / 0.68).sqrt() * spacing;
        let half = extent * 0.5;
        let area = Rect::new(-half, -half, half, half);
        let trees = Scatter { seed: 1, ..Scatter::new(area, spacing) }.points();
        let positions: Vec<Vec2> = trees.iter().map(|t| t.position).collect();
        let mut rng = SeededRng::new(9);
        let queries: Vec<Vec2> = (0..200_000).map(|_| Vec2::new(rng.range(-half, half), rng.range(-half, half))).collect();
        println!("{} trunks over {:.0} m, 200 000 queries at 1.5 m", positions.len(), extent);

        for cell in [1.5f32, 2.0, 8.0] {
            let t = Instant::now();
            let mut grid = PointGrid::with_cell(cell);
            grid.extend(positions.iter().copied());
            let build = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let hits = queries.iter().filter(|&&at| grid.near(at, 1.5)).count();
            let ask = t.elapsed().as_secs_f64() * 1e3;
            println!("  grid cell {cell:>3}: build {build:7.2} ms, 200k queries {ask:8.2} ms ({:.3} us each), {hits} near", ask * 1e3 / queries.len() as f64);
        }
        // The list, on 2 000 queries and scaled up.
        let sample = &queries[..2_000];
        let t = Instant::now();
        let hits = sample.iter().filter(|&&at| positions.iter().any(|p| p.distance(at) < 1.5)).count();
        let ask = t.elapsed().as_secs_f64() * 1e3;
        println!("  list           : 2000 queries {ask:8.2} ms ({:.3} us each, {} near of 2000); 200k would take {:.0} ms", ask * 1e3 / 2000.0, hits, ask * 100.0);
        // An unbounded radius.
        let grid: PointGrid = positions.iter().copied().collect();
        let t = Instant::now();
        let all = grid.near(Vec2::ZERO, f32::INFINITY);
        println!("  infinite radius: {all} in {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    }
}
