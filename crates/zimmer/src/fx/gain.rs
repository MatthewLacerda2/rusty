//! gain — how a gain track moves: the ballistics [`super::compress`] and [`super::limiter`] share.

/// Seconds: half a cycle of 25 Hz, so a kick's ripple is held and read as a level.
pub(crate) const HOLD: f32 = 0.02;

/// Deeper is an upstream bug; the cap keeps `reach_frames` finite.
const FLOOR_DB: f32 = -120.0;

/// A release this close to its target snaps to it, so unity comes back bit-identical.
const SETTLE_DB: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ballistics {
    hold: usize,
    attack: usize,
    release: f32,
    release_frames: f32,
}

impl Ballistics {
    pub(crate) fn new(attack: f32, release: f32, rate: f32) -> Self {
        let frames = |seconds: f32| (seconds * rate).round().max(1.0);
        let release_frames = frames(release);
        Self {
            hold: frames(HOLD) as usize,
            attack: frames(attack) as usize,
            release: (-1.0 / release_frames).exp(),
            release_frames,
        }
    }

    /// Frames, either way, one target can move another frame's gain: what a looped render wraps.
    pub(crate) fn reach_frames(&self) -> usize {
        let settle = self.release_frames * (-FLOOR_DB / SETTLE_DB).ln();
        self.hold + 2 * self.attack + settle.ceil() as usize
    }

    /// Linear gains in `0..=1`; each pass only lowers them, so no frame ends above its target.
    pub(crate) fn smooth(&self, target: &[f32]) -> Vec<f32> {
        let depth: Vec<f32> = target.iter().map(|gain| decibels(*gain)).collect();
        let held = deepest(&depth, self.hold, Side::Behind);
        let ahead = deepest(&held, self.attack, Side::Ahead);
        let ramped = averaged(&ahead, self.attack);
        released(&ramped, self.release)
            .into_iter()
            .map(|db| {
                if db == 0.0 {
                    1.0
                } else {
                    10f32.powf(db / 20.0)
                }
            })
            .collect()
    }
}

fn decibels(gain: f32) -> f32 {
    if gain >= 1.0 || gain.is_nan() {
        0.0
    } else if gain <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * gain.log10()).max(FLOOR_DB)
    }
}

#[derive(Clone, Copy)]
enum Side {
    Behind,
    Ahead,
}

fn deepest(values: &[f32], width: usize, side: Side) -> Vec<f32> {
    let n = values.len();
    let at = |k: usize| match side {
        Side::Behind => k,
        Side::Ahead => n - 1 - k,
    };
    let mut out = vec![0.0; n];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..n {
        while queue
            .back()
            .is_some_and(|&b| values[at(b)] >= values[at(k)])
        {
            queue.pop_back();
        }
        queue.push_back(k);
        while queue.front().is_some_and(|&f| f + width <= k) {
            queue.pop_front();
        }
        out[at(k)] = values[at(queue[0])];
    }
    out
}

fn averaged(values: &[f32], width: usize) -> Vec<f32> {
    let mut sum = 0.0f64;
    let mut ducked = 0usize;
    let mut out = Vec::with_capacity(values.len());
    for (k, value) in values.iter().enumerate() {
        sum += f64::from(*value);
        ducked += usize::from(*value != 0.0);
        if k >= width {
            let gone = values[k - width];
            sum -= f64::from(gone);
            ducked -= usize::from(gone != 0.0);
        }
        let count = (k + 1).min(width) as f64;
        out.push(if ducked == 0 {
            0.0
        } else {
            ((sum / count) as f32).min(0.0)
        });
    }
    out
}

