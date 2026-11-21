//! Standard MIDI File bytes, written by hand for the tests.
//!
//! Just enough of the format to say what a test means: a header chunk, and
//! track chunks of delta-timed events. Events are added in tick order.

/// One track chunk being written.
#[derive(Default)]
pub(crate) struct Track {
    bytes: Vec<u8>,
    now: u64,
}

impl Track {
    /// Advances to `tick` and writes the delta that gets there.
    fn at(&mut self, tick: u64) -> &mut Self {
        let delta = tick - self.now;
        self.now = tick;
        self.bytes.extend(vlq(delta));
        self
    }

    fn raw(&mut self, tick: u64, bytes: &[u8]) -> &mut Self {
        self.at(tick).bytes.extend_from_slice(bytes);
        self
    }

    pub(crate) fn on(&mut self, tick: u64, channel: u8, key: u8, vel: u8) -> &mut Self {
        self.raw(tick, &[0x90 | channel, key, vel])
    }

    pub(crate) fn off(&mut self, tick: u64, channel: u8, key: u8) -> &mut Self {
        self.raw(tick, &[0x80 | channel, key, 0])
    }

    /// A note from `start` for `len` ticks — for tests where nothing else
    /// happens between the two.
    pub(crate) fn note(&mut self, start: u64, len: u64, channel: u8, key: u8) -> &mut Self {
        self.on(start, channel, key, 100)
            .off(start + len, channel, key)
    }

    pub(crate) fn program(&mut self, tick: u64, channel: u8, program: u8) -> &mut Self {
        self.raw(tick, &[0xC0 | channel, program])
    }

    pub(crate) fn pedal(&mut self, tick: u64, channel: u8, value: u8) -> &mut Self {
        self.raw(tick, &[0xB0 | channel, 64, value])
    }

    fn meta(&mut self, tick: u64, kind: u8, data: &[u8]) -> &mut Self {
        self.raw(tick, &[0xFF, kind]);
        self.bytes.extend(vlq(data.len() as u64));
        self.bytes.extend_from_slice(data);
        self
    }

    pub(crate) fn name(&mut self, text: &str) -> &mut Self {
        let now = self.now;
        self.meta(now, 0x03, text.as_bytes())
    }

    pub(crate) fn tempo(&mut self, tick: u64, bpm: u32) -> &mut Self {
        let micros = 60_000_000 / bpm;
        self.meta(tick, 0x51, &micros.to_be_bytes()[1..])
    }

    pub(crate) fn meter(&mut self, tick: u64, numerator: u8, power: u8) -> &mut Self {
        self.meta(tick, 0x58, &[numerator, power, 24, 8])
    }

    pub(crate) fn key(&mut self, tick: u64, sharps: i8, minor: bool) -> &mut Self {
        self.meta(tick, 0x59, &[sharps as u8, u8::from(minor)])
    }
}

/// A whole file: `format`, `ppq` ticks per beat, and the tracks, each closed
/// with an end-of-track event where its last event was.
pub(crate) fn smf(format: u16, ppq: u16, tracks: &mut [Track]) -> Vec<u8> {
    let mut out = b"MThd".to_vec();
    out.extend(6_u32.to_be_bytes());
    out.extend(format.to_be_bytes());
    out.extend((tracks.len() as u16).to_be_bytes());
    out.extend(ppq.to_be_bytes());
    for track in tracks {
        let now = track.now;
        track.meta(now, 0x2F, &[]);
        out.extend(b"MTrk");
        out.extend((track.bytes.len() as u32).to_be_bytes());
        out.extend(&track.bytes);
    }
    out
}

/// A variable-length quantity: seven bits a byte, high bit set on all but the
/// last.
fn vlq(mut value: u64) -> Vec<u8> {
    let mut out = vec![(value & 0x7F) as u8];
    value >>= 7;
    while value > 0 {
        out.insert(0, 0x80 | (value & 0x7F) as u8);
        value >>= 7;
    }
    out
}
