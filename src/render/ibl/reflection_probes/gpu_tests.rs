//! The probe array on a GPU (#1206): what a frame copies, and what it leaves alone.

use glam::Vec3;

use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::render::Renderer;
use crate::scene::Scene;

/// A one-mip `size`² cube file named `name`, every texel `value`; returns its path.
fn cube_file(name: &str, size: u32, value: u8) -> String {
    let face = vec![value; (size * size * 4) as usize];
    let cube = PrefilteredCubemap {
        mips: vec![std::array::from_fn(|_| face.clone())],
        base_size: size,
    };
    let path = crate::test_temp::dir().join(name);
    crate::core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

/// A scene with one probe per `(x, cube path)`, each a 2-unit box around `x`.
fn probes_at(probes: &[(f32, &str)]) -> Scene {
    let mut scene = Scene::new();
    for (x, path) in probes {
        let set = &mut scene.reflection_probes;
        let i = set.add(Vec3::new(*x, 0.0, 0.0), Vec3::ONE);
        set.set_cubemap(i, path.to_string());
    }
    scene
}

fn renderer() -> Option<Renderer> {
    crate::render::test_gpu::headless_or_skip(4, 4)
}

/// The Inspector preview renders a probe-less scene every frame beside the viewport:
/// it must leave the viewport's cubes resident, so going back costs no copy.
#[test]
fn gpu_a_probe_less_scene_keeps_the_resident_cubes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (a, b) = (
        cube_file("keep_a.ktx2", 4, 40),
        cube_file("keep_b.ktx2", 4, 90),
    );
    let viewport = probes_at(&[(-2.0, &a), (2.0, &b)]);
    renderer.update_reflection_probes(&viewport, Vec3::ZERO);
    assert_eq!(renderer.reflection_probes.copies, 2);
    renderer.global_bind_group_dirty = false;
    renderer.update_reflection_probes(&Scene::new(), Vec3::ZERO);
    assert_eq!(
        renderer
            .reflection_probes
            .probe_for(Vec3::new(2.0, 0.0, 0.0)),
        0
    );
    renderer.update_reflection_probes(&viewport, Vec3::ZERO);
    assert_eq!(renderer.reflection_probes.copies, 2, "nothing copied again");
    assert_eq!(renderer.reflection_probes.face(), 4);
    assert!(
        !renderer.global_bind_group_dirty,
        "the array was not rebuilt"
    );
}

/// Two probes on one cube share its layer but keep their own boxes and centres.
#[test]
fn gpu_probes_sharing_a_cube_keep_their_own_boxes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let shared = cube_file("shared.ktx2", 4, 120);
    let scene = probes_at(&[(-2.0, &shared), (2.0, &shared)]);
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    let probes = &renderer.reflection_probes;
    assert_eq!(probes.copies, 1, "one cube, one copy");
    let left = probes.probe_for(Vec3::new(-2.0, 0.0, 0.0));
    let right = probes.probe_for(Vec3::new(2.0, 0.0, 0.0));
    assert!(left != 0 && right != 0 && left != right, "{left} {right}");
    let [l, r] = [left, right].map(|p| probes.uniforms()[p as usize - 1]);
    assert_eq!(l.layer, r.layer);
    assert_eq!(l.center, [-2.0, 0.0, 0.0]);
    assert_eq!(r.center, [2.0, 0.0, 0.0]);
    assert_eq!(r.box_min, [1.0, -1.0, -1.0]);
}

/// A cube that is not the array's face halved a whole number of times is left out:
/// its probe takes no layer and its objects reflect the sky.
#[test]
fn gpu_a_cube_of_another_size_is_left_out() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let big = cube_file("size_8.ktx2", 8, 10);
    let odd = cube_file("size_3.ktx2", 3, 10);
    let scene = probes_at(&[(-2.0, &big), (2.0, &odd)]);
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    let probes = &renderer.reflection_probes;
    assert_ne!(probes.probe_for(Vec3::new(-2.0, 0.0, 0.0)), 0);
    assert_eq!(probes.probe_for(Vec3::new(2.0, 0.0, 0.0)), 0);
}
