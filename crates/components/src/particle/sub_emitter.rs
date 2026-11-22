use serde::{Deserialize, Serialize};

use crate::RefVisit;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SubEmitTrigger {
        Birth = "birth",
        Death = "death",
        Collision = "collision",
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubEmitters {
    #[serde(default)]
    pub birth: Option<u32>,
    #[serde(default)]
    pub death: Option<u32>,
    #[serde(default)]
    pub collision: Option<u32>,
    #[serde(default)]
    pub inherit_velocity: f32,
}

impl SubEmitters {
    pub const REF_POINTERS: [&'static str; 3] = [
        "/sub_emitters/birth",
        "/sub_emitters/death",
        "/sub_emitters/collision",
    ];

    pub fn get(&self, trigger: SubEmitTrigger) -> Option<u32> {
        match trigger {
            SubEmitTrigger::Birth => self.birth,
            SubEmitTrigger::Death => self.death,
            SubEmitTrigger::Collision => self.collision,
        }
    }

    pub fn set(&mut self, trigger: SubEmitTrigger, target: Option<u32>) {
        match trigger {
            SubEmitTrigger::Birth => self.birth = target,
            SubEmitTrigger::Death => self.death = target,
            SubEmitTrigger::Collision => self.collision = target,
        }
    }

    pub fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        let slots = [&mut self.birth, &mut self.death, &mut self.collision];
        for (p, r) in Self::REF_POINTERS.into_iter().zip(slots) {
            visit(p, r);
        }
    }

    pub fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        let slots = [self.birth, self.death, self.collision];
        for (p, r) in Self::REF_POINTERS.into_iter().zip(slots) {
            r.inspect(|&r| visit(p, r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_rewrites_and_drops_targets() {
        let mut s = SubEmitters {
            birth: Some(1),
            death: Some(2),
            collision: None,
            inherit_velocity: 0.5,
        };
        s.visit_refs(&mut |_, r| *r = r.and_then(|id| (id != 2).then_some(id + 10)));
        assert_eq!((s.birth, s.death, s.collision), (Some(11), None, None));
    }
}
