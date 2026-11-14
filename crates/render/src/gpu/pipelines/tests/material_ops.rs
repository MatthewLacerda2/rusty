//! What a script's `Material.*` calls do, for the GPU tests of shader params:
//! through the scene ops the API wraps. These tests check the GPU side; the API's
//! own tests hold the Lua side.

use std::cell::RefCell;
use std::rc::Rc;

use rusty_scene::authoring::material as mat_ops;
use rusty_scene::Scene;

/// `Material.SetShaderParam(id, name, value)`: `id`'s own value.
pub(crate) fn set_param(scene: &Rc<RefCell<Scene>>, id: u32, name: &str, value: f32) {
    let scene = &mut scene.borrow_mut();
    mat_ops::set_entity_shader_param(scene, id, name, vec![value]).unwrap();
}

/// `Material.ClearShaderParam(id)`: every value of `id`'s own.
pub(crate) fn clear_params(scene: &Rc<RefCell<Scene>>, id: u32) {
    mat_ops::clear_entity_shader_param(&mut scene.borrow_mut(), id, None).unwrap();
}

/// `Material.SetAssetShaderParam(asset, name, value)`: the library asset's value.
pub(crate) fn set_asset_param(scene: &Rc<RefCell<Scene>>, asset: &str, name: &str, value: f32) {
    let scene = &mut *scene.borrow_mut();
    let layout = mat_ops::shader_layout(&scene.materials, asset).unwrap();
    mat_ops::set_shader_param(&mut scene.materials, asset, &layout, name, vec![value]).unwrap();
}

/// `Material.SetShaderTexture(id, slot, path)`.
pub(crate) fn set_texture(scene: &Rc<RefCell<Scene>>, id: u32, slot: &str, path: &str) {
    let scene = &mut *scene.borrow_mut();
    let key = mat_ops::ensure_material_key(scene, id).unwrap();
    mat_ops::set_shader_texture(&mut scene.materials, &key, slot, Some(path.into())).unwrap();
}
