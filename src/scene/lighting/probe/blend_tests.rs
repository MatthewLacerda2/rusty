//! The nearest-probe blend fades a probe out before it leaves (#1737): a point
//! swept past a swap of its nearest four moves its sample by no more than the step
//! warrants, where plain inverse squared distance jumped by about a seventh.

use glam::Vec3;

use super::index::scan;
use super::{ProbeVolume, BLEND_PROBES};

/// A row of free probes zig-zagging off the x axis, the DC term of probe `k`
/// being `k`, so every swap of the nearest four changes what they blend.
fn row() -> ProbeVolume {
    let mut vol = ProbeVolume::new();
    for k in 0..12 {
        let i = vol.add_probe(Vec3::new(
            k as f32,
            0.0,
            if k % 2 == 0 { 0.4 } else { -0.7 },
        ));
        vol.probes[i].sh.coeffs[0] = [k as f32, 0.0, 0.0];
    }
    vol
}

#[test]
fn a_point_swept_past_a_swap_never_jumps() {
    let vol = row();
    let step = 1.0e-3;
    let at = |x: f32| Vec3::new(x, 0.0, 0.0);
    let dc = |x: f32| vol.sample(at(x)).unwrap().coeffs[0][0];
    let four = |x: f32| {
        let mut ids: Vec<usize> = scan(&vol.probes, at(x))[..BLEND_PROBES]
            .iter()
            .map(|n| n.1)
            .collect();
        ids.sort_unstable();
        ids
    };
    let (mut swaps, mut worst) = (0, 0.0f32);
    let mut x = 2.0;
    while x < 9.0 {
        swaps += usize::from(four(x) != four(x + step));
        worst = worst.max((dc(x + step) - dc(x)).abs());
        x += step;
    }
    assert!(swaps >= 5, "the sweep crosses swaps: {swaps}");
    assert!(worst < 0.02, "a jump of {worst} over a {step} m step");
}

/// With fewer probes than the blend takes, it is plain inverse squared distance.
#[test]
fn fewer_probes_than_the_blend_still_blend() {
    let mut vol = ProbeVolume::new();
    vol.add_probe(Vec3::ZERO);
    let i = vol.add_probe(Vec3::X * 2.0);
    vol.probes[i].sh.coeffs[0] = [1.0, 0.0, 0.0];
    let s = vol.sample(Vec3::X).unwrap().coeffs[0][0];
    assert!((s - 0.5).abs() < 1.0e-5, "{s}");
}
