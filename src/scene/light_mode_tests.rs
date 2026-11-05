//! A light changed after a `Shadowmask` bake (#1695) draws as its new mode next
//! frame: a fallen-back lamp set to `Realtime` draws live, and a masked lamp set to
//! `Realtime` (or its shadows turned off) is masked by no stale channel.

use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::authoring::default_light;
use crate::scene::lighting::MixedLightingMode;
use crate::scene::Scene;

/// A Shadowmask scene with one shadowed `Mixed` lamp the "bake" gave channel 2,
/// and one that fell back.
fn baked() -> (Scene, u32, u32) {
    let mut scene = Scene::default();
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    let light = LightComponent {
        light_type: LightType::Point,
        mode: LightMode::Mixed,
        cast_shadows: true,
        ..default_light()
    };
    let [masked, fallen] = ["Masked", "Fallen"].map(|name| {
        let id = scene.add_entity(name.to_string());
        scene.world.set_light(id, Some(light.clone()));
        id
    });
    let set = &mut *scene.lightmaps;
    set.channels.insert(masked, 2);
    set.fell_back.insert(fallen);
    (scene, masked, fallen)
}

fn set_mode(scene: &mut Scene, id: u32, mode: LightMode) -> LightComponent {
    let mut light = scene.world.light(id).unwrap().clone();
    light.mode = mode;
    scene.world.set_light(id, Some(light.clone()));
    light
}

#[test]
fn a_fallen_back_lamp_set_to_another_mode_draws_as_that_mode() {
    let (mut scene, _, fallen) = baked();
    let light = scene.world.light(fallen).unwrap().clone();
    assert_eq!(scene.light_mode(fallen, &light), LightMode::Baked);
    let light = set_mode(&mut scene, fallen, LightMode::Realtime);
    assert_eq!(scene.light_mode(fallen, &light), LightMode::Realtime);
    assert!(scene.light_mode(fallen, &light).renders_live(false));
}

#[test]
fn a_masked_lamp_no_longer_masked_by_mode_loses_its_channel_next_frame() {
    let (mut scene, masked, _) = baked();
    assert_eq!(scene.mask_channel(masked), Some(2));
    set_mode(&mut scene, masked, LightMode::Realtime);
    assert_eq!(scene.mask_channel(masked), None, "Realtime: no mask");
    set_mode(&mut scene, masked, LightMode::Mixed);
    assert_eq!(scene.mask_channel(masked), Some(2));
    let mut light = scene.world.light(masked).unwrap().clone();
    light.cast_shadows = false;
    scene.world.set_light(masked, Some(light));
    assert_eq!(scene.mask_channel(masked), None, "shadows off: no mask");
}

#[test]
fn each_bake_verb_says_once_when_the_other_half_is_missing() {
    use super::{NO_CHANNELS, PROBES_STALE};
    let (mut scene, _, _) = baked();
    assert_eq!(scene.shadowmask_note(true), None, "no probe: nothing stale");
    scene.probes.add_probe(glam::Vec3::ZERO);
    assert_eq!(scene.shadowmask_note(true), Some(PROBES_STALE));
    assert_eq!(
        scene.shadowmask_note(false),
        None,
        "the record has channels"
    );
    scene.lightmaps.channels.clear();
    assert_eq!(scene.shadowmask_note(false), Some(NO_CHANNELS));
    assert_eq!(scene.shadowmask_note(true), None);
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::BakedIndirect;
    assert_eq!(scene.shadowmask_note(false), None, "off: no note");
}
