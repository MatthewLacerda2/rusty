use glam::Vec3;

use super::fixture::{block, draw, fog, glow, renderer, room, tinted, view};
use crate::RenderCounters;
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
    // Grey, so the spot lights it whether or not the fog takes its light.
    tinted(
        &mut scene,
        "tile",
        (Vec3::new(0.8, 0.005, -5.0), Vec3::new(1.0, 0.01, 1.0)),
        [0.5; 3],
    );
    // Each frame's counts are kept: a tile baked wrong on frame 1 and reused reads
    // `rebaked 0 cached 1` on the last, as a healthy run does.
    let mut v = view(&r);
    let (mut frame, mut counts) = (Vec::new(), String::new());
    for n in 1..=4 {
        frame = draw(&mut r, &mut v, &scene);
        counts += &format!("\n  frame {n}: {}", counters(&r.frame_counters));
    }
    let lit = glow(&frame, Vec3::new(0.8, 1.2, -5.0));
    let dark = glow(&frame, Vec3::new(-0.8, 1.2, -5.0));
    let tile = glow(&frame, Vec3::new(0.8, 0.01, -5.0));
    eprintln!("[volumetric] shaft: {lit} lit side, {dark} shadowed side, {tile} tile");
    // A dark tile is the spot gone from the frame; a lit one, from the air alone.
    let missing = format!(
        "tile under the cone {tile} (about 20.7 lit, 0.23 with no spot, on lavapipe); \
         the lit side reads about 7.8, and 0.114 with no spot{counts}"
    );
    assert!(lit > 0.3, "the lit cone glows: {lit}; {missing}");
    assert!(
        lit > 2.0 * dark,
        "the plank's shadow is dark: {lit} vs {dark}; {missing}"
    );
}

fn counters(c: &RenderCounters) -> String {
    format!(
        "lights {} visible {} culled {} dropped {}, cluster refs {}; \
         shadowed {} dropped {}, atlas tiles {} cached {} rebaked {}",
        c.lights,
        c.lights_visible,
        c.lights_culled,
        c.lights_dropped,
        c.light_cluster_refs,
        c.shadowed_lights,
        c.shadow_lights_dropped,
        c.shadow_atlas_tiles,
        c.shadow_atlas_cached,
        c.shadow_atlas_rebaked,
    )
}
