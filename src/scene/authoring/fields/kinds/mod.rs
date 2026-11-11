//! src/scene/authoring/fields/kinds/mod.rs — every component `Component.*` reaches, by name.
//!
//! One [`Kind`] per first-class component (each row of `components::registry`)
//! plus the mandatory `Transform`; the field tables live beside, grouped
//! by area.

mod ai;
mod fx;
mod graphic;
mod nav;
mod physics;
mod render;
mod ui;

use super::introspect::field_type;
use super::table::{kind, Kind};
use crate::components::animator::Playback;
use crate::components::registry::first_class;
use crate::components::*;
use crate::scene::authoring::{ui::shader as ui_shader, ComponentKind as K};

/// The reason on a field no inspector op writes yet; #1368 gates them in.
const NO_OP: &str = "no inspector op writes it yet (#1368)";

/// Declares [`KINDS`]: `Transform`, then each registry row (`components::registry`)
/// under the name `Component.*` gives it ([`FirstClass::NAME`]).
macro_rules! kinds {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        /// Every component, by the name `Component.*` answers to first.
        pub const KINDS: &[Kind] = &[
            kind::<TransformComponent>("Transform", None, "transform"),
            $(kind::<$ty>(<$ty as FirstClass>::NAME, Some(K::$kind), stringify!($field)),)*
        ];
    };
}

first_class!(kinds);

/// An Animator field's type: its own derive's, else, for the fields it flattens
/// from its `Playback` (which serde reads as a map, typeless), `Playback`'s.
pub(super) fn animator_field_type(name: &'static str) -> (bool, Option<&'static str>) {
    match field_type::<AnimatorComponent>(name) {
        (false, None) => field_type::<Playback>(name),
        known => known,
    }
}

/// The component `name` names: its own name in any case, else any name
/// `Scene.AddComponent` takes. An unknown name is an error listing the names.
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

/// A UI graphic's shader, whole, through the `UI.SetShader` / `SetShaderParam`
/// ops: `None` clears it; a name keeps the params when it is unchanged, and each
/// param given is then written (checked against the baked shader's layout). A
/// renamed shader drops the old one's params, so those still in the table at
/// their old values (a `Get` table whose `name` alone was edited) are not written.
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
