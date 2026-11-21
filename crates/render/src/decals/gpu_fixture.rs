use glam::{Vec2, Vec3};

use crate::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::decal::DecalSpec;
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 96;

pub(super) fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 12.0, 0.0), -90.0, -89.0)
}

pub(super) fn stage(floor: MaterialAsset, ambient: f32) -> Scene {
    let mut scene = Scene::new();
    crate::test_gpu::black_sky(&mut scene);
    scene.ambient_intensity = ambient;
    let id = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(id).unwrap().scale = Vec3::new(30.0, 1.0, 30.0);
    scene.materials.insert("floor".into(), floor);
    let material = "floor".to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(material)));
    scene
}

pub(super) fn grey(albedo: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [albedo; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    }
}

pub(super) fn lamp(scene: &mut Scene, height: f32) {
    let id = rusty_scene::create::create_live_light(scene, "Lamp", Primitive::PointLight);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, height, 0.0);
    let mut light = scene.world.light(id).unwrap().clone();
    (light.range, light.intensity) = (12.0, 30.0);
    scene.world.set_light(id, Some(light));
}

pub(super) fn stamp(scene: &mut Scene, normal: Vec3, spec: DecalSpec) {
    let spec = DecalSpec {
        size: 4.0,
        depth: 2.0,
        ..spec
    };
    scene.spawn_decal(Vec3::ZERO, normal, spec);
}

pub(super) fn red() -> DecalSpec {
    DecalSpec {
        color: [1.0, 0.0, 0.0, 1.0],
        ..DecalSpec::default()
    }
}

pub(super) fn centre(
    renderer: &mut Renderer,
    scene: &Scene,
    playing: bool,
) -> ([i32; 3], RenderCounters) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    renderer.render(&mut view, scene, &camera(), &out, !playing);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, W, H);
    let at = camera().world_to_screen(Vec3::ZERO, Vec2::new(W as f32, H as f32));
    let (x, y) = (at.position.x as u32, H - 1 - at.position.y as u32);
    let i = ((y * W + x) * 4) as usize;
    let rgb = [px[i], px[i + 1], px[i + 2]].map(i32::from);
    (rgb, renderer.frame_counters)
}

pub(super) fn close(a: [i32; 3], b: [i32; 3], tolerance: i32) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tolerance)
}
