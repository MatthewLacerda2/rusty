//! The probe corpus: a handful of documents reaching every main path.
//!
//! Each is short, because the whole fingerprint renders on every test run, and
//! each leans on one part of the signal path so a change there moves its row
//! rather than averaging out across the corpus. A probe is never edited in
//! place: a new path gets a new probe, and the table is re-recorded.

/// A one-shot: the patch, the note it plays and how long it holds.
pub(crate) struct Note {
    pub(crate) name: &'static str,
    pub(crate) note: &'static str,
    pub(crate) duration: f32,
    pub(crate) patch: &'static str,
}

/// The one-shots, one per source plus the stages that sit on every source.
pub(crate) const NOTES: &[Note] = &[
    // Oscillators, unison, a curved envelope and a resonant filter on an envelope.
    Note {
        name: "saw_filtered",
        note: "A2",
        duration: 0.3,
        patch: r#"{
        "source": { "kind": "osc_stack", "oscs": [
            { "wave": "saw", "detune_cents": -7, "voices": 3, "width": 0.5 },
            { "wave": "square", "octave": -1, "gain": 0.5 } ] },
        "amp": { "a": 0.01, "d": 0.1, "s": 0.6, "r": 0.1, "curve": 2.0 },
        "filter": { "kind": "lowpass", "cutoff": 800, "resonance": 0.5,
            "env_octaves": 2.5, "adsr": { "a": 0.0, "d": 0.15, "s": 0.2, "r": 0.1 } } }"#,
    },
    // Noise, a highpass, saturation and a reverb tail.
    Note {
        name: "noise_snare",
        note: "C4",
        duration: 0.1,
        patch: r#"{
        "source": { "kind": "noise", "color": "pink" },
        "amp": { "a": 0.001, "d": 0.12, "s": 0.0, "r": 0.05 },
        "filter": { "kind": "highpass", "cutoff": 1200 },
        "fx": [ { "fx": "saturate", "drive": 2.0, "mix": 0.6 },
                { "fx": "reverb", "size": 0.3, "damp": 0.5, "mix": 0.2 } ] }"#,
    },
    // A sine with a pitch envelope: the kick, and the pitch path.
    Note {
        name: "sine_kick",
        note: "A1",
        duration: 0.12,
        patch: r#"{
        "source": { "kind": "osc_stack", "oscs": [{ "wave": "sine" }] },
        "amp": { "a": 0.001, "d": 0.22, "s": 0.0, "r": 0.02, "curve": 4.0 },
        "pitch_env": { "semitones": 10,
            "adsr": { "a": 0.0, "d": 0.04, "s": 0.0, "r": 0.0, "curve": 5.0 } } }"#,
    },
    // Two operators, and a cutoff LFO under a bandpass.
    Note {
        name: "fm2_bell",
        note: "E4",
        duration: 0.25,
        patch: r#"{
        "source": { "kind": "fm2", "ratio": 3.5, "index": 2.0, "vel_index": 3.0,
            "mod_decay": 0.2 },
        "amp": { "a": 0.002, "d": 0.4, "s": 0.0, "r": 0.2 },
        "filter": { "kind": "bandpass", "cutoff": 2000, "resonance": 0.3 },
        "lfo": { "rate": 6.0, "depth": 1.0, "target": "cutoff" } }"#,
    },
    // Four operators with feedback, a ping-pong delay.
    Note {
        name: "fm4_growl",
        note: "F2",
        duration: 0.25,
        patch: r#"{
        "source": { "kind": "fm4", "algorithm": "twin", "operators": [
            { "ratio": 1.0, "level": 4.0, "feedback": 0.4,
              "env": { "a": 0.02, "d": 0.2, "s": 0.4, "r": 0.1 } },
            { "ratio": 1.0, "level": 1.0 },
            { "ratio": 2.0, "level": 2.0 },
            { "ratio": 1.0, "level": 0.6 } ] },
        "amp": { "a": 0.01, "d": 0.1, "s": 0.8, "r": 0.1 },
        "fx": [ { "fx": "delay", "time": 0.08, "feedback": 0.3, "mix": 0.3,
            "ping_pong": true } ] }"#,
    },
    // Partials, a pitch LFO, a chorus and an EQ.
    Note {
        name: "additive_organ",
        note: "C4",
        duration: 0.3,
        patch: r#"{
        "source": { "kind": "additive", "partials": [
            { "ratio": 1, "gain": 1.0 }, { "ratio": 2, "gain": 0.5, "decay": 0.3 },
            { "ratio": 3, "gain": 0.3, "detune_cents": 4 } ] },
        "amp": { "a": 0.02, "d": 0.0, "s": 1.0, "r": 0.1 },
        "lfo": { "rate": 5.0, "depth": 0.2, "target": "pitch" },
        "fx": [ { "fx": "chorus", "rate": 0.6, "depth": 0.5, "voices": 3, "mix": 0.4 },
                { "fx": "eq", "bands": [ { "kind": "highpass" },
                    { "kind": "peak", "freq": 800, "gain_db": 4, "q": 1.0 } ] } ] }"#,
    },
    // A plucked string, and a tremolo.
    Note {
        name: "karplus_pluck",
        note: "G3",
        duration: 0.2,
        patch: r#"{
        "source": { "kind": "karplus", "damping": 0.995, "brightness": 0.5 },
        "amp": { "a": 0.001, "d": 0.3, "s": 0.0, "r": 0.15 },
        "lfo": { "rate": 8.0, "depth": 0.4, "target": "amp" } }"#,
    },
];

/// The song sequencer: chords and plain notes, swing, pan, a sidechain
/// compressor, a track delay and the master chain, then the master limiter.
pub(crate) const SONG: &str = r#"{
    "bpm": 140, "swing": 0.2,
    "tracks": [
        { "name": "kick", "patch": {
            "source": { "kind": "osc_stack", "oscs": [{ "wave": "sine" }] },
            "amp": { "a": 0.001, "d": 0.15, "s": 0.0, "r": 0.02 },
            "pitch_env": { "semitones": 8, "adsr": { "a": 0.0, "d": 0.03, "s": 0.0, "r": 0.0 } } } },
        { "name": "keys", "gain": 0.6, "pan": -0.3, "patch": {
            "source": { "kind": "osc_stack", "oscs": [{ "wave": "triangle" }] },
            "amp": { "a": 0.01, "d": 0.2, "s": 0.5, "r": 0.2 } },
          "fx": [ { "fx": "compress", "threshold": -24, "ratio": 6,
              "attack": 0.005, "release": 0.1, "sidechain": "kick" },
              { "fx": "delay", "time": 0.2, "feedback": 0.2, "mix": 0.2 } ] }
    ],
    "patterns": { "a": { "beats": 4, "notes": [
        { "track": "kick", "note": "A1", "start": 0, "dur": 0.5 },
        { "track": "kick", "note": "A1", "start": 2, "dur": 0.5 },
        { "track": "keys", "chord": "Dm7", "oct": 3, "start": 0, "dur": 1.5, "vel": 0.8 },
        { "track": "keys", "note": "A4", "start": 1.5, "dur": 0.5 },
        { "track": "keys", "chord": "G7", "oct": 3, "start": 2, "dur": 2, "vel": 0.7 }
    ] } },
    "arrangement": ["a"],
    "fx": [ { "fx": "eq", "bands": [ { "kind": "lowshelf", "gain_db": 2 } ] },
            { "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.15 } ]
}"#;
