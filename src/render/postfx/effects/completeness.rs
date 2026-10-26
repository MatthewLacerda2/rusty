//! The post effects' completeness gate (#1467): every effect the renderer lists has
//! every part another layer expects, and no layer lists an effect the renderer
//! lacks. The four-axis component gate's counterpart for post effects.
//!
//! The parts, per effect: its settings (a volume field, or a camera field named
//! after it for a camera effect), its authoring row (a volume effect), its
//! `Graphics.*` registrar, its Inspector section, its section in
//! `docs/api/Graphics.md` (`<!-- effect: <name> -->`) naming every knob's tween
//! path, and, when it is player-facing, its `GraphicsOption` named after it with a
//! row in the doc's *Player options* table.

use super::{Owner, POST_EFFECTS};
use crate::components::PostProcessingComponent;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::scene::authoring::fields;
use crate::scene::authoring::rendering::post_processing as authoring;

/// What one layer of the engine lists, by effect name.
#[derive(Clone, Default)]
pub(super) struct Layers {
    /// The renderer's effects: name, owner, the name of the option that gates it.
    pub render: Vec<(String, Owner, Option<String>)>,
    /// The volume's effect fields.
    pub settings: Vec<String>,
    /// The camera's serialized fields.
    pub camera: Vec<String>,
    /// The authoring registry's effects, each with its knob paths.
    pub ops: Vec<(String, Vec<String>)>,
    /// The `Graphics.*` registrars.
    pub api: Vec<String>,
    /// The Inspector's sections (both cards).
    pub sections: Vec<String>,
    /// Every `GraphicsOption`.
    pub options: Vec<String>,
    /// `docs/api/Graphics.md` (and the test-only effects' own sections).
    pub docs: String,
}

/// The test-only effects' sections, which no shipped doc holds.
const TEST_ONLY_DOCS: &[&str] = &[
    // sorted: one row per test-only effect
    super::dummy::DOC,
];

/// The layers as the engine has them.
pub(super) fn engine() -> Layers {
    let fields_of = |kind: &str| -> Vec<String> {
        let kind = fields::find(kind).expect("a first-class component");
        (kind.fields)()
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect()
    };
    let not_effect = |f: &String| !PostProcessingComponent::NOT_EFFECTS.contains(&f.as_str());
    let docs = std::fs::read_to_string("docs/api/Graphics.md").expect("docs/api/Graphics.md");
    Layers {
        render: POST_EFFECTS
            .iter()
            .map(|e| (e.name.into(), e.owner, e.option.map(|o| o.name().into())))
            .collect(),
        settings: fields_of("PostProcessing")
            .into_iter()
            .filter(not_effect)
            .collect(),
        camera: fields_of("Camera"),
        ops: authoring::EFFECTS
            .iter()
            .map(|e| {
                (
                    e.name.into(),
                    e.knobs.iter().map(|k| k.field.into()).collect(),
                )
            })
            .collect(),
        api: crate::api::graphics::effects::REGISTRARS
            .iter()
            .map(|(n, _)| n.to_string())
            .collect(),
        sections: sections(),
        options: GraphicsOption::ALL
            .iter()
            .map(|o| o.name().into())
            .collect(),
        docs: TEST_ONLY_DOCS.iter().fold(docs, |all, doc| all + doc),
    }
}

#[cfg(feature = "editor")]
fn sections() -> Vec<String> {
    use crate::editor::inspector::components::camera::{post_processing, CAMERA_SECTIONS};
    let volume = post_processing::SECTIONS
        .iter()
        .map(|s| s.effect.to_string());
    volume
        .chain(CAMERA_SECTIONS.iter().map(|s| s.effect.to_string()))
        .collect()
}

/// Without the editor there is no card to check: every effect counts as drawn.
#[cfg(not(feature = "editor"))]
fn sections() -> Vec<String> {
    POST_EFFECTS.iter().map(|e| e.name.to_string()).collect()
}

fn has(list: &[String], name: &str) -> bool {
    list.iter().any(|n| n == name)
}

/// Every part missing, or listed for no effect, as one line each.
pub(super) fn check(l: &Layers) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, owner, option) in &l.render {
        let lacks = |part: &str| format!("`{name}` has no {part}");
        problems.extend(
            missing(l, name, *owner, option.as_deref())
                .into_iter()
                .map(lacks),
        );
    }
    for (name, knobs) in &l.ops {
        for knob in knobs {
            let path = format!("`Graphics.{name}.{knob}`");
            if !l.docs.contains(&path) {
                problems.push(format!(
                    "`{name}`'s knob {path} is not in docs/api/Graphics.md"
                ));
            }
        }
    }
    problems.extend(unknown(l));
    problems
}

/// The parts the effect `name` lacks.
fn missing(l: &Layers, name: &str, owner: Owner, option: Option<&str>) -> Vec<&'static str> {
    let mut lacks = Vec::new();
    match owner {
        Owner::Volume if !has(&l.settings, name) => lacks.push("field on PostProcessingComponent"),
        Owner::Camera if !l.camera.iter().any(|f| f.starts_with(&format!("{name}_"))) => {
            lacks.push("field on CameraComponent")
        }
        _ => {}
    }
    if owner == Owner::Volume && !l.ops.iter().any(|(n, _)| n == name) {
        lacks.push("row in authoring's EFFECTS");
    }
    if !has(&l.api, name) {
        lacks.push("Graphics.* registrar in api::graphics::effects::REGISTRARS");
    }
    if !has(&l.sections, name) {
        lacks.push("Inspector section");
    }
    if !l.docs.contains(&format!("<!-- effect: {name} -->")) {
        lacks.push("section in the Graphics doc, marked `<!-- effect: <name> -->`");
    }
    if let Some(option) = option {
        if option != name || !has(&l.options, option) {
            lacks.push("GraphicsOption named after it");
        }
        if !l.docs.contains(&format!("| `{option}` |")) {
            lacks.push("row in the docs' Player options table");
        }
    }
    lacks
}

/// An effect a layer lists that the renderer doesn't. The volume's lighting
/// settings (`shadows`, `ssao`: `PostProcessingComponent::NOT_EFFECTS`) have their
/// own sections and are no effect.
fn unknown(l: &Layers) -> Vec<String> {
    let rendered: Vec<String> = l.render.iter().map(|(n, ..)| n.clone()).collect();
    let ops = l.ops.iter().map(|(n, _)| n.clone()).collect();
    let marked = l
        .docs
        .split("<!-- effect: ")
        .skip(1)
        .filter_map(|rest| Some(rest.split_once(" -->")?.0.to_string()))
        .collect();
    let listed = [
        ("PostProcessingComponent", l.settings.clone()),
        ("authoring's EFFECTS", ops),
        ("REGISTRARS", l.api.clone()),
        ("the Inspector's sections", l.sections.clone()),
        ("docs/api/Graphics.md", marked),
    ];
    let lighting = |n: &String| PostProcessingComponent::NOT_EFFECTS.contains(&n.as_str());
    let mut problems = Vec::new();
    for (layer, names) in listed {
        for name in names.iter().filter(|n| !has(&rendered, n) && !lighting(n)) {
            problems.push(format!(
                "{layer} lists `{name}`, which render::postfx::effects lacks"
            ));
        }
    }
    problems
}

#[cfg(test)]
#[path = "completeness_tests.rs"]
mod tests;
