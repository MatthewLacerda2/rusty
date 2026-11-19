//! A shadowed spot through fog (#1703): its cone lights the air, and the part of the
//! cone a plank shadows stays dark: a shaft with a dark side.

use glam::Vec3;

use super::fixture::{block, fog, glow, renderer, room, settled};
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::create::default_light;

#[test]
fn gpu_a_shadowed_spot_casts_a_shaft_with_a_dark_side() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(fog());
    let spot = scene.add_entity("Spot".to_string());
    {
        let mut t = scene.world.transform_mut(spot).unwrap();
        t.position = Vec3::new(0.0, 4.0, -5.0);
        // Looking straight down: -Z turned onto -Y.
        t.rotation = glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    }
    let light = LightComponent {
        light_type: LightType::Spotlight,
        intensity: 60.0,
        range: 8.0,
        inner_cone: 30.0,
        outer_cone: 40.0,
        cast_shadows: true,
        mode: LightMode::Realtime,
        ..default_light()
    };
    scene.world.set_light(spot, Some(light));
    // A plank under the left half of the cone.
    block(
        &mut scene,
        "plank",
        (Vec3::new(-1.0, 2.5, -5.0), Vec3::new(2.0, 0.1, 4.0)),
    );
    let frame = settled(&mut r, &scene, 4);
    let lit = glow(&frame, Vec3::new(0.8, 1.2, -5.0));
    let dark = glow(&frame, Vec3::new(-0.8, 1.2, -5.0));
    eprintln!("[volumetric] shaft: {lit} lit side, {dark} shadowed side");
    assert!(lit > 0.3, "the lit cone glows: {lit}");
    assert!(
        lit > 2.0 * dark,
        "the plank's shadow is dark: {lit} vs {dark}"
    );
}

// PROBE (#2385), removed before ready.
#[test]
fn gpu_probe_shaft_stress() {
    let n: u32 = std::env::var("PROBE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let mut misses = 0;
    for run in 0..n {
        let Some(mut r) = renderer() else { return };
        let mut scene = room(fog());
        let spot = scene.add_entity("Spot".to_string());
        {
            let mut t = scene.world.transform_mut(spot).unwrap();
            t.position = Vec3::new(0.0, 4.0, -5.0);
            t.rotation = glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        }
        let light = LightComponent {
            light_type: LightType::Spotlight,
            intensity: 60.0,
            range: 8.0,
            inner_cone: 30.0,
            outer_cone: 40.0,
            cast_shadows: true,
            mode: LightMode::Realtime,
            ..default_light()
        };
        scene.world.set_light(spot, Some(light));
        block(
            &mut scene,
            "plank",
            (Vec3::new(-1.0, 2.5, -5.0), Vec3::new(2.0, 0.1, 4.0)),
        );
        let mut v = super::fixture::view(&r);
        let mut rows = Vec::new();
        let mut miss = false;
        for f in 1..=6 {
            let frame = super::fixture::draw(&mut r, &mut v, &scene);
            let lit = glow(&frame, Vec3::new(0.8, 1.2, -5.0));
            let dark = glow(&frame, Vec3::new(-0.8, 1.2, -5.0));
            let floor = glow(&frame, Vec3::new(0.8, 0.0, -5.0));
            let c = r.frame_counters;
            miss |= lit < 0.3;
            rows.push(format!(
                "f{f} lit {lit} dark {dark} floor {floor} | lights {} vis {} culled {} dropped {} cldrop {} refs {} | shadowed {} sdropped {} tiles {} cached {} rebaked {} sdraws {} stested {} sfrust {} ssize {} socc {} | froxels {} draws {} vis_ent {}",
                c.lights, c.lights_visible, c.lights_culled, c.lights_dropped, c.cluster_lights_dropped, c.light_cluster_refs,
                c.shadowed_lights, c.shadow_lights_dropped, c.shadow_atlas_tiles, c.shadow_atlas_cached, c.shadow_atlas_rebaked,
                c.shadow_draws, c.shadow_cull_tested, c.shadow_frustum_culled, c.shadow_size_culled, c.shadow_occlusion_culled,
                c.volumetric_froxels, c.draw_calls, c.visible_entities
            ));
        }
        if miss || run == 0 {
            misses += u32::from(miss);
            eprintln!("PROBE run {run} miss {miss}:\n  {}", rows.join("\n  "));
        }
    }
    eprintln!("PROBE {misses} of {n} runs missed the shaft");
}
