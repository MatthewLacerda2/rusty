use super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

fn run(s: &mut Session, line: &str) -> String {
    s.command(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

#[test]
fn enter_play_takes_effect_at_the_next_frame() {
    let mut s = session();
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "false");
    assert_eq!(
        run(
            &mut s,
            "local asked = Editor.EnterPlay(); return asked, Editor.IsPlaying()"
        ),
        run_shape(&s, "true", "false")
    );
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "true");
    assert!(s.world().world.scene.borrow().is_playing());
}

fn run_shape(s: &Session, a: &str, b: &str) -> String {
    s.eval(&format!("return {a}, {b}"))
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
    let both = run(
        &mut s,
        "local a = Editor.ExitPlay(); return a, Editor.ExitPlay()",
    );
    assert_eq!(both, run_shape(&s, "true", "false"));
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
    let x = format!("(Transform.GetPosition({id})) == 1");
    assert_eq!(run(&mut s, &x), "true");
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
fn a_host_request_pending_this_frame_counts() {
    let s = session();
    let app = &s.world().resources.application;
    app.borrow_mut().set_host_play(true);
    assert_eq!(s.eval("Editor.EnterPlay()").as_deref(), Ok("false"));
    assert_eq!(s.eval("Editor.ExitPlay()").as_deref(), Ok("true"));
}

#[test]
fn a_standalone_run_refuses_with_a_warning() {
    let mut s = session();
    let app = &s.world().resources.application;
    app.borrow_mut().mark_standalone();
    assert_eq!(run(&mut s, "Editor.EnterPlay()"), "false");
    assert_eq!(run(&mut s, "Editor.IsPlaying()"), "false");
    let console = s.world().console().borrow();
    let warned = console
        .messages
        .iter()
        .any(|(m, _)| m.contains("standalone"));
    assert!(warned, "the refusal says why");
}

#[test]
fn an_error_in_the_frame_rides_along_with_the_reply() {
    let mut s = session();
    let id = run(&mut s, "Scene.CreateEntity(\"Broken\", \"Box\")");
    run(
        &mut s,
        &format!("Scene.AddScript({id}, 'no_such_script_1360.lua')"),
    );
    s.take_warnings();
    run(&mut s, "Editor.EnterPlay()");
    let warnings = s.take_warnings();
    let named = warnings.iter().any(|w| w.contains("no_such_script_1360"));
    assert!(
        named,
        "the script's load error reaches the agent: {warnings:?}"
    );
}

#[test]
fn editor_is_on_the_surface() {
    let surface = session().world().script_manager().api_surface();
    let editor = surface.get("Editor").expect("`Editor` is installed");
    for verb in ["EnterPlay", "ExitPlay", "IsPlaying"] {
        assert!(editor.contains(verb), "`Editor.{verb}` is missing");
    }
}
