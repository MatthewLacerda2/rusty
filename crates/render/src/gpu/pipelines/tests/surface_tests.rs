use std::collections::BTreeMap;

use glam::Vec3;

use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::{create::create_entity, create::Primitive, Camera, RenderMode, Scene};
use rusty_shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir};

const RES: u32 = 32;

fn workspace(tag: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_surface_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_str().unwrap().to_owned()
}

fn bake(dir: &str, name: &str, pass: PassKind, color: [f32; 3]) {
    let params = BTreeMap::from([("color".to_string(), ParamValue::Vector(color.to_vec()))]);
    let blocks = vec![BlockSel {
        id: "tint".into(),
        params,
    }];
    let recipe = ShaderRecipe {
        pass,
        name: name.into(),
        blocks,
    };
    bake_recipe(&recipe, engine_shader_dir(), dir).expect("bake succeeds");
}

fn sphere(shader: &str, mode: RenderMode) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let id = create_entity(&mut scene, "Ball", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(2.0);
    let key = scene.ensure_material_key(id).unwrap();
    scene.materials.get_mut(&key).unwrap().shader =
        (!shader.is_empty()).then(|| shader.to_string());
    scene.materials.get_mut(&key).unwrap().render_mode = mode;
    scene.materials.get_mut(&key).unwrap().alpha = 0.8;
    scene
}

fn render(renderer: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 3.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, RES, RES)
}

fn red_blue(px: &[u8]) -> (u32, u32) {
    let row = &px[(RES / 2 * RES * 4) as usize..((RES / 2 + 1) * RES * 4) as usize];
    let sum = |c: usize| row.chunks(4).map(|p| p[c] as u32).sum();
    (sum(0), sum(2))
}

fn renderer_in(dir: &str) -> Option<Renderer> {
    let mut renderer = crate::test_gpu::headless_or_skip(RES, RES)?;
    renderer.surface_shaders.set_dirs(vec![dir.to_owned()]);
    Some(renderer)
}

#[test]
fn gpu_a_material_naming_a_baked_shader_renders_with_it() {
    let dir = workspace("variant");
    let Some(mut renderer) = renderer_in(&dir) else {
        return;
    };
    bake(&dir, "blue_only", PassKind::Surface, [0.0, 0.0, 1.0]);
    let standard = render(&mut renderer, &sphere("", RenderMode::Opaque));
    let variant = render(&mut renderer, &sphere("blue_only", RenderMode::Opaque));
    let (r0, _) = red_blue(&standard);
    let (r1, b1) = red_blue(&variant);
    assert!(
        r0 > 0 && b1 > 0,
        "both lit: standard r={r0}, variant b={b1}"
    );
    assert!(r1 < r0 / 4, "the tint removed red: {r1} vs standard {r0}");
    assert_eq!(
        renderer.surface_shaders.pipeline_id(&renderer.device, None),
        0
    );

    let clear = red_blue(&render(&mut renderer, &sphere("", RenderMode::Transparent)));
    let glass = red_blue(&render(
        &mut renderer,
        &sphere("blue_only", RenderMode::Transparent),
    ));
    assert!(
        glass.0 * 3 < clear.0 * 2,
        "tinted glass: {glass:?} vs {clear:?}"
    );
}

#[test]
fn gpu_a_missing_or_postfx_shader_falls_back_to_the_standard_look() {
    let dir = workspace("fallback");
    let Some(mut renderer) = renderer_in(&dir) else {
        return;
    };
    bake(&dir, "a_postfx", PassKind::Postfx, [0.0, 0.0, 1.0]);
    let standard = render(&mut renderer, &sphere("", RenderMode::Opaque));
    for name in ["not_baked", "a_postfx", "../escape"] {
        let px = render(&mut renderer, &sphere(name, RenderMode::Opaque));
        assert_eq!(px, standard, "{name:?} falls back to the standard shader");
        let id = renderer
            .surface_shaders
            .pipeline_id(&renderer.device, Some(name));
        assert_eq!(id, 0, "{name:?} resolves to the standard pipeline");
    }
}

#[test]
fn gpu_re_baking_a_cached_shader_rebuilds_it() {
    let dir = workspace("reload");
    let Some(mut renderer) = renderer_in(&dir) else {
        return;
    };
    let scene = sphere("look", RenderMode::Opaque);
    let before = render(&mut renderer, &scene);
    bake(&dir, "look", PassKind::Surface, [0.0, 0.0, 1.0]);
    let blue = red_blue(&render(&mut renderer, &scene));
    assert!(
        blue.0 < red_blue(&before).0 / 4,
        "the new bake is picked up: {blue:?}"
    );
    bake(&dir, "look", PassKind::Surface, [1.0, 0.25, 0.0]);
    let red = red_blue(&render(&mut renderer, &scene));
    assert!(
        red.0 > blue.0 && red.1 < blue.1,
        "re-bake rebuilt: {blue:?} → {red:?}"
    );
}
