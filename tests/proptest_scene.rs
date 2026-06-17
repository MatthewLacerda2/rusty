//! Property-based scene round-trip tests (#123). Distinct from the example-based
//! `scene_roundtrip.rs`: here proptest generates thousands of arbitrary scenes and
//! asserts save -> load preserves their values, shrinking any counterexample to a
//! minimal failing case. Generators are seeded by proptest (no wall-clock), so the
//! determinism contract holds.

use proptest::prelude::*;
use rusty::scene::{HealthComponent, Scene};

/// One generated entity: a name, a position, and an optional Health component.
/// Kept small and finite (bounded coords/health) so the generated scenes are
/// always valid documents the save/load path accepts.
#[derive(Clone, Debug)]
struct EntitySpec {
    name: String,
    pos: [f32; 3],
    health: Option<f32>,
}

fn entity_spec() -> impl Strategy<Value = EntitySpec> {
    (
        "[A-Za-z][A-Za-z0-9 ]{0,11}",
        prop::array::uniform3(-1.0e4_f32..1.0e4),
        prop::option::of(1.0_f32..1.0e4),
    )
        .prop_map(|(name, pos, health)| EntitySpec { name, pos, health })
}

fn tmp(tag: u64) -> String {
    std::env::temp_dir()
        .join(format!("rusty_proptest_scene_{tag}.scene"))
        .to_string_lossy()
        .into_owned()
}

proptest! {
    /// For any generated scene, save -> load reproduces every entity's name,
    /// position, and health exactly (positions are finite, so bitwise-equal after
    /// the JSON round-trip).
    #[test]
    fn save_load_preserves_generated_entities(
        specs in prop::collection::vec(entity_spec(), 0..12),
        tag in any::<u64>(),
    ) {
        let mut scene = Scene::new();
        let mut ids = Vec::new();
        for spec in &specs {
            let id = scene.add_entity(spec.name.clone());
            ids.push(id);
            let mut e = scene.get_entity_mut(id).unwrap();
            e.transform.position = glam::Vec3::from_array(spec.pos);
            if let Some(max) = spec.health {
                e.health = Some(HealthComponent {
                    current_health: max,
                    max_health: max,
                    is_dead: false,
                });
            }
        }

        let path = tmp(tag);
        scene.save_to_file(&path).unwrap();
        let mut loaded = Scene::new();
        loaded.load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        prop_assert_eq!(loaded.entity_count(), specs.len());
        for (id, spec) in ids.iter().zip(&specs) {
            let e = loaded.get_entity(*id).unwrap();
            prop_assert_eq!(&e.name, &spec.name);
            prop_assert_eq!(e.transform.position.to_array(), spec.pos);
            match (&e.health, spec.health) {
                (Some(h), Some(max)) => prop_assert_eq!(h.max_health, max),
                (None, None) => {}
                _ => prop_assert!(false, "health presence must round-trip"),
            }
        }
    }
}
