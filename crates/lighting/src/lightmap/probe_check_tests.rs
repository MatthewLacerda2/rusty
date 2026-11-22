//! A check that contradicts a unanimous probe verdict makes the texel cast the panel's whole set, so its light is the exhaustive estimate, never a weighted correction.

use super::shadow_probe_tests::Fixture;
use super::slit_probe_tests::under;

#[test]
fn a_rod_the_probe_misses_and_a_check_finds_gives_the_exhaustive_light() {
    // The rod hides 1/16 of the panel's width. A texel straight under it whose
    // 20 probe rays all miss it is taken as seeing the whole panel; when one of
    // its checks then finds the rod, it casts the rest of the set: more rays
    // than a penumbra's 260, and the exhaustive light, rod's shadow included.
    let rod = Fixture::new(under(0.0625 * 0.6, 1.0));
    let seeds = 0..256u64;
    let probed: Vec<(f32, u64)> = seeds
        .clone()
        .map(|s| rod.direct(0.0, s, true, 256))
        .collect();
    let exhaustive = seeds.map(|s| rod.direct(0.0, s, false, 256).0).sum::<f32>() / 256.0;
    let mean = |pick: &dyn Fn(u64) -> bool| {
        let got: Vec<f32> = probed
            .iter()
            .filter(|(_, r)| pick(*r))
            .map(|(e, _)| *e)
            .collect();
        (got.iter().sum::<f32>() / got.len() as f32, got.len())
    };
    let (found, n_found) = mean(&|r| r > 260);
    let (missed, n_missed) = mean(&|r| r < 256);
    println!("found {n_found}: {found}; missed {n_missed}: {missed}; exhaustive {exhaustive}");
    assert!(
        n_found > 0 && n_missed > 0,
        "{n_found} found, {n_missed} missed"
    );
    assert!(
        (found - exhaustive).abs() < exhaustive * 0.005,
        "a contradicted verdict gives {found}, the exhaustive set {exhaustive}"
    );
    // What a missed rod costs: its shadow on that texel, at most its share.
    assert!(
        missed > exhaustive && missed < exhaustive * 1.08,
        "{missed}"
    );
}
