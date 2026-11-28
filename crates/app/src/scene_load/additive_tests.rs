use rusty_core::paths;
use rusty_scene::{Scene, ScriptComponent};

use super::fixture::{logs, world_at};
use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

/// `base.scene` (`Base`) and two pieces, `a.scene` and `b.scene`, each a root whose script
/// reports its teardown and a child; `a`'s fog is 0.25 dense.
pub(super) fn scenes(tag: &str) -> [String; 3] {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_2499_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = |name: &str| paths::to_slash(&dir.join(name));
    let lua = file("piece.lua");
    let code = "return { OnDestroy = function(id) print('destroy ' .. Scene.GetName(id)) end }";
    std::fs::write(&lua, code).unwrap();
    let write = |name: &str, fog: Option<f32>| {
        let mut scene = Scene::new();
        let root = scene.add_entity(name.to_string());
        let child = scene.add_entity(format!("{name}Child"));
        scene.set_parent(child, Some(root)).unwrap();
        *scene.world.scripts_mut(root).unwrap() = vec![ScriptComponent {
            path: lua.clone(),
            ..Default::default()
        }];
        if let Some(density) = fog {
            scene.fog.density = density;
        }
        let path = file(&format!("{}.scene", name.to_lowercase()));
        rusty_edit::save_to_file(&scene, &path).unwrap();
        path
    };
    [
        write("Base", None),
        write("A", Some(0.25)),
        write("B", None),
    ]
}

fn eval(gw: &GameWorld, code: &str) -> String {
    gw.script_manager().eval(code).unwrap()
}

fn playing(
    tag: &str,
) -> (
    GameWorld,
    [String; 3],
    std::rc::Rc<std::cell::RefCell<rusty_script::scripting::ConsoleLogs>>,
) {
    let paths = scenes(tag);
    let (mut gw, console) = world_at(&paths[0]);
    gw.set_playing(true);
    gw.tick(DT);
    (gw, paths, console)
}

fn loaded(gw: &GameWorld) -> String {
    eval(gw, "return table.concat(Scene.GetLoaded(), ' ')")
}

fn id(gw: &GameWorld, name: &str) -> Option<u32> {
    gw.scene().borrow().find_entity_by_name(name)
}

#[test]
fn two_additive_loads_join_one_a_tick_and_an_unload_tears_one_down() {
    let (mut gw, [base, a, b], console) = playing("join");
    eval(
        &gw,
        &format!("Scene.Load('{a}', 'Additive'); Scene.LoadAsync('{b}', 'additive')"),
    );
    gw.tick(DT);
    assert_eq!(loaded(&gw), format!("{base} {a}"), "one activation a tick");
    gw.tick(DT);
    assert_eq!(loaded(&gw), format!("{base} {a} {b}"));
    assert_eq!(
        eval(&gw, "return Scene.GetActivePath()"),
        base,
        "an additive load is not active"
    );
    let a_child = id(&gw, "AChild").unwrap();
    assert_eq!(eval(&gw, &format!("return Scene.Of({a_child})")), a);
    assert!(id(&gw, "Base").is_some() && id(&gw, "B").is_some());

    eval(&gw, &format!("Scene.Unload('{a}')"));
    assert_eq!(
        loaded(&gw),
        format!("{base} {b}"),
        "gone from the set at the call"
    );
    gw.tick(DT);
    assert!(id(&gw, "A").is_none() && id(&gw, "AChild").is_none());
    assert!(
        logs(&console).iter().any(|m| m == "destroy A"),
        "OnDestroy ran"
    );
    assert!(!gw.scene().borrow().world.contains(a_child));
    let err = gw
        .script_manager()
        .eval(&format!("Scene.Unload('{a}')"))
        .unwrap_err();
    assert!(err.contains("not a loaded scene"), "{err}");
}

#[test]
fn an_unloaded_scenes_ids_are_dead_and_a_reload_gives_new_ones() {
    let (mut gw, [_, a, _], _) = playing("reload");
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    gw.tick(DT);
    let first = id(&gw, "A").unwrap();
    eval(&gw, &format!("held = {first}; Scene.Unload('{a}')"));
    gw.tick(DT);
    assert_eq!(eval(&gw, "return tostring(Scene.GetName(held))"), "nil");
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    gw.tick(DT);
    let again = id(&gw, "A").unwrap();
    assert!(again > first, "ids are never reused: {again} after {first}");
}

#[test]
fn the_active_scenes_settings_follow_set_active_scene() {
    let (mut gw, [base, a, _], _) = playing("active");
    let fog = |gw: &GameWorld| gw.scene().borrow().fog.density;
    let base_fog = fog(&gw);
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    gw.tick(DT);
    assert_eq!(fog(&gw), base_fog, "the additive scene's fog waits");
    eval(&gw, &format!("Scene.SetActiveScene('{a}')"));
    assert_eq!(fog(&gw), 0.25);
    assert_eq!(eval(&gw, "return Scene.GetActivePath()"), a);
    let spawned = eval(&gw, "return Scene.CreateEntity('Spawned')");
    assert_eq!(eval(&gw, &format!("return Scene.Of({spawned})")), a);
    eval(&gw, &format!("Scene.SetActiveScene('{base}')"));
    assert_eq!(fog(&gw), base_fog);
}

#[test]
fn a_single_load_replaces_every_loaded_scene_but_the_survivors() {
    let (mut gw, [base, a, b], _) = playing("single");
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    gw.tick(DT);
    let keep = id(&gw, "A").unwrap();
    eval(
        &gw,
        &format!("Scene.DontDestroyOnLoad({keep}); Scene.Load('{b}')"),
    );
    gw.tick(DT);
    assert_eq!(loaded(&gw), b);
    assert!(id(&gw, "Base").is_none() && id(&gw, "B").is_some());
    assert_eq!(
        eval(&gw, &format!("return Scene.Of({keep})")),
        "DontDestroyOnLoad"
    );
    let err = gw
        .script_manager()
        .eval(&format!("Scene.SetActiveScene('{base}')"));
    assert!(err.unwrap_err().contains("not a loaded scene"));
}

#[test]
fn stop_puts_back_the_one_scene_play_began_in() {
    let (mut gw, [base, a, _], _) = playing("stop");
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    gw.tick(DT);
    eval(&gw, &format!("Scene.SetActiveScene('{a}')"));
    gw.set_playing(false);
    gw.poll_transition();
    gw.init_edit_runtime().unwrap();
    assert_eq!(loaded(&gw), base);
    assert_eq!(eval(&gw, "return Scene.GetActivePath()"), base);
    assert!(
        id(&gw, "A").is_none(),
        "the additive scene's entities went with Play"
    );
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    assert_eq!(
        loaded(&gw),
        format!("{base} {a}"),
        "edit mode joins it at once"
    );
}
