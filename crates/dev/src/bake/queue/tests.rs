use std::path::{Path, PathBuf};

use super::file::{write, Entry};
use super::{read, resolve, Outcome, Owner, Queue, Read, Resumed, Runner, Selection, State};

fn project(name: &str) -> PathBuf {
    let dir = rusty_test_support::temp::dir().join(format!("bake_queue_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/scenes")).unwrap();
    dir
}

fn states(root: &Path) -> Vec<State> {
    match read(root) {
        Read::Found(q) => q.scenes.iter().map(|e| e.state).collect(),
        other => panic!("no queue: {other:?}"),
    }
}

fn resumed(root: &Path) -> Runner {
    match Runner::resume(root).unwrap() {
        Resumed::Runner(r) => r,
        Resumed::Nothing(why) => panic!("nothing to resume: {why}"),
    }
}

#[test]
fn all_is_every_scene_in_path_order_but_the_excepted_and_the_cache() {
    let root = project("select");
    for p in [
        "assets/scenes/b.scene",
        "assets/a.scene",
        "cache/x.scene",
        "c.scene",
    ] {
        std::fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
        std::fs::write(root.join(p), "{}").unwrap();
    }
    std::fs::write(root.join("assets/notes.txt"), "").unwrap();
    let except = vec!["./assets/scenes/b.scene".to_string()];
    let got = resolve(&root, &Selection::All { except });
    assert_eq!(got, ["assets/a.scene", "c.scene"]);
    let list = Selection::List(vec!["z.scene".into(), "a.scene".into()]);
    assert_eq!(resolve(&root, &list), ["z.scene", "a.scene"], "as listed");
}

#[test]
fn a_failed_scene_is_marked_with_why_and_the_queue_goes_on() {
    let root = project("fails");
    let paths = vec!["a.scene".to_string(), "b.scene".into(), "c.scene".into()];
    let mut runner = Runner::start(&root, paths).unwrap();
    assert_eq!(
        states(&root),
        [State::Pending; 3],
        "written before the first step"
    );
    let mut seen = Vec::new();
    runner
        .run_to_end(&mut |path| {
            seen.push(path.to_string());
            assert_eq!(
                states(&root)[seen.len() - 1],
                State::Running,
                "marked first"
            );
            match path {
                "b.scene" => Outcome::Failed("test hook: b fails".into()),
                _ => Outcome::Done("baked".into()),
            }
        })
        .unwrap();
    assert_eq!(seen, ["a.scene", "b.scene", "c.scene"]);
    assert_eq!(states(&root), [State::Done, State::Failed, State::Done]);
    let Read::Found(q) = read(&root) else {
        panic!()
    };
    assert_eq!(q.scenes[1].reason.as_deref(), Some("test hook: b fails"));
    assert!(runner.summary().contains("2 of 3 scene(s) done, 1 failed"));
}

#[test]
fn a_scene_left_running_is_retried_once_then_failed() {
    let root = project("killed");
    let mut queue = Queue::new(vec!["a.scene".into(), "b.scene".into(), "c.scene".into()]);
    queue.scenes[0].state = State::Done;
    queue.scenes[1] = Entry {
        state: State::Running,
        attempts: 1,
        ..Entry::pending("b.scene".into())
    };
    write(&root, &queue).unwrap();
    let mut seen = Vec::new();
    // The retry dies too: the process ends inside it, leaving `running` with two starts.
    let mut runner = resumed(&root);
    let (index, path) = runner.start_next().unwrap().unwrap();
    assert_eq!(
        (index, path.as_str()),
        (1, "b.scene"),
        "the done scene is skipped"
    );
    drop(runner);
    let mut runner = resumed(&root);
    runner
        .run_to_end(&mut |p| {
            seen.push(p.to_string());
            Outcome::Done("baked".into())
        })
        .unwrap();
    assert_eq!(seen, ["c.scene"], "no third start");
    assert_eq!(states(&root), [State::Done, State::Failed, State::Done]);
}

#[test]
fn a_cancel_leaves_the_rest_pending_and_spends_no_retry() {
    let root = project("cancel");
    let mut runner = Runner::start(&root, vec!["a.scene".into(), "b.scene".into()]).unwrap();
    runner.run_to_end(&mut |_| Outcome::Cancelled).unwrap();
    assert!(runner.was_cancelled());
    drop(runner);
    let Read::Found(q) = read(&root) else {
        panic!()
    };
    assert_eq!(states(&root), [State::Pending; 2]);
    assert_eq!(q.scenes[0].attempts, 0);
    let mut runner = resumed(&root);
    runner
        .run_to_end(&mut |_| Outcome::Done("ok".into()))
        .unwrap();
    assert_eq!(states(&root), [State::Done; 2]);
    drop(runner);
    let nothing = Runner::resume(&root).unwrap();
    assert!(matches!(nothing, Resumed::Nothing(why) if why.contains("finished")));
}

#[test]
fn a_second_owner_is_refused_while_the_first_holds_the_queue() {
    let root = project("owner");
    let runner = Runner::start(&root, vec!["a.scene".into()]).unwrap();
    assert!(Owner::is_held(&root));
    let err = Runner::resume(&root).err().expect("held");
    assert!(err.contains("already being baked by this process"), "{err}");
    let err = Runner::start(&root, vec![]).err().expect("held");
    assert!(err.contains("already being baked"), "{err}");
    drop(runner);
    assert!(!Owner::is_held(&root), "freed with its owner");
}

#[test]
fn an_unreadable_queue_file_is_reported_and_read_as_none() {
    let root = project("corrupt");
    let file = root.join(super::QUEUE_PATH);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    for text in [
        "{\"scenes\": [{\"path\": \"a.scene\", \"sta",
        "not json",
        "{\"scenes\": [{\"path\": \"a.scene\", \"state\": \"baking\"}]}",
    ] {
        std::fs::write(&file, text).unwrap();
        assert!(matches!(read(&root), Read::Unreadable(_)), "{text}");
        let nothing = Runner::resume(&root).unwrap();
        assert!(matches!(nothing, Resumed::Nothing(why) if why.contains("no queue")));
    }
    assert!(matches!(read(&project("none")), Read::Missing));
}

#[test]
fn the_command_line_names_scenes_all_but_some_or_a_resume() {
    use super::cli::{parse, Command};
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    assert_eq!(parse(&args("--out x")).unwrap(), None, "another verb's");
    let all = parse(&args("--bake all --except k.scene j.scene")).unwrap();
    let except = vec!["k.scene".to_string(), "j.scene".into()];
    assert_eq!(all, Some(Command::Bake(Selection::All { except })));
    let list = parse(&args("--bake a.scene b.scene")).unwrap();
    let paths = vec!["a.scene".to_string(), "b.scene".into()];
    assert_eq!(list, Some(Command::Bake(Selection::List(paths))));
    assert_eq!(
        parse(&args("--bake-resume")).unwrap(),
        Some(Command::Resume)
    );
    for bad in [
        "--bake",
        "--bake a.scene --except b.scene",
        "--bake-resume --bake all",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad}");
    }
}
