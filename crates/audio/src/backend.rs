#[cfg(test)]
use super::mixer::settings::LOW_PASS_OFF;
use std::sync::Arc;

use super::device::tap::Tap;
use super::mixer::{GroupId, GroupMix};
use super::speaker::SpeakerMode;
use rusty_components::ReverbParams;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceMix {
    pub gain: f32,
    pub pan: f32,
    /// The source's `spatial_blend` in `[0, 1]`: how far a stereo clip is downmixed
    /// to mono before panning (`0` keeps its stereo image untouched).
    pub spatial_blend: f32,
    /// Playback rate (`Time.timeScale` for a time-scaled voice, else `1.0`).
    pub speed: f32,
    pub doppler: f32,
    pub paused: bool,
    pub low_pass: f32,
}

impl VoiceMix {
    #[cfg(test)]
    pub const FLAT: VoiceMix = VoiceMix {
        gain: 1.0,
        pan: 0.0,
        spatial_blend: 0.0,
        speed: 1.0,
        doppler: 1.0,
        paused: false,
        low_pass: LOW_PASS_OFF,
    };

    pub fn with_doppler(self, doppler: f32) -> Self {
        Self { doppler, ..self }
    }

    pub fn with_master(self, master: f32) -> Self {
        Self {
            gain: self.gain * master,
            ..self
        }
    }

    pub fn for_output(self, master: f32, mode: SpeakerMode) -> Self {
        mode.shape(self.with_master(master))
    }
}

#[derive(Clone, Debug)]
pub struct PlayParams {
    pub clip: String,
    pub looping: bool,
    /// The initial mix, so the first samples already play at the resolved state.
    pub mix: VoiceMix,
    pub group: GroupId,
}

pub trait AudioBackend {
    /// `false`: the clip could not play; the maestro logs the attempt anyway.
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool;

    fn stop(&mut self, id: VoiceId);

    fn set_mix(&mut self, id: VoiceId, mix: &VoiceMix);

    fn is_live(&self, id: VoiceId) -> bool;

    fn stop_all(&mut self);

    fn set_speaker_mode(&mut self, mode: SpeakerMode);

    /// Replaces any group at `id`; `parent` must exist first, or the group plays silent.
    fn add_group(&mut self, id: GroupId, parent: Option<GroupId>);

    fn set_group(&mut self, id: GroupId, mix: &GroupMix);

    fn set_reverb(&mut self, params: &ReverbParams);

    fn tap(&self) -> Option<Arc<Tap>> {
        None
    }

    /// `seconds` of engine time went by: an offline mixer renders them now, a
    /// device runs on its own clock and ignores it.
    fn advance(&mut self, _seconds: f64) {}
}

#[derive(Default)]
pub struct NullBackend;

impl AudioBackend for NullBackend {
    fn play(&mut self, _id: VoiceId, _params: &PlayParams) -> bool {
        true
    }
    fn stop(&mut self, _id: VoiceId) {}
    fn set_mix(&mut self, _id: VoiceId, _mix: &VoiceMix) {}
    fn is_live(&self, _id: VoiceId) -> bool {
        false
    }
    fn stop_all(&mut self) {}
    fn set_speaker_mode(&mut self, _mode: SpeakerMode) {}
    fn add_group(&mut self, _id: GroupId, _parent: Option<GroupId>) {}
    fn set_group(&mut self, _id: GroupId, _mix: &GroupMix) {}
    fn set_reverb(&mut self, _params: &ReverbParams) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_accepts_but_makes_no_sound() {
        let mut b = NullBackend;
        let params = PlayParams {
            clip: "x.ogg".to_string(),
            looping: false,
            mix: VoiceMix::FLAT,
            group: GroupId::MASTER,
        };
        assert!(b.play(VoiceId(1), &params));
        b.set_mix(VoiceId(1), &VoiceMix::FLAT.with_master(0.5));
        assert!(!b.is_live(VoiceId(1)));
        b.stop(VoiceId(1));
        b.stop_all();
        b.set_speaker_mode(SpeakerMode::Tv);
        b.add_group(GroupId(1), Some(GroupId::MASTER));
    }
}
