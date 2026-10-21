//! Runtime shader params on the GPU (#399): a hit-flash amount set from Lua on a
//! material asset changes the pixel on the next frame, through a buffer write alone —
//! no module rebuild, no new bind group — and each material keeps its own values.
//! Skips with no adapter.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, material as mat_ops, Primitive};
use crate::scene::{Camera, Scene};
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

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
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A lit white sphere at `x` whose own material names `shader`; returns its id.
pub(super) fn sphere(scene: &mut Scene, x: f32, shader: &str) -> u32 {
    let id = create_entity(scene, "Ball", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
    let key = mat_ops::ensure_material_key(scene, id).unwrap();
    mat_ops::set_shader(&mut scene.materials, &key, shader.to_string());
    id
}

pub(super) fn lua(scene: &RefCell<Scene>, script: &str) {
    let lua = Lua::new();
    lua.scope(|s| {
        crate::api::material::register(&lua, s, scene).unwrap();
        lua.load(script).exec()
    })
    .unwrap();
}

/// The summed (red, blue) of the centre row's left and right halves.
pub(super) fn halves(renderer: &mut Renderer, scene: &Scene) -> [(u32, u32); 2] {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
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
fn gpu_a_hit_flash_set_from_lua_changes_the_next_frame_by_a_buffer_write() {
    let flash = Flash::bake();
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let left = sphere(&mut scene, -1.2, &flash.0);
    sphere(&mut scene, 1.2, &flash.0);
    let scene = RefCell::new(scene);

    let [l0, r0] = halves(&mut renderer, &scene.borrow());
    assert!(l0.1 > 0 && r0.1 > 0, "both lit, unflashed: {l0:?} {r0:?}");
    let builds = renderer.surface_shaders.builds();
    let groups = renderer.material_group_count();

    lua(
        &scene,
        &format!(
            r#"Material.SetAssetShaderParam("entity_{left}_material", "hit_flash.amount", 1)"#
        ),
    );
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

    lua(
        &scene,
        &format!(
            r#"Material.SetAssetShaderParam("entity_{left}_material", "hit_flash.amount", 0)"#
        ),
    );
    assert_eq!(
        halves(&mut renderer, &scene.borrow())[0],
        l0,
        "the flash fades back"
    );
}

#[path = "overrides_tests.rs"]
mod overrides_tests;
