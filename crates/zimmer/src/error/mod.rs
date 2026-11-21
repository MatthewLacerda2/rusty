//! Why a recipe could not be rendered.

mod place;
mod wording;

pub use place::NoteAt;

#[derive(Debug, Clone, PartialEq)]
pub enum SynthError {
    EmptyOscStack,

    TooManyOscs {
        found: usize,
        limit: usize,
    },

    SilentOscStack,

    BadVoiceCount {
        found: usize,
        limit: usize,
    },

    BadFmRatio {
        ratio: f32,
    },

    EmptyPartials,

    TooManyPartials {
        found: usize,
        limit: usize,
    },

    SilentPartials,

    BadPartialRatio {
        index: usize,
        ratio: f32,
    },

    BadOperatorRatio {
        operator: usize,
        ratio: f32,
    },

    SilentCarriers {
        algorithm: &'static str,
    },

    BadCutoff {
        cutoff: f32,
    },

    TooManyEqBands {
        found: usize,
        limit: usize,
    },

    UnknownSidechain {
        track: String,
        key: String,
    },

    SelfSidechain {
        track: String,
    },

    MisplacedSidechain {
        place: &'static str,
        key: String,
    },

    MisplacedBeats {
        place: &'static str,
        beats: f32,
    },

    DelayTwoTimes {
        time: f32,
        beats: f32,
    },

    NegativeLfoRate {
        rate: f32,
    },

    LfoTwoRates {
        rate: f32,
        beats: f32,
    },

    LfoNoRate,

    BadLfoBeats {
        beats: f32,
    },

    LfoBeatsWithoutTempo {
        beats: f32,
    },

    BadLfoOnset {
        field: &'static str,
        seconds: f32,
    },

    PulseWidthOffSquare {
        wave: crate::patch::Wave,
    },

    NoPulseToModulate,

    TooManyLayers {
        found: usize,
        limit: usize,
    },

    SilentLayer,

    InLayer {
        layer: usize,
        error: Box<SynthError>,
    },

    // ──────────────────────────────────────────────────────────────────── A note as it is written:
    // how long it lasts, and what it is called.
    // ──────────────────────────────────────────────────────────────────── A note with no length
    // renders no samples.
    BadDuration {
        duration: f32,
    },

    NoteTooLong {
        at: Option<NoteAt>,
        held: f32,
        tail: f32,
        limit: f32,
    },

    EmptyNoteName,

    BadNoteLetter {
        name: String,
        letter: char,
    },

    BadOctave {
        name: String,
        octave: String,
    },

    NoteOutOfRange {
        name: String,
        midi: i32,
    },

    BadBpm {
        bpm: f32,
    },

    NoTracks,

    EmptyArrangement,

    NoLayers,

    UnknownPattern {
        pattern: String,
    },

    UnknownTrackFilter {
        pattern: String,
        track: String,
    },

    BadTranspose {
        pattern: String,
        transpose: f32,
    },

    BadVelocityScale {
        pattern: String,
        scale: f32,
    },

    BadPatternBeats {
        pattern: String,
        beats: f32,
    },

    UnknownSoloTrack {
        track: String,
    },

    UnknownTrack {
        pattern: String,
        index: usize,
        track: String,
    },

    BadNoteStart {
        pattern: String,
        index: usize,
        start: f32,
    },

    BadNoteDuration {
        pattern: String,
        index: usize,
        dur: f32,
    },

    UnknownChord {
        chord: String,
    },

    ChordOutOfRange {
        chord: String,
        oct: i32,
        midi: i32,
    },

    SpelledChordOctave {
        track: String,
        start: f32,
        oct: i32,
    },

    EmptyChord {
        track: String,
        start: f32,
    },

    BadArp {
        track: String,
        start: f32,
        why: &'static str,
    },

    BadVoicing {
        track: String,
        start: f32,
        why: &'static str,
    },

    BadStep {
        track: String,
        character: char,
        step: usize,
    },

    StepsDoNotFit {
        track: String,
        start: f32,
        div: f32,
        beats: f32,
        written: usize,
        needed: usize,
    },

    BadStepDiv {
        track: String,
        start: f32,
        div: f32,
        beats: f32,
    },

    AccentWithoutHeadroom {
        track: String,
        vel: f32,
    },

    TwiceAccented {
        track: String,
    },

    TwiceGhosted {
        track: String,
    },

    BadKey {
        key: String,
    },

    BadDegree {
        degree: String,
    },

    DegreeWithoutKey {
        track: String,
        start: f32,
    },

    DegreeOutOfRange {
        degree: String,
        oct: i32,
        midi: i32,
    },

    DiatonicWithoutKey {
        pattern: String,
    },

    TwoTransposes {
        pattern: String,
    },

    BadSwing {
        swing: f32,
    },

    BadHumanize {
        field: &'static str,
        amount: f32,
    },

    BadTrackFeel {
        track: String,
        field: &'static str,
        amount: f32,
    },

    BadTempoChange {
        index: usize,
        beat: f32,
        bpm: f32,
    },

    TempoOutOfOrder {
        index: usize,
        beat: f32,
        previous: f32,
    },

    UnknownAutomationTrack {
        track: String,
        param: &'static str,
    },

    DuplicateAutomation {
        track: String,
        param: &'static str,
    },

    BadAutomationCurve {
        track: String,
        param: &'static str,
        why: &'static str,
    },

    BadAutomationPoint {
        track: String,
        param: &'static str,
        field: &'static str,
        value: f32,
    },

    BadAutomationCutoff {
        track: String,
        cutoff: f32,
    },

    AutomationWithoutFilter {
        track: String,
    },

    AutomationWithoutRoom {
        track: String,
    },

    BadFitSeconds {
        seconds: f32,
    },

    FitLength {
        why: &'static str,
    },

    BadAnchor {
        section: usize,
        why: String,
    },

    BadFade {
        seconds: f32,
    },

    StretchTooFar {
        bpm: f32,
        needed: f32,
        limit: f32,
    },

    WrapWith {
        field: &'static str,
        why: &'static str,
    },

    WrapOverhang {
        overhang: f32,
        length: f32,
    },

    SongTooLong {
        seconds: f32,
        limit: f32,
    },

    UnresolvedPatch {
        track: String,
        reference: String,
        reason: String,
    },
}
