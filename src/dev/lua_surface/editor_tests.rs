//! `Editor.EnterPlay/ExitPlay/IsPlaying` (#1360): the play switch an agent drives
//! through the headless session, taking effect at the frame after the call.

use super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

/// Run `line` as one channel command, panicking with the Lua error on failure.
fn run(s: &mut Session, line: &str) -> String {
    s.command(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

#[test]
fn enter_play_takes_effect_at_the_next_frame() {
    let mut s = session();
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "false");
    // The call only requests: inside it, the world is still in edit mode.
    assert_eq!(
        run(
            &mut s,
            "local asked = Editor.EnterPlay(); return asked, Editor.IsPlaying()"
        ),
        run_shape("true", "false")
    );
    // The command's frame took the request, so the next call runs in Play.
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "true");
    assert!(s.world().world.scene.borrow().is_playing());
}

/// How the evaluator renders two results, read from the evaluator itself.
fn run_shape(a: &str, b: &str) -> String {
    session()
        .eval(&format!("return {a}, {b}"))
        .expect("two literals evaluate")
}

#[test]
fn a_repeated_transition_is_a_no_op_that_returns_false() {
    let mut s = session();
    assert_eq!(
        run(&mut s, "Editor.ExitPlay()"),
        "false",
        "already in edit mode"
    );
    assert_eq!(run(&mut s, "Editor.EnterPlay()"), "true");
    assert_eq!(
        run(&mut s, "Editor.EnterPlay()"),
        "false",
        "already playing"
    );
    // Two requests before one frame: the second sees where the first is heading.
    let both = run(
        &mut s,
        "local a = Editor.ExitPlay(); return a, Editor.ExitPlay()",
    );
    assert_eq!(both, run_shape("true", "false"));
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "false");
}

#[test]
fn exit_play_restores_the_edit_scene_and_drops_play_globals() {
    let mut s = session();
    let id = run(&mut s, "Scene.CreateEntity(\"Crate\", \"Box\")");
    run(&mut s, &format!("Transform.SetPosition({id}, 1, 2, 3)"));
    run(&mut s, "Editor.EnterPlay()");
    run(&mut s, &format!("Transform.SetPosition({id}, 9, 9, 9)"));
    run(&mut s, "played = true");
    run(&mut s, "Editor.ExitPlay()");
    // Stop restored the edit-mode position, as the Stop button does.
    let x = format!("(Transform.GetPosition({id})) == 1");
    assert_eq!(run(&mut s, &x), "true");
    // Each Play/Stop builds a fresh VM: the play-mode global is gone.
    assert_eq!(run(&mut s, "played == nil"), "true");
}

#[test]
fn steps_queued_in_play_run_before_the_next_command() {
    let mut s = session();
    run(&mut s, "Editor.EnterPlay()");
    run(&mut s, "Time.Pause()");
    run(&mut s, "Time.Step(3)");
    assert_eq!(s.world().play_frame(), 3);
    assert_eq!(run(&mut s, "Time.PendingSteps()"), "0");
}

#[test]
fn editor_is_on_the_surface() {
    let surface = session().world().script_manager().api_surface();
    let editor = surface.get("Editor").expect("`Editor` is installed");
    for verb in ["EnterPlay", "ExitPlay", "IsPlaying"] {
        assert!(editor.contains(verb), "`Editor.{verb}` is missing");
    }
}
