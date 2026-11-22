use glam::Vec3;

use super::ProbeCapture;
use crate::{project_light_probe, CubemapCapture};
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{analytic_fill, AmbientMode, AmbientRadiance, MeshComponent, Scene};

const RES: u32 = 32;
const NORMALS: [Vec3; 6] = [
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::X,
    Vec3::NEG_Z,
    Vec3::new(0.6, 0.8, 0.0),
    Vec3::new(0.0, -0.6, 0.8),
];

fn scene(mode: AmbientMode) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::new(0.4, 0.8, 1.2);
    scene.ambient_intensity = 1.5;
    scene.ambient.mode = mode;
    scene.ambient.sky = Vec3::new(0.1, 0.3, 1.2);
    scene.ambient.equator = Vec3::splat(0.5);
    scene.ambient.ground = Vec3::new(1.2, 0.5, 0.1);
    scene
}

fn probed(mode: AmbientMode) -> Scene {
    let mut scene = scene(mode);
    scene.probes.add_probe(Vec3::ZERO);
    scene
}

fn analytic(mode: AmbientMode) -> Sh9 {
    let mut scene = probed(mode);
    analytic_fill(&mut scene);
    scene.probes.probes()[0].sh
}

fn assert_matches(got: &Sh9, want: &Sh9, what: &str) {
    for n in NORMALS.map(Vec3::normalize) {
        let (g, w) = (got.eval(n), want.eval(n));
        let (g, w) = (g / std::f32::consts::PI, w / std::f32::consts::PI);
        assert!(g.abs_diff_eq(w, 5e-3), "{what} {n}: {g} vs {w}");
    }
}

#[test]
fn an_open_capture_projects_the_ambient() {
    let res = 16;
    let black = || vec![Vec3::ZERO; (res * res) as usize];
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
    let mut ringing = scene(AmbientMode::Gradient);
    ringing.ambient.equator = Vec3::ZERO;
    let sh = project_light_probe(&capture, ringing.drawn_ambient().radiance());
    ringing.probes.add_probe(Vec3::ZERO);
    analytic_fill(&mut ringing);
    let fill = ringing.probes.probes()[0].sh;
    let lift = NORMALS.map(|n| sh.eval(n.normalize()) - fill.eval(n.normalize()));
    assert!(lift.iter().all(|d| d.min_element() > -1e-4), "{lift:?}");
    assert!(lift.iter().any(|d| d.max_element() > 1e-2), "{lift:?}");
    let mut closed = capture;
    closed.open = std::array::from_fn(|_| vec![false; (res * res) as usize]);
    let sh = project_light_probe(&closed, AmbientRadiance::Hemisphere(Vec3::ONE));
    assert_eq!(sh.eval(Vec3::Y), Vec3::ZERO);
}

#[test]
fn gpu_an_empty_scene_bakes_the_analytic_fill() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    for mode in AmbientMode::ALL {
        let baked = renderer.bake_probe(&scene(mode), Vec3::ZERO, RES);
        assert_matches(&baked, &analytic(mode), mode.name());
        let mut solved = probed(mode);
        renderer.bake_probes(&mut solved, RES, rusty_lighting::lightmap::probe_direct);
        assert_matches(&solved.probes.probes()[0].sh, &baked, "multi-bounce");
    }
}

#[test]
fn gpu_a_closed_room_bakes_no_ambient() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = scene(AmbientMode::Gradient);
    scene.materials.insert(
        "black".to_string(),
        MaterialAsset {
            base_color: [0.0; 3],
            ..MaterialAsset::default()
        },
    );
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for side in [-1.0, 1.0] {
            let id = scene.add_entity("Wall".to_string());
            let (v, i) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
            scene
                .world
                .set_mesh(id, Some(MeshComponent::new("Box", v, i)));
            scene.world.set_static(id, true);
            let mat = MaterialComponent::new("black".to_string());
            scene.world.set_material(id, Some(mat));
            let mut t = scene.world.transform_mut(id).unwrap();
            t.position = axis * 2.0 * side;
            t.scale = Vec3::splat(5.0) - axis * 4.5;
        }
    }
    let capture = renderer.capture_light_probe(&scene, Vec3::ZERO, RES, false);
    let open = capture.open.iter().flatten().filter(|o| **o).count();
    assert_eq!(open, 0, "texels open inside a closed room");
    let sh = renderer.bake_probe(&scene, Vec3::ZERO, RES);
    for n in NORMALS.map(Vec3::normalize) {
        assert!(sh.eval(n).max_element() < 1e-2, "{n}: {}", sh.eval(n));
    }
}
