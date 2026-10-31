//! The inventory against the lists it governs (#1649): the scene document's
//! fields, and the gates' own flag and pass cases.

use serde::de::{self, Deserializer, Visitor};

use super::check::{disagreements, mismatched, Observed};
use super::inventory::fields;
use super::{Class, Lifecycle, Outcome};
use crate::scene::SceneData;

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
fn the_scene_document_holds_exactly_the_saved_classes() {
    let mut names = FieldNames(&[]);
    let _ = <SceneData as serde::Deserialize>::deserialize(&mut names);
    assert!(!names.0.is_empty(), "SceneData's field names were read");
    let saved: Vec<&str> = fields().iter().flat_map(|f| f.document_names()).collect();
    assert_eq!(
        mismatched("SceneData", names.0, &saved),
        Vec::<String>::new()
    );
}

#[test]
fn every_field_is_classified_once() {
    let mut names: Vec<&str> = fields().iter().map(|f| f.name).collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "a field is listed twice");
}

#[test]
fn a_field_classified_for_nothing_and_one_never_classified_are_flagged() {
    let flagged = mismatched("SceneData", &["fog", "new_knob"], &["fog", "gone"]);
    assert_eq!(flagged.len(), 2, "{flagged:?}");
    assert!(
        flagged[0].contains("`new_knob` is not classified"),
        "{flagged:?}"
    );
    assert!(flagged[1].contains("`gone`"), "{flagged:?}");
    assert!(mismatched("SceneData", &["fog"], &["fog"]).is_empty());
}

#[test]
fn a_lifecycle_breaking_its_class_is_flagged_and_one_keeping_it_passes() {
    let seen = |class, lifecycle, outcome| Observed {
        mark: "fog",
        class,
        lifecycle,
        outcome,
    };
    let forgotten = [seen(Class::Authored, Lifecycle::Undo, Outcome::Left)];
    let flagged = disagreements(&forgotten);
    assert_eq!(flagged.len(), 1);
    assert!(
        flagged[0].contains("Undo owes Authored Back"),
        "{flagged:?}"
    );
    let kept = [
        seen(Class::Authored, Lifecycle::Undo, Outcome::Back),
        seen(Class::BakeOutput, Lifecycle::Undo, Outcome::Left),
        // Owed nothing, so whatever happened passes.
        seen(Class::Follow, Lifecycle::Stop, Outcome::Left),
    ];
    assert!(disagreements(&kept).is_empty());
}
