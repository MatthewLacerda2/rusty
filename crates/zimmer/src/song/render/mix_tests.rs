//! Pinned by the number: a whole-song peak or length cannot see a part's sign; these can.

use super::*;

/// Left is `value`, right its negative, so a swapped channel or a flipped sign shows.
fn lopsided(frames: usize, value: f32) -> Stereo {
    Stereo {
        l: vec![value; frames],
        r: vec![-value; frames],
    }
}

#[test]
fn a_sent_sum_is_built_for_a_partial_or_moving_send_into_a_room() {
    let song = |send: f32, room: bool, param: &str| -> Song {
        let fx = if room {
            r#"{ "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.2 }"#
        } else {
            ""
        };
        serde_json::from_str(&format!(
            r#"{{ "bpm": 120, "tracks": [{{ "name": "pad", "patch": "p", "send": {send} }}],
                 "patterns": {{}}, "arrangement": [], "fx": [{fx}],
                 "automation": [{{ "track": "pad", "param": "{param}",
                                  "points": [{{ "beat": 0, "value": 0.5 }}] }}] }}"#
        ))
        .expect("the fixture reads")
    };
    let whole = |song: &Song| {
        let scope = Scope::of(song, &Default::default(), &Clock::at(120.0)).expect("whole");
        sends(song, &scope)
    };
    assert!(
        !whole(&song(1.0, true, "gain")),
        "everything sent, a fader moving"
    );
    assert!(whole(&song(1.0, true, "send")), "a moving send");
    assert!(whole(&song(0.5, true, "gain")), "a written partial send");
    assert!(!whole(&song(0.5, false, "send")), "no room to send into");
}

#[test]
fn a_part_is_added_where_it_starts_and_the_way_up_it_came() {
    let mut master = Stereo::centred(vec![0.25; 6]);
    mix_into(&mut master, &lopsided(2, 1.0), 3, UNITY);
    assert_eq!(master.l, vec![0.25, 0.25, 0.25, 1.25, 1.25, 0.25]);
    assert_eq!(master.r, vec![0.25, 0.25, 0.25, -0.75, -0.75, 0.25]);
}

#[test]
fn a_part_landing_past_the_end_extends_the_mix() {
    let mut master = Stereo::silence(2);
    mix_into(&mut master, &lopsided(3, 0.5), 4, UNITY);
    assert_eq!(master.frames(), 7, "four frames in, three long");
    assert_eq!(master.l, vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5]);
}

#[test]
fn each_side_arrives_at_its_own_gain() {
    let mut master = Stereo::silence(1);
    mix_into(&mut master, &Stereo::centred(vec![1.0]), 0, (0.25, 0.75));
    assert_eq!(master.l, vec![0.25]);
    assert_eq!(master.r, vec![0.75]);
}

#[test]
fn scaling_a_bus_multiplies_each_side_by_its_own_gain() {
    let mut bus = lopsided(2, 1.0);
    scale(&mut bus, (0.5, 0.25));
    assert_eq!(bus.l, vec![0.5, 0.5]);
    assert_eq!(bus.r, vec![-0.25, -0.25]);
}

#[test]
fn a_placement_is_the_fader_times_the_position() {
    assert_eq!(placement(0.8, 0.0), (0.8, 0.8));
    let (l, r) = placement(0.5, -1.0);
    assert!(
        (l - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-6,
        "left {l}"
    );
    assert_eq!(r, 0.0);
    assert_eq!(
        UNITY,
        placement(1.0, 0.0),
        "a bus is summed at the identity"
    );
}
