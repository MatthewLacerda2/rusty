//! Tests for a song's `tail`, `fit` and `fade` driven from Lua (#377): the
//! fields pass through to zimmer's `Song`, and the file comes out the length —
//! in samples — that each one promises.

use super::bake_tests::{lua_with_sound, tmp};

/// Four beats at 120 bpm: exactly two seconds, so 88,200 frames at 44.1 kHz.
const LOOP_FRAMES: usize = 88_200;

/// One sustained sine across the whole bar, with a release that rings a second
/// past it — the tail every mode has to do something about. `{extra}` is spliced
/// into the song table.
fn song(extra: &str) -> String {
    format!(
        r#"{{
        bpm = 120, seed = 3,
        tracks = {{ {{ name = "pad", patch = {{
            source = {{ kind = "osc_stack", oscs = {{ {{ wave = "sine" }} }} }},
            amp = {{ a = 0.01, d = 0.0, s = 1.0, r = 1.0 }} }} }} }},
        patterns = {{ bar = {{ beats = 4, notes = {{
            {{ track = "pad", note = "A3", start = 0.0, dur = 4.0, vel = 0.5 }} }} }} }},
        arrangement = {{ "bar" }},
        {extra}
    }}"#
    )
}

/// Bake `song(extra)` and return the left channel of the WAV it wrote.
fn bake(name: &str, extra: &str) -> Vec<i16> {
    let path = tmp(name);
    lua_with_sound()
        .load(format!("Sound.BakeSong({}, \"{path}\")", song(extra)))
        .exec()
        .expect("the song bakes");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).ok();
    bytes[44..]
        .chunks_exact(4)
        .map(|frame| i16::from_le_bytes([frame[0], frame[1]]))
        .collect()
}

#[test]
fn ring_lets_the_release_run_past_the_last_beat() {
    let frames = bake("rusty_tail_ring.wav", "").len();
    assert!(
        frames > LOOP_FRAMES + 44_100 / 2,
        "absent `tail` is `ring`: the release adds to the file, got {frames}"
    );
}

#[test]
fn exact_and_wrap_end_on_the_last_beat_to_the_sample() {
    for tail in ["exact", "wrap"] {
        let frames = bake(
            &format!("rusty_tail_{tail}.wav"),
            &format!("tail = \"{tail}\""),
        );
        assert_eq!(frames.len(), LOOP_FRAMES, "`tail = \"{tail}\"`");
    }
}

#[test]
fn wrap_carries_the_ring_out_round_so_the_loop_point_has_no_seam() {
    let exact = bake("rusty_tail_seam_exact.wav", "tail = \"exact\"");
    let wrap = bake("rusty_tail_seam_wrap.wav", "tail = \"wrap\"");
    // The ring-out was summed onto the start: the first tenth of a second is
    // louder than the same stretch of a file that faded its tail away.
    let energy = |s: &[i16]| s.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>();
    assert!(energy(&wrap[..4_410]) > energy(&exact[..4_410]) * 1.5);
    // And the jump from the last sample back to the first is no bigger than any
    // step the sine takes on its own — the loop point is just another sample.
    let step = |a: i16, b: i16| (i32::from(a) - i32::from(b)).abs();
    let biggest = wrap.windows(2).map(|w| step(w[0], w[1])).max().unwrap();
    let seam = step(wrap[LOOP_FRAMES - 1], wrap[0]);
    assert!(
        seam <= biggest,
        "the seam jumps {seam}, the music only {biggest}"
    );
}

#[test]
fn fit_makes_the_file_the_length_asked_for() {
    for mode in ["loop", "once"] {
        let frames = bake(
            &format!("rusty_fit_{mode}.wav"),
            &format!("fit = {{ seconds = 3.0, mode = \"{mode}\" }}"),
        );
        assert_eq!(frames.len(), 132_300, "`fit` mode `{mode}`: 3 s exactly");
    }
    let stretched = bake(
        "rusty_fit_stretch.wav",
        "tail = \"wrap\", fit = { seconds = 2.2, mode = \"stretch\" }",
    );
    assert_eq!(stretched.len(), 97_020, "one bar stretched onto 2.2 s");
}

#[test]
fn fade_in_starts_the_piece_from_silence() {
    let plain = bake("rusty_fade_none.wav", "tail = \"exact\"");
    let faded = bake(
        "rusty_fade_in.wav",
        "tail = \"exact\", fade = { in_seconds = 0.5, out_seconds = 0.5 }",
    );
    let peak = |s: &[i16]| s.iter().map(|x| x.unsigned_abs()).max().unwrap();
    assert!(peak(&faded[..441]) * 4 < peak(&plain[..441]), "fade in");
    let end = LOOP_FRAMES - 441;
    assert!(peak(&faded[end..]) * 4 < peak(&plain[end..]), "fade out");
}

#[test]
fn wrap_refuses_what_would_put_a_seam_back() {
    let lua = lua_with_sound();
    let path = tmp("rusty_tail_refused.wav");
    for extra in [
        "tail = \"wrap\", fade = { out_seconds = 1.0 }",
        "tail = \"wrap\", fit = { seconds = 3.0, mode = \"loop\" }",
    ] {
        let err = lua
            .load(format!("Sound.BakeSong({}, \"{path}\")", song(extra)))
            .exec()
            .expect_err(extra);
        assert!(err.to_string().contains("tail: wrap"), "{err}");
    }
    assert!(!std::path::Path::new(&path).exists());
}
