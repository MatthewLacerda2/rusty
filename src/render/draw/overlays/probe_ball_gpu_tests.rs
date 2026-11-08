//! The probe ball on a GPU (#1911): a baked probe's ball mirrors its cube along the
//! reflected view ray, an unbaked one's is grey, and the gizmo toggle hides both.

use glam::Vec3;

use crate::core::application::{Gizmo, SceneGizmos};
use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 64;
/// Where the probe sits: off the floor grid, which the camera would see edge-on.
const AT: Vec3 = Vec3::new(0.0, 1.0, 0.0);

/// Linear colour of each cube face, in `+X, -X, +Y, -Y, +Z, -Z` order: all distinct,
/// so the centre pixel names the face it reflected.
pub(super) const FACES: [[f32; 3]; 6] = [
    [0.9, 0.1, 0.1],
    [0.1, 0.9, 0.1],
    [0.1, 0.1, 0.9],
    [0.9, 0.9, 0.1],
    [0.2, 0.6, 0.9],
    [0.9, 0.1, 0.9],
];

/// A one-mip 4² cube file whose faces are [`FACES`], in half floats; returns its path.
pub(super) fn faced_cube(name: &str) -> String {
    let face = |c: [f32; 3]| {
        let texel: Vec<u8> = [c[0], c[1], c[2], 1.0]
            .iter()
            .flat_map(|v| crate::core::color::f32_to_f16(*v).to_le_bytes())
            .collect();
        texel.repeat(16)
    };
    let cube = PrefilteredCubemap {
        mips: vec![std::array::from_fn(|i| face(FACES[i]))],
        base_size: 4,
    };
    let path = crate::test_temp::dir().join(name);
    crate::core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

/// A scene with one reflection probe at [`AT`] whose cube is at `path`.
fn probe_scene(path: &str) -> Scene {
    let mut scene = Scene::new();
    let i = scene.reflection_probes.add(AT, Vec3::splat(2.0));
    scene.reflection_probes.set_cubemap(i, path.to_string());
    scene
}

/// The editor frame (RGBA8, sRGB, `RES`²) `cam` renders of `scene` with `gizmos`.
pub(super) fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera, gizmos: SceneGizmos) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    view.gizmos = gizmos;
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, true);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), RES, RES)
}

/// The pixel at column `x`, row `y` of a [`frame`].
pub(super) fn pixel(px: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = (y * RES + x) as usize * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

/// The centre pixel of the editor frame looking down -Z at the probe from 2 m.
fn centre(r: &mut Renderer, scene: &Scene, gizmos: SceneGizmos) -> [u8; 4] {
    let cam = Camera::new(AT + Vec3::Z * 2.0, -90.0, 0.0);
    pixel(&frame(r, scene, &cam, gizmos), RES / 2, RES / 2)
}

/// `linear` as the sRGB target stores it.
pub(super) fn srgb(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| (crate::core::color::linear_to_srgb(c) * 255.0).round() as u8)
}

pub(super) fn assert_near(got: [u8; 4], want: [u8; 3], what: &str) {
    let close = (0..3).all(|k| got[k].abs_diff(want[k]) <= 8);
    assert!(close, "{what}: got {got:?}, want about {want:?}");
}

/// Head-on, the eye ray meets the ball's normal +Z and reflects back along +Z: the
/// centre pixel is the cube's +Z face, raw (no lighting, no tonemapping).
#[test]
fn gpu_a_baked_probe_ball_mirrors_its_cube() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = probe_scene(&faced_cube("probe_ball_faces.ktx2"));
    let got = centre(&mut r, &scene, SceneGizmos::default());
    assert_near(got, srgb(FACES[4]), "the +Z face");
}

/// A probe whose cube is not resident (here, never baked) shows a flat grey ball.
#[test]
fn gpu_an_unbaked_probe_ball_is_grey() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let missing = crate::test_temp::dir().join("probe_ball_never_baked.ktx2");
    let scene = probe_scene(&missing.to_string_lossy());
    let got = centre(&mut r, &scene, SceneGizmos::default());
    assert_near(got, srgb([0.5; 3]), "unbaked grey");
}

/// With the Reflection Probes gizmo off, no ball is drawn: the centre is whatever the
/// scene shows there without it.
#[test]
fn gpu_the_toggle_hides_the_ball() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = probe_scene(&faced_cube("probe_ball_hidden.ktx2"));
    let mut off = SceneGizmos::default();
    off.set(Gizmo::ReflectionProbes, false);
    let hidden = centre(&mut r, &scene, off);
    let bare = centre(&mut r, &Scene::new(), SceneGizmos::default());
    assert_eq!(hidden, bare, "the probe draws nothing with its gizmo off");
}
