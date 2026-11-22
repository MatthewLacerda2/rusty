use glam::Vec3;

use super::super::fixture::frame;
use super::gpu_tests::{level, resident};
use crate::warm::Warming;
use crate::Renderer;
use rusty_components::particle::{Particle, ParticleEmitterComponent};
use rusty_components::{ParticleRender, ParticleRenderMode};
use rusty_scene::create::Primitive;
use rusty_scene::parse_scene_text;
use rusty_scene::serialize::SceneData;
use rusty_scene::Scene;

fn with_casings(prims: &[Primitive], panel: &str, live: usize) -> (Scene, u32) {
    let mut scene = level(prims, (panel, 48));
    let id = scene.add_entity("Casings".to_string());
    let mut emitter = ParticleEmitterComponent {
        render: ParticleRender {
            mode: ParticleRenderMode::Mesh,
            mesh: Some("Capsule".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let at = Vec3::new(0.0, 0.5, -4.0);
    emitter.runtime.particles = vec![Particle::at(at, 0.3, [1.0; 4], 99.0); live];
    scene.world.set_particles(id, Some(emitter));
    (scene, id)
}

fn has(r: &Renderer, id: &str) -> bool {
    resident(r).iter().any(|m| m == id)
}

#[test]
fn gpu_a_mesh_emitter_emitting_nothing_after_a_load_keeps_its_mesh() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    frame(&mut r, &with_casings(&[Primitive::Box], "A", 1).0);
    let (mut b, emitter) = with_casings(&[Primitive::Sphere], "B", 0);
    frame(&mut r, &b);
    assert!(has(&r, "Capsule|"), "kept: {:?}", resident(&r));
    let mut e = b.world.particles_mut(emitter).expect("an emitter");
    e.runtime.particles = vec![Particle::at(Vec3::new(0.0, 0.5, -4.0), 0.3, [1.0; 4], 99.0)];
    drop(e);
    let (_, shot) = frame(&mut r, &b);
    assert_eq!(shot.uploads.count, 0, "the first casing uploads nothing");
}

#[test]
fn gpu_a_play_entry_that_loaded_a_level_holds_the_edit_scene() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut edit = level(&[Primitive::Box], ("A", 48));
    frame(&mut r, &edit);
    let mut level1 = level(&[Primitive::Sphere], ("B", 48));
    level1.set_playing(true);
    let (_, first) = frame(&mut r, &level1);
    assert_eq!(
        first.mesh_count,
        4,
        "the edit scene is held: {:?}",
        resident(&r)
    );
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop uploads nothing");
}

fn document(scene: &Scene) -> SceneData {
    let sparse = r#"{"entities": [], "next_entity_id": 1, "selected_entity_id": null}"#;
    let mut data = parse_scene_text(sparse, "warm.scene").expect("a sparse scene");
    data.entities = scene.world.collect_entities();
    data.skybox_path = scene.skybox_path.clone();
    data
}

fn warmed(r: &mut Renderer, data: &SceneData) -> Warming {
    let mut warming = Warming::plan(data);
    while r.warm_step(data, &mut warming) {}
    assert!(warming.is_done());
    warming
}

#[test]
fn gpu_a_release_during_a_warm_up_keeps_what_it_made_until_it_is_dropped() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    frame(&mut r, &level(&[Primitive::Box], ("A", 48)));
    let next = level(&[Primitive::Capsule], ("C", 48));
    let data = document(&next);
    let warming = warmed(&mut r, &data);
    frame(&mut r, &level(&[Primitive::Sphere], ("B", 48)));
    assert!(
        has(&r, "Capsule|") && has(&r, "Asset|test://C"),
        "{:?}",
        resident(&r)
    );
    let (_, swapped) = frame(&mut r, &next);
    assert_eq!(swapped.uploads.count, 0, "the swap uploads nothing");
    drop(warming);
    let abandoned = warmed(&mut r, &document(&level(&[Primitive::Plane], ("D", 48))));
    drop(abandoned);
    frame(&mut r, &level(&[Primitive::Box], ("A", 48)));
    assert!(
        !has(&r, "Plane|") && !has(&r, "Asset|test://D"),
        "{:?}",
        resident(&r)
    );
}

#[test]
fn gpu_a_refused_mesh_re_uploaded_without_clusters_is_no_longer_refused() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    r.hiz.clusters.pool.limit = 4 * 6;
    frame(&mut r, &level(&[], ("A", 48)));
    let id = crate::MeshId("Asset|test://A".into());
    assert!(
        r.retained.residency.refused.contains(&id),
        "the pool refused it"
    );
    r.attach_clusters(&id, (None, &[]));
    assert!(
        r.retained.residency.refused.is_empty(),
        "nothing left to retry"
    );
}
