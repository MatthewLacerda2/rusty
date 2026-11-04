//! A lightmap page's two paths render alike (#1694): the BC6H KTX2 page (the GPU's
//! own BC decoder, lavapipe's in CI) and its RGBM PNG fallback, decoded on the CPU
//! to `Rgba16Float`, light a floor to within a few levels of each other, colour for
//! colour: a channel swapped or a range clipped in the encoder shows here, where a
//! grey scene would hide it. Skips when no GPU / software adapter is present.

use glam::Vec3;
use rusty::asset::lightmap_page;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::{colour_page, encode_texels, LightmapEntry};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;
const EDGE: u32 = 64;

/// Warm HDR light on the left half (past 1, under RGBM's 8), cool on the right.
fn light() -> Vec<Vec3> {
    (0..EDGE * EDGE)
        .map(|i| match i % EDGE < EDGE / 2 {
            true => Vec3::new(1.6, 0.5, 0.08),
            false => Vec3::new(0.06, 0.3, 0.9),
        })
        .collect()
}

/// A floor wearing `page` as its lightmap, under a black sky and no lights.
fn scene(page: String) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let floor = scene.add_entity("Floor".to_string());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    scene.lightmaps.pages = vec![page];
    scene.lightmaps.insert(LightmapEntry {
        entity: floor,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene
}

fn shot(scene: &Scene, name: &str) -> Option<image::RgbImage> {
    let camera = Camera::new(Vec3::new(0.0, 6.0, 0.0), 0.0, -89.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1694_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).unwrap().to_rgb8())
}

#[test]
fn the_bc6h_page_and_its_png_fallback_light_a_floor_alike() {
    let dir = crate::temp::dir().join("rusty_gpu_1694");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let texels = light();
    let ktx2 = dir.join("page.ktx2");
    std::fs::write(
        &ktx2,
        lightmap_page::write(&colour_page(&texels, EDGE, 2, &|| false).unwrap()),
    )
    .unwrap();
    let png = ktx2.with_extension("png");
    image::RgbaImage::from_raw(EDGE, EDGE, encode_texels(&texels))
        .unwrap()
        .save(&png)
        .unwrap();
    let as_str = |p: &std::path::Path| p.to_string_lossy().into_owned();
    let Some(compressed) = shot(&scene(as_str(&ktx2)), "ktx2") else {
        eprintln!("[1694] no GPU/software adapter — skipping");
        return;
    };
    let fallback = shot(&scene(as_str(&png)), "png").unwrap();
    let worst = compressed
        .pixels()
        .zip(fallback.pixels())
        .flat_map(|(a, b)| (0..3).map(move |c| a[c].abs_diff(b[c])))
        .max()
        .unwrap();
    eprintln!("[1694] worst channel difference {worst}");
    assert!(worst <= 8, "the two paths differ by {worst} levels");
    // Warm stays warm and cool stays cool, in both: no channel moved. The halves
    // meet across the frame's middle; which half is up is the camera's business.
    let warm = |p: &image::Rgb<u8>| p[0] > p[1] && p[1] > p[2];
    let cool = |p: &image::Rgb<u8>| p[2] > p[1] && p[1] > p[0];
    for (img, what) in [(&fallback, "png"), (&compressed, "bc6h")] {
        let (a, b) = (img.get_pixel(32, 8), img.get_pixel(32, 56));
        let split = (warm(a) && cool(b)) || (cool(a) && warm(b));
        assert!(
            split,
            "{what}: one warm and one cool half, got {a:?} and {b:?}"
        );
    }
}
