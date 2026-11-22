//! Material-map visual test: the renderer samples a material's map textures, not
//! just its scalar factors. The shader gates ambient diffuse by `(1 - metallic)`, so
//! `scalar 1 + black map (.b=0)` bright proves the MAP is sampled (1.0 * 0.0 = 0).
//! Skips without an adapter (`capture` returns `Ok(false)`).

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, PostProcessingComponent, Scene};

/// 2x2 black RGBA PNG. Metallic reads the BLUE channel (glTF convention), so
/// `.b = 0` drives metallic to zero.
fn write_black_png(path: &std::path::Path) {
    let mut img = image::RgbaImage::new(2, 2);
    for px in img.pixels_mut() {
        *px = image::Rgba([0, 0, 0, 255]);
    }
    img.save(path).expect("write metallic png");
}

fn vc() -> PostProcessingComponent {
    PostProcessingComponent {
        bloom: rusty::components::Bloom {
            active: false,
            threshold: 0.6,
            ..Default::default()
        },
        ssr: rusty::components::Ssr {
            active: false,
            ..Default::default()
        },
        ..rusty::edit::authoring::default_post_processing()
    }
}

/// An ambient-lit box with the given metallic scalar and optional metallic map.
fn scene(metallic: f32, metallic_map: Option<String>) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;

    let id = scene.add_entity("Box".to_string());
    // Large, so the mean brightness is the surface's, not the background's.
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("mat".to_string())));
    scene.world.set_post_processing(id, Some(vc()));
    // A volume rides on its camera: that camera is the view's.
    scene
        .world
        .set_camera(id, Some(rusty::components::CameraComponent::default()));
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.9, 0.9, 0.9],
            metallic,
            roughness: 1.0,
            metallic_map: metallic_map.map(Into::into),
            ..MaterialAsset::default()
        },
    );
    scene
}

#[test]
fn metallic_map_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);
    let map_png = tmp("rusty_metallic_map.png");
    write_black_png(&map_png);
    let map = map_png.to_string_lossy().into_owned();

    let p_bright = tmp("rusty_metal_s0.png");
    let p_dark = tmp("rusty_metal_s1.png");
    let p_mapped = tmp("rusty_metal_mapped.png");

    let c0 = capture(&scene(0.0, None), &cam, &p_bright, 96, 96).expect("capture");
    let c1 = capture(&scene(1.0, None), &cam, &p_dark, 96, 96).expect("capture");
    let c2 = capture(&scene(1.0, Some(map)), &cam, &p_mapped, 96, 96).expect("capture");
    if !c0 || !c1 || !c2 {
        eprintln!("[material-maps] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let bright = mean_brightness(&p_bright);
    let dark = mean_brightness(&p_dark);
    let mapped = mean_brightness(&p_mapped);
    eprintln!("[material-maps] metallic scalar0={bright:.3} scalar1={dark:.3} mapped={mapped:.3}");
    assert!(
        bright > dark + 2.0,
        "metallic scalar must gate ambient diffuse (scalar0={bright:.3}, scalar1={dark:.3})"
    );
    assert!(
        mapped > dark + 2.0,
        "the metallic MAP must drive metallic to 0 like the scalar (mapped={mapped:.3}, scalar1={dark:.3})"
    );
}
