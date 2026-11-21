use super::{halves, sphere, Flash, RES};
use crate::gpu::material_cache::ParamOwner;
use rusty_scene::create::{create_live_light, Primitive};
use rusty_scene::Scene;

fn flashing(shader: &str, color: [f32; 3]) -> (Scene, String) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let id = sphere(&mut scene, -1.2, shader);
    let key = scene.world.material(id).unwrap().first().to_string();
    let params = &mut scene.materials.get_mut(&key).unwrap().shader_params;
    params.insert("hit_flash.amount".into(), vec![1.0]);
    params.insert("hit_flash.color".into(), color.to_vec());
    (scene, key)
}

#[test]
fn gpu_two_kept_scenes_draw_a_same_named_material_with_their_own_values() {
    let flash = Flash::bake_as("_scenes");
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (main, key) = flashing(&flash.0, [1.0, 0.0, 0.0]);
    let (preview, other) = flashing(&flash.0, [0.0, 0.0, 1.0]);
    assert_eq!(key, other, "one name in both libraries");
    let red = halves(&mut renderer, &main)[0];
    let blue = renderer.aside(|r| halves(r, &preview)[0]);
    assert!(
        red.0 > red.1 && blue.1 > blue.0,
        "red {red:?}, blue {blue:?}"
    );
    assert_eq!(
        halves(&mut renderer, &main)[0],
        red,
        "the main scene stays red"
    );
    assert_eq!(renderer.aside(|r| halves(r, &preview)[0]), blue);
    let owner = |s: &Scene| ParamOwner::Material(s.id(), key.clone());
    let values = [&main, &preview].map(|s| renderer.materials.params_of(&owner(s)).copied());
    assert!(values[0].is_some() && values[0] != values[1], "{values:?}");
    for _ in 0..4 {
        halves(&mut renderer, &flashing(&flash.0, [0.0, 1.0, 0.0]).0);
    }
    assert!(renderer.materials.params_of(&owner(&main)).is_none());
    assert_eq!(halves(&mut renderer, &main)[0], red, "drawn again, rebuilt");
}

#[test]
fn gpu_a_param_edit_rewrites_its_buffer_and_re_derives_only_the_overriding() {
    let flash = Flash::bake_as("_edit");
    let Some((mut r, pair)) = crate::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let left = sphere(&mut scene, -1.2, &flash.0);
    let right = sphere(&mut scene, 1.2, &flash.0);
    let shared = scene.world.material(left).map(|m| m.clone());
    scene.world.set_material(right, shared.clone());
    let blue = vec![0.0, 0.0, 1.0];
    scene
        .shader_overrides
        .set(right, "hit_flash.color".into(), blue);
    let key = shared.unwrap().first().to_string();
    let [l0, r0] = halves(&mut r, &scene);
    for amount in [1.0, 0.5] {
        let params = &mut scene.materials.get_mut(&key).unwrap().shader_params;
        params.insert("hit_flash.amount".into(), vec![amount]);
        let [l, rt] = halves(&mut r, &scene);
        let c = r.frame_counters;
        assert_eq!(c.render_extracted, 1, "the overriding sphere alone");
        assert!(
            l.0 > l0.0 && l.1 <= l0.1,
            "the left flashes red: {l0:?} → {l:?}"
        );
        assert!(
            rt.1 > r0.1,
            "the right flashes its own blue: {r0:?} → {rt:?}"
        );
        let mut fresh = pair.renderer(RES, RES);
        assert_eq!(halves(&mut fresh, &scene), [l, rt], "amount {amount}");
    }
}
