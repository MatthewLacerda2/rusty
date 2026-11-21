//! Standard MIDI File bytes, read back by hand for the tests.
//!
//! The exporter encodes with `midly`, so reading its output with `midly` too
//! — which is what `import` does — would let the two agree about a mistake.
//! This reads just enough of the format, straight from the specification, to
//! say what an exported file holds: chunks, delta times, running status, and
//! the events the exporter writes.

/// One event, as the tests care about it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Name(String),
    Tempo(u32),
    Key(i8, bool),
    On { channel: u8, key: u8, vel: u8 },
    Off { channel: u8, key: u8 },
    End,
}

/// A whole file: its format, its ticks per beat, and each track's events at
/// absolute ticks.
pub(crate) struct File {
    pub(crate) format: u16,
    pub(crate) ppq: u16,
    pub(crate) tracks: Vec<Vec<(u64, Event)>>,
}

pub(crate) fn read(bytes: &[u8]) -> File {
    assert_eq!(
        &bytes[..8],
        b"MThd\0\0\0\x06",
        "a header chunk of six bytes"
    );
    let word = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
    let (format, count, ppq) = (word(8), word(10), word(12));
    let mut at = 14;
    let mut tracks = Vec::new();
    for _ in 0..count {
        assert_eq!(&bytes[at..at + 4], b"MTrk", "a track chunk at {at}");
        let len = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().expect("four")) as usize;
        tracks.push(events(&bytes[at + 8..at + 8 + len]));
        at += 8 + len;
    }
    assert_eq!(at, bytes.len(), "nothing after the last chunk");
    File {
        format,
        ppq,
        tracks,
    }
}

fn events(mut chunk: &[u8]) -> Vec<(u64, Event)> {
    let (mut tick, mut status, mut out) = (0, 0_u8, Vec::new());
    while !chunk.is_empty() {
        tick += vlq(&mut chunk);
        if chunk[0] & 0x80 != 0 {
            status = chunk[0];
            chunk = &chunk[1..];
        }
        let event = if status == 0xFF {
            status = 0;
            let kind = chunk[0];
            chunk = &chunk[1..];
            let len = vlq(&mut chunk) as usize;
            let data = &chunk[..len];
            chunk = &chunk[len..];
            match kind {
                0x03 => Event::Name(String::from_utf8(data.to_vec()).expect("utf-8")),
                0x51 => Event::Tempo(u32::from_be_bytes([0, data[0], data[1], data[2]])),
                0x59 => Event::Key(data[0] as i8, data[1] == 1),
                0x2F => Event::End,
                other => panic!("the exporter wrote meta event {other:#x}"),
            }
        } else {
            let (channel, key, vel) = (status & 0x0F, chunk[0], chunk[1]);
            chunk = &chunk[2..];
            match status & 0xF0 {
                0x90 if vel > 0 => Event::On { channel, key, vel },
                0x80 | 0x90 => Event::Off { channel, key },
                other => panic!("the exporter wrote status {other:#x}"),
            }
        };
        out.push((tick, event));
    }
    out
}

fn vlq(chunk: &mut &[u8]) -> u64 {
    let mut value = 0;
    loop {
        let byte = chunk[0];
        *chunk = &chunk[1..];
        value = (value << 7) | u64::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return value;
        }
    }
}

/// Every note-on in `track`, as `(tick, channel, key, vel)`.
pub(crate) fn strikes(track: &[(u64, Event)]) -> Vec<(u64, u8, u8, u8)> {
    track
        .iter()
        .filter_map(|(tick, event)| match *event {
            Event::On { channel, key, vel } => Some((*tick, channel, key, vel)),
            _ => None,
        })
        .collect()
}
