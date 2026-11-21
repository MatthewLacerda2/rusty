use glam::Vec3;

use crate::ibl::ktx2_encode::encode_cubemap;
use crate::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::application::{Gizmo, SceneGizmos};
use rusty_scene::{Camera, Scene};

pub(super) const RES: u32 = 64;
const AT: Vec3 = Vec3::new(0.0, 1.0, 0.0);

pub(super) const FACES: [[f32; 3]; 6] = [
    [0.9, 0.1, 0.1],
    [0.1, 0.9, 0.1],
    [0.1, 0.1, 0.9],
    [0.9, 0.9, 0.1],
    [0.2, 0.6, 0.9],
    [0.9, 0.1, 0.9],
];

pub(super) fn faced_cube(name: &str) -> String {
    let face = |c: [f32; 3]| {
        let texel: Vec<u8> = [c[0], c[1], c[2], 1.0]
            .iter()
            .flat_map(|v| rusty_core::color::f32_to_f16(*v).to_le_bytes())
            .collect();
        texel.repeat(16)
    };
    let cube = PrefilteredCubemap {
        mips: vec![std::array::from_fn(|i| face(FACES[i]))],
        base_size: 4,
    };
    let path = rusty_test_support::temp::dir().join(name);
    rusty_core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

fn probe_scene(path: &str) -> Scene {
    let mut scene = Scene::new();
    let i = scene.reflection_probes.add(AT, Vec3::splat(2.0));
    scene.reflection_probes.set_cubemap(i, path.to_string());
    scene
}

pub(super) fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera, gizmos: SceneGizmos) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    view.gizmos = gizmos;
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, true);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), RES, RES)
}

pub(super) fn pixel(px: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = (y * RES + x) as usize * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

fn centre(r: &mut Renderer, scene: &Scene, gizmos: SceneGizmos) -> [u8; 4] {
    let cam = Camera::new(AT + Vec3::Z * 2.0, -90.0, 0.0);
    pixel(&frame(r, scene, &cam, gizmos), RES / 2, RES / 2)
}

pub(super) fn srgb(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| (rusty_core::color::linear_to_srgb(c) * 255.0).round() as u8)
}

pub(super) fn assert_near(got: [u8; 4], want: [u8; 3], what: &str) {
    let close = (0..3).all(|k| got[k].abs_diff(want[k]) <= 8);
    assert!(close, "{what}: got {got:?}, want about {want:?}");
}

#[test]
fn gpu_a_baked_probe_ball_mirrors_its_cube() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = probe_scene(&faced_cube("probe_ball_faces.ktx2"));
    let got = centre(&mut r, &scene, SceneGizmos::default());
    assert_near(got, srgb(FACES[4]), "the +Z face");
}

#[test]
fn gpu_an_unbaked_probe_ball_is_grey() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let missing = rusty_test_support::temp::dir().join("probe_ball_never_baked.ktx2");
    let scene = probe_scene(&missing.to_string_lossy());
    let got = centre(&mut r, &scene, SceneGizmos::default());
    assert_near(got, srgb([0.5; 3]), "unbaked grey");
}

#[test]
fn gpu_the_toggle_hides_the_ball() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = probe_scene(&faced_cube("probe_ball_hidden.ktx2"));
    let mut off = SceneGizmos::default();
    off.set(Gizmo::ReflectionProbes, false);
    let hidden = centre(&mut r, &scene, off);
    let bare = centre(&mut r, &Scene::new(), SceneGizmos::default());
    assert_eq!(hidden, bare, "the probe draws nothing with its gizmo off");
}
