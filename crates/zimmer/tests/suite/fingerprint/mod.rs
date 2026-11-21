//! The sound did not move unless `SYNTH_VERSION` did.

mod probes;
mod table;

use rusty_zimmer::level::BandMeter;
use rusty_zimmer::song::InlineOnly;
use rusty_zimmer::{NoteOpts, Patch, Song, SAMPLE_RATE, SYNTH_VERSION};
use table::{Row, RECORDED_AT, TABLE};

const FRAMES: u64 = 44;
const DB: f64 = 0.05;
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
            let bake = rusty_zimmer::bake_named_note(&patch, probe.note, &opts).expect("bakes");
            measure(probe.name, &bake.wav)
        })
        .collect();
    let song = Song::from_json(probes::SONG).expect("the probe song parses");
    let bake = rusty_zimmer::bake_song(&song, &InlineOnly).expect("bakes");
    rows.push(measure("song", &bake.wav));
    rows
}

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

fn reprint(rows: &[Row]) -> String {
    let mut out = format!("pub(crate) const RECORDED_AT: u32 = {SYNTH_VERSION};\n\n");
    out.push_str("#[rustfmt::skip]\npub(crate) const TABLE: &[Row] = &[\n");
    for row in rows {
        out.push_str(&format!("    {row:?},\n"));
    }
    out.push_str("];\n");
    out
}
