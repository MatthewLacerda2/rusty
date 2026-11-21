//! What re-reads the console note's lights, and what its line advises (#2522).

use super::UnbakedNote;
use rusty_scene::create::default_light;
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::runtime::despawn::Despawn;
use rusty_scene::Scene;

fn lamp(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_light(id, Some(default_light()));
    id
}

/// A despawn and an Undo's restore move the lightmaps' revision without
/// touching the bake's record: the waiting light is not named again. A bake that
/// changes the record names afresh what still waits.
#[test]
fn a_despawn_or_restore_names_nothing_and_a_new_record_names_again() {
    let mut scene = Scene::default();
    let waits = lamp(&mut scene, "Lamp");
    let baked = lamp(&mut scene, "Baked");
    scene.lightmaps.took_in_baked(baked);
    let bullet = scene.add_entity("Bullet".to_string());
    let mut note = UnbakedNote::default();
    assert!(note.note(&scene, true).is_some());
    scene.despawn_batch(&[bullet].into(), Despawn::EditMode);
    assert_eq!(note.note(&scene, true), None, "a despawn: named already");
    let entry = LightmapEntry {
        entity: baked,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    };
    scene.lightmaps.insert(entry);
    assert_eq!(note.note(&scene, true), None, "a restore: named already");
    let other = lamp(&mut scene, "Other");
    scene.lightmaps.took_in_baked(other);
    let line = note.note(&scene, true).expect("a new record names again");
    assert!(
        line.contains("`Lamp`") && !line.contains("`Other`"),
        "{line}"
    );
    assert!(scene.waits_for_bake(waits));
}

/// A waiting light that is off or dark is not named, and its writes do not
/// make the note read every light again.
#[test]
fn an_off_or_dark_waiting_light_reads_nothing_on_its_writes() {
    let mut scene = Scene::default();
    let [off, dark] = ["Off", "Dark"].map(|n| lamp(&mut scene, n));
    scene.world.set_active(off, false);
    scene.world.light_mut(dark).unwrap().intensity = 0.0;
    let mut note = UnbakedNote::default();
    assert_eq!(note.note(&scene, true), None);
    for _ in 0..3 {
        scene.world.light_mut(off).unwrap().range = 4.0;
        scene.world.light_mut(dark).unwrap().range = 4.0;
        assert!(!note.changed(&scene), "a write to an off or dark light");
    }
    let other = scene.add_entity("Other".to_string());
    scene.world.set_active(other, false);
    assert!(!note.changed(&scene), "an entity with no light");
    scene.world.set_active(off, true);
    assert!(note.changed(&scene), "switched on: it waits now");
}

#[test]
fn the_line_says_what_will_help() {
    let line = |scene: &Scene| UnbakedNote::default().note(scene, true).unwrap();
    let mut scene = Scene::default();
    lamp(&mut scene, "Lamp");
    let never = line(&scene);
    assert!(
        never.contains("run Generate Lighting (Lighting.Generate()), or"),
        "{never}"
    );
    assert!(!never.contains("probe"), "no probes: {never}");
    let sun = scene.add_entity("Sun".to_string());
    scene.lightmaps.took_in_baked(sun);
    let rebake = line(&scene);
    assert!(
        rebake.contains("with it active and above intensity 0"),
        "{rebake}"
    );
    scene.probes.add_probe(glam::Vec3::ZERO);
    let probes = line(&scene);
    assert!(probes.contains("over its probe bake"), "{probes}");
    scene.set_playing(true);
    let play = line(&scene);
    assert!(play.contains("spawned during Play is in no bake"), "{play}");
}
