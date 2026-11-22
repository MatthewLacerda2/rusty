use std::any::Any;

use super::game::{GameWorld, PlayTransition};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapPoint {
    BeforeTick,
    Before(PlayTransition),
    /// After the transition, before input is published: host acts replay and the `InputFrame` is read or written.
    BeforeInput,
    AfterTick,
    Transition(PlayTransition),
    /// A paused frame's poll is done with no tick run; an edit-mode frame ends here.
    AfterPoll,
}

pub trait TickTap: Any {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld);

    fn dt(&mut self, dt: f32) -> f32 {
        dt
    }

    /// A replay hands back the recorded operations.
    fn loads(&mut self, ready: Vec<u32>) -> Vec<u32> {
        ready
    }

    fn load_progress(&mut self, moved: Option<u8>) -> Option<u8> {
        moved
    }
}

impl GameWorld {
    pub fn set_tap(&mut self, tap: Option<Box<dyn TickTap>>) {
        self.resources.tap = tap;
    }

    pub fn tap_mut<T: TickTap>(&mut self) -> Option<&mut T> {
        let tap: &mut dyn Any = self.resources.tap.as_deref_mut()?;
        tap.downcast_mut::<T>()
    }

    pub fn take_tap(&mut self) -> Option<Box<dyn TickTap>> {
        self.resources.tap.take()
    }

    pub(super) fn tap(&mut self, point: TapPoint) {
        if let Some(mut tap) = self.resources.tap.take() {
            tap.at(point, self);
            self.resources.tap.get_or_insert(tap);
        }
    }

    pub(super) fn tap_input(&mut self, dt: f32) -> f32 {
        self.tap_timed(TapPoint::BeforeInput);
        let dt = self.resources.tap.as_mut().map_or(dt, |tap| tap.dt(dt));
        self.publish_loads();
        dt
    }

    pub(super) fn transition_tapped(&mut self) -> PlayTransition {
        let heading = self.resources.application.borrow().play_heading();
        if heading != self.world.scene.borrow().is_playing() {
            let next = if heading {
                PlayTransition::Entered
            } else {
                PlayTransition::Exited
            };
            self.tap(TapPoint::Before(next));
        }
        let transition = self.handle_transition();
        if transition != PlayTransition::None {
            self.tap(TapPoint::Transition(transition));
        }
        transition
    }
}
