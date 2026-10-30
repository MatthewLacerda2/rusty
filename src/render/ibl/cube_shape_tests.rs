//! Reading a cube file's header alone (#1584), and a re-bake reaching the probe array
//! (#1207).

use super::cubemap_tests::{build_cubemap_ktx2, R16G16B16A16_SFLOAT, R8G8B8A8_SRGB};
use super::*;

/// The shape a blob's first 48 bytes state, as [`read_cube_shape`] reads them.
fn shape(bytes: &[u8]) -> Option<CubeShape> {
    cube_shape(bytes.get(..48)?.try_into().unwrap())
}

#[test]
fn the_header_states_the_size_and_mips_of_a_cube_the_loader_accepts() {
    let bytes = build_cubemap_ktx2(8, R8G8B8A8_SRGB, 6);
    assert_eq!(shape(&bytes), Some(CubeShape { size: 8, mips: 1 }));
    assert!(
        parse_ktx2_cubemap_mips(&bytes).is_some(),
        "the loader agrees"
    );
}

#[test]
fn the_header_refuses_what_the_loader_refuses() {
    for bytes in [
        build_cubemap_ktx2(1, R8G8B8A8_SRGB, 1),
        build_cubemap_ktx2(1, R16G16B16A16_SFLOAT, 6),
        vec![0; 80],
    ] {
        assert_eq!(shape(&bytes), None);
        assert!(parse_ktx2_cubemap_mips(&bytes).is_none());
    }
    assert_eq!(read_cube_shape("no/such/cube.ktx2"), None);
}

/// A reflection cubemap re-baked at a path the probe array already read (or failed
/// to) is read again on the next frame (#1207).
#[test]
fn gpu_a_cubemap_rebaked_at_a_cached_path_reloads() {
    use crate::core::fs::written::contract::{follows_rebakes, Bake};
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let size = |bake| if bake == Bake::A { 2 } else { 4 };
    let path = crate::test_temp::dir().join("probe.ktx2");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            let bytes = build_cubemap_ktx2(size(bake), R8G8B8A8_SRGB, 6);
            crate::core::fs::written::write(std::path::Path::new(p), &bytes).unwrap();
        },
        |p| {
            renderer.refresh_textures();
            let mut scene = crate::scene::Scene::new();
            let i = (scene.reflection_probes).add(glam::Vec3::ZERO, glam::Vec3::ONE);
            scene.reflection_probes.set_cubemap(i, p.to_string());
            renderer.update_reflection_probes(&scene, glam::Vec3::ZERO);
            let probes = &renderer.reflection_probes;
            (probes.probe_for(glam::Vec3::ZERO) != 0).then(|| probes.face())
        },
        |loaded, bake| *loaded == Some(size(bake)),
    );
}
