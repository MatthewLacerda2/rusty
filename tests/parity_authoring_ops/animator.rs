//! `Animator.Stop` vs `authoring::animator`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::AnimatorComponent;
use rusty::scene::authoring::animator as animator_ops;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

#[test]
fn animator_stop_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // The only Animator field-write binding is `Stop` (is_playing = false); it and the
    // card's "Is Playing" toggle share `authoring::animator::set_playing`.
    let scene = RefCell::new(Scene::new());
    let console = RefCell::new(ConsoleLogs::new());
    let playing = || AnimatorComponent {
        is_playing: true,
        ..AnimatorComponent::default()
    };
    let via_lua = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("ViaLua".to_string());
        sc.world.set_animator(id, Some(playing()));
        id
    };
    let via_op = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("ViaOp".to_string());
        sc.world.set_animator(id, Some(playing()));
        id
    };

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, &scene, &console).unwrap();
        lua.load(format!("Animator.Stop({via_lua})"))
            .exec()
            .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.animator_mut(via_op).unwrap();
        animator_ops::set_playing(&mut e, false);
    }

    let sc = scene.borrow();
    let a = sc.world.animator(via_lua).unwrap().clone();
    let b = sc.world.animator(via_op).unwrap().clone();
    assert_eq!(a.is_playing, b.is_playing);
    assert!(!a.is_playing);
    Ok(())
}
