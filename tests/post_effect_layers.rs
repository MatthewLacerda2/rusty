//! The post effects' completeness gate (`render::postfx::completeness`) over every
//! layer, the three above `render`'s live code included: the authoring layer
//! (`scene::authoring`'s field tables, effect rows and knob tables, read here as
//! `render`'s own tests read them), and each effect has its `Graphics.*`
//! registrar (`script::api::graphics::effects::REGISTRARS`) and its Inspector section, and
//! neither lists an effect the renderer lacks. The gate's own cases are `render`'s
//! tests.

use rusty::render::postfx::completeness::{problems, Authored};
use rusty::scene::authoring::fields;
use rusty::scene::authoring::rendering::{camera, post_processing};

/// What the authoring layer lists: the field tables, the volume's effect rows and
/// the camera's knob tables.
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

/// The Inspector's sections, by the effect each draws (both cards).
#[cfg(feature = "editor")]
fn sections() -> Vec<&'static str> {
    rusty::editor::inspector::components::camera::drawn_effects()
}

#[test]
fn every_effect_has_every_part_in_every_layer() {
    let api: Vec<&str> = rusty::script::api::graphics::effects::REGISTRARS
        .iter()
        .map(|(n, _)| *n)
        .collect();
    // Without the editor there is no card to check: every effect the renderer has
    // counts as drawn, so only the registrars are checked (and a missing one is
    // reported once, as a registrar).
    #[cfg(not(feature = "editor"))]
    let sections = rusty::render::postfx::completeness::effect_names();
    #[cfg(feature = "editor")]
    let sections = sections();
    let problems = problems(authored(), &api, &sections);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
