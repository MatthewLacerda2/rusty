//! Integration coverage for the `Selectable` namespace (#420): every setter writes
//! through the shared ops (clamped, unknown names ignored), the getters read it
//! back, and the whole component survives a scene save → load — while the runtime
//! state tint on the Image is never saved.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;
use rusty::components::{
    ImageComponent, NavigationMode, SelectableComponent, SelectableTransition,
};
use rusty::scene::Scene;
use rusty::ui::EventSystem;

fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let events = RefCell::new(EventSystem::default());
    lua.scope(|scope| {
        rusty::api::selectable::register(&lua, scope, scene, &events).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

fn eval(lua: &Lua, code: &str) -> String {
    let v: mlua::MultiValue = lua.load(code).eval().unwrap();
    let parts: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
    parts.join(", ")
}

#[test]
fn setters_write_through_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Button".to_string());
    let target = scene.add_entity("Icon".to_string());
    scene
        .world
        .set_selectable(id, Some(SelectableComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Selectable.SetTransition({id}, 'spriteswap')
             Selectable.SetTransition({id}, 'wobble')
             Selectable.SetTargetGraphic({id}, {target})
             Selectable.SetColor({id}, 'Pressed', 2, 0.5, 0)
             Selectable.SetFadeDuration({id}, -1)
             Selectable.SetSprite({id}, 'Highlighted', 'hover.png')
             Selectable.SetNavigation({id}, 'Explicit')
             Selectable.SetSelectOn({id}, 'left', {target})
             Selectable.SetInteractable({id}, false)"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "return Selectable.GetTransition({id}), Selectable.GetSprite({id}, 'highlighted'),
                 Selectable.GetNavigation({id}), Selectable.IsInteractable({id}),
                 Selectable.GetState({id}), Selectable.GetState(99)"
            ),
        );
        let expected = r#"String("SpriteSwap"), String("hover.png"), String("Explicit"), Boolean(false), String("Disabled"), Nil"#;
        assert_eq!(got, expected);
        let rgba = eval(lua, &format!("return Selectable.GetColor({id}, 'pressed')"));
        assert_eq!(rgba, "Number(1), Number(0.5), Number(0), Number(1)");
    });
    let sel = scene
        .borrow()
        .world
        .selectable(id)
        .map(|s| s.clone())
        .unwrap();
    assert_eq!(sel.target_graphic, Some(target));
    assert_eq!(sel.select_on, [None, None, Some(target), None]);
    assert_eq!(sel.fade_duration, 0.0);
}

#[test]
fn a_selectable_survives_save_and_load_but_the_runtime_tint_does_not() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Button".to_string());
    let mut sel = SelectableComponent {
        transition: SelectableTransition::SpriteSwap,
        navigation: NavigationMode::None,
        target_graphic: Some(id),
        ..Default::default()
    };
    sel.sprites[3] = Some("focus.png".to_string());
    scene.world.set_selectable(id, Some(sel.clone()));
    let image = ImageComponent {
        state_tint: Vec4::splat(0.5),
        override_texture: Some("focus.png".to_string()),
        ..Default::default()
    };
    scene.world.set_image(id, Some(image));
    let path = std::env::temp_dir()
        .join(format!("rusty_selectable_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.world.selectable(id).map(|s| s.clone()), Some(sel));
    let image = loaded.world.image(id).map(|i| i.clone()).unwrap();
    assert_eq!(image.state_tint, Vec4::ONE, "runtime-only, never saved");
    assert_eq!(image.override_texture, None);
}
