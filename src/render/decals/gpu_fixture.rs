//! The surface-decal GPU tests' stage (#638): a floor seen straight down from 12 m,
//! no sun and no sky texture, and helpers to stamp, light and read it back.

use glam::{Vec2, Vec3};

use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::decal::DecalSpec;
use crate::scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 96;

/// Looking straight down at the floor's centre.
pub(super) fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 12.0, 0.0), -90.0, -89.0)
}

/// A 30 m floor of `floor` (also defined as the library material `"floor"`), with
/// `ambient` intensity and nothing else lighting it.
pub(super) fn stage(floor: MaterialAsset, ambient: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = ambient;
    let id = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(id).unwrap().scale = Vec3::new(30.0, 1.0, 30.0);
    scene.materials.insert("floor".into(), floor);
    let material = "floor".to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent { material }));
    scene
}

/// A floor material: `albedo` grey, fully rough.
pub(super) fn grey(albedo: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [albedo; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    }
}

/// A point light `height` metres over the floor's centre.
pub(super) fn lamp(scene: &mut Scene, height: f32) {
    let id = create_entity(scene, "Lamp", Some(Primitive::PointLight));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, height, 0.0);
    let mut light = scene.world.light(id).unwrap().clone();
    (light.range, light.intensity) = (12.0, 30.0);
    scene.world.set_light(id, Some(light));
}

/// Stamp a 4 m decal at the floor's centre, projected against `normal`.
pub(super) fn stamp(scene: &mut Scene, normal: Vec3, spec: DecalSpec) {
    let spec = DecalSpec {
        size: 4.0,
        depth: 2.0,
        ..spec
    };
    scene.spawn_decal(Vec3::ZERO, normal, spec);
}

/// A red decal that changes only the albedo (the positional `Decals.Spawn`).
pub(super) fn red() -> DecalSpec {
    DecalSpec {
        color: [1.0, 0.0, 0.0, 1.0],
        ..DecalSpec::default()
    }
}

/// Render `scene` (in play mode when `playing`, so its cameras stack) and return
/// the RGB under the floor's centre with the frame's counters.
pub(super) fn centre(
    renderer: &mut Renderer,
    scene: &Scene,
    playing: bool,
) -> ([i32; 3], RenderCounters) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H, 2);
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

/// Whether two pixels match within `tolerance` per channel.
pub(super) fn close(a: [i32; 3], b: [i32; 3], tolerance: i32) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tolerance)
}
