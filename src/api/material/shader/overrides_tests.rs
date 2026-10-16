//! Per-entity shader-param overrides from Lua (#670): two entities share one
//! material, `SetShaderParam` on one leaves the other — and the asset — untouched,
//! `GetShaderParam` reads override-else-material, and `ClearShaderParam` drops one
//! override or all of them.

use super::tests::{run, scene_with, Baked};
use crate::scene::Scene;

/// Two entities sharing one material whose shader is `shader`: (scene, a, b, key).
fn pair(shader: &str) -> (std::cell::RefCell<Scene>, u32, u32, String) {
    let (scene, a) = scene_with(shader);
    let key = scene.borrow().world.material(a).unwrap().material.clone();
    let b = {
        let mut s = scene.borrow_mut();
        let b = s.add_entity("B".into());
        let material = s.world.material(a).map(|m| m.clone());
        s.world.set_material(b, material);
        b
    };
    (scene, a, b, key)
}

#[test]
fn an_override_flashes_one_entity_and_leaves_the_shared_material_alone() {
    let shader = Baked::new("override");
    let (scene, a, b, key) = pair(&shader.0);
    run(
        &scene,
        &format!(
            r#"Material.SetAssetShaderParam("{key}", "hit_flash.amount", 0.25)
               Material.SetShaderParam({a}, "hit_flash.1.amount", 1)
               assert(Material.GetShaderParam({a}, "hit_flash.amount") == 1)
               assert(Material.GetShaderParam({b}, "hit_flash.amount") == 0.25)
               assert(Material.GetAssetShaderParam("{key}", "hit_flash.amount") == 0.25)"#
        ),
    )
    .unwrap();
    let s = scene.borrow();
    assert_eq!(s.materials[&key].shader_params["hit_flash.amount"], [0.25]);
    assert_eq!(s.shader_overrides.of(a).unwrap()["hit_flash.amount"], [1.0]);
    assert!(!s.shader_overrides.has(b), "b has no override");
}

#[test]
fn clear_drops_one_override_or_all_and_the_material_value_shows_again() {
    let shader = Baked::new("clear");
    let (scene, a, _, _) = pair(&shader.0);
    run(
        &scene,
        &format!(
            r#"Material.SetShaderParam({a}, "hit_flash.amount", 1)
               Material.SetShaderParam({a}, "hit_flash.color", {{0, 0.25, 0}})
               Material.ClearShaderParam({a}, "hit_flash.amount")
               assert(Material.GetShaderParam({a}, "hit_flash.amount") == 0)
               assert(Material.GetShaderParam({a}, "hit_flash.color")[2] == 0.25)
               Material.ClearShaderParam({a})
               assert(Material.GetShaderParam({a}, "hit_flash.color")[2] ~= 0.25)"#
        ),
    )
    .unwrap();
    assert!(scene.borrow().shader_overrides.is_empty());
    let err = run(
        &scene,
        &format!(r#"Material.ClearShaderParam({a}, "nope.x")"#),
    )
    .unwrap_err();
    assert!(err.contains("hit_flash.amount"), "lists the params: {err}");
}

#[test]
fn overrides_never_save_and_go_with_their_entity() {
    let shader = Baked::new("transient");
    let (scene, a, b, _) = pair(&shader.0);
    let script = format!(
        r#"Material.SetShaderParam({a}, "hit_flash.amount", 1)
           Material.SetShaderParam({b}, "hit_flash.amount", 1)"#
    );
    run(&scene, &script).unwrap();
    let mut s = scene.borrow_mut();
    let json = serde_json::to_string(&crate::scene::to_scene_data(&s)).unwrap();
    assert!(
        !json.contains("hit_flash"),
        "the override is not scene data"
    );
    s.destroy_entity(a);
    assert!(!s.shader_overrides.has(a) && s.shader_overrides.has(b));
    // Stop restores the edit scene through the same document apply a load uses.
    crate::scene::SceneSnapshot::capture(&Scene::new()).restore(&mut s);
    assert!(s.shader_overrides.is_empty(), "Stop discards them");
}

#[test]
fn an_unknown_asset_is_an_error_naming_it() {
    let (scene, _) = scene_with("");
    let err = run(&scene, r#"Material.SetAssetShaderParam("ghost", "a.b", 1)"#).unwrap_err();
    assert!(err.contains("ghost"), "{err}");
}
