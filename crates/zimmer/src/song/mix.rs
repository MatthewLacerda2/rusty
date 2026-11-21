//! Where the notes are summed, and where an effect that belongs to the *mix* runs rather than to
//! one instrument.

use super::automate::{self, Param, Riding};
use super::clock::Clock;
use super::excerpt::Scope;
use super::Song;
use crate::core::{RATE, SAMPLE_RATE};
use crate::fx;
use crate::level::{Cut, Layer};
use crate::patch::{sidechains, Fx};
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

pub(super) fn has_a_room(chain: &[Fx]) -> bool {
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
mod tests {
    use super::*;

    fn lopsided(frames: usize, value: f32) -> Stereo {
        Stereo {
            l: vec![value; frames],
            r: vec![-value; frames],
        }
    }

    #[test]
    fn a_sent_sum_is_built_for_a_partial_or_moving_send_into_a_room() {
        let song = |send: f32, room: bool, param: &str| -> Song {
            let fx = if room {
                r#"{ "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.2 }"#
            } else {
                ""
            };
            serde_json::from_str(&format!(
                r#"{{ "bpm": 120, "tracks": [{{ "name": "pad", "patch": "p", "send": {send} }}],
                     "patterns": {{}}, "arrangement": [], "fx": [{fx}],
                     "automation": [{{ "track": "pad", "param": "{param}",
                                      "points": [{{ "beat": 0, "value": 0.5 }}] }}] }}"#
            ))
            .expect("the fixture reads")
        };
        let whole = |song: &Song| {
            let scope = Scope::of(song, &Default::default(), &Clock::at(120.0)).expect("whole");
            sends(song, &scope)
        };
        assert!(
            !whole(&song(1.0, true, "gain")),
            "everything sent, a fader moving"
        );
        assert!(whole(&song(1.0, true, "send")), "a moving send");
        assert!(whole(&song(0.5, true, "gain")), "a written partial send");
        assert!(!whole(&song(0.5, false, "send")), "no room to send into");
    }

    #[test]
    fn a_part_is_added_where_it_starts_and_the_way_up_it_came() {
        let mut master = Stereo::centred(vec![0.25; 6]);
        mix_into(&mut master, &lopsided(2, 1.0), 3, UNITY);
        assert_eq!(master.l, vec![0.25, 0.25, 0.25, 1.25, 1.25, 0.25]);
        assert_eq!(master.r, vec![0.25, 0.25, 0.25, -0.75, -0.75, 0.25]);
    }

    #[test]
    fn a_part_landing_past_the_end_extends_the_mix() {
        let mut master = Stereo::silence(2);
        mix_into(&mut master, &lopsided(3, 0.5), 4, UNITY);
        assert_eq!(master.frames(), 7, "four frames in, three long");
        assert_eq!(master.l, vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn each_side_arrives_at_its_own_gain() {
        let mut master = Stereo::silence(1);
        mix_into(&mut master, &Stereo::centred(vec![1.0]), 0, (0.25, 0.75));
        assert_eq!(master.l, vec![0.25]);
        assert_eq!(master.r, vec![0.75]);
    }

    #[test]
    fn scaling_a_bus_multiplies_each_side_by_its_own_gain() {
        let mut bus = lopsided(2, 1.0);
        scale(&mut bus, (0.5, 0.25));
        assert_eq!(bus.l, vec![0.5, 0.5]);
        assert_eq!(bus.r, vec![-0.25, -0.25]);
    }

    #[test]
    fn a_placement_is_the_fader_times_the_position() {
        assert_eq!(placement(0.8, 0.0), (0.8, 0.8));
        let (l, r) = placement(0.5, -1.0);
        assert!(
            (l - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-6,
            "left {l}"
        );
        assert_eq!(r, 0.0);
        assert_eq!(
            UNITY,
            placement(1.0, 0.0),
            "a bus is summed at the identity"
        );
    }
}
