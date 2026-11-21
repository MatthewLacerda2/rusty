//! A level's lights by how they are shaded (#2523): the `level_lights` and
//! `level_lights_live` rows, and the line under the table when any light is `Mixed`.

use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::create::default_light;
use rusty_scene::Scene;

use super::{lights, mixed_lights, push_lights, Lights};
use crate::bench::scenes::SceneReport;

/// A scene with a point light of each of `modes`, and their ids.
fn scene_ids(modes: &[LightMode]) -> (Scene, Vec<u32>) {
    let mut scene = Scene::default();
    let mut ids = Vec::new();
    for (k, &mode) in modes.iter().enumerate() {
        let id = scene.add_entity(format!("Light {k}"));
        let light = LightComponent {
            light_type: LightType::Point,
            mode,
            ..default_light()
        };
        scene.world.set_light(id, Some(light));
        ids.push(id);
    }
    (scene, ids)
}

fn scene(modes: &[LightMode]) -> Scene {
    scene_ids(modes).0
}

#[test]
fn a_light_of_each_mode_is_three_lights_two_live_and_one_mixed_named() {
    let modes = [LightMode::Realtime, LightMode::Mixed, LightMode::Baked];
    let counted = lights(&scene(&modes));
    let want = Lights {
        total: 3,
        live: 2,
        mixed: 1,
    };
    assert_eq!(counted, want);
    let note = mixed_lights(counted).expect("a Mixed light is named");
    assert_eq!(
        note,
        "1 of 3 lights is Mixed: their direct light is shaded live every frame"
    );
}

/// A report beside the frame counter `lights` (what a frame drew, ambient
/// included) holding `drawn` of them, with the level's lights pushed after it.
fn report(drawn: f64, level: Lights) -> SceneReport {
    let mut report = SceneReport::default();
    report.push_value("lights", drawn);
    push_lights(&mut report, level);
    report
}

/// The level's rows are named apart from the frame counter `lights`: each is
/// found by its own name, and a comparison pairs each with its own.
#[test]
fn the_level_rows_sit_beside_the_frame_counter_and_compare_with_their_own() {
    let level = lights(&scene(&[
        LightMode::Realtime,
        LightMode::Mixed,
        LightMode::Baked,
    ]));
    let now = report(4.0, level);
    let value = |r: &SceneReport, m: &str| r.row(m).map(|row| row.avg);
    assert_eq!(value(&now, "lights"), Some(4.0));
    assert_eq!(value(&now, "level_lights"), Some(3.0));
    assert_eq!(value(&now, "level_lights_live"), Some(2.0));
    let text = now.render(Some(&report(5.0, level)));
    let row = |m: &str| {
        let line = text
            .lines()
            .find(|l| l.split_whitespace().next() == Some(m));
        line.unwrap_or_else(|| panic!("no {m} row in\n{text}"))
    };
    assert!(row("lights").ends_with("-1.000 (-20.0%)"), "{text}");
    assert!(row("level_lights").ends_with(" ="), "{text}");
    assert!(row("level_lights_live").ends_with(" ="), "{text}");
    assert!(text.contains("warning: 1 of 3 lights is Mixed"), "{text}");
}

#[test]
fn an_all_baked_level_prints_no_note() {
    let counted = lights(&scene(&[LightMode::Baked, LightMode::Baked]));
    assert_eq!((counted.total, counted.live), (2, 0));
    assert_eq!(mixed_lights(counted), None);
    assert_eq!(mixed_lights(Lights::default()), None, "no lights, no note");
}

/// What the level's file means, not its `mode` field: an `Area` light is always
/// baked, a `Mixed` light a `Shadowmask` bake fell back counts as `Baked`, an
/// inactive light is not shaded, and the ambient light ignores modes.
#[test]
fn lights_are_counted_by_the_mode_they_are_shaded_as() {
    let (mut scene, ids) = scene_ids(&[LightMode::Mixed, LightMode::Mixed]);
    scene.lightmaps.fell_back.insert(ids[1]);
    for (name, light_type) in [("Area", LightType::Area), ("Ambient", LightType::Ambient)] {
        let id = scene.add_entity(name.into());
        let light = LightComponent {
            light_type,
            mode: LightMode::Mixed,
            ..default_light()
        };
        scene.world.set_light(id, Some(light));
    }
    let off = scene.add_entity("Off".into());
    scene.world.set_light(off, Some(default_light()));
    scene.world.set_active(off, false);
    let want = Lights {
        total: 3,
        live: 1,
        mixed: 1,
    };
    assert_eq!(lights(&scene), want);
    let note = mixed_lights(lights(&scene)).unwrap();
    assert!(note.starts_with("1 of 3 lights is Mixed"), "{note}");
}
