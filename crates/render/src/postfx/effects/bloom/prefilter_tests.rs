use super::PREFILTER_DIVISOR;

type Image = Vec<Vec<f32>>;

fn bilinear(img: &Image, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |i: f32, j: f32| {
        let row = &img[(j.max(0.0) as usize).min(img.len() - 1)];
        row[(i.max(0.0) as usize).min(row.len() - 1)]
    };
    at(x0, y0) * (1.0 - tx) * (1.0 - ty)
        + at(x0 + 1.0, y0) * tx * (1.0 - ty)
        + at(x0, y0 + 1.0) * (1.0 - tx) * ty
        + at(x0 + 1.0, y0 + 1.0) * tx * ty
}

fn reduce(img: &Image, ratio: f32, n: usize, karis: bool) -> Image {
    let boxes = [
        (0, 0, 0.5),
        (-1, -1, 0.125),
        (1, -1, 0.125),
        (-1, 1, 0.125),
        (1, 1, 0.125),
    ];
    let texel = |i: usize, j: usize| {
        let (cx, cy) = ((i as f32 + 0.5) * ratio, (j as f32 + 0.5) * ratio);
        let (mut sum, mut weights) = (0.0, 0.0);
        for (bx, by, share) in boxes {
            let corners = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
            let tap = |(x, y): (i32, i32)| {
                let (x, y) = ((bx + x) as f32 * ratio / 2.0, (by + y) as f32 * ratio / 2.0);
                bilinear(img, cx + x, cy + y)
            };
            let b = corners.map(tap).iter().sum::<f32>() / 4.0;
            let w = if karis { share / (1.0 + b) } else { share };
            (sum, weights) = (sum + b * w, weights + w);
        }
        sum / weights
    };
    (0..n)
        .map(|j| (0..n).map(|i| texel(i, j)).collect())
        .collect()
}

const SIDE: usize = 48;

fn energy(divisor: u32, (x, y): (usize, usize), value: f32, sparse: bool) -> f32 {
    let mut scene = vec![vec![0.0; SIDE]; SIDE];
    scene[y][x] = value;
    let (d, karis) = (divisor as usize, value > 1.0);
    let first = if divisor <= PREFILTER_DIVISOR || sparse {
        reduce(&scene, d as f32, SIDE / d, karis)
    } else {
        let p = PREFILTER_DIVISOR as usize;
        let pre = reduce(&scene, p as f32, SIDE / p, karis);
        reduce(&pre, d as f32 / p as f32, SIDE / d, false)
    };
    let total: f32 = first.iter().flatten().sum();
    total * (d * d) as f32 / value
}

fn spread(divisor: u32, value: f32, sparse: bool) -> (Vec<f32>, f32, f32) {
    let d = divisor as usize;
    let at = |k: usize| (24 + k % d, 24 + k / d);
    let e: Vec<f32> = (0..d * d)
        .map(|k| energy(divisor, at(k), value, sparse))
        .collect();
    let (lo, hi) = e
        .iter()
        .fold((f32::MAX, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    (e, lo, hi)
}

#[test]
fn every_scene_pixel_reaches_the_chain_at_every_divisor() {
    for divisor in 1..=rusty_core::settings::quality::MAX_RESOLUTION_DIVISOR {
        let (_, lo, hi) = spread(divisor, 0.01, false);
        assert!(lo > 0.9 && hi < 1.1, "÷{divisor}: {lo:.3}..{hi:.3}");
        let (_, lo, hi) = spread(divisor, 40.0, false);
        assert!(
            lo > 0.0 && hi / lo < 1.25,
            "÷{divisor}, bright: {lo:.3}..{hi:.3}"
        );
    }
}

#[test]
fn thirteen_taps_straight_from_the_scene_drop_pixels_past_half() {
    for (divisor, dropped) in [(3, 4), (4, 8)] {
        let (e, ..) = spread(divisor, 0.01, true);
        let zero = e.iter().filter(|&&v| v < 1e-6).count();
        assert_eq!(zero, dropped, "÷{divisor}");
    }
}
