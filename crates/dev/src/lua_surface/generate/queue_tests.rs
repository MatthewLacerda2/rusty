use super::selection;
use crate::bake::queue::Selection;
use crate::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

#[test]
fn a_selection_is_a_list_all_or_all_but_some() {
    let lua = mlua::Lua::new();
    let pick = |src: &str| selection("GenerateScenes", lua.load(src).eval().unwrap());
    let list = pick("{ 'a.scene', 'b.scene' }").unwrap();
    assert_eq!(
        list,
        Selection::List(vec!["a.scene".into(), "b.scene".into()])
    );
    assert_eq!(pick("'all'").unwrap(), Selection::All { except: vec![] });
    let except = pick("{ all = true, except = { 'k.scene' } }").unwrap();
    assert_eq!(
        except,
        Selection::All {
            except: vec!["k.scene".into()]
        }
    );
    for bad in [
        "'some'",
        "42",
        "{ all = false }",
        "{}",
        "{ except = { 'k.scene' } }",
    ] {
        let err = pick(bad).unwrap_err().to_string();
        assert!(err.contains("list of scene paths"), "{bad}: {err}");
    }
}

#[test]
fn the_async_queue_needs_the_editor_and_names_the_blocking_verb() {
    let err = session()
        .eval("Lighting.GenerateScenesAsync('all')")
        .unwrap_err();
    assert!(err.contains("Lighting.GenerateScenes()"), "{err}");
}

#[test]
fn the_queue_is_refused_in_play_and_while_a_bake_runs() {
    let s = session();
    s.world().scene().borrow_mut().lighting_bake_running = true;
    let err = s.eval("Lighting.GenerateScenes('all')").unwrap_err();
    assert!(err.contains("already running"), "{err}");
    s.world().scene().borrow_mut().lighting_bake_running = false;
    s.world().scene().borrow_mut().set_playing(true);
    let err = s.eval("Lighting.GenerateScenes('all')").unwrap_err();
    assert!(err.contains("during Play"), "{err}");
}

#[test]
fn an_ask_that_names_no_scene_is_refused_by_both_verbs_before_any_write() {
    let s = session();
    for verb in ["GenerateScenes", "GenerateScenesAsync"] {
        for ask in ["{}", "{ except = { 'assets/scenes/kolvik.scene' } }"] {
            let err = s.eval(&format!("Lighting.{verb}({ask})")).unwrap_err();
            let says = format!("Lighting.{verb}: pass a list of scene paths");
            assert!(err.contains(&says), "{verb}({ask}): {err}");
        }
    }
}
