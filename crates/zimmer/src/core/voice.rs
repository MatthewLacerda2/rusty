//! voice: one `source → filter → amp` path of a note, and how a patch's
//! layers share the note they are summed into.
//!
//! A patch is its own path plus up to [`MAX_LAYERS`](crate::patch::MAX_LAYERS)
//! more, each rendered here the same way and added before the fx chain, which
//! [`render_swept`](super::render_swept) runs once on the sum. What a path
//! takes from the note ([`Struck`]) is the same for all of them; what it brings
//! of its own ([`Voice`]) is a source, an amp envelope, an optional filter and
//! pitch envelope, and a gain.

use super::lfo::Lfos;
use super::tracks::{cutoff_track, pitch_track};
use super::{RATE, env, filter, source};
use crate::hash::hash3;
use crate::note::{NoteOpts, midi_to_freq};
use crate::patch::{Adsr, Filter, Layer, Patch, PitchEnv, Source};
use crate::stereo::Stereo;

/// Hash channel a layer's seed is drawn on, apart from every draw the seed
/// itself feeds.
const LAYER_CHANNEL: u64 = 0x4c59; // "LY"

/// The seed layer `index` is struck with: the note's, re-drawn per layer, so
/// two noise layers are two noises and two stacks start in different places.
/// The top-level sound keeps the note's own seed, which is what every patch
/// before layers was struck with.
pub(super) fn layer_seed(seed: u64, index: usize) -> u64 {
    u64::from(hash3(index as i64, 0, LAYER_CHANNEL, seed))
}

/// What every layer of one note shares: the pitch it is played at, how it is
/// played, the patch's LFOs, and how long it is held and rings.
pub(super) struct Struck<'a> {
    pub(super) midi: f32,
    pub(super) opts: &'a NoteOpts,
    /// The patch's LFOs over this note, worked out once so every layer moves
    /// together.
    pub(super) lfos: Lfos,
    pub(super) gate: f32,
    pub(super) n: usize,
    /// How far an LFO aimed at `pulse_width` swings every square's width at
    /// each sample, in every layer, or nothing.
    pub(super) pulse: Option<Vec<f32>>,
}

/// One `source → filter → amp` path: the patch's own, or one of its layers.
pub(super) struct Voice<'a> {
    source: &'a Source,
    amp: &'a Adsr,
    filter: Option<Filter>,
    pitch_env: Option<PitchEnv>,
    gain: f32,
}

impl<'a> Voice<'a> {
    pub(super) fn top(patch: &'a Patch) -> Self {
        Self {
            source: &patch.source,
            amp: &patch.amp,
            filter: patch.filter,
            pitch_env: patch.pitch_env,
            gain: 1.0,
        }
    }

    pub(super) fn layer(layer: &'a Layer) -> Self {
        Self {
            source: &layer.source,
            amp: &layer.amp,
            filter: layer.filter,
            pitch_env: layer.pitch_env,
            gain: layer.gain,
        }
    }
}

impl Struck<'_> {
    /// Renders one path of the note, `n` frames long, before the fx chain.
    /// `base` is a cutoff moved from outside the note, for the one path whose
    /// filter a song may move.
    pub(super) fn voice(&self, voice: &Voice<'_>, seed: u64, base: Option<&[f32]>) -> Stereo {
        let (gate, n) = (self.gate, self.n);
        let velocity = self.opts.velocity.clamp(0.0, 1.0);
        let brightness = (self.opts.velocity + self.opts.timbre).clamp(0.0, 1.0);
        let glide = self.opts.glide;
        let freq = midi_to_freq(self.midi);
        let freqs = pitch_track(&self.lfos, voice.pitch_env, glide, freq, gate, n);
        let pulse = self.pulse.as_deref();
        let mut buf = source::render(voice.source, &freqs, pulse, seed, brightness, gate, RATE);
        if let Some(f) = voice.filter {
            // One cutoff track, both channels: the filter's *state* belongs to a
            // channel, and the curve it is following belongs to the note.
            let cutoffs = cutoff_track(&f, &self.lfos, gate, n, brightness, self.midi, base);
            buf.each(|channel| filter::apply(channel, &f, &cutoffs, RATE));
        }
        apply_amp(&mut buf, voice.amp, &self.lfos, gate, velocity * voice.gain);
        buf
    }
}

