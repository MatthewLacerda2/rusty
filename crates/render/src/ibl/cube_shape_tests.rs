use super::cubemap_tests::{build_cubemap_ktx2, BC6H_UFLOAT, R16G16B16A16_SFLOAT, R8G8B8A8_SRGB};
use super::*;

fn shape(bytes: &[u8]) -> Option<CubeShape> {
    cube_shape(bytes.get(..48)?.try_into().unwrap()).ok()
}

#[test]
fn the_header_states_the_size_and_mips_of_a_cube_the_loader_accepts() {
    let bytes = build_cubemap_ktx2(8, BC6H_UFLOAT, 6);
    assert_eq!(shape(&bytes), Some(CubeShape { size: 8, mips: 1 }));
    assert!(parse_ktx2_cubemap_mips(&bytes).is_ok(), "the loader agrees");
}

#[test]
fn the_header_refuses_what_the_loader_refuses() {
    for bytes in [
        build_cubemap_ktx2(4, BC6H_UFLOAT, 1),
        build_cubemap_ktx2(4, R16G16B16A16_SFLOAT, 6),
        build_cubemap_ktx2(1, R8G8B8A8_SRGB, 6),
        vec![0; 80],
    ] {
        assert_eq!(shape(&bytes), None);
        assert!(parse_ktx2_cubemap_mips(&bytes).is_err());
    }
    assert!(read_cube_shape("no/such/cube.ktx2").is_err());
}

#[test]
fn rejects_a_cube_under_a_block() {
    let why = parse_ktx2_cubemap_mips(&build_cubemap_ktx2(2, BC6H_UFLOAT, 6)).err();
    assert_eq!(why, Some("its faces are not whole 4×4 blocks"));
}

#[test]
fn gpu_a_cubemap_rebaked_at_a_cached_path_reloads() {
    use rusty_test_support::rebakes::{follows_rebakes, Bake};
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let size = |bake| if bake == Bake::A { 4 } else { 8 };
    let path = rusty_test_support::temp::dir().join("probe.ktx2");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            let bytes = build_cubemap_ktx2(size(bake), BC6H_UFLOAT, 6);
            rusty_core::fs::written::write(std::path::Path::new(p), &bytes).unwrap();
        },
        |p| {
            renderer.refresh_textures();
            let mut scene = rusty_scene::Scene::new();
            let i = (scene.reflection_probes).add(glam::Vec3::ZERO, glam::Vec3::ONE);
            scene.reflection_probes.set_cubemap(i, p.to_string());
            renderer.update_reflection_probes(&scene, glam::Vec3::ZERO);
            let probes = &renderer.reflection_probes;
            (probes.probe_for(glam::Vec3::ZERO) != 0).then(|| probes.face())
        },
        |loaded, bake| *loaded == Some(size(bake)),
    );
}
