//! Play, then Stop, through the real restore (`edit::snapshot::SceneSnapshot`): the
//! material library swapped back, the entities respawned under their ids, the decals
//! and per-entity overrides dropped. A renderer kept across it draws what a new one
//! draws, and what it drew before Play. Skips with no adapter.

use glam::Vec3;

use rusty::components::MaterialComponent;
use rusty::edit::snapshot::SceneSnapshot;
use rusty::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty::scene::create::{create_entity, Primitive};
use rusty::scene::decal::DecalSpec;
use rusty::scene::{Camera, MaterialAsset, Scene};

const RES: u32 = 32;
const PROPS: u32 = 6;

/// A sun over `PROPS` primitives on a row, every other one wearing `red`.
fn yard() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty::scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let red = MaterialAsset {
        base_color: [0.9, 0.1, 0.1],
        ..MaterialAsset::default()
    };
    scene.materials.insert("red".into(), red);
    for i in 0..PROPS {
        let kind = [Primitive::Box, Primitive::Sphere, Primitive::Plane][i as usize % 3];
        let id = create_entity(&mut scene, &format!("Prop_{i}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(i as f32 - 2.0, 0.0, -4.0);
        let red = (i % 2 == 0).then(|| MaterialComponent::new("red"));
        scene.world.set_material(id, red);
    }
    scene
}

/// Render `scene` once down the row; its pixels and counters.
fn frame(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, RenderCounters) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let eye = Camera::new(Vec3::new(0.0, 1.0, 3.0), -90.0, -10.0);
    r.render(&mut view, scene, &eye, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, texture, RES, RES);
    (px, r.frame_counters)
}

#[test]
fn gpu_a_play_restore_draws_the_edit_world_a_new_renderer_draws() {
    let Some((mut r, pair)) = rusty::render::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard();
    let (edit, _) = frame(&mut r, &scene);
    let snapshot = SceneSnapshot::capture(&scene);
    // Play: a prop gone, one moved, the shared material recoloured, a decal stamped.
    let props = scene.world.ids_with_mesh();
    scene.world.despawn(props[0]);
    scene.world.transform_mut(props[1]).unwrap().position = Vec3::new(0.0, 1.5, -2.0);
    scene.materials.get_mut("red").unwrap().base_color = [0.1, 0.9, 0.1];
    let spec = DecalSpec {
        size: 2.0,
        color: [0.0, 0.0, 1.0, 1.0],
        ..DecalSpec::default()
    };
    let stamp = Vec3::new(0.0, 0.5, -4.0);
    scene.spawn_decal(stamp, Vec3::Z, spec).expect("a decal");
    scene
        .shader_overrides
        .set(props[2], "tint".into(), vec![1.0]);
    let (played, _) = frame(&mut r, &scene);
    assert_ne!(played, edit, "Play's edits are on screen");
    // Stop.
    snapshot.restore(&mut scene);
    assert!(scene.decals.is_empty() && scene.shader_overrides.is_empty());
    let (restored, stopped) = frame(&mut r, &scene);
    assert_eq!(
        stopped.render_extracted, PROPS,
        "Stop rebuilds: {stopped:?}"
    );
    let mut other = pair.renderer(RES, RES);
    assert_eq!(
        restored,
        frame(&mut other, &scene).0,
        "a new renderer's frame"
    );
    assert_eq!(restored, edit, "the frame before Play");
}
