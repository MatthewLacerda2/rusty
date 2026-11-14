//! Which entities read light probes (#240, #1704): dynamic ones, and a static mesh
//! only at Scale In Lightmap 0, which the lightmap bake leaves to the probes.

use glam::Vec3;

use super::entity_probe_sh;
use crate::ibl::cubemap_capture::CaptureMode;
use rusty_scene::authoring::{primitive_mesh_component, Primitive};
use rusty_scene::Scene;

#[test]
fn a_static_mesh_reads_probes_only_at_scale_in_lightmap_zero() {
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    let id = scene.add_entity("Wall".to_string());
    scene
        .world
        .set_mesh(id, primitive_mesh_component(Primitive::Box));
    let runtime = CaptureMode::default();
    let use_sh = |scene: &Scene| entity_probe_sh(scene, id, Vec3::ZERO, &runtime).0;
    assert_eq!(use_sh(&scene), 1, "a dynamic mesh reads them");
    scene.world.set_static(id, true);
    assert_eq!(use_sh(&scene), 0, "a lightmapped static one doesn't");
    scene.world.mesh_mut(id).unwrap().scale_in_lightmap = 0.0;
    assert_eq!(use_sh(&scene), 1, "at scale 0 it reads probes, as in Unity");
    let bake = CaptureMode {
        static_only: true,
        ..runtime
    };
    let in_bake = entity_probe_sh(&scene, id, Vec3::ZERO, &bake).0;
    assert_eq!(
        in_bake, 0,
        "a bounce-1 bake capture never reads its own probes"
    );
}
