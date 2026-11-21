//! feedback echo.

use super::Tempo;
use crate::stereo::Stereo;

#[derive(Clone, Copy)]
pub(crate) enum Spacing<'a> {
    Seconds(f32),
    Beats(f32, &'a dyn Tempo),
}

impl<'a> Spacing<'a> {
    pub(crate) fn new(time: f32, beats: Option<f32>, tempo: &'a dyn Tempo) -> Self {
        match beats {
            Some(beats) => Self::Beats(beats, tempo),
            None => Self::Seconds(time),
        }
    }

    pub(crate) fn longest(self) -> f32 {
        match self {
            Self::Seconds(time) => time,
            Self::Beats(beats, tempo) => beats * tempo.longest_beat(),
        }
    }

    fn at(self, frame: usize, cap: usize, rate: f32) -> usize {
        match self {
            Self::Seconds(_) => cap,
            Self::Beats(beats, tempo) => {
                let len = (beats * tempo.beat_seconds(frame) * rate).round();
                (len as usize).clamp(1, cap)
            }
        }
    }
}

pub(crate) fn apply(buf: &mut [f32], spacing: Spacing<'_>, feedback: f32, mix: f32, rate: f32) {
    line(buf, None, spacing, feedback, mix, rate);
}

pub(crate) fn apply_sent(
    buf: &mut [f32],
    sent: &[f32],
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    line(buf, Some(sent), spacing, feedback, mix, rate);
}

fn line(
    buf: &mut [f32],
    sent: Option<&[f32]>,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    let Some(cap) = line_length(spacing.longest(), buf.len(), rate) else {
        return;
    };
    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut line = vec![0.0f32; cap];
    for (i, s) in buf.iter_mut().enumerate() {
        let input = sent.map_or(*s, |sent| sent.get(i).copied().unwrap_or(0.0));
        let write = i % cap;
        let echo = line[(write + cap - spacing.at(i, cap, rate)) % cap];
        line[write] = input + echo * feedback;
        *s = *s * (1.0 - mix) + echo * mix;
    }
}

pub(crate) fn ping_pong(
    buf: &mut Stereo,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    crossed(buf, None, spacing, feedback, mix, rate);
}

pub(crate) fn ping_pong_sent(
    buf: &mut Stereo,
    sent: &Stereo,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    crossed(buf, Some(sent), spacing, feedback, mix, rate);
}

fn crossed(
    buf: &mut Stereo,
    sent: Option<&Stereo>,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    let Some(cap) = line_length(spacing.longest(), buf.frames(), rate) else {
        return;
    };
    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut left = vec![0.0f32; cap];
    let mut right = vec![0.0f32; cap];
    for i in 0..buf.frames() {
        let (dry_l, dry_r) = (buf.l[i], buf.r[i]);
        let (in_l, in_r) = sent.map_or((dry_l, dry_r), |sent| sent.frame(i));
        let write = i % cap;
        let read = (write + cap - spacing.at(i, cap, rate)) % cap;
        let (echo_l, echo_r) = (left[read], right[read]);
        left[write] = (in_l + in_r) * 0.5 + echo_r * feedback;
        right[write] = echo_l * feedback;
        buf.l[i] = dry_l * (1.0 - mix) + echo_l * mix;
        buf.r[i] = dry_r * (1.0 - mix) + echo_r * mix;
    }
}

fn line_length(time: f32, frames: usize, rate: f32) -> Option<usize> {
    if !time.is_finite() || time <= 0.0 {
        return None;
    }
    let len = ((time * rate).round() as usize).max(1);
    (len < frames.max(1) * 8).then_some(len)
}

pub(crate) fn tail_seconds(time: f32, feedback: f32) -> f32 {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    let feedback = feedback.clamp(0.0, 0.95);
    if feedback <= 0.0 {
        return time;
    }
    // Echo `k` lands at `k · time` at `feedback^(k-1)`; the last one kept is the last still above
    // −60 dB. The chain's own cap bounds the product.
    let repeats = (0.001f32.ln() / feedback.ln()).ceil() + 1.0;
    time * repeats
}

#[cfg(test)]
mod tests;
