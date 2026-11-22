//! Emissive-factor visual test: the renderer reads a material's flat `emissive` factor and adds
//! it to the lit colour through `MaterialData`. Zero ambient and no lights on a black-base
//! box leave the emissive term the only change between shots; a factor >1.0 also exercises
//! the HDR target and bloom bright-pass.

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, PostProcessingComponent, Scene};

fn vc(bloom: bool) -> PostProcessingComponent {
    PostProcessingComponent {
        bloom: rusty::components::Bloom {
            active: bloom,
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

fn scene(emissive: [f32; 3], bloom: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;

    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("mat".to_string())));
    scene.world.set_post_processing(id, Some(vc(bloom)));
    // A volume rides on its camera: that camera is the view's.
    scene
        .world
        .set_camera(id, Some(rusty::components::CameraComponent::default()));
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.0, 0.0, 0.0],
            emissive,
            ..MaterialAsset::default()
        },
    );
    scene
}

#[test]
fn emissive_factor_visibly_lights_an_otherwise_dark_surface() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let p_off = tmp("rusty_emissive_off.png");
    let p_dim = tmp("rusty_emissive_dim.png");
    let p_bloom = tmp("rusty_emissive_bloom.png");

    let c0 = capture(&scene([0.0, 0.0, 0.0], false), &cam, &p_off, 96, 96).expect("capture");
    let c1 = capture(&scene([0.6, 0.6, 0.6], false), &cam, &p_dim, 96, 96).expect("capture");
    let c2 = capture(&scene([6.0, 6.0, 6.0], true), &cam, &p_bloom, 96, 96).expect("capture");
    if !c0 || !c1 || !c2 {
        eprintln!("[emissive] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let off = mean_brightness(&p_off);
    let dim = mean_brightness(&p_dim);
    let bloom = mean_brightness(&p_bloom);
    eprintln!("[emissive] off={off:.3} dim={dim:.3} bloom={bloom:.3}");

    assert!(
        dim > off + 2.0,
        "emissive factor must brighten an unlit surface (off={off:.3}, dim={dim:.3})"
    );
    assert!(
        bloom > dim + 2.0,
        "a >1.0 emissive factor must clear a <1.0 one via the HDR/bloom path \
         (bloom={bloom:.3}, dim={dim:.3})"
    );
}
