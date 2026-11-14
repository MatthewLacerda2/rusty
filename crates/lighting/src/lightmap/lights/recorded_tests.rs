//! The probe bake's lights take the channels the last lightmap bake recorded
//! (#1695, `BakeScene::use_recorded_channels`), never fresh ones: after a light is
//! removed, the probes still write each kept light's occlusion on the channel the
//! frame masks it by, and a light the record gave none writes none.

use glam::{Vec3, Vec4};

use crate::lightmap::{assign_channels, mixed_occlusion, BakeScene};
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::authoring::{create_entity, default_light, Primitive};
use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::Scene;

/// Two overlapping shadowed `Mixed` lamps (`a`, `b`) over a static wall at x = 1,
/// with the lightmap bake's record written as `assign_channels` gives it.
fn recorded() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    let light = LightComponent {
        light_type: LightType::Point,
        mode: LightMode::Mixed,
        cast_shadows: true,
        range: 6.0,
        ..default_light()
    };
    let [a, b] = [-0.5, 0.0].map(|z| {
        let id = scene.add_entity("Lamp".to_string());
        scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 1.0, z);
        scene.world.set_light(id, Some(light.clone()));
        id
    });
    let wall = create_entity(&mut scene, "wall", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(wall).unwrap();
    (t.position, t.scale) = (Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.2, 4.0, 4.0));
    drop(t);
    scene.world.set_static(wall, true);
    let record = assign_channels(&scene);
    scene.lightmaps.channels = record.channels;
    (scene, a, b)
}

fn probe_lights(scene: &Scene) -> Vec<Option<u8>> {
    let mut statics = BakeScene::gather(scene, &|_| None);
    statics.use_recorded_channels(scene);
    statics.lights.iter().map(|l| l.mask_channel).collect()
}

#[test]
fn a_probe_bake_after_a_light_is_removed_keeps_the_recorded_channel() {
    let (mut scene, a, b) = recorded();
    assert_eq!(scene.mask_channel(b), Some(1));
    scene.world.set_light(a, None);
    assert_eq!(
        assign_channels(&scene).channels.get(&b),
        Some(&0),
        "afresh: 0"
    );
    assert_eq!(probe_lights(&scene), [Some(1)], "the record's");
    let mut statics = BakeScene::gather(&scene, &|_| None);
    statics.use_recorded_channels(&scene);
    let behind = mixed_occlusion(&statics, &[Vec3::new(2.0, 1.0, 0.0)]);
    assert_eq!(
        behind,
        [Vec4::new(1.0, 0.0, 1.0, 1.0)],
        "on b's channel only"
    );
}

#[test]
fn a_light_the_record_gave_no_channel_writes_no_occlusion() {
    let (mut scene, _, _) = recorded();
    scene.lightmaps.channels.clear();
    assert_eq!(probe_lights(&scene), [None, None]);
    let mut statics = BakeScene::gather(&scene, &|_| None);
    statics.use_recorded_channels(&scene);
    let behind = mixed_occlusion(&statics, &[Vec3::new(2.0, 1.0, 0.0)]);
    assert_eq!(behind, [Vec4::ONE]);
}
