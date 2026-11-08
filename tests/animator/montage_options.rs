//! `Animator.PlayMontage` (#1894): a graph that exists but fails to load warns
//! with its error and returns `false` (never the clip of the same name), and
//! the options' `blend_in` / `blend_out` override the montage's own.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::scene::Scene;
use rusty::scripting::{ConsoleLogs, LogLevel};

use super::montage_api::with_flinch;

/// `script` against `scene` with a console to read; its value and the
/// console's new lines.
fn eval(scene: &Rc<RefCell<Scene>>, script: &str) -> (bool, Vec<(String, LogLevel)>) {
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let lua = Lua::new();
    rusty::api::animator::register(&lua, bind(&lua, scene), bind(&lua, &console)).unwrap();
    let mark = console.borrow().logged();
    let value: bool = lua.load(format!("return {script}")).eval().unwrap();
    let added = console
        .borrow()
        .since(mark)
        .map(|(m, l)| (m.clone(), l.clone()))
        .collect();
    (value, added)
}

#[test]
fn a_graph_that_fails_to_load_warns_and_plays_nothing() {
    let (scene, id) = with_flinch();
    let path = crate::temp::dir().join("rusty_1894_broken.animgraph");
    std::fs::write(&path, "{ \"nodes\": [ oops").unwrap();
    let path = path.to_string_lossy().replace('\\', "/");
    scene.borrow_mut().world.animator_mut(id).unwrap().graph = Some(path.clone());
    let (started, added) = eval(&scene, &format!("Animator.PlayMontage({id}, 'Flinch')"));
    assert!(!started, "never the clip of the same name");
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0].1, LogLevel::Warning);
    assert!(
        added[0].0.contains(&format!("graph '{path}': ")),
        "{added:?}"
    );
    let anim = scene.borrow();
    assert!(anim.world.animator(id).unwrap().montages.is_empty());
}

#[test]
fn the_blend_options_override_the_montages_own() {
    let (scene, id) = with_flinch();
    let call = format!("Animator.PlayMontage({id}, 'Flinch', {{ blend_in = 0.1, blend_out = 0 }})");
    assert!(eval(&scene, &call).0);
    let s = scene.borrow();
    let playing = &s.world.animator(id).unwrap().montages.playing;
    let montage = &playing[0].montage;
    assert_eq!((montage.blend_in, montage.blend_out), (0.1, 0.0));
}
