use super::textures::{composite_textures, PlacedTexture, GROUP};
pub use super::Owner;
use super::{assemble, History, PostEffect, POST_EFFECTS};
use rusty_components::PostProcessingComponent;
use rusty_core::settings::graphics_options::GraphicsOption;

pub use rusty_scene::authored::Authored;

#[derive(Clone, Default)]
pub struct Layers {
    pub render: Vec<(String, Owner, Option<String>)>,
    pub history: Vec<(String, Option<&'static str>)>,
    pub settings: Vec<String>,
    pub camera: Vec<String>,
    pub ops: Vec<(String, Vec<String>)>,
    pub camera_knobs: Vec<(String, Vec<String>)>,
    pub api: Vec<String>,
    pub sections: Vec<String>,
    pub options: Vec<String>,
    pub docs: String,
    pub textures: Vec<TextureUse>,
    pub composite: String,
}

#[derive(Clone)]
pub struct TextureUse {
    pub effect: String,
    pub name: String,
    pub call: String,
    pub position: usize,
}

pub fn problems(authored: Authored, api: &[&str], sections: &[&str]) -> Vec<String> {
    let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect();
    check(&engine_with(authored, names(api), names(sections)))
}

pub fn effect_names() -> Vec<&'static str> {
    POST_EFFECTS.iter().map(|e| e.name).collect()
}

pub fn engine_from(authored: Authored) -> Layers {
    let own: Vec<String> = effect_names().into_iter().map(String::from).collect();
    engine_with(authored, own.clone(), own)
}

fn engine_with(authored: Authored, api: Vec<String>, sections: Vec<String>) -> Layers {
    let not_effect = |f: &String| !PostProcessingComponent::NOT_EFFECTS.contains(&f.as_str());
    let docs = rusty_core::project::checkout().join("docs/api/Graphics.md");
    let docs = std::fs::read_to_string(docs).expect("docs/api/Graphics.md");
    Layers {
        render: POST_EFFECTS.iter().map(rendered).collect(),
        history: POST_EFFECTS.iter().map(history).collect(),
        settings: authored
            .volume_fields
            .into_iter()
            .filter(not_effect)
            .collect(),
        camera: authored.camera_fields,
        ops: authored.ops,
        camera_knobs: authored.camera_knobs,
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

fn rendered(e: &PostEffect) -> (String, Owner, Option<String>) {
    let option = e.gate.option.map(|o| o.name().into());
    (e.name.into(), e.owner, option)
}

fn history(e: &PostEffect) -> (String, Option<&'static str>) {
    let why = match e.history {
        History::None(why) => Some(why),
        History::Reads(_) => None,
    };
    (e.name.into(), why)
}

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

pub fn check(l: &Layers) -> Vec<String> {
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
