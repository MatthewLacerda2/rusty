//! Reading a cube file's header alone (#1584), and a re-bake reaching the probe array
//! (#1207).

use super::cubemap_tests::{build_cubemap_ktx2, BC6H_UFLOAT, R16G16B16A16_SFLOAT, R8G8B8A8_SRGB};
use super::*;

/// The shape a blob's first 48 bytes state, as [`read_cube_shape`] reads them.
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

/// A BC6H cube's base level is whole 4×4 blocks: a 2-texel one is refused.
#[test]
fn rejects_a_cube_under_a_block() {
    let why = parse_ktx2_cubemap_mips(&build_cubemap_ktx2(2, BC6H_UFLOAT, 6)).err();
    assert_eq!(why, Some("its faces are not whole 4×4 blocks"));
}

/// A reflection cubemap re-baked at a path the probe array already read (or failed
/// to) is read again on the next frame (#1207).
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
