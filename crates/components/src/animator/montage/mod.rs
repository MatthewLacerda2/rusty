use rusty_asset::animation_graph::{GraphSlot, Montage, MontageSection};

mod resolve;
pub use resolve::MontageRefused;

/// Fired when a montage starts blending out: it finished, or was stopped or
/// interrupted (Unreal's `OnMontageBlendingOut`).
pub const MONTAGE_BLEND_OUT: &str = "MontageBlendOut";
pub const MONTAGE_END: &str = "MontageEnd";
const DEFAULT_SECTION: &str = "Default";

#[derive(Clone, Debug, PartialEq)]
pub struct MontageEvent {
    pub montage: String,
    pub slot: String,
    pub interrupted: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Montages {
    pub playing: Vec<ActiveMontage>,
    pub pending: Vec<MontageEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendOut {
    pub from: f32,
    pub elapsed: f32,
    pub duration: f32,
    /// Stopped or interrupted, not ending on its own: its markers are silent.
    pub interrupted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveMontage {
    pub montage: Montage,
    pub slot: GraphSlot,
    pub order: usize,
    /// The clip's length in seconds.
    pub length: f32,
    pub rate: f32,
    /// Clip seconds.
    pub time: f32,
    pub section: usize,
    pub next: Vec<Option<usize>>,
    /// Unscaled seconds since it started: what blending in reads.
    pub elapsed: f32,
    pub out: Option<BlendOut>,
}

impl ActiveMontage {
    pub fn start(&self, index: usize) -> f32 {
        let at = self.montage.sections.get(index).map_or(0.0, |s| s.time);
        at.clamp(0.0, self.length.max(0.0))
    }

    pub fn end(&self, index: usize) -> f32 {
        let end = match index + 1 < self.montage.sections.len() {
            true => self.start(index + 1),
            false => self.length.max(0.0),
        };
        end.max(self.start(index))
    }

    pub fn section_index(&self, name: &str) -> Option<usize> {
        find(&self.montage.sections, name)
    }

    pub fn section_name(&self) -> &str {
        self.montage
            .sections
            .get(self.section)
            .map_or("", |s| s.name.as_str())
    }

    pub fn weight(&self) -> f32 {
        if let Some(out) = self.out {
            if out.duration <= 0.0 {
                return 0.0;
            }
            return (out.from * (1.0 - out.elapsed / out.duration)).clamp(0.0, 1.0);
        }
        match self.montage.blend_in > 0.0 {
            true => (self.elapsed / self.montage.blend_in).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    pub fn blend_out(&mut self, duration: f32, interrupted: bool) -> bool {
        if self.out.is_some() {
            return false;
        }
        self.out = Some(BlendOut {
            from: self.weight(),
            elapsed: 0.0,
            duration: duration.max(0.0),
            interrupted,
        });
        true
    }

    /// Whether its authored markers fire: not once it was stopped or
    /// interrupted.
    pub fn fires_markers(&self) -> bool {
        !self.out.is_some_and(|o| o.interrupted)
    }

    pub fn event(&self) -> MontageEvent {
        MontageEvent {
            montage: self.montage.name.clone(),
            slot: self.slot.name.clone(),
            interrupted: self.out.is_some_and(|o| o.interrupted),
        }
    }
}

fn find(sections: &[MontageSection], name: &str) -> Option<usize> {
    sections.iter().position(|s| s.name == name)
}

impl Montages {
    /// The active (not blending-out) montage in `slot`, or in any slot.
    pub fn active(&self, slot: Option<&str>) -> Option<&ActiveMontage> {
        self.playing
            .iter()
            .find(|m| m.out.is_none() && slot.is_none_or(|s| m.slot.name == s))
    }

    fn active_mut(&mut self, slot: Option<&str>) -> Option<&mut ActiveMontage> {
        self.playing
            .iter_mut()
            .find(|m| m.out.is_none() && slot.is_none_or(|s| m.slot.name == s))
    }

    pub fn play(&mut self, montage: ActiveMontage) {
        let blend = montage.montage.blend_in;
        let slot = montage.slot.name.clone();
        self.stop(Some(&slot), Some(blend));
        let at = self.playing.partition_point(|m| m.order <= montage.order);
        self.playing.insert(at, montage);
    }

    pub fn stop(&mut self, slot: Option<&str>, blend: Option<f32>) -> bool {
        let mut stopped = false;
        for m in &mut self.playing {
            if slot.is_some_and(|s| m.slot.name != s) {
                continue;
            }
            let duration = blend.unwrap_or(m.montage.blend_out);
            if m.blend_out(duration, true) {
                self.pending.push(m.event());
                stopped = true;
            }
        }
        stopped
    }

    pub fn jump(&mut self, section: &str, slot: Option<&str>) -> bool {
        let Some(m) = self.with_section(section, slot) else {
            return false;
        };
        let Some(index) = m.section_index(section) else {
            return false;
        };
        m.section = index;
        m.time = m.start(index);
        true
    }

    pub fn set_next(&mut self, from: &str, to: &str, slot: Option<&str>) -> bool {
        let Some(m) = self.with_section(from, slot) else {
            return false;
        };
        let target = match to.is_empty() {
            true => None,
            false => match m.section_index(to) {
                Some(i) => Some(i),
                None => return false,
            },
        };
        let Some(index) = m.section_index(from) else {
            return false;
        };
        m.next[index] = target;
        true
    }

    fn with_section(&mut self, section: &str, slot: Option<&str>) -> Option<&mut ActiveMontage> {
        if slot.is_some() {
            return self.active_mut(slot);
        }
        self.playing
            .iter_mut()
            .find(|m| m.out.is_none() && m.section_index(section).is_some())
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_empty(&self) -> bool {
        self.playing.is_empty() && self.pending.is_empty()
    }
}
