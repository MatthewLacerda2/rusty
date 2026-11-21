//! Freeverb.

use crate::stereo::Stereo;

const COMB_LENGTHS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_LENGTHS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const TUNED_RATE: f32 = 44_100.0;
const FIXED_GAIN: f32 = 0.015;
const ROOM_SCALE: f32 = 0.28;
const ROOM_OFFSET: f32 = 0.7;
const DAMP_SCALE: f32 = 0.4;
const ALLPASS_FEEDBACK: f32 = 0.5;
pub(crate) const MAX_PREDELAY: f32 = 0.5;

struct Comb {
    line: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn step(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.line[self.index];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.line[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.line.len();
        output
    }
}

struct Allpass {
    line: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn step(&mut self, input: f32) -> f32 {
        let buffered = self.line[self.index];
        self.line[self.index] = input + buffered * ALLPASS_FEEDBACK;
        self.index = (self.index + 1) % self.line.len();
        buffered - input
    }
}

struct Bank {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Bank {
    fn new(offset: usize, scale: f32) -> Self {
        let scaled = |len: usize| (((len + offset) as f32 * scale).round() as usize).max(1);
        Self {
            combs: COMB_LENGTHS.iter().map(|l| Comb::new(scaled(*l))).collect(),
            allpasses: ALLPASS_LENGTHS
                .iter()
                .map(|l| Allpass::new(scaled(*l)))
                .collect(),
        }
    }

    fn step(&mut self, send: f32, feedback: f32, damp: f32) -> f32 {
        let mut wet: f32 = self
            .combs
            .iter_mut()
            .map(|c| c.step(send, feedback, damp))
            .sum();
        for ap in self.allpasses.iter_mut() {
            wet = ap.step(wet);
        }
        wet
    }
}

pub(crate) fn apply(buf: &mut Stereo, size: f32, damp: f32, predelay: f32, mix: f32, rate: f32) {
    room(buf, None, [size, damp, predelay], mix, rate);
}

pub(crate) fn apply_sent(
    buf: &mut Stereo,
    sent: &Stereo,
    size: f32,
    damp: f32,
    predelay: f32,
    mix: f32,
    rate: f32,
) {
    room(buf, Some(sent), [size, damp, predelay], mix, rate);
}

fn room(
    buf: &mut Stereo,
    sent: Option<&Stereo>,
    [size, damp, predelay]: [f32; 3],
    mix: f32,
    rate: f32,
) {
    let scale = if rate > 0.0 { rate / TUNED_RATE } else { 1.0 };
    let mut late = Late::new(predelay, rate);
    let mut left = Bank::new(0, scale);
    let mut right = Bank::new(STEREO_SPREAD, scale);

    let feedback = size.clamp(0.0, 1.0) * ROOM_SCALE + ROOM_OFFSET;
    let damp = damp.clamp(0.0, 1.0) * DAMP_SCALE;
    let mix = mix.clamp(0.0, 1.0);
    for (i, (l, r)) in buf.l.iter_mut().zip(buf.r.iter_mut()).enumerate() {
        let (in_l, in_r) = sent.map_or((*l, *r), |sent| sent.frame(i));
        let send = late.step((in_l + in_r) * 0.5 * FIXED_GAIN);
        let wet_l = left.step(send, feedback, damp);
        let wet_r = right.step(send, feedback, damp);
        *l = *l * (1.0 - mix) + wet_l * mix;
        *r = *r * (1.0 - mix) + wet_r * mix;
    }
}

struct Late {
    line: Vec<f32>,
    index: usize,
}

impl Late {
    fn new(predelay: f32, rate: f32) -> Self {
        let len = (predelay_seconds(predelay) * rate).round() as usize;
        Self {
            line: vec![0.0; len],
            index: 0,
        }
    }

    fn step(&mut self, send: f32) -> f32 {
        let Some(slot) = self.line.get_mut(self.index) else {
            return send;
        };
        let out = std::mem::replace(slot, send);
        self.index = (self.index + 1) % self.line.len();
        out
    }
}

pub(crate) fn predelay_seconds(predelay: f32) -> f32 {
    if predelay.is_finite() {
        predelay.clamp(0.0, MAX_PREDELAY)
    } else {
        0.0
    }
}

pub(crate) fn tail_seconds(size: f32) -> f32 {
    let feedback = size.clamp(0.0, 1.0) * ROOM_SCALE + ROOM_OFFSET;
    let samples = |lengths: &[usize]| lengths.iter().copied().max().unwrap_or(0) + STEREO_SPREAD;
    let loop_seconds = samples(&COMB_LENGTHS) as f32 / TUNED_RATE;
    let diffusion: usize = ALLPASS_LENGTHS.iter().sum::<usize>() + STEREO_SPREAD;
    loop_seconds * (0.001f32.ln() / feedback.ln()) + diffusion as f32 / TUNED_RATE
}

#[cfg(test)]
mod tests;
