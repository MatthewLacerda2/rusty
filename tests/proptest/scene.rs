use proptest::prelude::*;
use rusty::scene::{LightComponent, Scene};

#[derive(Clone, Debug)]
struct EntitySpec {
    name: String,
    pos: [f32; 3],
    intensity: Option<f32>,
}

fn entity_spec() -> impl Strategy<Value = EntitySpec> {
    (
        "[A-Za-z][A-Za-z0-9 ]{0,11}",
        prop::array::uniform3(-1.0e4_f32..1.0e4),
        prop::option::of(1.0_f32..1.0e4),
    )
        .prop_map(|(name, pos, intensity)| EntitySpec {
            name,
            pos,
            intensity,
        })
}

fn tmp(tag: u64) -> String {
    crate::temp::dir()
        .join(format!("rusty_proptest_scene_{tag}.scene"))
        .to_string_lossy()
        .into_owned()
}

proptest! {
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
            scene.world.transform_mut(id).unwrap().position = glam::Vec3::from_array(spec.pos);
            if let Some(intensity) = spec.intensity {
                scene.world.set_light(
                    id,
                    Some(LightComponent {
                        area_size: glam::Vec2::ONE,
intensity,
..rusty::edit::authoring::default_light()
}),
                );
            }
        }

        let path = tmp(tag);
        rusty::edit::save_to_file(&scene, &path).unwrap();
        let mut loaded = Scene::new();
        rusty::edit::load_from_file(&mut loaded, &path).unwrap();
        let _ = std::fs::remove_file(&path);

        prop_assert_eq!(loaded.entity_count(), specs.len());
        for (id, spec) in ids.iter().zip(&specs) {
            let e = loaded.world.entity_document(*id).unwrap();
            prop_assert_eq!(&e.name, &spec.name);
            prop_assert_eq!(e.transform.position.to_array(), spec.pos);
            match (&e.light, spec.intensity) {
                (Some(l), Some(intensity)) => prop_assert_eq!(l.intensity, intensity),
                (None, None) => {}
                _ => prop_assert!(false, "light presence must round-trip"),
            }
        }
    }
}
