use serde_json::{json, Map, Value};

use super::hold::has_number;
use super::tests::dressed;
use super::{Skip, KINDS};
use crate::{apply_scene_data, to_scene_data};
use rusty_scene::Scene;

/// An integer goes to 250 or 0, the widest every integer type, `u8` too, still reads.
fn push_out(v: &mut Value, up: bool) {
    match v {
        Value::Number(n) if n.is_f64() => {
            let far = n.as_f64().unwrap().abs() * 1000.0 + 1000.0;
            *v = json!(if up { far } else { -far });
        }
        Value::Number(_) => *v = json!(if up { 250 } else { 0 }),
        Value::Array(list) => list.iter_mut().for_each(|v| push_out(v, up)),
        Value::Object(map) => map.values_mut().for_each(|v| push_out(v, up)),
        _ => {}
    }
}

fn load_pushed(up: bool) -> (Scene, u32) {
    let (scene, id) = dressed();
    let mut doc = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let entities = doc["entities"].as_array_mut().unwrap();
    let entity = entities.iter_mut().find(|e| e["id"] == id).unwrap();
    for kind in KINDS {
        for (name, _) in (kind.fields)() {
            let field = entity.get_mut(kind.key).and_then(|c| c.get_mut(name));
            if let Some(field) = field.filter(|_| (kind.held_on_load)(name)) {
                push_out(field, up);
            }
        }
    }
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_value(doc).unwrap());
    (loaded, id)
}

#[test]
fn every_held_field_loads_as_its_op_would_hold_it() {
    for up in [true, false] {
        let (mut scene, id) = load_pushed(up);
        for kind in KINDS {
            let held = (kind.read)(&scene.world, id).unwrap().unwrap();
            for (name, _) in (kind.fields)() {
                if !(kind.held_on_load)(name) || !has_number(&held[name]) {
                    continue;
                }
                let one = Map::from_iter([(name.to_string(), held[name].clone())]);
                let written = (kind.write)(&mut scene.world, id, &one, Skip::Nothing);
                assert_eq!(written, Ok(true), "{}.{name}", kind.name);
                let again = (kind.read)(&scene.world, id).unwrap().unwrap();
                assert_eq!(
                    again[name], held[name],
                    "{}.{name} loaded (up: {up}) as a value its op moves",
                    kind.name
                );
            }
        }
    }
}

#[test]
fn out_of_range_values_are_held_on_load() {
    use rusty_components::{ShadowSettings, SsaoSettings};
    let (scene, id) = load_pushed(true);
    assert_eq!(scene.world.audio(id).unwrap().volume, 1.0);
    assert_eq!(scene.world.canvas_group(id).unwrap().alpha, 1.0);
    let vc = scene.world.post_processing(id).unwrap();
    assert_eq!(vc.ssao.intensity, SsaoSettings::MAX_INTENSITY);
    assert_eq!(vc.shadows.cascades, ShadowSettings::MAX_CASCADES);
    let (scene, id) = load_pushed(false);
    assert_eq!(scene.world.audio(id).unwrap().volume, 0.0);
    assert_eq!(scene.world.audio(id).unwrap().final_distance, 0.0);
    assert_eq!(scene.world.post_processing(id).unwrap().shadows.cascades, 1);
}

#[test]
fn an_in_range_scene_loads_unchanged() {
    use crate::authoring::{audio::reverb_zone, physics::joint};
    let (mut scene, id) = dressed();
    let hall = rusty_components::ReverbPreset::Hall;
    reverb_zone::set_preset(&mut scene.world.reverb_zone_mut(id).unwrap(), hall);
    // Diagonals: renormalizing a unit diagonal moves it an ulp, which a load must not do.
    let mut j = scene.world.joint_mut(id).unwrap();
    joint::set_axis(&mut j, glam::Vec3::new(1.0, 1.0, 0.0));
    joint::set_swing_axis(&mut j, glam::Vec3::new(0.0, 1.0, 1.0));
    drop(j);
    let saved = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let mut doc = saved.clone();
    for _ in 0..3 {
        let mut loaded = Scene::new();
        apply_scene_data(&mut loaded, serde_json::from_value(doc).unwrap());
        doc = serde_json::to_value(to_scene_data(&loaded)).unwrap();
        assert_eq!(doc, saved);
    }
}

#[test]
fn a_snapshot_restore_is_not_held_again() {
    let (mut scene, id) = dressed();
    scene.world.audio_mut(id).unwrap().volume = 4.0;
    crate::SceneSnapshot::capture(&scene).restore(&mut scene);
    assert_eq!(scene.world.audio(id).unwrap().volume, 4.0);
}
