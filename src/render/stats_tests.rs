use super::*;
use crate::components::LightComponent;
use glam::Vec3;

fn light(light_type: LightType) -> LightComponent {
    LightComponent {
        light_type,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        inner_cone: 20.0,
        outer_cone: 30.0,
    }
}

#[test]
fn lights_past_the_forward_slots_count_as_dropped() {
    let mut scene = Scene::new();
    let types = std::iter::repeat_n(LightType::Point, 6).chain([
        LightType::Directional,
        LightType::Spotlight,
        LightType::Spotlight,
    ]);
    for (i, t) in types.enumerate() {
        let id = scene.world.spawn(format!("L{i}"));
        scene.world.set_light(id, Some(light(t)));
    }
    // 6 points (2 over the 4 slots) + a second spotlight overwriting the first.
    assert_eq!(count_lights(&scene), (9, 3));
}

#[test]
fn add_draws_counts_calls_and_triangles() {
    let mut c = RenderCounters::default();
    c.add_draws([(36, 1), (6, 1)]);
    assert_eq!((c.draw_calls, c.triangles), (2, 14));
    // An instanced draw (#470) is one call carrying every instance's triangles.
    c.add_draws([(36, 100)]);
    assert_eq!((c.draw_calls, c.triangles), (3, 1214));
    assert_eq!(c.pairs()[0], ("draw_calls", 3));
}
