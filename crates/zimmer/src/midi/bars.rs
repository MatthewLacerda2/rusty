//! Where the bars fall, and how many of them make one pattern.
//!
//! A song has no time signature — nothing in the renderer needs bars — but a
//! MIDI file usually has one, and it is the only thing in the file that says
//! where a phrase could begin. So the bars are counted here, from the file's
//! own meters, and used for exactly one thing: cutting the piece into
//! patterns of [`BARS_PER_PATTERN`] bars each.
//!
//! **Why eight bars, and not one pattern for the whole piece.** One pattern
//! would be the more literal reading, and it would cost the bake report its
//! rows: a song's sections *are* its arrangement entries, so a one-pattern
//! import measures as one undivided block, and "the second half is too quiet"
//! has nowhere to be said. Eight bars is the phrase length most written music
//! is built from, it keeps a three-minute piece to a dozen rows, and the names
//! (`bars-17-24`) are bar numbers a person can find on the sheet music the
//! file was transcribed from. It is a **grid**, not a reading of the music:
//! nothing here looks for where a phrase actually starts, repeats or ends,
//! and a pickup bar shifts every boundary by one, exactly as it would on the
//! page. Finding the real structure is the author's edit afterwards.
//!
//! A meter change lands on the tick the file puts it: a bar still open there
//! is cut short, the way a sequencer does it, rather than the change being
//! moved to a barline the file did not write.

/// How many bars each imported pattern holds.
pub const BARS_PER_PATTERN: usize = 8;

/// The most bars an import will count — about five and a half hours of 4/4 at
/// 120. A file claiming more is corrupt or hostile, and counting its bars is an
/// allocation nobody asked for.
const MAX_BARS: usize = 10_000;

/// A meter change: `(tick, numerator, denominator as a power of two)`.
pub(super) type Meter = (u64, u8, u8);

/// A run of bars that becomes one pattern, in ticks.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Phrase {
    /// The first bar in it, counting from one as a score does.
    pub(super) first: usize,
    /// The last bar in it.
    pub(super) last: usize,
    /// Where it starts.
    pub(super) start: f64,
    /// Where the next one starts, or where the last bar ends.
    pub(super) end: f64,
}

impl Phrase {
    /// The pattern's name: the bars it covers, as the sheet music numbers them.
    pub(super) fn name(&self) -> String {
        if self.first == self.last {
            format!("bar-{}", self.first)
        } else {
            format!("bars-{}-{}", self.first, self.last)
        }
    }
}

/// Cuts `0..end` ticks into phrases along the bars `meters` draw, or `None`
/// if that is more than [`MAX_BARS`] bars.
///
/// Before the first meter — and throughout, in a file that writes none — the
/// meter is 4/4, which is what the MIDI specification says an unmarked file
/// is in.
pub(super) fn phrases(meters: &[Meter], ppq: u16, end: u64) -> Option<Vec<Phrase>> {
    let bar = |(numerator, power): (u8, u8)| {
        f64::from(numerator) * f64::from(ppq) * 4.0 / f64::from(1_u32 << power)
    };
    let mut meters = meters.to_vec();
    meters.sort_by_key(|meter| meter.0);

    let (mut current, mut next, mut tick) = ((4, 2), 0, 0.0_f64);
    let mut starts = Vec::new();
    loop {
        // Every change at or before this barline is in force by it; of two on
        // one tick, the later-written wins.
        while let Some(&(at, numerator, power)) = meters.get(next) {
            if at as f64 > tick {
                break;
            }
            current = (numerator, power);
            next += 1;
        }
        starts.push(tick);
        if starts.len() > MAX_BARS {
            return None;
        }
        let mut after = tick + bar(current);
        if let Some(&(at, ..)) = meters.get(next) {
            after = after.min(at as f64);
        }
        tick = after;
        if tick >= end as f64 {
            break;
        }
    }

    let chunks: Vec<&[f64]> = starts.chunks(BARS_PER_PATTERN).collect();
    Some(
        chunks
            .iter()
            .enumerate()
            .map(|(index, bars)| Phrase {
                first: index * BARS_PER_PATTERN + 1,
                last: index * BARS_PER_PATTERN + bars.len(),
                start: bars[0],
                end: chunks.get(index + 1).map_or(tick, |following| following[0]),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmarked_file_is_in_four_four() {
        let cut = phrases(&[], 100, 3_300).expect("short");
        // 3300 ticks at 400 a bar is nine bars: one phrase of eight, one of one.
        assert_eq!(cut.len(), 2);
        assert_eq!((cut[0].start, cut[0].end), (0.0, 3_200.0));
        assert_eq!(cut[1].name(), "bar-9");
        assert_eq!(cut[1].end, 3_600.0, "the last bar is whole");
    }

    #[test]
    fn a_meter_change_mid_bar_cuts_that_bar_short() {
        // 3/4 from the start, then 2/4 written at tick 450 — halfway into the
        // second bar of 300.
        let cut = phrases(&[(0, 3, 2), (450, 2, 2)], 100, 1_000).expect("short");
        assert_eq!(cut[0].name(), "bars-1-5");
        assert_eq!(cut[0].end, 1_050.0, "300 + 150 + three bars of 200");
    }

    #[test]
    fn an_absurd_length_is_refused_rather_than_counted() {
        assert!(phrases(&[], 1, u64::from(u32::MAX)).is_none());
    }
}
