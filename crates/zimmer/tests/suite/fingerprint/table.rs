//! What the probes measured, and the version they were measured at. Written by
//! the test's own failure message, never by hand: a row edited to make the
//! test pass is a missed bump.

/// One probe's bake, measured.
#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) probe: &'static str,
    /// Stereo frames in the file.
    pub(crate) frames: u64,
    /// The whole file's RMS level, in dBFS.
    pub(crate) rms_db: f64,
    /// The energy below 250 Hz, as a share of the whole.
    pub(crate) low: f64,
    /// Between 250 Hz and 4 kHz.
    pub(crate) mid: f64,
    /// Above 4 kHz.
    pub(crate) high: f64,
}

pub(crate) const RECORDED_AT: u32 = 10;

// One row per line, the form the failure message prints, so a re-record is a
// paste and a diff reads probe by probe.
#[rustfmt::skip]
pub(crate) const TABLE: &[Row] = &[
    Row { probe: "saw_filtered", frames: 17614, rms_db: -13.433, low: 0.8468, mid: 0.1525, high: 0.0007 },
    Row { probe: "noise_snare", frames: 26794, rms_db: -19.782, low: 0.002, mid: 0.7316, high: 0.2664 },
    Row { probe: "sine_kick", frames: 5970, rms_db: -10.922, low: 0.9874, mid: 0.0126, high: 0.0 },
    Row { probe: "fm2_bell", frames: 19784, rms_db: -12.024, low: 0.0036, mid: 0.9351, high: 0.0613 },
    Row { probe: "fm4_growl", frames: 30948, rms_db: -14.908, low: 0.794, mid: 0.2056, high: 0.0004 },
    Row { probe: "additive_organ", frames: 18379, rms_db: -13.712, low: 0.2844, mid: 0.7154, high: 0.0002 },
    Row { probe: "karplus_pluck", frames: 15273, rms_db: -20.325, low: 0.0131, mid: 0.7973, high: 0.1896 },
    Row { probe: "song", frames: 203902, rms_db: -17.068, low: 0.8123, mid: 0.1877, high: 0.0 },
];
