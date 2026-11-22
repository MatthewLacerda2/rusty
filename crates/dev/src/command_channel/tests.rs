use super::*;
use crate::session::Session;

fn session() -> Session {
    Session::new("").expect("session boots in edit mode")
}

fn done(reply: Reply) -> String {
    match reply {
        Reply::Done(line) => line,
        Reply::Progress(line) => panic!("no bake ran, yet it told {line:?}"),
    }
}

fn all_replies(s: &Session, tx: &Sender<Command>, ch: &CommandChannel, line: &str) -> Vec<String> {
    let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
    tx.send((line.to_string(), reply_tx))
        .expect("queue command");
    ch.drain(s.world().script_manager(), s.world().console());
    let tag = |r| match r {
        Reply::Progress(text) => format!("progress {text}"),
        Reply::Done(line) => line,
    };
    reply_rx.try_iter().map(tag).collect()
}

#[test]
fn only_a_command_that_asks_gets_its_bakes_progress() {
    let s = session();
    let path = rusty_test_support::temp::dir().join("channel_progress.scene");
    let path = path.to_string_lossy().into_owned();
    rusty_edit::save_to_file(&s.world().scene().borrow(), &path).unwrap();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path);
    let (tx, ch) = local_channel();
    let plain = all_replies(&s, &tx, &ch, "Lighting.BakeOcclusion()");
    assert_eq!(plain.len(), 1, "{plain:?}");
    assert!(plain[0].contains("\"ok\":true"), "{plain:?}");
    let asked = all_replies(&s, &tx, &ch, &format!("{ASK}Lighting.BakeOcclusion()"));
    let (last, progress) = asked.split_last().expect("a reply");
    assert!(last.contains("\"ok\":true"), "{asked:?}");
    assert!(!progress.is_empty(), "{asked:?}");
    assert!(
        progress.iter().all(|l| l.starts_with("progress [")),
        "{asked:?}"
    );
}

fn local_channel() -> (Sender<Command>, CommandChannel) {
    let (tx, rx) = mpsc::channel::<Command>();
    let channel = CommandChannel {
        rx,
        socket_path: std::path::PathBuf::from("/tmp/rusty-test-unused.sock"),
    };
    (tx, channel)
}

fn round_trip(s: &Session, tx: &Sender<Command>, ch: &CommandChannel, line: &str) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
    tx.send((line.to_string(), reply_tx))
        .expect("queue command");
    let ran = ch.drain(s.world().script_manager(), s.world().console());
    assert_eq!(ran, 1, "drain counts the one line it ran");
    done(reply_rx.recv().expect("drain replies once"))
}

#[test]
fn drain_frames_a_successful_command() {
    let s = session();
    let (tx, ch) = local_channel();
    let reply = round_trip(&s, &tx, &ch, "1 + 1");
    assert!(reply.contains("\"ok\":true") && reply.contains("\"result\":\"2\""));
}

#[test]
fn drain_round_trips_a_setter_and_getter_against_the_live_world() {
    let s = session();
    s.world()
        .scene()
        .borrow_mut()
        .add_entity("Probe".to_string());
    let (tx, ch) = local_channel();

    let id_reply = round_trip(&s, &tx, &ch, "Scene.FindEntityByName(\"Probe\")");
    let id: serde_json::Value = serde_json::from_str(&id_reply).unwrap();
    let id = id["result"].as_str().unwrap().to_string();
    assert_ne!(id, "0", "the live scene must resolve the authored entity");

    round_trip(
        &s,
        &tx,
        &ch,
        &format!("Transform.SetPosition({}, 1, 2, 3)", id),
    );
    let pos = round_trip(&s, &tx, &ch, &format!("Transform.GetPosition({})", id));
    assert!(
        pos.contains("\"ok\":true") && pos.contains("\"result\":\"1, 2, 3\""),
        "the setter must persist on the live world: {pos}"
    );
}

#[test]
fn malformed_line_frames_an_error_and_channel_survives() {
    let s = session();
    let (tx, ch) = local_channel();

    let bad = round_trip(&s, &tx, &ch, "this is not lua %%%");
    assert!(
        bad.contains("\"ok\":false") && bad.contains("\"error\""),
        "a malformed line must frame an error: {bad}"
    );

    let good = round_trip(&s, &tx, &ch, "1 + 1");
    assert!(good.contains("\"ok\":true") && good.contains("\"result\":\"2\""));
}

#[test]
fn drain_replies_once_per_queued_command_in_order() {
    let s = session();
    let (tx, ch) = local_channel();
    let (a_tx, a_rx) = mpsc::channel::<Reply>();
    let (b_tx, b_rx) = mpsc::channel::<Reply>();
    tx.send(("1 + 1".to_string(), a_tx)).unwrap();
    tx.send(("2 + 2".to_string(), b_tx)).unwrap();

    ch.drain(s.world().script_manager(), s.world().console());
    assert!(done(a_rx.recv().unwrap()).contains("\"result\":\"2\""));
    assert!(done(b_rx.recv().unwrap()).contains("\"result\":\"4\""));
}

#[test]
fn start_binds_a_unix_socket_then_drops_cleanly() {
    // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is 104 bytes on macOS)
    let dir = std::env::temp_dir();
    let path = dir.join(format!("rusty-cmd-test-{}.sock", std::process::id()));

    let console = RefCell::new(ConsoleLogs::new());
    // The path goes to the bind: setting `RUSTY_CMD_SOCK` would leak into every test of
    // this process.
    let channel =
        CommandChannel::listen(&console, |tx| transport::bind_and_listen(tx, path.clone()))
            .expect("bind the unix socket");
    assert!(path.exists(), "start() must create the socket file");

    drop(channel);
    assert!(!path.exists(), "drop must remove the socket file");
}

#[test]
fn a_command_wakes_a_sleeping_window() {
    use std::io::Write;
    // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is 104 bytes on macOS)
    let dir = std::env::temp_dir();
    let path = dir.join(format!("rusty-cmd-wake-{}.sock", std::process::id()));
    let console = RefCell::new(ConsoleLogs::new());
    let channel =
        CommandChannel::listen(&console, |tx| transport::bind_and_listen(tx, path.clone()))
            .expect("bind the unix socket");
    let before = rusty_core::wake::rings();
    let mut client = std::os::unix::net::UnixStream::connect(&path).expect("connect");
    writeln!(client, "return 1").expect("send a command");
    let (line, _reply) = channel.rx.recv().expect("the handler forwards it");
    assert_eq!(line, "return 1");
    for _ in 0..200 {
        if rusty_core::wake::rings() > before {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the command never woke the window");
}
