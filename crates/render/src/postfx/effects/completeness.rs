//! The post effects' completeness gate (#1467): every effect the renderer lists has
//! every part another layer expects, and no layer lists an effect the renderer
//! lacks. The four-axis component gate's counterpart for post effects.
//!
//! The parts, per effect: its settings (a volume field, or a camera field named
//! after it for a camera effect), its authoring row (a volume effect), its
//! `Graphics.*` registrar, its Inspector section, its section in
//! `docs/api/Graphics.md` (`<!-- effect: <name> -->`) naming every knob's tween
//! path, and, when it is player-facing, its `GraphicsOption` named after it with a
//! row in the doc's *Player options* table. It declares whether it reads history
//! (#2092), with a reason when it reads none. Each composite texture it declares
//! is bound in the assembled composite and passed to its own function, at its
//! place (#1473).
//!
//! Two of the layers sit above this crate, the `Graphics.*` registrars (`api`) and
//! the Inspector's sections (`editor`), so the root's test hands them in
//! ([`problems`]); this crate's own tests fill them with the renderer's names and
//! check the rest. A test hook, compiled into every build for that.

use super::textures::{composite_textures, PlacedTexture, GROUP};
use super::{assemble, History, Owner, PostEffect, POST_EFFECTS};
use rusty_components::PostProcessingComponent;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_scene::authoring::fields;
use rusty_scene::authoring::rendering::camera;
use rusty_scene::authoring::rendering::post_processing as authoring;

/// What one layer of the engine lists, by effect name.
#[derive(Clone, Default)]
pub(super) struct Layers {
    /// The renderer's effects: name, owner, the name of the option that gates it.
    pub render: Vec<(String, Owner, Option<String>)>,
    /// Each effect's history declaration (#2092): `None` when it reads one, else
    /// its reason for reading none.
    pub history: Vec<(String, Option<&'static str>)>,
    /// The volume's effect fields.
    pub settings: Vec<String>,
    /// The camera's serialized fields.
    pub camera: Vec<String>,
    /// The authoring registry's effects, each with its knob paths.
    pub ops: Vec<(String, Vec<String>)>,
    /// The camera effects' knob tables (`camera::TAA_KNOBS`), each with its knob
    /// paths: their tween paths need their doc line as a volume knob's do.
    pub camera_knobs: Vec<(String, Vec<String>)>,
    /// The `Graphics.*` registrars.
    pub api: Vec<String>,
    /// The Inspector's sections (both cards).
    pub sections: Vec<String>,
    /// Every `GraphicsOption`.
    pub options: Vec<String>,
    /// `docs/api/Graphics.md` (and the test-only effects' own sections).
    pub docs: String,
    /// The composite textures the effects declare, in binding order.
    pub textures: Vec<TextureUse>,
    /// The assembled composite's WGSL.
    pub composite: String,
}

/// One declared composite texture, as the gate expects the composite to use it.
#[derive(Clone)]
pub(super) struct TextureUse {
    pub effect: String,
    /// Its name in the composite.
    pub name: String,
    /// The start of its function's call, up to the argument after `c`
    /// (`assemble::call_head`).
    pub call: String,
    /// Its place among the textures that call passes.
    pub position: usize,
}

/// Every part missing from the engine's layers, given the two that sit above this
/// crate: the `Graphics.*` registrars' names and the Inspector's sections' (the
/// effect each draws). Empty when every effect is whole. The root's test calls it.
pub fn problems(api: &[&str], sections: &[&str]) -> Vec<String> {
    let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect();
    check(&engine_with(names(api), names(sections)))
}

/// The renderer's effect names: the root's test passes them for a layer its build lacks.
pub fn effect_names() -> Vec<&'static str> {
    POST_EFFECTS.iter().map(|e| e.name).collect()
}

/// The layers, the two above this crate read as the renderer's own names.
#[cfg(test)]
pub(super) fn engine() -> Layers {
    let own: Vec<String> = effect_names().into_iter().map(String::from).collect();
    engine_with(own.clone(), own)
}

/// The layers as the engine has them, with `api` and `sections` as given.
fn engine_with(api: Vec<String>, sections: Vec<String>) -> Layers {
    let fields_of = |kind: &str| -> Vec<String> {
        let kind = fields::find(kind).expect("a first-class component");
        (kind.fields)()
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect()
    };
    let not_effect = |f: &String| !PostProcessingComponent::NOT_EFFECTS.contains(&f.as_str());
    let docs = rusty_core::project::checkout().join("docs/api/Graphics.md");
    let docs = std::fs::read_to_string(docs).expect("docs/api/Graphics.md");
    Layers {
        render: POST_EFFECTS.iter().map(rendered).collect(),
        history: POST_EFFECTS.iter().map(history).collect(),
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
        camera_knobs: camera_knobs(),
        api,
        sections,
        options: GraphicsOption::ALL
            .iter()
            .map(|o| o.name().into())
            .collect(),
        docs,
        textures: texture_uses(),
        composite: assemble::composite_source(),
    }
}

/// An effect as [`Layers::render`] holds it.
fn rendered(e: &PostEffect) -> (String, Owner, Option<String>) {
    let option = e.gate.option.map(|o| o.name().into());
    (e.name.into(), e.owner, option)
}

/// An effect's history declaration, as [`Layers::history`] holds it.
fn history(e: &PostEffect) -> (String, Option<&'static str>) {
    let why = match e.history {
        History::None(why) => Some(why),
        History::Reads(_) => None,
    };
    (e.name.into(), why)
}

/// The camera effects' knob tables, each with its knob paths.
fn camera_knobs() -> Vec<(String, Vec<String>)> {
    let taa = camera::TAA_KNOBS.iter().map(|k| k.field.into());
    vec![("taa".into(), taa.collect())]
}

/// Every declared composite texture, as the composite should use it.
fn texture_uses() -> Vec<TextureUse> {
    (composite_textures().iter())
        .map(|t| TextureUse {
            effect: t.effect.name.into(),
            name: t.wgsl_name(),
            call: assemble::call_head(assemble::module_of(t.effect), t.function.function),
            position: PlacedTexture::of(t.function)
                .position(|u| std::ptr::eq(u, t))
                .expect("its own function's"),
        })
        .collect()
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
    for (name, knobs) in l.ops.iter().chain(&l.camera_knobs) {
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
    problems.extend(unbound(l));
    problems
}

/// Each declared composite texture its binding in group 1 lacks, or its own
/// function's call doesn't pass at its place (after `c, f`).
fn unbound(l: &Layers) -> Vec<String> {
    let mut problems = Vec::new();
    for (binding, t) in l.textures.iter().enumerate() {
        let (effect, name) = (&t.effect, &t.name);
        let declared = format!("@group({GROUP}) @binding({binding}) var {name}: texture_2d<f32>;");
        if !l.composite.contains(&declared) {
            problems.push(format!(
                "`{effect}`'s composite texture `{name}` is not bound at group {GROUP}, binding {binding}"
            ));
        }
        let passed = (l.composite.lines()).any(|line| {
            let Some(args) = line.strip_prefix(&t.call) else {
                return false;
            };
            let args = args.trim_end_matches(");");
            args.split(", ").nth(1 + t.position) == Some(name.as_str())
        });
        if !passed {
            problems.push(format!(
                "`{effect}`'s composite texture `{name}` is not passed to its function at its place"
            ));
        }
    }
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
    match l.history.iter().find(|(n, _)| n == name) {
        None => lacks.push("history declaration"),
        Some((_, Some(why))) if why.trim().is_empty() => {
            lacks.push("reason it reads no history (`History::None`)")
        }
        _ => {}
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
