//! A `Shadowmask` bake as an authoring action (#1695): its mask pages beside the
//! lightmaps, each light's channel and the lights that fell back recorded in the
//! scene, saved and loaded with it, and a re-bake on `BakedIndirect` clearing them.

use std::path::Path;

use glam::Vec3;

use super::tests::{quick, scene, scene_path};
use super::*;
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_lighting::lightmap::BakeScene;
use rusty_scene::authoring::default_light;
use rusty_scene::lighting::MixedLightingMode;

/// `count` shadowed `Mixed` lamps over the floor, all overlapping.
fn lamps(scene: &mut Scene, count: usize) -> Vec<u32> {
    let light = LightComponent {
        light_type: LightType::Point,
        mode: LightMode::Mixed,
        cast_shadows: true,
        range: 4.0,
        ..default_light()
    };
    (0..count)
        .map(|k| {
            let id = scene.add_entity(format!("Lamp {k}"));
            scene.world.transform_mut(id).unwrap().position = Vec3::new(0.1 * k as f32, 1.0, 0.0);
            scene.world.set_light(id, Some(light.clone()));
            id
        })
        .collect()
}

#[test]
fn a_shadowmask_bake_writes_mask_pages_and_records_channels_and_fallbacks() {
    let (mut scene, floor, _) = scene();
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    let ids = lamps(&mut scene, 5);
    let path = scene_path("shadowmask");
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &quick()),
        Ok(1)
    );
    let set = &scene.lightmaps;
    assert_eq!(set.masks.len(), set.pages.len());
    assert!(set.masks[0].contains("lightmap_mask_0_") && Path::new(&set.masks[0]).exists());
    assert_eq!(
        (0..4).map(|k| set.mask_channel(ids[k])).collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2), Some(3)]
    );
    assert_eq!(set.fell_back.iter().copied().collect::<Vec<_>>(), [ids[4]]);
    // The fifth bakes as `Baked`: its direct light is in the lightmap, and the
    // frame draws it as `Baked`.
    let input = BakeScene::gather(&scene, &|_| None);
    let direct: Vec<bool> = input.lights.iter().map(|l| l.bakes_direct).collect();
    assert_eq!(direct, [false, false, false, false, true]);
    let light = scene.world.light(ids[4]).unwrap().clone();
    assert_eq!(scene.light_mode(ids[4], &light), LightMode::Baked);
    assert_eq!(scene.mask_channel(ids[0]), Some(0));
    assert!(scene.lightmaps.get(floor).is_some());

    // Saved and loaded with the scene.
    let json = serde_json::to_string(&*scene.lightmaps).unwrap();
    let back: rusty_scene::lighting::lightmap_set::LightmapSet =
        serde_json::from_str(&json).unwrap();
    assert_eq!(back, *scene.lightmaps);

    // Back on `BakedIndirect`: the frame ignores the mask at once, the next bake
    // drops it, and its colour pages are a `BakedIndirect` bake's.
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::BakedIndirect;
    assert_eq!(scene.mask_channel(ids[0]), None);
    let masked_pages = scene.lightmaps.pages.clone();
    let old_mask = scene.lightmaps.masks[0].clone();
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let set = &scene.lightmaps;
    assert!(set.masks.is_empty() && set.channels.is_empty() && set.fell_back.is_empty());
    assert!(!Path::new(&old_mask).exists(), "the old mask page is gone");
    assert_ne!(
        set.pages, masked_pages,
        "the fifth lamp's direct light left the page"
    );
    let light = scene.world.light(ids[4]).unwrap().clone();
    assert_eq!(scene.light_mode(ids[4], &light), LightMode::Mixed);
}

#[test]
fn a_bake_with_four_lamps_or_on_baked_indirect_bakes_the_same_pages() {
    let bake_pages = |mode: MixedLightingMode, name: &str| {
        let (mut scene, _, _) = scene();
        scene.lighting_settings.mixed_lighting_mode = mode;
        lamps(&mut scene, 4);
        bake_scene_lightmaps(&mut scene, Some(&scene_path(name)), &quick()).unwrap();
        scene.lightmaps.clone()
    };
    let masked = bake_pages(MixedLightingMode::Shadowmask, "four_masked");
    let plain = bake_pages(MixedLightingMode::BakedIndirect, "four_plain");
    let name = |p: &String| Path::new(p).file_name().unwrap().to_owned();
    assert_eq!(
        masked.pages.iter().map(name).collect::<Vec<_>>(),
        plain.pages.iter().map(name).collect::<Vec<_>>(),
        "a mask never changes the lightmap"
    );
    assert!(plain.masks.is_empty() && !masked.masks.is_empty());
}
