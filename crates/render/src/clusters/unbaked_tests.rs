//! The console's word on `Baked` lights no bake has taken in yet (#2522).

use super::UnbakedNote;
use rusty_components::LightMode;
use rusty_scene::create::default_light;
use rusty_scene::Scene;

fn lamp(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_light(id, Some(default_light()));
    id
}

#[test]
fn a_new_default_light_is_named_once_until_a_bake_takes_it_in() {
    let mut scene = Scene::default();
    let mut note = UnbakedNote::default();
    assert_eq!(note.note(&scene, true), None, "no light: nothing to say");
    let lamp_id = lamp(&mut scene, "Lamp");
    let line = note.note(&scene, true).expect("a new Baked light is named");
    assert!(
        line.contains("1 Baked light waits") && line.contains("`Lamp`"),
        "{line}"
    );
    assert!(
        line.contains("drawn live") && line.contains("Lighting.Generate()"),
        "{line}"
    );
    assert_eq!(note.note(&scene, true), None, "named once, not every frame");
    scene.lightmaps.lights.insert(lamp_id);
    assert_eq!(note.note(&scene, true), None, "baked: nothing waits");
    scene.lightmaps.lights.clear();
    assert!(
        note.note(&scene, true).is_some(),
        "waiting again: named again"
    );
}

#[test]
fn a_second_light_names_them_all_and_a_player_says_they_light_nothing() {
    let mut scene = Scene::default();
    let mut note = UnbakedNote::default();
    lamp(&mut scene, "A");
    note.note(&scene, false);
    for name in ["B", "C", "D"] {
        lamp(&mut scene, name);
    }
    let line = note.note(&scene, false).expect("new lights are named");
    assert!(
        line.contains("4 Baked lights wait") && line.contains("1 more"),
        "{line}"
    );
    assert!(line.contains("lighting nothing"), "{line}");
}

#[test]
fn realtime_dark_and_inactive_lights_wait_for_nothing() {
    let mut scene = Scene::default();
    let [live, dark, off] = ["Live", "Dark", "Off"].map(|n| lamp(&mut scene, n));
    let mut light = default_light();
    light.mode = LightMode::Realtime;
    scene.world.set_light(live, Some(light));
    light = default_light();
    light.intensity = 0.0;
    scene.world.set_light(dark, Some(light));
    scene.world.set_active(off, false);
    assert_eq!(UnbakedNote::default().note(&scene, true), None);
}
