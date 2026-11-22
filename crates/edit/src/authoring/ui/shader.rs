//! Naming a different shader drops the old one's param values (their names would not resolve); a
//! param is stored by its canonical name.

use rusty_components::UiShader;
use rusty_ecs::World;
use rusty_scene::rules::sanitize::finite_values;
use rusty_shadergen::params::{self, ParamLayout};

/// Re-naming the same shader keeps its param values.
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

pub fn layout(slot: &Option<UiShader>) -> Result<ParamLayout, String> {
    let shader = slot
        .as_ref()
        .ok_or("the graphic has no ui shader (UI.SetShader first)")?;
    params::load(&shader.name)
}

/// One number broadcasts to every lane.
pub fn set_param(
    slot: &mut Option<UiShader>,
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let s = layout.resolve(name)?;
    let value = layout.coerce(s, finite_values(value)?)?;
    if let Some(shader) = slot {
        shader.params.insert(layout.name(s), value);
    }
    Ok(())
}

pub fn param(
    slot: &Option<UiShader>,
    layout: &ParamLayout,
    name: &str,
) -> Result<Vec<f32>, String> {
    let s = layout.resolve(name)?;
    let stored = slot.as_ref().and_then(|u| u.params.get(&layout.name(s)));
    Ok(stored.cloned().unwrap_or_else(|| s.default.clone()))
}

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
