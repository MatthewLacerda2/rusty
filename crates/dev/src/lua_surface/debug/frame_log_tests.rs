use super::query;
use crate::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

#[test]
fn frame_log_returns_its_path_and_records() {
    let s = session();
    let line = "local l = Debug.FrameLog{ last_seconds = 60 }
        return l.path .. ' ' .. type(l.records) .. ' ' .. l.skipped .. ' ' .. l.truncated .. ' ' .. l.dropped";
    assert_eq!(s.eval(line).unwrap(), "saved/logs table 0 0 0");
}

#[test]
fn an_unknown_option_or_a_bad_value_is_refused() {
    let s = session();
    for (opts, says) in [
        ("{ last_second = 1 }", "unknown option"),
        ("{ last_seconds = -1 }", "at least 0"),
        ("{ over_ms = 0/0 }", "at least 0"),
        ("{ limit = 2.5 }", "whole number"),
        ("{ limit = 0 }", "whole number"),
        ("{ channel = 'audi' }", "none of"),
        ("{ channel = 3 }", "none of"),
    ] {
        let err = s.eval(&format!("return Debug.FrameLog{opts}")).unwrap_err();
        assert!(err.contains(says), "{opts}: {err}");
    }
}

#[test]
fn options_become_the_query() {
    let lua = mlua::Lua::new();
    let opts = lua
        .load("return { last_seconds = 10, over_ms = 33, channel = 'console', limit = math.huge }")
        .eval()
        .unwrap();
    let q = query(Some(opts)).unwrap();
    assert_eq!((q.last_seconds, q.over_ms), (Some(10.0), Some(33.0)));
    assert_eq!(
        (q.channel.as_deref(), q.limit),
        (Some("console"), Some(usize::MAX))
    );
    assert_eq!(query(None).unwrap(), crate::frame_log::LogQuery::default());
}

#[test]
fn channels_are_switched_and_read_back() {
    let s = session();
    s.eval("Debug.SetLogChannels{ audio = false, gpu_passes = false }")
        .unwrap();
    let read = "local c = Debug.GetLogChannels()
        return tostring(c.audio) .. tostring(c.gpu_passes) .. tostring(c.frames)";
    assert_eq!(s.eval(read).unwrap(), "falsefalsetrue");
    let err = s.eval("Debug.SetLogChannels{ audi = false }").unwrap_err();
    assert!(err.contains("no log channel"), "{err}");
    let err = s.eval("Debug.SetLogChannels{ audio = 1 }").unwrap_err();
    assert!(err.contains("SetLogChannels"), "{err}");
    let settings = s.world().resources.application.borrow().log_settings();
    assert!(
        !settings.channels.audio,
        "the project's settings hold the switch"
    );
}
