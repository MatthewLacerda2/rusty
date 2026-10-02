//! src/audio/recording.rs — a test backend that records what the maestro hands over.
//!
//! Test-only. The device is the one thing the headless suite cannot hear, so the
//! maestro and shell tests assert on the exact [`PlayParams`] / [`VoiceMix`] values
//! that would have reached it. The shared [`Recording`] outlives the boxed backend.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use super::mixer::{GroupId, GroupMix};
use super::speaker::SpeakerMode;

/// Everything a [`RecordingBackend`] was told, in order.
#[derive(Default)]
pub struct Recording {
    pub plays: Vec<(VoiceId, PlayParams)>,
    pub mixes: Vec<(VoiceId, VoiceMix)>,
    pub stops: Vec<VoiceId>,
    /// Every speaker mode the output stage was set to, in order.
    pub speaker_modes: Vec<SpeakerMode>,
    /// Voices still "sounding" — started voices join, a test removes one to finish it.
    pub live: BTreeSet<VoiceId>,
    /// Every group created, with its parent, in order.
    pub groups_added: Vec<(GroupId, Option<GroupId>)>,
    /// Every group mix applied, in order.
    pub group_mixes: Vec<(GroupId, GroupMix)>,
}

impl Recording {
    /// The mix a voice is currently at: its last `set_mix`, else its play mix.
    pub fn current(&self, id: VoiceId) -> Option<VoiceMix> {
        let mixed = self.mixes.iter().rev().find(|(v, _)| *v == id);
        let played = self.plays.iter().rev().find(|(v, _)| *v == id);
        mixed.map(|m| m.1).or(played.map(|p| p.1.mix))
    }

    /// The mix group `id` was last set to.
    pub fn group(&self, id: GroupId) -> Option<GroupMix> {
        let last = self.group_mixes.iter().rev().find(|(g, _)| *g == id);
        last.map(|(_, mix)| *mix)
    }

    /// The voice id of the most recent play.
    pub fn last_voice(&self) -> VoiceId {
        self.plays.last().expect("nothing played").0
    }
}

/// An [`AudioBackend`] that accepts every voice and records every call.
pub struct RecordingBackend(pub Rc<RefCell<Recording>>);

impl RecordingBackend {
    /// A backend plus the handle to read its recording back.
    pub fn new() -> (Box<Self>, Rc<RefCell<Recording>>) {
        let rec = Rc::new(RefCell::new(Recording::default()));
        (Box::new(Self(Rc::clone(&rec))), rec)
    }
}

impl AudioBackend for RecordingBackend {
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool {
        let mut rec = self.0.borrow_mut();
        rec.plays.push((id, params.clone()));
        rec.live.insert(id);
        true
    }
    fn stop(&mut self, id: VoiceId) {
        let mut rec = self.0.borrow_mut();
        rec.stops.push(id);
        rec.live.remove(&id);
    }
    fn set_mix(&mut self, id: VoiceId, mix: &VoiceMix) {
        self.0.borrow_mut().mixes.push((id, *mix));
    }
    fn is_live(&self, id: VoiceId) -> bool {
        self.0.borrow().live.contains(&id)
    }
    fn stop_all(&mut self) {
        self.0.borrow_mut().live.clear();
    }
    fn set_speaker_mode(&mut self, mode: SpeakerMode) {
        self.0.borrow_mut().speaker_modes.push(mode);
    }
    fn add_group(&mut self, id: GroupId, parent: Option<GroupId>) {
        self.0.borrow_mut().groups_added.push((id, parent));
    }
    fn set_group(&mut self, id: GroupId, mix: &GroupMix) {
        self.0.borrow_mut().group_mixes.push((id, *mix));
    }
}
