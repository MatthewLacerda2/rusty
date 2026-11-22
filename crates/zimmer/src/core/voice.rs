//! One `source → filter → amp` path of a note, and how a patch's layers share the note they are
//! summed into.

use super::lfo::Lfos;
use super::tracks::{cutoff_track, pitch_track};
use super::{env, filter, source, NoteContext, RATE};
use crate::hash::hash3;
use crate::note::{midi_to_freq, NoteOpts};
use crate::patch::{Adsr, Filter, Layer, Patch, PitchEnv, Source};
use crate::stereo::Stereo;

/// Hash channel ("LY") for layer seeds, apart from every draw the seed itself feeds.
const LAYER_CHANNEL: u64 = 0x4c59;

pub(super) fn layer_seed(seed: u64, index: usize) -> u64 {
    u64::from(hash3(index as i64, 0, LAYER_CHANNEL, seed))
}

pub(super) struct Struck<'a> {
    pub(super) midi: f32,
    pub(super) opts: &'a NoteOpts,
    pub(super) lfos: Lfos,
    pub(super) gate: f32,
    pub(super) n: usize,
    pub(super) pulse: Option<Vec<f32>>,
}

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
    pub(super) fn voice(&self, voice: &Voice<'_>, seed: u64, base: Option<&[f32]>) -> Stereo {
        let (gate, n) = (self.gate, self.n);
        // Velocity and brightness are clamped once, here: no stage reading them clamps its own way.
        let velocity = self.opts.velocity.clamp(0.0, 1.0);
        let brightness = (self.opts.velocity + self.opts.timbre).clamp(0.0, 1.0);
        let glide = self.opts.glide;
        let freq = midi_to_freq(self.midi);
        let freqs = pitch_track(&self.lfos, voice.pitch_env, glide, freq, gate, n);
        let pulse = self.pulse.as_deref();
        let mut buf = source::render(
            voice.source,
            &freqs,
            pulse,
            brightness,
            NoteContext {
                gate,
                seed,
                rate: RATE,
            },
        );
        if let Some(f) = voice.filter {
            let cutoffs = cutoff_track(&f, &self.lfos, gate, n, (brightness, self.midi), base);
            buf.each(|channel| filter::apply(channel, &f, &cutoffs, RATE));
        }
        apply_amp(&mut buf, voice.amp, &self.lfos, gate, velocity * voice.gain);
        buf
    }
}

fn apply_amp(buf: &mut Stereo, amp: &Adsr, lfos: &Lfos, gate: f32, velocity: f32) {
    let envelope = env::track(amp, gate, buf.frames(), RATE);
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
        expected.resize(part.frames().max(expected.frames()));
        // Summed by hand rather than with `Stereo::add`, which is under test.
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
