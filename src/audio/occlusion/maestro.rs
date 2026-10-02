//! The maestro's half of occlusion (#467): the cast a voice gets as it starts, the
//! per-`LateUpdate` round-robin that re-casts and smooths, and the read-back.

use glam::Vec3;

use super::{cast, Occluder, OcclusionSettings, VoiceOcclusion};
use crate::audio::backend::{VoiceId, VoiceMix};
use crate::audio::maestro::AudioMaestro;
use crate::audio::mix::resolve_voice;
use crate::components::AudioSourceComponent;

/// Whether `source` is occluded at all: spatialized and not opted out.
pub fn eligible(source: &AudioSourceComponent) -> bool {
    source.occlusion_enabled && source.spatial_blend > 0.0
}

/// One voice as the round-robin sees it: id, owning entity, position, eligible.
type Candidate = (VoiceId, Option<u32>, Vec3, bool);

impl AudioMaestro {
    /// Install the physics query occlusion casts through (the app does, at boot).
    pub fn set_occluder(&mut self, occluder: Occluder) {
        self.occlusion.occluder = Some(occluder);
    }

    /// The global occlusion settings.
    pub fn occlusion_settings(&self) -> OcclusionSettings {
        self.occlusion.settings
    }

    /// Replace the global occlusion settings (strength clamped to `0..1`, at least
    /// one voice per tick). Live voices pick the change up on their next cast.
    pub fn set_occlusion_settings(&mut self, settings: OcclusionSettings) {
        self.occlusion.settings = OcclusionSettings {
            strength: settings.strength.clamp(0.0, 1.0),
            voices_per_tick: settings.voices_per_tick.max(1),
            ..settings
        };
    }

    /// Entity `id`'s smoothed occlusion factor; `0` when it has no live voice.
    pub fn source_occlusion(&self, id: u32) -> f32 {
        self.entity_voices
            .get(&id)
            .map_or(0.0, |live| self.occlusion.factor(live.voice))
    }

    /// The factor a voice about to start at `at` gets: cast now, so its first
    /// sample is already muffled. `0` when it is not occluded or occlusion is off.
    pub(in crate::audio) fn cast_voice(
        &self,
        source: &AudioSourceComponent,
        entity: Option<u32>,
        at: Vec3,
    ) -> f32 {
        if self.occlusion.settings.strength <= 0.0 || !eligible(source) {
            return 0.0;
        }
        self.cast_at(entity, at)
    }

    /// A voice about to start: its cast factor and its first mix with it applied.
    pub(in crate::audio) fn resolve_start(
        &self,
        source: &AudioSourceComponent,
        entity: Option<u32>,
        volume: f32,
        at: Vec3,
    ) -> (f32, VoiceMix) {
        let occluded = self.cast_voice(source, entity, at);
        let applied = occluded * self.occlusion.settings.strength;
        let mix = resolve_voice(&self.env, source, volume, at, applied);
        (occluded, mix)
    }

    /// Track a voice that just started at factor `value`.
    pub(in crate::audio) fn track_occlusion(&mut self, voice: VoiceId, value: f32) {
        self.occlusion
            .voices
            .insert(voice, VoiceOcclusion::at(value));
    }

    /// The fraction of rays blocked from the listener to `at`, skipping `entity`.
    fn cast_at(&self, entity: Option<u32>, at: Vec3) -> f32 {
        let Some(occluder) = self.occlusion.occluder.as_ref() else {
            return 0.0;
        };
        let mask = self.occlusion.settings.layer_mask;
        cast(self.occlusion.listener, at, |from, to| {
            occluder(from, to, entity, mask)
        })
    }

    /// The per-`LateUpdate` step: re-cast up to `voices_per_tick` voices (new ones
    /// first, then round-robin in voice order) against the listener at `listener`,
    /// then move every factor toward its cast over `dt` unscaled seconds. `lookup`
    /// gives an entity source's current settings and position.
    pub fn occlude(
        &mut self,
        listener: Vec3,
        dt: f32,
        lookup: impl Fn(u32) -> Option<(AudioSourceComponent, Vec3)>,
    ) {
        self.occlusion.listener = listener;
        let candidates = self.candidates(lookup);
        let off = self.occlusion.settings.strength <= 0.0;
        let voices = &mut self.occlusion.voices;
        voices.retain(|id, _| candidates.binary_search_by_key(id, |c| c.0).is_ok());
        for c in candidates.iter().filter(|c| off || !c.3) {
            voices.insert(c.0, VoiceOcclusion::default());
        }
        let picks = if off {
            Vec::new()
        } else {
            self.picks(&candidates)
        };
        let casts: Vec<_> = picks
            .iter()
            .map(|&(id, entity, at)| (id, self.cast_at(entity, at)))
            .collect();
        let occlusion = &mut self.occlusion;
        for (id, value) in casts {
            match occlusion.voices.get_mut(&id) {
                Some(v) => {
                    v.target = value;
                    occlusion.cursor = Some(id);
                }
                None => {
                    occlusion.voices.insert(id, VoiceOcclusion::at(value));
                }
            }
        }
        for v in occlusion.voices.values_mut() {
            v.step(dt);
        }
    }

    /// Every live voice, in voice order, with entity sources refreshed by `lookup`.
    fn candidates(
        &mut self,
        lookup: impl Fn(u32) -> Option<(AudioSourceComponent, Vec3)>,
    ) -> Vec<Candidate> {
        for (&id, live) in self.entity_voices.iter_mut() {
            if let Some((source, position)) = lookup(id) {
                live.source = source;
                live.position = position;
            }
        }
        let entities = self
            .entity_voices
            .iter()
            .map(|(&id, live)| (live.voice, Some(id), live.position, eligible(&live.source)));
        let shots =
            (self.oneshots.iter()).map(|(&v, s)| (v, None, s.position, eligible(&s.source)));
        let mut all: Vec<Candidate> = entities.chain(shots).collect();
        all.sort_by_key(|c| c.0);
        all
    }

    /// This tick's casts: eligible voices never cast yet, then the round-robin
    /// resuming after the cursor, up to the budget.
    fn picks(&self, candidates: &[Candidate]) -> Vec<(VoiceId, Option<u32>, Vec3)> {
        let occ = &self.occlusion;
        let (fresh, known): (Vec<&Candidate>, Vec<&Candidate>) = candidates
            .iter()
            .filter(|c| c.3)
            .partition(|c| !occ.voices.contains_key(&c.0));
        let start = known
            .iter()
            .position(|c| Some(c.0) > occ.cursor)
            .unwrap_or(0);
        let rotation = known.iter().cycle().skip(start).take(known.len());
        fresh
            .iter()
            .chain(rotation)
            .take(occ.settings.voices_per_tick as usize)
            .map(|c| (c.0, c.1, c.2))
            .collect()
    }
}
