use super::*;

fn session() -> Session {
    Session::new("").expect("session boots in edit mode")
}

#[test]
fn stays_in_edit_mode_with_live_evaluator() {
    let s = session();
    assert!(
        !s.world().play_requested(),
        "session must not force play mode"
    );
    assert!(
        s.world().script_manager().is_live(),
        "edit-mode evaluator must be live"
    );
}

#[test]
fn getter_setter_round_trip_through_channel() {
    let s = session();
    // Author an entity into the live edit world, then round-trip a setter and a
    // getter through the same command channel against that live state.
    let authored = s
        .world()
        .scene()
        .borrow_mut()
        .add_entity("Probe".to_string());
    let id = s.eval("Scene.FindEntityByName(\"Probe\")").unwrap();
    assert_eq!(
        id,
        authored.to_string(),
        "the live scene resolves the entity"
    );

    s.eval(&format!("Transform.SetPosition({}, 1, 2, 3)", id))
        .unwrap();
    let pos = s.eval(&format!("Transform.GetPosition({})", id)).unwrap();
    assert_eq!(pos, "1, 2, 3", "the setter must persist on the live world");
}

#[test]
fn state_persists_across_commands() {
    let s = session();
    s.world()
        .scene()
        .borrow_mut()
        .add_entity("Keep".to_string());
    let id = s.eval("Scene.FindEntityByName(\"Keep\")").unwrap();
    s.eval(&format!("Transform.SetPosition({}, 5, 0, 0)", id))
        .unwrap();
    // A later, independent command still sees the earlier mutation.
    let pos = s.eval(&format!("Transform.GetPosition({})", id)).unwrap();
    assert_eq!(pos, "5, 0, 0");
}

#[test]
fn errors_do_not_kill_the_session() {
    let s = session();
    assert!(s.eval("this is not lua %%%").is_err());
    // The session is still usable after a bad command.
    assert_eq!(s.eval("1 + 1").unwrap(), "2");
}

#[test]
fn logging_through_the_real_shared_cell_does_not_panic() {
    // Regression for #208: the session's single console cell is the very one
    // `print` / `Debug.*` write into during eval. The fix evaluates with that
    // cell free, so logging from a command no longer double-borrows and panics.
    let s = session();
    assert_eq!(s.eval("print(\"hello-headless\")").unwrap(), "");
    assert_eq!(s.eval("Debug.Log(\"debug-headless\")").unwrap(), "");
    assert!(s.eval("Debug.Warn(\"warn-headless\")").is_ok());
    assert!(s.eval("Debug.Error(\"err-headless\")").is_ok());

    let log = s.world().console().borrow();
    for expected in [
        "hello-headless",
        "debug-headless",
        "warn-headless",
        "err-headless",
    ] {
        assert!(
            log.messages.iter().any(|(m, _)| m == expected),
            "{expected} must reach the shared console"
        );
    }
}

#[test]
fn run_frames_one_response_per_command() {
    let s = session();
    let input = b"1 + 1\n\nnope %%%\n" as &[u8];
    let mut out = Vec::new();
    run(&s, input, &mut out).expect("channel runs to EOF");
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // Two non-blank commands -> two response lines (the blank line is skipped).
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"ok\":true") && lines[0].contains("\"result\":\"2\""));
    assert!(lines[1].contains("\"ok\":false") && lines[1].contains("\"error\""));
}

#[test]
fn debug_snapshot_round_trips_authored_state() {
    let s = session();
    // Author + configure an entity purely through the command channel, then read
    // it back via the rich snapshot on the same channel (#180 acceptance).
    let id = s.eval("Scene.CreateEntity(\"Widget\", \"Box\")").unwrap();
    s.eval(&format!("Transform.SetPosition({}, 4, 5, 6)", id))
        .unwrap();

    let snap = s.eval("Debug.Snapshot()").unwrap();
    let world: serde_json::Value = serde_json::from_str(&snap).unwrap();
    assert_eq!(world["play_state"], "editor", "session is edit-mode");

    let entities = world["entities"].as_array().unwrap();
    let widget = entities
        .iter()
        .find(|e| e["name"] == "Widget")
        .expect("authored entity appears in the snapshot");
    assert_eq!(widget["transform"]["pos"][0].as_f64(), Some(4.0));
    assert!(
        widget["components"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "Mesh"),
        "the Box primitive's mesh shows in the inventory"
    );
}

#[test]
fn engine_warnings_ride_along_with_the_reply() {
    // #1139: a `log::warn!` from engine code reaches the agent driving the session.
    crate::logging::init();
    let s = session();
    log::warn!("[Test] session: an asset was skipped");
    let mut out = Vec::new();
    run(&s, b"1 + 1\n2 + 2\n" as &[u8], &mut out).expect("channel runs to EOF");
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("\"warnings\":[") && lines[0].contains("session: an asset"));
    assert!(
        !lines[1].contains("session: an asset"),
        "each warning is reported once"
    );
    let log = s.world().console().borrow();
    assert!(log
        .messages
        .iter()
        .any(|(m, _)| m == "[Test] session: an asset was skipped"));
}
