//! The scene level's field list is the saved scene minus what is no edit (#1631):
//! a field added to `SceneData` and not to `scene::undo::level` fails here, so
//! it can't silently stay out of Undo.

use serde::de::{self, Deserializer, Visitor};

use super::level::FIELDS;
use crate::scene::SceneData;

/// What `SceneData` saves that a scene-level step does not: the entities and
/// the id allocator (the journal's), the selection (no edit), and the lightmaps
/// (a bake's output, never undone).
const NOT_LEVEL: &[&str] = &[
    "entities",
    "next_entity_id",
    "selected_entity_id",
    "lightmaps",
];

/// A deserializer that only learns a struct's field names, then gives up.
struct FieldNames(&'static [&'static str]);

impl<'de> Deserializer<'de> for &mut FieldNames {
    type Error = de::value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
        Err(de::Error::custom("only a struct's field names are read"))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Self::Error> {
        self.0 = fields;
        Err(de::Error::custom("field names read"))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map
        enum identifier ignored_any
    }
}

#[test]
fn the_scene_level_is_every_saved_field_but_the_entities_selection_and_bake() {
    let mut names = FieldNames(&[]);
    let _ = <SceneData as serde::Deserialize>::deserialize(&mut names);
    let mut saved: Vec<&str> = names
        .0
        .iter()
        .copied()
        .filter(|f| !NOT_LEVEL.contains(f))
        .collect();
    let mut level = FIELDS.to_vec();
    saved.sort_unstable();
    level.sort_unstable();
    assert!(!saved.is_empty(), "SceneData's field names were read");
    assert_eq!(level, saved);
}

#[test]
fn a_nan_probe_position_is_no_edit_on_an_idle_frame() {
    let mut scene = crate::scene::Scene::new();
    scene.probes.add_probe(glam::Vec3::NAN);
    scene.begin_edits();
    scene.pause_edits();
    assert!(!scene.commit_edits(None), "NaN never equals itself");
}
