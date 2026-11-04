//! Directional specular from a directional lightmap (#1695, step 1): a `Baked` point
//! lamp over a static plane, every bit of whose light comes from the lightmap.
//!
//! * `highlight`: on a glossy plane the baked lobe's highlight sits where the same
//!   lamp's `Realtime` one does, its peak within a stated factor; at roughness 1 it
//!   changes nothing visible.
//! * `capture`: a bake capture never draws it.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

mod capture;
mod highlight;

use glam::Vec3;
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::bake::lightmap::bake_scene_lightmaps;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::BakeSettings;
use rusty::scene::Scene;

/// A 6 x 6 static plane of `roughness` under a point lamp of `mode` a metre up,
/// under a black sky.
pub(super) fn scene(mode: LightMode, roughness: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let plane = scene.add_entity("Plane".to_string());
    scene
        .world
        .set_mesh(plane, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(plane, true);
    scene.world.transform_mut(plane).unwrap().scale = Vec3::splat(0.4);
    let material = MaterialAsset {
        base_color: [0.5, 0.5, 0.5],
        roughness,
        ..MaterialAsset::default()
    };
    scene.materials.insert("plane".to_string(), material);
    let plane_material = MaterialComponent::new("plane".to_string());
    scene.world.set_material(plane, Some(plane_material));
    let lamp = scene.add_entity("Lamp".to_string());
    let light = LightComponent {
        light_type: LightType::Point,
        intensity: 1.0,
        range: 8.0,
        mode,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene.world.transform_mut(lamp).unwrap().position = Vec3::Y;
    scene
}

/// Bake `scene`'s lightmaps into a temp folder named `name`; returns its path.
pub(super) fn bake(scene: &mut Scene, name: &str, directional: bool) -> String {
    let dir = crate::temp::dir().join(format!("rusty_gpu_baked_spec_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 10.0,
        samples: 16,
        bounces: 1,
        seed: 5,
        max_lightmap_size: 64,
        filter_radius: 1,
        directional,
    };
    assert_eq!(bake_scene_lightmaps(scene, Some(&path), &settings), Ok(1));
    path
}
