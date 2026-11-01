//! A light probe bakes against the ambient, not the sky (#1668): where its capture
//! escapes the scene it projects `AmbientRadiance::incoming`, so an open probe's SH
//! is the analytic fill's (`scene::analytic_fill`) in `Color` and in `Gradient`.
//! The projection is held on a synthetic capture (any host) and the whole bake on
//! the GPU (lavapipe; skips with no adapter).

use glam::Vec3;

use super::ProbeCapture;
use crate::render::{project_light_probe, CubemapCapture};
use crate::scene::lighting::sh::Sh9;
use crate::scene::{analytic_fill, AmbientMode, AmbientRadiance, Scene};

const RES: u32 = 32;
const NORMALS: [Vec3; 6] = [
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::X,
    Vec3::NEG_Z,
    Vec3::new(0.6, 0.8, 0.0),
    Vec3::new(0.0, -0.6, 0.8),
];

/// An empty scene in `Color` mode, or `Gradient` with a blue sky over an orange
/// ground, neither colour the procedural sky's.
fn scene(mode: AmbientMode) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::new(0.4, 0.8, 1.2);
    scene.ambient_intensity = 1.5;
    scene.ambient.mode = mode;
    scene.ambient.sky = Vec3::new(0.1, 0.3, 1.2);
    scene.ambient.ground = Vec3::new(1.2, 0.5, 0.1);
    scene
}

/// [`scene`] with one probe at the origin.
fn probed(mode: AmbientMode) -> Scene {
    let mut scene = scene(mode);
    scene.probes.add_probe(Vec3::ZERO);
    scene
}

/// What the analytic fill gives that probe (its height stand-in is 1 there).
fn analytic(mode: AmbientMode) -> Sh9 {
    let mut scene = probed(mode);
    analytic_fill(&mut scene);
    scene.probes.probes[0].sh
}

/// Both read back as a surface does (irradiance over π), equal at every normal.
fn assert_matches(got: &Sh9, want: &Sh9, what: &str) {
    for n in NORMALS.map(Vec3::normalize) {
        let (g, w) = (got.eval(n), want.eval(n));
        let (g, w) = (g / std::f32::consts::PI, w / std::f32::consts::PI);
        assert!(g.abs_diff_eq(w, 5e-3), "{what} {n}: {g} vs {w}");
    }
}

/// A black capture open everywhere projects the ambient alone, as the fill does.
#[test]
fn an_open_capture_projects_the_ambient() {
    let res = 16;
    let black = || vec![0u8; (res * res * 4) as usize];
    let capture = ProbeCapture {
        cube: CubemapCapture {
            resolution: res,
            faces: std::array::from_fn(|_| black()),
        },
        open: std::array::from_fn(|_| vec![true; (res * res) as usize]),
    };
    for mode in AmbientMode::ALL {
        let ambient = scene(mode).drawn_ambient().radiance();
        let sh = project_light_probe(&capture, ambient);
        assert_matches(&sh, &analytic(mode), mode.name());
    }
    // A covered (black) texel takes no ambient.
    let mut closed = capture;
    closed.open = std::array::from_fn(|_| vec![false; (res * res) as usize]);
    let sh = project_light_probe(&closed, AmbientRadiance::Hemisphere(Vec3::ONE));
    assert_eq!(sh.eval(Vec3::Y), Vec3::ZERO);
}

/// The GPU bake of an empty scene's probe is the analytic fill, in both modes: the
/// gradient reaches it, and the procedural sky does not.
#[test]
fn gpu_an_empty_scene_bakes_the_analytic_fill() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    for mode in AmbientMode::ALL {
        let baked = renderer.bake_probe(&scene(mode), Vec3::ZERO, RES);
        assert_matches(&baked, &analytic(mode), mode.name());
        let mut solved = probed(mode);
        renderer.bake_probes(&mut solved, RES);
        assert_matches(&solved.probes.probes[0].sh, &baked, "multi-bounce");
    }
}
