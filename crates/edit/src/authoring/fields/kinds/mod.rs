mod ai;
mod fx;
mod graphic;
mod nav;
mod physics;
mod render;
mod ui;

use super::introspect::field_type;
use super::table::{kind, Kind};
use crate::authoring::{ui::shader as ui_shader, ComponentKind as K};
use rusty_components::animator::Playback;
use rusty_components::registry::first_class;
use rusty_components::*;

const NO_OP: &str = "no inspector op writes it yet (#1368)";

macro_rules! kinds {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        pub const KINDS: &[Kind] = &[
            kind::<TransformComponent>("Transform", None, "transform"),
            $(kind::<$ty>(<$ty as FirstClass>::NAME, Some(K::$kind), stringify!($field)),)*
        ];
    };
}

first_class!(kinds);

pub(super) fn animator_field_type(name: &'static str) -> (bool, Option<&'static str>) {
    match field_type::<AnimatorComponent>(name) {
        (false, None) => field_type::<Playback>(name),
        known => known,
    }
}

pub fn find(name: &str) -> Result<&'static Kind, String> {
    let added = K::parse(name);
    KINDS
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(name) || (added.is_some() && k.add == added))
        .ok_or_else(|| {
            let names: Vec<&str> = KINDS.iter().map(|k| k.name).collect();
            format!(
                "unknown component '{name}' (components: {})",
                names.join(", ")
            )
        })
}

fn set_ui_shader(slot: &mut Option<UiShader>, shader: Option<UiShader>) -> Result<(), String> {
    let Some(mut shader) = shader else {
        ui_shader::set_shader(slot, None);
        return Ok(());
    };
    if let Some(old) = slot.as_ref().filter(|old| old.name != shader.name) {
        shader
            .params
            .retain(|name, value| old.params.get(name) != Some(value));
    }
    ui_shader::set_shader(slot, Some(shader.name));
    if shader.params.is_empty() {
        return Ok(());
    }
    let layout = ui_shader::layout(slot)?;
    for (name, value) in shader.params {
        ui_shader::set_param(slot, &layout, &name, value)?;
    }
    Ok(())
}
