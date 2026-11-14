//! The authoring layer as this crate's tests hand it to the gate: read from
//! `scene::authoring`, as the root's test (`tests/post_effect_layers.rs`) reads it.

use super::{engine_from, Authored, Layers};
use rusty_scene::authoring::fields;
use rusty_scene::authoring::rendering::{camera, post_processing};

/// The layers, the authoring one as `scene::authoring` has it.
pub(super) fn engine() -> Layers {
    engine_from(authored())
}

/// What `scene::authoring` lists.
fn authored() -> Authored {
    let fields_of = |kind: &str| -> Vec<String> {
        let kind = fields::find(kind).expect("a first-class component");
        (kind.fields)()
            .into_iter()
            .map(|(n, _)| n.to_string())
            .collect()
    };
    Authored {
        volume_fields: fields_of("PostProcessing"),
        camera_fields: fields_of("Camera"),
        ops: (post_processing::EFFECTS.iter())
            .map(|e| {
                (
                    e.name.into(),
                    e.knobs.iter().map(|k| k.field.into()).collect(),
                )
            })
            .collect(),
        camera_knobs: vec![(
            "taa".into(),
            camera::TAA_KNOBS.iter().map(|k| k.field.into()).collect(),
        )],
    }
}
