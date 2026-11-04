//! src/render/scene/fixture.rs — what the retained scene's tests share: a scene of
//! primitives, the objects a renderer holds for it, and the objects a build from
//! scratch gives.

use glam::Vec3;

use super::{RenderObject, RenderScene};
use crate::components::{MaterialAsset, MaterialComponent, RenderMode};
use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 32;

/// `n` primitives on a row (boxes, spheres, planes), every third red, every fifth
/// glass, a sun above; the library holds `red` and `glass`.
pub(super) fn yard(n: u32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let red = MaterialAsset {
        base_color: [0.9, 0.1, 0.1],
        ..MaterialAsset::default()
    };
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.4,
        ..MaterialAsset::default()
    };
    scene.materials.insert("red".into(), red);
    scene.materials.insert("glass".into(), glass);
    for i in 0..n {
        let kind = [Primitive::Box, Primitive::Sphere, Primitive::Plane][i as usize % 3];
        let id = create_entity(&mut scene, &format!("Prop_{i}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(i as f32 - 2.0, 0.0, -4.0);
        let material = match i {
            i if i % 5 == 4 => Some("glass"),
            i if i % 3 == 1 => Some("red"),
            _ => None,
        };
        let material = material.map(|m| MaterialComponent::new(m.to_string()));
        scene.world.set_material(id, material);
    }
    scene
}

/// The camera every test looks through: down the row.
pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.0, 3.0), -90.0, -10.0)
}

/// Render `scene` once into a fresh view; its pixels and counters.
pub(super) fn frame(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, RenderCounters) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, &eye(), &out, false);
    let texture = view.color_target().unwrap();
    let px = crate::render::readback::read_texture_rgba8(&r.device, &r.queue, texture, RES, RES);
    (px, r.frame_counters)
}

/// The objects `r` holds for `scene` now, in draw order.
pub(super) fn held(r: &Renderer, scene: &Scene) -> Vec<(u32, RenderObject)> {
    let kept = r.retained.scene(scene.id()).expect("extracted");
    kept.objects().map(|(id, o)| (id, o.clone())).collect()
}

/// The objects a build of `scene` from scratch gives, through `r`'s own handles
/// and resident buffers.
pub(super) fn fresh(r: &mut Renderer, scene: &Scene) -> Vec<(u32, RenderObject)> {
    let mut rs = RenderScene::default();
    r.rebuild_objects(scene, &mut rs);
    rs.objects().map(|(id, o)| (id, o.clone())).collect()
}
