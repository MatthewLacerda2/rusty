//! Capturing the game's final mix in a scenario (#1579): the harness mixes offline
//! in step with the fixed tick, so two runs of one scenario write byte-identical
//! WAVs, and `Sound.Level` measures the capture on a grid of the scenario's choice.

use rusty::dev::scenario;

/// A decaying noise burst played at the listener, one second recorded and measured.
const SCENARIO: &str = r#"
local tone = Sound.Bake({ source = { kind = "noise" }, amp = { a = 0.001, d = 0.2, s = 0.0, r = 0.05 } },
    "A4", "DIR/tone.wav")
Harness.Step(1)
local path = Harness.RecordAudio()
Harness.Expect(not pcall(Harness.RecordAudio, "other.wav"), "one capture at a time")
Harness.Expect(Audio.PlayAt(tone, 0, 0, 0, 1, 1000, 2000), "the burst plays")
Harness.Step(60)
Harness.Expect(Harness.StopAudio() == path, "StopAudio names the file RecordAudio did")
local level = Sound.Level(path, { sections = 0.25 })
Harness.Expect(#level.sections == 4, "one second on a quarter-second grid: " .. #level.sections)
Harness.Expect(not level.sections[1].silent, "the burst is in the mix")
Harness.Expect(level.sections[4].silent, "and has died away by the end")
Harness.RecordAudio("left_running.wav")
Harness.Step(2)
"#;

/// Run [`SCENARIO`] in `name`; the bytes of its two captures.
fn run(name: &str) -> (Vec<u8>, Vec<u8>) {
    let dir = crate::temp::dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let posix = dir.to_string_lossy().replace('\\', "/");
    let path = dir.join("scenario.lua");
    std::fs::write(&path, SCENARIO.replace("DIR", &posix)).unwrap();
    let out = dir.join("out");
    let report = scenario::run_isolated(&path, &out).expect("scenario runs");
    let json = std::fs::read_to_string(report.results_path).unwrap();
    assert!(report.passed, "scenario expectations pass: {json}");
    let read = |file: &str| std::fs::read(out.join(file)).unwrap();
    (read("audio.wav"), read("left_running.wav"))
}

#[test]
fn a_scenario_capture_is_measured_and_replays_byte_identically() {
    let (first, tail) = run("rusty_audio_capture_a");
    assert_eq!(
        first.len(),
        44 + 48_000 * 8,
        "one second of 48 kHz float stereo"
    );
    let (second, _) = run("rusty_audio_capture_b");
    assert!(
        first == second,
        "two runs of one scenario capture the same bytes"
    );
    let data = u32::from_le_bytes(tail[40..44].try_into().unwrap());
    assert_eq!(
        data,
        1_600 * 8,
        "a capture left running is closed whole at the end"
    );
}