/// Apply velocity, the amp envelope and any tremolo — the stage that turns a
/// continuous tone into a note. `velocity` arrives already clamped from
/// [`Struck::voice`], times the layer's gain.
fn apply_amp(buf: &mut Stereo, amp: &Adsr, lfos: &Lfos, gate: f32, velocity: f32) {
    let envelope = env::track(amp, gate, buf.frames(), RATE);
    // The gain curve is worked out once and both channels are walked with it:
    // an envelope is a property of the note, and a tremolo that ran at a
    // slightly different phase on each side would be a stereo effect nobody
    // asked for.
    let gains: Vec<f32> = (0..buf.frames())
        .map(|i| velocity * envelope[i] * lfos.tremolo(i))
        .collect();
    buf.each(|channel| {
        for (s, gain) in channel.iter_mut().zip(&gains) {
            *s *= gain;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::render_note;
    use crate::patch::{Filter, FilterKind, NoiseColor, Osc, PitchEnv, Slope, Wave};

    fn adsr(d: f32, r: f32) -> Adsr {
        Adsr {
            a: 0.001,
            d,
            s: 0.0,
            r,
            curve: 0.0,
        }
    }

    /// A sine body under a filtered, bending noise crack.
    fn layered() -> Patch {
        let crack = Layer {
            source: Source::Noise {
                color: NoiseColor::Pink,
            },
            amp: adsr(0.04, 0.3),
            filter: Some(Filter {
                kind: FilterKind::Highpass,
                cutoff: 1500.0,
                slope: Slope::default(),
                resonance: 0.2,
                env_octaves: 1.0,
                vel_octaves: 0.5,
                key_track: 0.0,
                adsr: adsr(0.05, 0.0),
            }),
            pitch_env: Some(PitchEnv {
                semitones: 5.0,
                adsr: adsr(0.02, 0.0),
            }),
            gain: 0.6,
        };
        Patch {
            source: Source::OscStack {
                oscs: vec![Osc {
                    wave: Wave::Sine,
                    detune_cents: 0.0,
                    gain: 1.0,
                    octave: 0,
                    voices: 1,
                    spread: 12.0,
                    width: 0.0,
                    pulse_width: 0.5,
                }],
            },
            amp: adsr(0.1, 0.05),
            filter: None,
            pitch_env: None,
            layers: vec![crack],
            lfo: None,
            lfo2: None,
            fx: Vec::new(),
        }
    }

    /// A layer alone, as the patch it would be with no layers.
    fn alone(patch: &Patch, layer: &Layer) -> Patch {
        Patch {
            source: layer.source.clone(),
            amp: layer.amp,
            filter: layer.filter,
            pitch_env: layer.pitch_env,
            layers: Vec::new(),
            ..patch.clone()
        }
    }

    /// The promise: a layered note is the sample-wise sum of its paths, each
    /// rendered as a patch of its own and struck with its layer's seed.
    #[test]
    fn a_layered_note_is_the_sum_of_its_layers_played_alone() {
        let patch = layered();
        let opts = NoteOpts {
            duration: 0.1,
            velocity: 0.8,
            timbre: 0.1,
            glide: None,
            seed: 11,
        };
        let sum = render_note(&patch, 60.0, &opts).expect("renders");
        let top = Patch {
            layers: Vec::new(),
            ..patch.clone()
        };
        let mut expected = render_note(&top, 60.0, &opts).expect("renders");
        let layer = &patch.layers[0];
        let seeded = NoteOpts {
            seed: layer_seed(opts.seed, 0),
            ..opts
        };
        let part = render_note(&alone(&patch, layer), 60.0, &seeded).expect("renders");
        // Summed by hand rather than with `Stereo::add`, which is under test.
        expected.resize(part.frames().max(expected.frames()));
        for (sample, added) in expected.l.iter_mut().zip(&part.l) {
            *sample += added * layer.gain;
        }
        assert_eq!(sum.frames(), expected.frames());
        for (i, (a, b)) in sum.l.iter().zip(&expected.l).enumerate() {
            assert!((a - b).abs() < 1e-6, "frame {i}: {a} against {b}");
        }
        assert!(
            sum.l.iter().zip(&sum.r).any(|(l, r)| l != r),
            "noise is stereo"
        );
    }

    #[test]
    fn each_layer_has_a_seed_of_its_own() {
        let seeds = [
            layer_seed(5, 0),
            layer_seed(5, 1),
            layer_seed(5, 2),
            layer_seed(6, 0),
        ];
        for (i, a) in seeds.iter().enumerate() {
            assert_ne!(*a, 5);
            assert!(seeds[i + 1..].iter().all(|b| a != b), "{seeds:?}");
        }
    }
}
