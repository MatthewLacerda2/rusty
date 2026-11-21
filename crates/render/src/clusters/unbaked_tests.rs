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
    scene.lightmaps.took_in_baked(lamp_id);
    assert_eq!(note.note(&scene, true), None, "baked: nothing waits");
    scene.lightmaps.clear();
    assert!(
        note.note(&scene, true).is_some(),
        "a bake's record changed and it waits: named again"
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
    assert!(
        line.contains("shipped without a bake") && !line.contains("Lighting."),
        "the player names no verb it lacks: {line}"
    );
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

#[test]
fn another_scene_has_its_waiting_lights_named_afresh() {
    let (mut a, mut b) = (Scene::default(), Scene::default());
    lamp(&mut a, "Lamp");
    lamp(&mut b, "Lamp");
    let mut note = UnbakedNote::default();
    assert!(note.note(&a, true).is_some());
    assert!(
        note.note(&b, true).is_some(),
        "the same id in another scene"
    );
    assert!(note.note(&b, true).is_none());
}

#[test]
fn a_named_light_is_not_named_again_for_its_activity_or_intensity() {
    let mut scene = Scene::default();
    let id = lamp(&mut scene, "Lamp");
    let mut note = UnbakedNote::default();
    assert!(note.note(&scene, true).is_some());
    scene.world.set_active(id, false);
    assert_eq!(note.note(&scene, true), None);
    scene.world.set_active(id, true);
    assert_eq!(note.note(&scene, true), None, "reactivated: already named");
    scene.world.light_mut(id).unwrap().intensity = 0.0;
    assert_eq!(note.note(&scene, true), None);
    scene.world.light_mut(id).unwrap().intensity = 3.0;
    assert_eq!(note.note(&scene, true), None, "raised: already named");
}

#[test]
fn a_mixed_light_made_baked_after_its_bake_is_named() {
    let mut scene = Scene::default();
    let [mixed, fell] = ["Mixed", "Fell"].map(|n| lamp(&mut scene, n));
    for id in [mixed, fell] {
        scene.world.light_mut(id).unwrap().mode = LightMode::Mixed;
    }
    scene.lightmaps.fell_back.insert(fell);
    let taken = [(mixed, LightMode::Mixed), (fell, LightMode::Mixed)];
    scene.lightmaps.record_taken(&taken.into_iter().collect());
    let mut note = UnbakedNote::default();
    assert_eq!(note.note(&scene, true), None, "both baked");
    let fell_light = scene.world.light(fell).unwrap().clone();
    assert!(!scene.awaits_bake(fell, &fell_light), "not previewed twice");
    scene.world.light_mut(mixed).unwrap().mode = LightMode::Baked;
    let line = note
        .note(&scene, true)
        .expect("its direct light is in no bake");
    assert!(
        line.contains("1 Baked light waits") && line.contains("`Mixed`"),
        "{line}"
    );
}
