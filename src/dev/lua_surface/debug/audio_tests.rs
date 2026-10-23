//! `Debug.AudioEvents` / `Debug.AudioVoices` (#1247), read with no sound card: the
//! session's maestro carries the `NullBackend`.

use crate::dev::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

/// Move the frame counter, the tick every audio action is logged under.
fn at_frame(s: &Session, frame: u64) {
    s.world().time().borrow_mut().frame_count = frame;
}

#[test]
fn a_play_at_one_shot_is_logged_with_its_tick_clip_and_position() {
    let s = session();
    at_frame(&s, 7);
    s.eval("Audio.PlayAt('sfx/shot.ogg', 1, 2, 3, 0.5)")
        .unwrap();
    let read = "local e = Debug.AudioEvents()[1]
        return table.concat({ #Debug.AudioEvents(), e.kind, e.tick, e.clip,
            e.position[1], e.position[2], e.position[3], e.volume, e.entity }, ' ')";
    assert_eq!(
        s.eval(read).unwrap(),
        "1 PlayAt 7 sfx/shot.ogg 1.0 2.0 3.0 0.5 0"
    );
}

#[test]
fn since_tick_keeps_events_at_or_after_it() {
    let s = session();
    at_frame(&s, 3);
    s.eval("Audio.PlayAt('a.ogg', 0, 0, 0)").unwrap();
    at_frame(&s, 9);
    s.eval("Audio.PlayAt('b.ogg', 0, 0, 0)").unwrap();
    let clips = |since: &str| {
        let line = format!(
            "local t = {{}} for _, e in ipairs(Debug.AudioEvents({since})) do
                t[#t + 1] = e.clip end return table.concat(t, ',')"
        );
        s.eval(&line).unwrap()
    };
    assert_eq!(clips(""), "a.ogg,b.ogg");
    assert_eq!(clips("9"), "b.ogg");
    assert_eq!(clips("10"), "");
}

#[test]
fn the_roster_has_a_row_per_audio_source_and_none_for_a_one_shot() {
    let s = session();
    s.eval("Audio.PlayAt('a.ogg', 0, 0, 0)").unwrap();
    assert_eq!(s.eval("return #Debug.AudioVoices()").unwrap(), "0");
    let id = s
        .eval("local id = Scene.CreateEntity('radio') Scene.AddComponent(id, 'AudioSource') return id")
        .unwrap();
    let read = "local v = Debug.AudioVoices()[1]
        return table.concat({ #Debug.AudioVoices(), v.entity, tostring(v.playing),
            type(v.spatial.gain), type(v.occlusion) }, ' ')";
    assert_eq!(s.eval(read).unwrap(), format!("1 {id} false number number"));
}
