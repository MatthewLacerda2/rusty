//! Where the notes are summed, and where an effect that belongs to the *mix* runs rather than to
//! one instrument.

use crate::core::{RATE, SAMPLE_RATE};
use crate::fx;
use crate::level::{Cut, Layer};
use crate::patch::{sidechains, Fx};
use crate::song::automate::{self, Param, Riding};
use crate::song::clock::Clock;
use crate::song::excerpt::Scope;
use crate::song::Song;
use crate::stereo::{self, Stereo};

pub(super) struct Mix<'a> {
    song: &'a Song,
    master: Stereo,
    parts: Vec<Option<Stereo>>,
    keys: Vec<Option<Stereo>>,
    measured: bool,
    scope: &'a Scope,
    riding: Vec<Riding<'a>>,
    clock: &'a Clock,
    sent: Option<Stereo>,
}

impl<'a> Mix<'a> {
    pub(super) fn new(
        song: &'a Song,
        arrangement_end: usize,
        clock: &'a Clock,
        scope: &'a Scope,
    ) -> Self {
        Self {
            song,
            master: Stereo::silence(arrangement_end),
            parts: song.tracks.iter().map(|_| None).collect(),
            keys: keyed(song),
            measured: scope.heard_count() > 1,
            riding: automate::riding(song),
            clock,
            sent: sends(song, scope).then(|| Stereo::silence(arrangement_end)),
            scope,
        }
    }

    pub(super) fn needs(&self, track: usize) -> bool {
        self.scope.heard(track) || self.keys[track].is_some()
    }

    pub(super) fn add(&mut self, track: usize, src: &Stereo, at: usize) {
        if let Some(key) = self.keys[track].as_mut() {
            mix_into(key, src, at, UNITY);
        }
        if !self.scope.heard(track) {
            return;
        }
        let played = &self.song.tracks[track];
        if !played.fx.is_empty() || self.riding[track].needs_a_bus() {
            mix_into(self.parts[track].get_or_insert_default(), src, at, UNITY);
            return;
        }
        let placed = placement(played.gain, played.pan);
        mix_into(&mut self.master, src, at, placed);
        if let Some(sent) = self.sent.as_mut() {
            mix_into(sent, src, at, sending(placed, played.send));
        }
        if self.measured {
            mix_into(self.parts[track].get_or_insert_default(), src, at, placed);
        }
    }

    pub(super) fn finish(mut self, cuts: &[Cut]) -> (Stereo, Vec<Layer>) {
        let song = self.song;
        let mut parts = std::mem::take(&mut self.parts);
        let keys = Keyed {
            song,
            parts: std::mem::take(&mut self.keys),
        };
        for (index, (track, part)) in song.tracks.iter().zip(&mut parts).enumerate() {
            let riding = self.riding[index];
            let rides = riding.moves_the_fader();
            let bussed = !track.fx.is_empty() || riding.needs_a_bus();
            let Some(bus) = part.as_mut().filter(|_| bussed) else {
                continue;
            };
            ring_out(bus, &track.fx, self.clock);
            fx::apply_chain_keyed(bus, &track.fx, RATE, &keys, self.clock);
            fx::close_tail(bus, &track.fx, RATE, self.clock);
            let placed = if rides {
                automate::ride(bus, track, riding, self.clock);
                UNITY
            } else {
                placement(track.gain, track.pan)
            };
            mix_into(&mut self.master, bus, 0, placed);
            if let Some(sent) = self.sent.as_mut() {
                match riding.send {
                    Some(curve) => {
                        automate::send_into(sent, bus, placed, curve, track.send, self.clock);
                    }
                    None => mix_into(sent, bus, 0, sending(placed, track.send)),
                }
            }
            if !rides {
                // Scaled after the fold-down rather than before it, so the master is summed from
                // exactly the samples it always was and only the copy being measured moves.
                scale(bus, placed);
            }
        }
        ring_out(&mut self.master, &song.fx, self.clock);
        match self.sent.as_mut() {
            Some(sent) => {
                sent.grow_to(self.master.frames());
                fx::apply_chain_sent(&mut self.master, sent, &song.fx, RATE, self.clock);
            }
            None => fx::apply_chain_timed(&mut self.master, &song.fx, RATE, self.clock),
        }
        fx::close_tail(&mut self.master, &song.fx, RATE, self.clock);
        let layers = if self.measured {
            measure(song, parts, self.master.frames(), self.scope, cuts)
        } else {
            Vec::new()
        };
        (self.master, layers)
    }
}

