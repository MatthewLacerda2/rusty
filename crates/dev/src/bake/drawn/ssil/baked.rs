use crate::bake::lightmap::bake_scene_lightmaps;
use rusty_components::{LightMode, Ssil};
use rusty_render::test_gpu::ssil::{
    energy, on, patch, point, renderer, scene, set_ssil, settled, FLOOR, LAMP,
};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

/// The share of its light an all-baked scene may gain with SSIL on: render's `INDIRECT_GAIN`.
pub(super) const BAKED_GAIN: f32 = 0.005;

pub(super) fn baked(mode: LightMode, name: &str) -> (Scene, u32) {
    let (mut scene, lamp) = scene(Ssil::default(), LAMP);
    scene.world.set_light(lamp, Some(point(LAMP, mode)));
    let maps = bake(&mut scene, name);
    assert!(
        maps > 0 && scene.lightmaps.baked_light(lamp),
        "the bake took the lamp in"
    );
    (scene, lamp)
}

pub(super) fn bake(scene: &mut Scene, name: &str) -> usize {
    bake_with(scene, name, false)
}

pub(super) fn bake_with(scene: &mut Scene, name: &str, directional: bool) -> usize {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_ssil_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 16,
        bounces: 1,
        seed: 1,
        max_lightmap_size: 64,
        denoiser: rusty_scene::lighting::settings::Denoiser::Filter,
        filter_radius: 1,
        directional,
        backface_tolerance: BakeSettings::DEFAULT_BACKFACE_TOLERANCE,
        ..Default::default()
    };
    bake_scene_lightmaps(scene, Some(&path), &settings).expect("baked")
}

pub(super) fn off_and_on(r: &mut rusty_render::Renderer, scene: &mut Scene) -> (f32, f32) {
    set_ssil(scene, Ssil::default());
    let off = energy(&settled(r, scene, 4));
    set_ssil(scene, on(4.0));
    (off, energy(&settled(r, scene, 4)))
}

#[test]
fn gpu_a_lightmapped_scene_with_its_light_baked_gains_nothing() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, _) = baked(LightMode::Baked, "baked");
    let (off, with) = off_and_on(&mut r, &mut scene);
    eprintln!("[ssil] lightmapped, light baked: {off} off, {with} on");
    assert!(
        off > 0.0 && with <= off * (1.0 + BAKED_GAIN),
        "{with} vs {off}"
    );
}

#[test]
fn gpu_a_mixed_light_in_the_bake_bounces_once_and_one_spawned_after_bounces() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, lamp) = baked(LightMode::Mixed, "mixed");
    let (off, with) = off_and_on(&mut r, &mut scene);
    eprintln!("[ssil] lightmapped, Mixed light in the bake: {off} off, {with} on");
    assert!(
        with <= off * (1.0 + BAKED_GAIN),
        "its bounce is baked: {with} vs {off}"
    );

    let at = scene.world.transform(lamp).unwrap().position;
    scene.destroy_entity(lamp);
    let flash = scene.add_entity("Flash".to_string());
    scene.world.transform_mut(flash).unwrap().position = at;
    scene
        .world
        .set_light(flash, Some(point(LAMP, LightMode::Mixed)));
    let red_over_green = |r: &mut _, scene: &mut Scene, ssil| {
        set_ssil(scene, ssil);
        let [red, green, _] = patch(&settled(r, scene, 4), FLOOR);
        red / green
    };
    let off = red_over_green(&mut r, &mut scene, Ssil::default());
    let lit = red_over_green(&mut r, &mut scene, on(1.0));
    eprintln!("[ssil] a Mixed light spawned after the bake: red/green {off} off, {lit} on");
    assert!(lit >= off * 1.1, "it still bounces");
}
