//! fx — the post-chain: delay, reverb, drive, EQ, compression, chorus, and the mandatory limiter

pub(crate) mod chorus;
pub(crate) mod compress;
pub(crate) mod delay;
pub(crate) mod eq;
mod gain;
pub(crate) mod limiter;
pub(crate) mod reach;
pub(crate) mod reverb;
pub(crate) mod saturate;
mod send;

pub(crate) use delay::Spacing;
pub(crate) use reach::{close_tail, lookahead_seconds, tail_seconds};
pub(crate) use send::apply_chain_sent;

use crate::patch::Fx;
#[cfg(test)]
use crate::patch::{EqBand, EqKind};
use crate::stereo::Stereo;

pub(crate) trait Keys {
    fn part(&self, track: &str) -> Option<&Stereo>;
}

pub(crate) trait Tempo {
    fn beat_seconds(&self, frame: usize) -> f32;
    fn longest_beat(&self) -> f32;
}

pub(crate) struct NoTempo;

impl Tempo for NoTempo {
    fn beat_seconds(&self, _frame: usize) -> f32 {
        0.0
    }
    fn longest_beat(&self) -> f32 {
        0.0
    }
}

struct NoKeys;

impl Keys for NoKeys {
    fn part(&self, _track: &str) -> Option<&Stereo> {
        None
    }
}

pub(crate) fn apply_chain(buf: &mut Stereo, chain: &[Fx], rate: f32) {
    apply_chain_timed(buf, chain, rate, &NoTempo);
}

pub(crate) fn apply_chain_timed(buf: &mut Stereo, chain: &[Fx], rate: f32, tempo: &dyn Tempo) {
    apply_chain_keyed(buf, chain, rate, &NoKeys, tempo);
}

pub(crate) fn apply_chain_keyed(
    buf: &mut Stereo,
    chain: &[Fx],
    rate: f32,
    keys: &dyn Keys,
    tempo: &dyn Tempo,
) {
    for fx in chain {
        apply_one(buf, fx, rate, keys, tempo);
    }
}

fn apply_one(buf: &mut Stereo, fx: &Fx, rate: f32, keys: &dyn Keys, tempo: &dyn Tempo) {
    match fx {
        Fx::Delay {
            time,
            beats,
            feedback,
            mix,
            ping_pong,
        } => {
            let spacing = Spacing::new(*time, *beats, tempo);
            if *ping_pong {
                delay::ping_pong(buf, spacing, *feedback, *mix, rate);
            } else {
                buf.each(|channel| delay::apply(channel, spacing, *feedback, *mix, rate));
            }
        }
        Fx::Reverb {
            size,
            damp,
            mix,
            predelay,
        } => reverb::apply(buf, *size, *damp, *predelay, *mix, rate),
        Fx::Saturate { drive, mix } => {
            buf.each(|channel| saturate::apply(channel, *drive, *mix));
        }
        Fx::Eq { bands } => buf.each(|channel| eq::apply(channel, bands, rate)),
        Fx::Compress {
            threshold,
            ratio,
            attack,
            release,
            makeup,
            mix,
            sidechain,
        } => {
            let key = sidechain.as_deref().and_then(|track| keys.part(track));
            compress::Compressor::new(*threshold, *ratio, *attack, *release, *makeup, *mix)
                .apply(buf, key, rate);
        }
        Fx::Chorus {
            rate: sweep,
            depth,
            voices,
            mix,
        } => chorus::apply(buf, *sweep, *depth, *voices, *mix, rate),
    }
}

#[cfg(test)]
mod tests;
