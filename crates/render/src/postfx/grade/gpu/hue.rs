use super::{chain, Chain, W};
use rusty_components::Tonemap;
use rusty_core::color::srgb_to_linear;
use rusty_scene::create::default_post_processing;

const ORANGE: [f32; 3] = [1.0, 0.45, 0.1];

const BOUND: f32 = 3.0;

fn hue(c: [f32; 3]) -> f32 {
    let lo = c[1].min(c[2]);
    60.0 * (c[1] - c[2]) / (c[0] - lo)
}

fn scale(i: usize) -> f32 {
    0.02 * 2000f32.powf(i as f32 / (W - 1) as f32)
}

fn turns(c: &mut Chain, tonemap: Tonemap) -> Vec<Option<f32>> {
    let row: Vec<[f32; 3]> = (0..W as usize)
        .map(|i| ORANGE.map(|v| v * scale(i)))
        .collect();
    let mut vc = default_post_processing();
    vc.tonemapping.mode = tonemap;
    let out = c.run(&row, 0.0, Some(&vc));
    let want = hue(ORANGE);
    out.iter()
        .map(|p| {
            let coloured = p[0] >= p[2].saturating_add(16);
            coloured.then(|| (hue([0, 1, 2].map(|k| srgb_to_linear(p[k]))) - want).abs())
        })
        .collect()
}

fn measure(c: &mut Chain, tonemap: Tonemap) -> (f32, Option<f32>) {
    let t = turns(c, tonemap);
    let worst = t.iter().flatten().copied().fold(0.0, f32::max);
    let breaks = t
        .iter()
        .position(|v| v.is_some_and(|v| v > BOUND))
        .map(scale);
    (worst, breaks)
}

#[test]
fn gpu_a_saturated_orange_ramp_keeps_its_hue_through_the_default_tonemapper() {
    let Some(mut c) = chain() else { return };
    assert_eq!(default_post_processing().tonemapping.mode, Tonemap::Neutral);
    let all = Tonemap::ALL.map(|t| (t, measure(&mut c, t)));
    eprintln!("[hue] (worst turn in degrees, brightness it first passes {BOUND}°): {all:?}");
    let of = |t: Tonemap| {
        all.iter()
            .find(|(m, _)| *m == t)
            .map(|p| p.1)
            .expect("listed")
    };
    let (neutral, _) = of(Tonemap::Neutral);
    assert!(neutral <= BOUND, "Neutral turned orange by {neutral}°");
    assert!(of(Tonemap::Aces).0 > 4.0 * BOUND, "{all:?}");
}
