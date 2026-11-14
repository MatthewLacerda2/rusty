//! Runtime shader params on the GPU (#399): a hit-flash amount set on a
//! material asset changes the pixel on the next frame, through a buffer write alone —
//! no module rebuild, no new bind group — and each material keeps its own values.
//! Skips with no adapter.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::authoring::{create_entity, material as mat_ops, Primitive};
use rusty_scene::{Camera, Scene};
use rusty_shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

pub(super) const RES: u32 = 32;

/// A red hit-flash surface shader baked where scripts and the renderer resolve it,
/// under a name unique to this run; removed on drop.
pub(super) struct Flash(pub(super) String);

impl Flash {
    pub(super) fn bake() -> Self {
        Self::bake_as("")
    }

    /// Baked under a name carrying `tag`, so tests sharing a process never share it.
    pub(super) fn bake_as(tag: &str) -> Self {
        let name = format!("test_hit_flash{tag}_{}", std::process::id());
        let params = [("color".to_string(), ParamValue::Vector(vec![1.0, 0.0, 0.0]))];
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: "hit_flash".into(),
                params: params.into_iter().collect(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        Self(name)
    }
}

impl Drop for Flash {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A lit white sphere at `x` whose own material names `shader`; returns its id.
pub(super) fn sphere(scene: &mut Scene, x: f32, shader: &str) -> u32 {
    let id = create_entity(scene, "Ball", Some(Primitive::Sphere));
    if let Some(mut t) = scene.world.transform_mut(id) {
        // Diameter 2: the thresholds below were set on that size.
        (t.position, t.scale) = (Vec3::new(x, 0.0, 0.0), Vec3::splat(2.0));
    }
    let key = mat_ops::ensure_material_key(scene, id).unwrap();
    mat_ops::set_shader(&mut scene.materials, &key, shader.to_string());
    id
}

// What a script's `Material.*` calls do, through the scene ops the API wraps:
// these tests check the GPU side, and the API's own tests hold the Lua side.

/// `Material.SetShaderParam(id, name, value)`: `id`'s own value.
pub(super) fn set_param(scene: &Rc<RefCell<Scene>>, id: u32, name: &str, value: f32) {
    let scene = &mut scene.borrow_mut();
    mat_ops::set_entity_shader_param(scene, id, name, vec![value]).unwrap();
}

/// `Material.ClearShaderParam(id)`: every value of `id`'s own.
pub(super) fn clear_params(scene: &Rc<RefCell<Scene>>, id: u32) {
    mat_ops::clear_entity_shader_param(&mut scene.borrow_mut(), id, None).unwrap();
}

/// `Material.SetAssetShaderParam(asset, name, value)`: the library asset's value.
fn set_asset_param(scene: &Rc<RefCell<Scene>>, asset: &str, name: &str, value: f32) {
    let scene = &mut *scene.borrow_mut();
    let layout = mat_ops::shader_layout(&scene.materials, asset).unwrap();
    mat_ops::set_shader_param(&mut scene.materials, asset, &layout, name, vec![value]).unwrap();
}

/// `Material.SetShaderTexture(id, slot, path)`.
pub(super) fn set_texture(scene: &Rc<RefCell<Scene>>, id: u32, slot: &str, path: &str) {
    let scene = &mut *scene.borrow_mut();
    let key = mat_ops::ensure_material_key(scene, id).unwrap();
    mat_ops::set_shader_texture(&mut scene.materials, &key, slot, Some(path.into())).unwrap();
}

/// The summed (red, blue) of the centre row's left and right halves.
pub(super) fn halves(renderer: &mut Renderer, scene: &Scene) -> [(u32, u32); 2] {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let px = readback::read_texture_rgba8(
        &renderer.device,
        &renderer.queue,
        view.color_target().unwrap(),
        RES,
        RES,
    );
    let row = &px[(RES / 2 * RES * 4) as usize..((RES / 2 + 1) * RES * 4) as usize];
    let (left, right) = row.split_at(row.len() / 2);
    let sum = |half: &[u8], c: usize| half.chunks(4).map(|p| p[c] as u32).sum();
    [left, right].map(|h| (sum(h, 0), sum(h, 2)))
}

#[test]
fn gpu_a_hit_flash_set_on_the_asset_changes_the_next_frame_by_a_buffer_write() {
    let flash = Flash::bake();
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let left = sphere(&mut scene, -1.2, &flash.0);
    sphere(&mut scene, 1.2, &flash.0);
    let scene = std::rc::Rc::new(RefCell::new(scene));

    let [l0, r0] = halves(&mut renderer, &scene.borrow());
    assert!(l0.1 > 0 && r0.1 > 0, "both lit, unflashed: {l0:?} {r0:?}");
    let builds = renderer.surface_shaders.builds();
    let groups = renderer.material_group_count();

    let asset = format!("entity_{left}_material");
    set_asset_param(&scene, &asset, "hit_flash.amount", 1.0);
    let [l1, r1] = halves(&mut renderer, &scene.borrow());
    assert!(
        l1.1 < l0.1 / 4 && l1.0 > l0.0,
        "the left flashed red: {l0:?} → {l1:?}"
    );
    assert_eq!(r1, r0, "the right sphere's material keeps its own value");
    assert_eq!(
        renderer.surface_shaders.builds(),
        builds,
        "no module rebuilt"
    );
    assert_eq!(
        renderer.material_group_count(),
        groups,
        "no bind group rebuilt"
    );

    set_asset_param(&scene, &asset, "hit_flash.amount", 0.0);
    assert_eq!(
        halves(&mut renderer, &scene.borrow())[0],
        l0,
        "the flash fades back"
    );
}

#[path = "overrides_tests.rs"]
mod overrides_tests;
#[path = "scene_params_tests.rs"]
mod scene_params_tests;
#[path = "slot_overrides_tests.rs"]
mod slot_overrides_tests;
