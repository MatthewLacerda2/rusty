//! src/scene/authoring/ui_shader.rs — Shared ui-shader ops (#427).
//!
//! The ONE place a UI graphic's custom shader is named and its runtime params are
//! set, for `Image`, `Shape` and `Text` alike (each carries an `Option<UiShader>`). The editor's
//! cards and the Lua `UI.SetShader` / `UI.SetShaderParam` route through these, so the
//! rules live once: an empty name means "the standard shader"; naming a different
//! shader drops the old one's param values (their names would not resolve); a param
//! is checked against the shader's baked layout (#399's `<name>.params.json`) — an
//! unknown or baked param, or a wrong number of values, is an error naming it — and
//! stored by its canonical name.
//!
//! Pure apart from reading the sidecar.

use crate::components::UiShader;
use crate::ecs::World;
use crate::shadergen::params::{self, ParamLayout};

/// Name the shader a graphic draws with; `None` or an empty name restores the
/// standard one. Re-naming the same shader keeps its param values.
pub fn set_shader(slot: &mut Option<UiShader>, name: Option<String>) {
    match name.filter(|n| !n.is_empty()) {
        None => *slot = None,
        Some(n) if slot.as_ref().is_some_and(|s| s.name == n) => {}
        Some(name) => {
            *slot = Some(UiShader {
                name,
                ..Default::default()
            })
        }
    }
}

/// The runtime-param layout of the shader `slot` names, read where the renderer
/// resolves it. Errors when it names none or the shader is not baked.
pub fn layout(slot: &Option<UiShader>) -> Result<ParamLayout, String> {
    let shader = slot
        .as_ref()
        .ok_or("the graphic has no ui shader (UI.SetShader first)")?;
    params::load(&shader.name)
}

/// Set runtime param `name` on the shader `slot` names, checked against `layout`.
/// One number broadcasts to every lane.
pub fn set_param(
    slot: &mut Option<UiShader>,
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let s = layout.resolve(name)?;
    let value = layout.coerce(s, value)?;
    if let Some(shader) = slot {
        shader.params.insert(layout.name(s), value);
    }
    Ok(())
}

/// The value runtime param `name` draws with: the stored one, else the baked default.
pub fn param(
    slot: &Option<UiShader>,
    layout: &ParamLayout,
    name: &str,
) -> Result<Vec<f32>, String> {
    let s = layout.resolve(name)?;
    let stored = slot.as_ref().and_then(|u| u.params.get(&layout.name(s)));
    Ok(stored.cloned().unwrap_or_else(|| s.default.clone()))
}

/// Run `f` on the shader slot of every graphic entity `id` draws — its `Image`,
/// `Shape`, then `Text` (the draw order) — returning how many it has.
pub fn for_each_graphic(
    world: &mut World,
    id: u32,
    mut f: impl FnMut(&mut Option<UiShader>),
) -> usize {
    let mut n = 0;
    if let Some(mut image) = world.image_mut(id) {
        f(&mut image.shader);
        n += 1;
    }
    if let Some(mut shape) = world.shape_mut(id) {
        f(&mut shape.shader);
        n += 1;
    }
    if let Some(mut text) = world.text_mut(id) {
        f(&mut text.shader);
        n += 1;
    }
    n
}

/// The shader slot entity `id`'s graphics read from: its first graphic's in draw
/// order (`Image`, `Shape`, `Text`); `None` without any.
pub fn graphic_shader(world: &World, id: u32) -> Option<Option<UiShader>> {
    if let Some(image) = world.image(id) {
        return Some(image.shader.clone());
    }
    if let Some(shape) = world.shape(id) {
        return Some(shape.shader.clone());
    }
    world.text(id).map(|t| t.shader.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naming_a_new_shader_drops_old_values_and_empty_restores_standard() {
        let mut slot = None;
        set_shader(&mut slot, Some("glitch".into()));
        slot.as_mut()
            .unwrap()
            .params
            .insert("x.y".into(), vec![1.0]);
        set_shader(&mut slot, Some("glitch".into()));
        assert_eq!(
            slot.as_ref().unwrap().params.len(),
            1,
            "same name keeps values"
        );
        set_shader(&mut slot, Some("holo".into()));
        assert!(slot.as_ref().unwrap().params.is_empty());
        set_shader(&mut slot, Some(String::new()));
        assert_eq!(slot, None);
    }

    #[test]
    fn layout_without_a_shader_says_how_to_name_one() {
        assert!(layout(&None).unwrap_err().contains("UI.SetShader"));
    }
}
