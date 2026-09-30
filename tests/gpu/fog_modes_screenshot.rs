//! Scene fog per mode (#437): a red wall 20 m away, fogged toward pure blue, must
//! read exactly the fog formula's blend — one pixel per mode, checked against the
//! analytic factor — height falloff must thin fog for a camera above its base, and
//! the sky must take the fog colour at the horizon but keep its own overhead.

use rusty::dev::capture::CaptureHost;
use rusty::scene::{FogMode, Scene};

use super::fog_scene::{assert_close, centre, shot, srgb8, wall_scene};

const DISTANCE: f32 = 20.0;

fn fogged(mode: FogMode) -> Scene {
    let mut scene = wall_scene(DISTANCE, [1.0, 0.0, 0.0]);
    scene.fog.mode = mode;
    scene.fog.color = glam::Vec3::new(0.0, 0.0, 1.0);
    scene.fog.start = 0.0;
    scene.fog.end = 40.0;
    scene.fog.density = 0.03;
    scene
}

/// The pixel a red→blue blend by fog factor `f` stores.
fn expected(f: f32) -> [i32; 3] {
    [srgb8(1.0 - f), 0, srgb8(f)]
}

#[test]
fn each_fog_mode_blends_by_its_formula() {
    let mut host = CaptureHost::new();
    let x = 0.03 * DISTANCE;
    let cases = [
        (FogMode::Off, 0.0),
        (FogMode::Linear, DISTANCE / 40.0),
        (FogMode::Exponential, 1.0 - (-x).exp()),
        (FogMode::ExponentialSquared, 1.0 - (-x * x).exp()),
    ];
    for (mode, f) in cases {
        let Some(px) = centre(&mut host, &fogged(mode), mode.name()) else {
            return;
        };
        eprintln!("[fog] {} f={f:.3} -> {px:?}", mode.name());
        assert_close(px, expected(f), 3, mode.name());
    }
}

#[test]
fn height_falloff_thins_fog_above_its_base() {
    let mut host = CaptureHost::new();
    let uniform = fogged(FogMode::Exponential);
    let mut layered = fogged(FogMode::Exponential);
    // The camera and wall sit 2 m above the base: the fog there is e^(-0.5·2) as thick.
    layered.fog.height_falloff = 0.5;
    layered.fog.base_height = -2.0;
    let (Some(u), Some(l)) = (
        centre(&mut host, &uniform, "uniform"),
        centre(&mut host, &layered, "layered"),
    ) else {
        return;
    };
    let x = 0.03 * DISTANCE * (-1.0f32).exp();
    assert_close(l, expected(1.0 - (-x).exp()), 3, "layered");
    assert!(
        l[2] < u[2],
        "height fog must be thinner above its base ({l:?} vs {u:?})"
    );
}

#[test]
fn the_sky_blends_to_fog_at_the_horizon_only() {
    let mut host = CaptureHost::new();
    // The wall sits behind the camera, so the view is all sky.
    let mut scene = wall_scene(-20.0, [0.0, 0.0, 0.0]);
    scene.fog = fogged(FogMode::Exponential).fog;
    let Some(img) = shot(&mut host, &scene, "sky") else {
        return;
    };
    let horizon = img.get_pixel(32, 32).0;
    let overhead = img.get_pixel(32, 0).0;
    eprintln!("[fog] sky horizon={horizon:?} overhead={overhead:?}");
    assert_close(horizon, [0, 0, 255], 3, "horizon");
    assert!(
        overhead[0] > 20,
        "the sky above the horizon keeps its own colour ({overhead:?})"
    );
}
