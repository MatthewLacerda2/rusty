//! Where the energy sits: low, mid and high, as shares of the whole.

/// The foot of the 250-500 Hz mud region, so a swollen `low` share is what an author calls muddy.
pub(crate) const LOW_HZ: f32 = 250.0;

/// Above it a mix reads bright, harsh or thin; below it sit nearly all fundamentals.
pub(crate) const HIGH_HZ: f32 = 4_000.0;

/// One-poles per edge, 12 dB per octave: one alone counts a quarter of a 440 Hz tone's energy
/// as bass.
const POLES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bands {
    pub low: f64,
    pub mid: f64,
    pub high: f64,
}

impl Bands {
    pub fn percentages(&self) -> (u32, u32, u32) {
        let percent = |share: f64| (share * 100.0).round().clamp(0.0, 100.0) as i32;
        let mut shares = [percent(self.low), percent(self.mid), percent(self.high)];
        let biggest = shares
            .iter()
            .enumerate()
            .max_by_key(|(_, share)| **share)
            .map_or(0, |(at, _)| at);
        shares[biggest] += 100 - shares.iter().sum::<i32>();
        let read = |share: i32| share.clamp(0, 100) as u32;
        (read(shares[0]), read(shares[1]), read(shares[2]))
    }
}

#[derive(Debug, Clone)]
pub struct BandMeter {
    channels: usize,
    banks: Vec<Bank>,
    // Kept across runs, so a run ending mid-frame cannot rotate the channels.
    next: usize,
    low: f64,
    mid: f64,
    high: f64,
}

impl BandMeter {
    pub fn new(channels: usize, rate: u32) -> Self {
        let channels = channels.max(1);
        Self {
            channels,
            banks: vec![Bank::new(rate); channels],
            next: 0,
            low: 0.0,
            mid: 0.0,
            high: 0.0,
        }
    }

    pub fn feed(&mut self, samples: &[f32]) {
        for &sample in samples {
            let (low, mid, high) = self.banks[self.next].split(sample);
            self.low += f64::from(low) * f64::from(low);
            self.mid += f64::from(mid) * f64::from(mid);
            self.high += f64::from(high) * f64::from(high);
            self.next = (self.next + 1) % self.channels;
        }
    }

    pub fn finish(&self) -> Option<Bands> {
        let total = self.low + self.mid + self.high;
        if total <= 0.0 {
            return None;
        }
        Some(Bands {
            low: self.low / total,
            mid: self.mid / total,
            high: self.high / total,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Bank {
    low_alpha: f32,
    high_alpha: f32,
    low: [Pole; POLES],
    mid_above: [Pole; POLES],
    mid_below: [Pole; POLES],
    high: [Pole; POLES],
}

impl Bank {
    fn new(rate: u32) -> Self {
        Self {
            low_alpha: alpha(LOW_HZ, rate),
            high_alpha: alpha(HIGH_HZ, rate),
            low: [Pole::default(); POLES],
            mid_above: [Pole::default(); POLES],
            mid_below: [Pole::default(); POLES],
            high: [Pole::default(); POLES],
        }
    }

    fn split(&mut self, sample: f32) -> (f32, f32, f32) {
        let sample = if sample.is_finite() { sample } else { 0.0 };
        let (low_alpha, high_alpha) = (self.low_alpha, self.high_alpha);

        let mut low = sample;
        for pole in &mut self.low {
            low = pole.pass(low, low_alpha);
        }
        let mut mid = sample;
        for pole in &mut self.mid_above {
            mid -= pole.pass(mid, low_alpha);
        }
        for pole in &mut self.mid_below {
            mid = pole.pass(mid, high_alpha);
        }
        let mut high = sample;
        for pole in &mut self.high {
            high -= pole.pass(high, high_alpha);
        }
        (low, mid, high)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Pole {
    state: f32,
}

impl Pole {
    fn pass(&mut self, sample: f32, alpha: f32) -> f32 {
        self.state += alpha * (sample - self.state);
        self.state
    }
}

fn alpha(cutoff: f32, rate: u32) -> f32 {
    if rate == 0 {
        return 1.0;
    }
    let exponent = -std::f32::consts::TAU * cutoff / rate as f32;
    (1.0 - exponent.exp()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_always_sum_to_a_hundred() {
        for (low, mid, high) in [
            (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
            (0.425, 0.505, 0.07),
            (1.0, 0.0, 0.0),
            (0.005, 0.005, 0.99),
        ] {
            let (low, mid, high) = Bands { low, mid, high }.percentages();
            assert_eq!(low + mid + high, 100, "{low} + {mid} + {high}");
        }
    }

    #[test]
    fn a_poisoned_signal_still_produces_shares() {
        let mut meter = BandMeter::new(1, 48_000);
        meter.feed(&[0.5, f32::NAN, -0.5, f32::INFINITY, 0.5, 0.25]);
        let bands = meter.finish().expect("there was signal either side of it");
        for share in [bands.low, bands.mid, bands.high] {
            assert!((0.0..=1.0).contains(&share), "{share}");
        }
    }
}
