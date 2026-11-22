use std::cell::RefCell;
use std::rc::Rc;

use rusty_scene::Scene;
use rusty_shadergen::params::{self, ParamLayout};

fn layout(scene: &Scene, key: &str) -> ParamLayout {
    let shader = scene.materials[key].shader.as_deref().expect("a shader");
    params::load(shader).expect("a baked layout")
}

fn checked(layout: &ParamLayout, name: &str, value: f32) -> (String, Vec<f32>) {
    let slot = layout.resolve(name).expect("a known param");
    let value = layout.coerce(slot, vec![value]).expect("a fitting value");
    (layout.name(slot), value)
}

pub(crate) fn set_param(scene: &Rc<RefCell<Scene>>, id: u32, name: &str, value: f32) {
    let scene = &mut scene.borrow_mut();
    let key = scene.ensure_material_key(id).expect("an entity");
    let (name, value) = checked(&layout(scene, &key), name, value);
    scene.shader_overrides.set(id, name, value);
}

pub(crate) fn clear_params(scene: &Rc<RefCell<Scene>>, id: u32) {
    scene.borrow_mut().shader_overrides.clear(id, None);
}

pub(crate) fn set_asset_param(scene: &Rc<RefCell<Scene>>, asset: &str, name: &str, value: f32) {
    let scene = &mut *scene.borrow_mut();
    let (name, value) = checked(&layout(scene, asset), name, value);
    let material = scene.materials.get_mut(asset).expect("the asset");
    material.shader_params.insert(name, value);
}

pub(crate) fn set_texture(scene: &Rc<RefCell<Scene>>, id: u32, slot: &str, path: &str) {
    let scene = &mut *scene.borrow_mut();
    rusty_shadergen::textures::slot(slot).expect("a known slot");
    let key = scene.ensure_material_key(id).expect("an entity");
    let material = scene.materials.get_mut(&key).expect("its material");
    material
        .shader_textures
        .insert(slot.to_owned(), path.into());
}
