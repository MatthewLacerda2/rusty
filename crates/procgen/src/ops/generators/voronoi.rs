use super::{count, gray, Sampler};
use crate::hash;
use crate::ops::field::{Field, PerCell};
use crate::recipe::{VoronoiMetric, VoronoiOutput};

pub fn sampler(
    scale: [f32; 2],
    (output, metric): (VoronoiOutput, VoronoiMetric),
    randomness: Field<'_>,
    smoothness: f32,
    seed: u64,
) -> Sampler<'_> {
    let smooth = SmoothMin::new(smoothness);
    let cells = scale.map(count);
    let per = cells.map(|c| c as i64);
    let reach = reach(output, metric);
    let centre = move |c: i64, axis: usize| (c as f32 + 0.5) / cells[axis];
    let jitters = PerCell::grid((per[0], per[1]), &[randomness], move |cell: (i64, i64)| {
        randomness
            .at_uv(centre(cell.0, 0), centre(cell.1, 1))
            .clamp(0.0, 1.0)
    });
    Box::new(move |u, v| {
        let (px, py) = (u * cells[0], v * cells[1]);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let (mut f1, mut f2, mut best_cell) = (f32::MAX, f32::MAX, (0, 0));
        let mut soft = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let cell = (cx.rem_euclid(per[0]), cy.rem_euclid(per[1]));
                let jitter = jitters.get(cell);
                let at = |ch| 0.5 + jitter * (hash::unit2(cell.0, cell.1, ch, seed) - 0.5);
                let dist = measure(metric, cx as f32 + at(1) - px, cy as f32 + at(2) - py);
                if output == VoronoiOutput::SmoothF1 {
                    soft = Some(soft.map_or(dist, |s| smooth.fold(s, dist)));
                }
                if dist < f1 {
                    (f2, f1, best_cell) = (f1, dist, cell);
                } else if dist < f2 {
                    f2 = dist;
                }
            }
        }
        match output {
            VoronoiOutput::Distance => gray(f1.min(1.0)),
            VoronoiOutput::F2 => gray(f2.min(1.0)),
            VoronoiOutput::Edges => gray((f2 - f1).min(1.0)),
            VoronoiOutput::SmoothF1 => gray(soft.unwrap_or(f1).clamp(0.0, 1.0)),
            VoronoiOutput::Cells => {
                let c = |ch| hash::unit2(best_cell.0, best_cell.1, ch, seed);
                [c(3), c(4), c(5), 1.0]
            }
        }
    })
}

fn reach(output: VoronoiOutput, metric: VoronoiMetric) -> i64 {
    match (output, metric) {
        (VoronoiOutput::SmoothF1, _) => 2,
        (VoronoiOutput::Distance | VoronoiOutput::F2, _) | (_, VoronoiMetric::Chebyshev) => 1,
        (VoronoiOutput::Edges, VoronoiMetric::Manhattan) => 3,
        (VoronoiOutput::Cells | VoronoiOutput::Edges, _) => 2,
    }
}

#[derive(Clone, Copy)]
struct SmoothMin {
    k: f32,
}

impl SmoothMin {
    fn new(smoothness: f32) -> Self {
        let s = super::finite(smoothness, 0.0).clamp(0.0, 1.0);
        Self {
            k: (s * 0.5).max(f32::MIN_POSITIVE),
        }
    }

    fn fold(self, running: f32, dist: f32) -> f32 {
        let t = (0.5 + 0.5 * (running - dist) / self.k).clamp(0.0, 1.0);
        let h = t * t * (3.0 - 2.0 * t);
        running + (dist - running) * h - self.k * h * (1.0 - h)
    }
}

fn measure(metric: VoronoiMetric, dx: f32, dy: f32) -> f32 {
    match metric {
        VoronoiMetric::Euclidean => dx.hypot(dy),
        VoronoiMetric::Manhattan => dx.abs() + dy.abs(),
        VoronoiMetric::Chebyshev => dx.abs().max(dy.abs()),
    }
}

#[cfg(test)]
#[path = "voronoi_tests.rs"]
mod tests;
