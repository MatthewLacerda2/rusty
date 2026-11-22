use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use super::mixer::{GroupId, GroupMix};
use super::speaker::SpeakerMode;
use rusty_components::ReverbParams;

#[derive(Default)]
pub struct Recording {
    pub plays: Vec<(VoiceId, PlayParams)>,
    pub mixes: Vec<(VoiceId, VoiceMix)>,
    pub stops: Vec<VoiceId>,
    pub speaker_modes: Vec<SpeakerMode>,
    pub live: BTreeSet<VoiceId>,
    pub groups_added: Vec<(GroupId, Option<GroupId>)>,
    pub group_mixes: Vec<(GroupId, GroupMix)>,
    pub reverbs: Vec<ReverbParams>,
    /// Every advance of the output, in seconds, in order: one per pushed frame.
    pub advances: Vec<f64>,
}

impl Recording {
    pub fn current(&self, id: VoiceId) -> Option<VoiceMix> {
        let mixed = self.mixes.iter().rev().find(|(v, _)| *v == id);
        let played = self.plays.iter().rev().find(|(v, _)| *v == id);
        mixed.map(|m| m.1).or(played.map(|p| p.1.mix))
    }

    pub fn group(&self, id: GroupId) -> Option<GroupMix> {
        let last = self.group_mixes.iter().rev().find(|(g, _)| *g == id);
        last.map(|(_, mix)| *mix)
    }

    pub fn last_voice(&self) -> VoiceId {
        self.plays.last().expect("nothing played").0
    }
}

pub struct RecordingBackend(pub Rc<RefCell<Recording>>);

impl RecordingBackend {
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
    fn set_reverb(&mut self, params: &ReverbParams) {
        self.0.borrow_mut().reverbs.push(*params);
    }
    fn advance(&mut self, seconds: f64) {
        self.0.borrow_mut().advances.push(seconds);
    }
}
