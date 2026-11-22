use glam::Vec3;

use crate::Renderer;
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

pub(in crate::postfx::effects) const W: u32 = 320;
pub(in crate::postfx::effects) const H: u32 = 240;
pub(in crate::postfx::effects) const WARM: u32 = 27;
pub(super) const ON: f32 = 1.0;
pub(super) const DEFAULT: f32 = CameraComponent::DEFAULT_TAA_ANTI_FLICKER;

pub(super) fn renderer() -> Option<Renderer> {
    crate::test_gpu::headless_or_skip(W, H)
}

pub(in crate::postfx::effects) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0)
}

fn paint(s: &mut Scene, id: u32, name: &str, asset: MaterialAsset) {
    s.materials.insert(name.into(), asset);
    s.world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
}

pub(super) fn block(
    s: &mut Scene,
    name: &str,
    (at, size): (Vec3, Vec3),
    asset: MaterialAsset,
) -> u32 {
    let id = create_entity(s, name, Some(Primitive::Box));
    let mut t = s.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, size);
    drop(t);
    paint(s, id, name, asset);
    id
}

pub(super) fn matte(grey: f32) -> MaterialAsset {
    let base_color = [grey; 3];
    MaterialAsset {
        base_color,
        roughness: 1.0,
        ..MaterialAsset::default()
    }
}

pub(in crate::postfx::effects) fn room() -> Scene {
    let mut s = Scene::new();
    (s.skybox_path, s.ambient_intensity) = (String::new(), 0.0);
    let cam = s.add_entity("Camera".to_string());
    let camera = CameraComponent {
        taa_active: true,
        fxaa_active: false,
        taa_anti_flicker: 0.0,
        ..Default::default()
    };
    s.world.set_camera(cam, Some(camera));
    let mut volume = default_post_processing();
    (volume.ssr.active, volume.ssr.min_smoothness) = (true, 0.3);
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let bronze = MaterialAsset {
        base_color: [0.8, 0.55, 0.35],
        metallic: 1.0,
        roughness: 0.35,
        ..MaterialAsset::default()
    };
    block(
        &mut s,
        "Ceiling",
        (Vec3::new(0.0, 3.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        bronze,
    );
    let shell = [
        (Vec3::new(0.0, -0.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        (Vec3::new(0.0, 1.5, -10.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(-6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(0.0, 1.5, 2.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(0.0, 2.8, -3.0), Vec3::new(0.01, 0.5, 0.01)),
    ];
    for (i, place) in shell.into_iter().enumerate() {
        block(&mut s, &format!("Shell{i}"), place, matte(0.03));
    }
    let bulb = create_entity(&mut s, "Bulb", Some(Primitive::Sphere));
    let mut t = s.world.transform_mut(bulb).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 2.55, -3.0), Vec3::splat(0.08));
    drop(t);
    let glow = MaterialAsset {
        emissive: [40.0, 32.0, 22.0],
        ..MaterialAsset::default()
    };
    paint(&mut s, bulb, "Bulb", glow);
    s
}

pub(super) fn anti_flicker(mut s: Scene, value: f32) -> Scene {
    let cam = s.find_entity_by_name("Camera").unwrap();
    s.world.camera_mut(cam).unwrap().taa_anti_flicker = value;
    s
}
