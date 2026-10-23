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

/// The `field` of every event after `after_seq`, comma-joined.
fn read(s: &Session, after_seq: &str, field: &str) -> String {
    let line = format!(
        "local t = {{}} for _, e in ipairs(Debug.AudioEvents({after_seq})) do
            t[#t + 1] = tostring(e.{field}) end return table.concat(t, ',')"
    );
    s.eval(&line).unwrap()
}

#[test]
fn a_play_at_one_shot_is_logged_with_its_seq_tick_clip_and_position() {
    let s = session();
    at_frame(&s, 7);
    s.eval("Audio.PlayAt('sfx/shot.ogg', 1, 2, 3, 0.5)")
        .unwrap();
    let read = "local e = Debug.AudioEvents()[1]
        return table.concat({ #Debug.AudioEvents(), e.seq, e.kind, e.tick, e.clip,
            e.position.x, e.position.y, e.position.z, e.volume, e.entity }, ' ')";
    assert_eq!(
        s.eval(read).unwrap(),
        "1 1 PlayAt 7 sfx/shot.ogg 1.0 2.0 3.0 0.5 0"
    );
}

#[test]
fn after_seq_keeps_the_events_past_it() {
    let s = session();
    at_frame(&s, 3);
    s.eval("Audio.PlayAt('a.ogg', 0, 0, 0)").unwrap();
    at_frame(&s, 9);
    s.eval("Audio.PlayAt('b.ogg', 0, 0, 0)").unwrap();
    assert_eq!(read(&s, "", "clip"), "a.ogg,b.ogg");
    assert_eq!(read(&s, "1", "clip"), "b.ogg");
    assert_eq!(read(&s, "2", "clip"), "");
}

#[test]
fn an_event_logged_after_a_poll_in_the_same_frame_is_not_lost() {
    let s = session();
    at_frame(&s, 4);
    s.eval("Audio.PlayAt('a.ogg', 0, 0, 0)").unwrap();
    let cursor = s
        .eval("local ev = Debug.AudioEvents() return ev[#ev].seq")
        .unwrap();
    s.eval("Audio.PlayAt('b.ogg', 0, 0, 0)").unwrap();
    assert_eq!(read(&s, &cursor, "clip"), "b.ogg");
    assert_eq!(read(&s, &cursor, "tick"), "4", "same frame as the poll");
}

#[test]
fn the_oldest_seq_held_signals_events_that_rolled_off() {
    let s = session();
    s.eval("for i = 1, 1030 do Audio.PlayAt('a.ogg', 0, 0, 0) end")
        .unwrap();
    let read = "local ev, oldest = Debug.AudioEvents(0)
        return table.concat({ #ev, oldest, ev[1].seq, ev[#ev].seq }, ' ')";
    // A cursor of 0 is below `oldest - 1`: six events rolled off the ring.
    assert_eq!(s.eval(read).unwrap(), "1024 7 7 1030");
    let fresh = "local ev, oldest = Debug.AudioEvents(1030) return #ev .. ' ' .. oldest";
    assert_eq!(
        s.eval(fresh).unwrap(),
        "0 7",
        "an up-to-date cursor sees no gap"
    );
}

#[test]
fn play_and_stop_reach_the_log() {
    let s = session();
    s.eval("radio = Scene.CreateEntity('radio') Scene.AddComponent(radio, 'AudioSource')")
        .unwrap();
    s.eval("Audio.Play(radio) Audio.Stop(radio)").unwrap();
    let id = s.eval("return radio").unwrap();
    assert_eq!(read(&s, "", "kind"), "Play,Stop");
    assert_eq!(read(&s, "", "entity"), format!("{id},{id}"));
    assert_eq!(read(&s, "1", "clip"), "", "a Stop logs no clip");
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
        return table.concat({ #Debug.AudioVoices(), v.entity, tostring(v.playing) }, ' ')";
    assert_eq!(s.eval(read).unwrap(), format!("1 {id} false"));
}

#[test]
fn a_roster_row_matches_get_spatial_for_its_entity() {
    let s = session();
    s.eval(
        "radio = Scene.CreateEntity('radio') Scene.AddComponent(radio, 'AudioSource')
         Transform.SetPosition(radio, -4, 0, -3) Audio.Play(radio)",
    )
    .unwrap();
    let read = "local g, p, playing, o = Audio.GetSpatial(radio)
        local v = Debug.AudioVoices()[1]
        return table.concat({ tostring(v.spatial.gain == g), tostring(v.spatial.pan == p),
            tostring(v.playing == playing), tostring(v.occlusion == o), g }, ' ')";
    let out = s.eval(read).unwrap();
    assert!(out.starts_with("true true true true "), "{out}");
}