fn released(values: &[f32], coefficient: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(values.len());
    let mut previous = 0.0f32;
    for target in values {
        let next = if previous >= *target {
            *target
        } else {
            let eased = target + (previous - target) * coefficient;
            if target - eased < SETTLE_DB {
                *target
            } else {
                eased
            }
        };
        out.push(next);
        previous = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    fn step_at(frame: usize, frames: usize, depth: f32) -> Vec<f32> {
        (0..frames)
            .map(|k| if k >= frame { depth } else { 1.0 })
            .collect()
    }

    #[test]
    fn unity_everywhere_is_handed_back_exactly() {
        let smoothed = Ballistics::new(0.005, 0.1, RATE).smooth(&[1.0; 4410]);
        assert!(smoothed.iter().all(|g| *g == 1.0));
    }

    #[test]
    fn no_frame_ever_rises_above_what_it_asked_for() {
        let target: Vec<f32> = (0..20_000)
            .map(|k| 1.0 - 0.9 * ((k as f32 * 0.013).sin() * (k as f32 * 0.0007).cos()).abs())
            .collect();
        let smoothed = Ballistics::new(0.003, 0.05, RATE).smooth(&target);
        for (k, (got, asked)) in smoothed.iter().zip(&target).enumerate() {
            assert!(
                *got <= asked * (1.0 + 1e-5),
                "frame {k}: {got} over {asked}"
            );
        }
    }

    #[test]
    fn a_shallow_duck_and_a_deep_one_recover_over_the_same_time() {
        let ballistics = Ballistics::new(0.001, 0.2, RATE);
        let recovered = |db: f32| {
            let mut target = vec![1.0; 44_100];
            for slot in &mut target[1000..2000] {
                *slot = 10f32.powf(db / 20.0);
            }
            let gain = ballistics.smooth(&target);
            let after = 2000 + ballistics.hold + (0.2 * RATE) as usize;
            20.0 * gain[after].log10() / db
        };
        let (shallow, deep) = (recovered(-2.0), recovered(-12.0));
        assert!((shallow - (-1.0f32).exp()).abs() < 0.02, "{shallow}");
        assert!((deep - shallow).abs() < 0.01, "{deep} against {shallow}");
    }

    #[test]
    fn the_attack_reaches_full_depth_at_the_peak_and_starts_an_attack_before_it() {
        let ballistics = Ballistics::new(0.005, 0.05, RATE);
        let gain = ballistics.smooth(&step_at(1000, 2000, 0.25));
        let attack = ballistics.attack;
        assert_eq!(gain[1000 - attack], 1.0, "an attack ahead, nothing yet");
        assert!(gain[1001 - attack] < 1.0, "and then it moves");
        assert!(
            (gain[1000] - 0.25).abs() < 1e-6,
            "all the way down: {}",
            gain[1000]
        );
    }

    #[test]
    fn a_ripple_faster_than_the_hold_is_read_as_a_level() {
        let ballistics = Ballistics::new(0.002, 0.05, RATE);
        let rippled: Vec<f32> = (0..44_100)
            .map(|k| if (k / 220) % 2 == 0 { 0.5 } else { 1.0 })
            .collect();
        let gain = ballistics.smooth(&rippled);
        let settled = &gain[2000..40_000];
        assert!(
            settled.iter().all(|g| (g - 0.5).abs() < 1e-6),
            "the gain chased the ripple"
        );
    }

    #[test]
    fn a_frame_reaches_no_further_than_it_says() {
        let ballistics = Ballistics::new(0.005, 0.06, RATE);
        let mut target = vec![1.0; 60_000];
        target[0] = 0.0;
        let gain = ballistics.smooth(&target);
        let reach = ballistics.reach_frames();
        assert!(
            gain[reach..].iter().all(|g| *g == 1.0),
            "still moving past {reach}"
        );
    }

    #[test]
    fn an_uneven_duck_leaves_nothing_behind_once_it_has_passed() {
        let mut depths: Vec<f32> = (0..400)
            .map(|k| {
                if k % 2 == 0 {
                    -119.9
                } else {
                    -1.3e-9 * (k as f32 + 1.0)
                }
            })
            .collect();
        depths.extend([0.0; 64]);
        let out = averaged(&depths, 32);
        assert!(out[..400].iter().all(|d| *d < 0.0));
        assert!(out[432..].iter().all(|d| *d == 0.0), "{:?}", &out[432..436]);
    }

    #[test]
    fn a_gain_that_is_not_a_number_is_read_as_unity_rather_than_silence() {
        assert_eq!(decibels(f32::NAN), 0.0);
        assert_eq!(decibels(0.0), FLOOR_DB);
        assert_eq!(decibels(1.0), 0.0);
    }
}