fn sends(song: &Song, scope: &Scope) -> bool {
    let held_back = song.tracks.iter().enumerate().any(|(index, track)| {
        let moving = song
            .automation
            .iter()
            .any(|curve| curve.param == Param::Send && curve.track == track.name);
        scope.heard(index) && (track.send < 1.0 || moving)
    });
    has_a_room(&song.fx) && held_back
}

pub(in crate::song) fn has_a_room(chain: &[Fx]) -> bool {
    chain
        .iter()
        .any(|fx| matches!(fx, Fx::Reverb { .. } | Fx::Delay { .. }))
}

fn sending((left, right): (f32, f32), send: f32) -> (f32, f32) {
    let send = send.clamp(0.0, 1.0);
    (left * send, right * send)
}

fn keyed(song: &Song) -> Vec<Option<Stereo>> {
    let mut keys: Vec<Option<Stereo>> = song.tracks.iter().map(|_| None).collect();
    let named = song.tracks.iter().flat_map(|track| sidechains(&track.fx));
    for name in named {
        if let Some(index) = song.tracks.iter().position(|track| track.name == name) {
            keys[index] = Some(Stereo::default());
        }
    }
    keys
}

struct Keyed<'a> {
    song: &'a Song,
    parts: Vec<Option<Stereo>>,
}

impl fx::Keys for Keyed<'_> {
    fn part(&self, track: &str) -> Option<&Stereo> {
        let index = self.song.tracks.iter().position(|it| it.name == track)?;
        self.parts.get(index)?.as_ref()
    }
}

const UNITY: (f32, f32) = (1.0, 1.0);

fn placement(gain: f32, pan: f32) -> (f32, f32) {
    let (l, r) = stereo::pan_gains(pan);
    (gain * l, gain * r)
}

fn measure(
    song: &Song,
    parts: Vec<Option<Stereo>>,
    frames: usize,
    scope: &Scope,
    cuts: &[Cut],
) -> Vec<Layer> {
    let (from, to) = scope.keep(frames);
    song.tracks
        .iter()
        .zip(parts)
        .enumerate()
        .filter(|(index, _)| scope.heard(*index))
        .map(|(_, (track, part))| {
            let mut part = part.unwrap_or_default();
            if !part.is_empty() {
                part.grow_to(frames);
                part.cut(from, to);
            }
            Layer::of(
                track.name.clone(),
                &part.interleaved(),
                stereo::CHANNELS,
                SAMPLE_RATE,
                cuts.to_vec(),
            )
        })
        .collect()
}

fn ring_out(buf: &mut Stereo, chain: &[Fx], clock: &Clock) {
    let tail = (fx::tail_seconds(chain, clock) * RATE).round() as usize;
    buf.resize(buf.frames() + tail);
}

fn scale(buf: &mut Stereo, (left, right): (f32, f32)) {
    for (slot, gain) in [(&mut buf.l, left), (&mut buf.r, right)] {
        for sample in slot.iter_mut() {
            *sample *= gain;
        }
    }
}

fn mix_into(dst: &mut Stereo, src: &Stereo, at: usize, (left, right): (f32, f32)) {
    let end = at + src.frames();
    dst.grow_to(end);
    for (slot, (source, gain)) in [(&mut dst.l, (&src.l, left)), (&mut dst.r, (&src.r, right))] {
        for (into, sample) in slot[at..end].iter_mut().zip(source) {
            *into += sample * gain;
        }
    }
}

#[cfg(test)]
#[path = "mix_tests.rs"]
mod tests;
