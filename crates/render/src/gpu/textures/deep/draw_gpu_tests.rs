use std::path::Path;

use glam::Vec3;
use image::{ImageBuffer, Rgba};

use crate::gpu::textures::freshness_tests::temp_png;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_asset::sidecar::{set_texture, Compression, TextureSettings, TextureType};
use rusty_components::{
    LightComponent, LightMode, MaterialAsset, MaterialComponent, Tonemap, UvMapping,
};
use rusty_scene::create::defaults::{default_light, default_post_processing};
use rusty_scene::{Camera, MeshComponent, Scene};

// x and y of 0.5 as bytes; the 16-bit twin holds each times 257, the same value.
const TILT: [u8; 3] = [191, 191, 218];
const FLAT: [u8; 3] = [128, 128, 255];

fn map(name: &str, texel: [u8; 3], deep: bool) -> String {
    let path = temp_png(name);
    let [x, y, z] = texel;
    if deep {
        let wide = |v: u8| u16::from(v) * 257;
        let px = Rgba([wide(x), wide(y), wide(z), u16::MAX]);
        ImageBuffer::from_pixel(4, 4, px).save(&path).unwrap();
    } else {
        ImageBuffer::from_pixel(4, 4, Rgba([x, y, z, 255]))
            .save(&path)
            .unwrap();
    }
    let settings = TextureSettings {
        texture_type: TextureType::NormalMap,
        compression: Compression::None,
        ..Default::default()
    };
    set_texture(Path::new(&path), settings).unwrap();
    path
}

// A matte box under a lamp off its corner, so a tilt shows on each face.
fn lit_box(normal_map: &str, uv_mapping: UvMapping) -> Scene {
    let mut scene = Scene::new();
    crate::test_gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    let m = Some(MaterialComponent::new("m".to_string()));
    scene.world.set_material(id, m);
    let asset = MaterialAsset {
        base_color: [0.5; 3],
        metallic: 0.0,
        roughness: 1.0,
        normal_map: Some(normal_map.into()),
        uv_mapping,
        blend_sharpness: 64.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    let mut vc = default_post_processing();
    (vc.active, vc.bloom.active, vc.ssr.active) = (true, false, false);
    vc.tonemapping.mode = Tonemap::None;
    scene.world.set_post_processing(id, Some(vc));
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::splat(6.0);
    let light = LightComponent {
        intensity: 150.0,
        range: 20.0,
        cast_shadows: false,
        mode: LightMode::Realtime,
        ..default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

// The face's centre seen head-on: +z (yaw -90) or +x (yaw 180).
fn centre(renderer: &mut Renderer, scene: &Scene, yaw: f32) -> [u8; 3] {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    let eye = Vec3::new(yaw.to_radians().cos(), 0.0, yaw.to_radians().sin()) * -12.0;
    renderer.render(&mut view, scene, &Camera::new(eye, yaw, 0.0), &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 16, 16);
    let i = (8 * 16 + 8) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

fn off(a: [u8; 3], b: [u8; 3]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(y)).max().unwrap()
}

// Signed texels decoded as biased ones (a swapped `select` in `maps::stored_normal`,
// a triplanar lookup dropping `signed`) draw far off the 8-bit twin.
#[test]
fn gpu_a_16_bit_normal_map_draws_like_its_8_bit_twin() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let deep = map("tilt_16_2303", TILT, true);
    let narrow = map("tilt_8_2303", TILT, false);
    let flat = map("flat_8_2303", FLAT, false);
    let cases = [
        (UvMapping::Uv0, -90.0),
        (UvMapping::Triplanar, -90.0),
        (UvMapping::Triplanar, 180.0),
    ];
    for (mapping, yaw) in cases {
        let mut draw = |path: &str| centre(&mut renderer, &lit_box(path, mapping), yaw);
        let (wide, twin, plain) = (draw(&deep), draw(&narrow), draw(&flat));
        eprintln!("{mapping:?} at {yaw}: 16-bit {wide:?}, 8-bit {twin:?}, flat {plain:?}");
        assert!(off(twin, plain) > 12, "the tilt shows: {twin:?} {plain:?}");
        // Measured on lavapipe: 0. One step allows the half float's rounding.
        assert!(
            off(wide, twin) <= 1,
            "{mapping:?} at {yaw}: {wide:?} {twin:?}"
        );
    }
}
