//! The sound did not move unless `SYNTH_VERSION` did (#1216).
//!
//! A change to what a recipe renders to needs a [`SYNTH_VERSION`] bump, or
//! every project that baked the recipe keeps audio it no longer describes,
//! silently — and rusty with them. Determinism within one build
//! (`promises.rs`) cannot see that; this can. A small probe corpus
//! (`probes.rs`) is baked, and each bake measured: its length, its level, and
//! where its energy sits. The measures are recorded in `table.rs` beside the
//! version they were taken at.
//!
//! - **Measures moved, version did not**: the sound changed, so bump the
//!   version (its table, and `SYNTH_VERSION.md`) and re-record.
//! - **Version moved**: re-recording is expected, and asked for.
//!
//! **Measures, with tolerance, never a digest.** The constant's own doc
//! explains why a digest cannot work: `sin`, `exp` and `powf` are not
//! bit-identical between platforms. A length, a level and three band shares
//! carry a tolerance a platform's last bit cannot reach and a sound change
//! does.
//!
//! **Every platform runs it**, unlike the golden renders: those compare
//! frames an encoder and a font rasteriser shaped, these compare sums over a
//! whole 16-bit file, where libm's last-bit drift averages away. CI's x86_64
//! Linux records the table and is the authority if a platform ever disagrees;
//! widening a tolerance for one is where that would be said.
//!
//! **What it is sized to catch**: lowering the limiter's ceiling by a tenth of
//! a decibel moves two rows, and a pluck's delay line one sample shorter moves
//! its row. What it cannot see is a change no measure here reaches — a pitch
//! moved by well under a percent, a stereo image swapped — so a branch that
//! touches rendering maths still checks its own diff (`SYNTH_VERSION.md`).

mod probes;
mod table;

use scorsese_zimmer::level::BandMeter;
use scorsese_zimmer::song::InlineOnly;
use scorsese_zimmer::{NoteOpts, Patch, SAMPLE_RATE, SYNTH_VERSION, Song};
use table::{RECORDED_AT, Row, TABLE};

/// One millisecond: where a file's tail is cut can shift by a sample or two.
const FRAMES: u64 = 44;
/// A twentieth of a decibel on the whole file's RMS level.
const DB: f64 = 0.05;
/// Two thousandths of the energy, in each band's share.
const SHARE: f64 = 0.002;

#[test]
fn the_sound_moves_only_with_the_version() {
    let now = measure_all();
    let reprint = reprint(&now);
    assert_eq!(
        RECORDED_AT, SYNTH_VERSION,
        "SYNTH_VERSION is {SYNTH_VERSION} and the fingerprint was recorded at \
         {RECORDED_AT}: re-record it — replace crates/zimmer/tests/suite/fingerprint/table.rs's \
         body with:\n\n{reprint}"
    );
    let names: Vec<_> = now.iter().map(|row| row.probe).collect();
    let recorded: Vec<_> = TABLE.iter().map(|row| row.probe).collect();
    assert_eq!(
        names, recorded,
        "the probes and the table disagree; re-record:\n\n{reprint}"
    );
    let moved: Vec<String> = now
        .iter()
        .zip(TABLE)
        .filter(|(now, then)| !close(now, then))
        .map(|(now, then)| format!("  was {then:?}\n  now {now:?}"))
        .collect();
    assert!(
        moved.is_empty(),
        "the sound changed under SYNTH_VERSION {SYNTH_VERSION}:\n{}\n\nBump \
         SYNTH_VERSION in crates/zimmer/src/lib.rs (its table, and an entry in \
         crates/zimmer/SYNTH_VERSION.md), then re-record table.rs as:\n\n{reprint}",
        moved.join("\n")
    );
}

fn close(now: &Row, then: &Row) -> bool {
    now.frames.abs_diff(then.frames) <= FRAMES
        && (now.rms_db - then.rms_db).abs() <= DB
        && [
            (now.low, then.low),
            (now.mid, then.mid),
            (now.high, then.high),
        ]
        .iter()
        .all(|(a, b)| (a - b).abs() <= SHARE)
}

fn measure_all() -> Vec<Row> {
    let mut rows: Vec<Row> = probes::NOTES
        .iter()
        .map(|probe| {
            let patch = Patch::from_json(probe.patch).expect("a probe patch parses");
            let opts = NoteOpts {
                duration: probe.duration,
                velocity: 0.9,
                timbre: 0.0,
                glide: None,
                seed: 7,
            };
            let bake = scorsese_zimmer::bake_named_note(&patch, probe.note, &opts).expect("bakes");
            measure(probe.name, &bake.wav)
        })
        .collect();
    let song = Song::from_json(probes::SONG).expect("the probe song parses");
    let bake = scorsese_zimmer::bake_song(&song, &InlineOnly).expect("bakes");
    rows.push(measure("song", &bake.wav));
    rows
}

/// What a file is, read back from the bytes a project would store.
fn measure(probe: &'static str, wav: &[u8]) -> Row {
    let at = wav
        .windows(4)
        .position(|w| w == b"data")
        .expect("a data chunk")
        + 8;
    let samples: Vec<f32> = wav[at..]
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect();
    let energy: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    let mut bands = BandMeter::new(2, SAMPLE_RATE);
    bands.feed(&samples);
    let bands = bands.finish().expect("a probe makes a sound");
    let round = |x: f64, places: i32| (x * 10f64.powi(places)).round() / 10f64.powi(places);
    Row {
        probe,
        frames: samples.len() as u64 / 2,
        rms_db: round(10.0 * (energy / samples.len() as f64).log10(), 3),
        low: round(bands.low, 4),
        mid: round(bands.mid, 4),
        high: round(bands.high, 4),
    }
}

/// `table.rs`'s body as it would be recorded now.
fn reprint(rows: &[Row]) -> String {
    let mut out = format!("pub(crate) const RECORDED_AT: u32 = {SYNTH_VERSION};\n\n");
    out.push_str("#[rustfmt::skip]\npub(crate) const TABLE: &[Row] = &[\n");
    for row in rows {
        out.push_str(&format!("    {row:?},\n"));
    }
    out.push_str("];\n");
    out
}
