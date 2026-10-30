//! src/app/tap.rs — the tick's tap: where a demo recorder or replay sits (#1581).
//!
//! A demo records what reaches the world, tick by tick, and a replay feeds it back
//! at the same points. Every host (the editor, the player, the harness) advances
//! the world through `GameWorld::tick`, so the hook lives there, once: an installed
//! [`TickTap`] is called at each [`TapPoint`] of a tick, and at a play transition
//! however the transition happened (a paused Play or Stop runs no tick).
//!
//! Like the [`SystemProbe`](super::SystemProbe), the tap is the dev layer's: none is
//! installed by default and a ship build never installs one. A recorder only reads;
//! a replay writes the world's inputs, which is what replaying means.

use std::any::Any;

use super::game::{GameWorld, PlayTransition};

/// Where in a tick the tap is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapPoint {
    /// The top of `tick`, before its play transition: where a host's acts since
    /// the last tick are drained.
    BeforeTick,
    /// Play is about to be entered or left, by a tick or by a paused frame's poll:
    /// the world is still as the last tick (or edit mode) left it.
    Before(PlayTransition),
    /// After the transition, right before input is published: where the tick's
    /// host acts are replayed and its `InputFrame` is read or written.
    BeforeInput,
    /// The tick is done.
    AfterTick,
    /// Play was entered or left, by a tick or by a paused frame's poll.
    Transition(PlayTransition),
    /// A paused frame's poll is done, with no tick run: an edit-mode frame ends
    /// here as a ticked one ends at [`AfterTick`](Self::AfterTick).
    AfterPoll,
}

/// Watches (a recorder) or feeds (a replay) a world's ticks.
pub trait TickTap: Any {
    /// The world reached `point`. The tap is out of the world for the call.
    fn at(&mut self, point: TapPoint, world: &mut GameWorld);

    /// The tick's `dt`, right after [`TapPoint::BeforeInput`]: a recorder notes
    /// it, a replay hands back the recorded one.
    fn dt(&mut self, dt: f32) -> f32 {
        dt
    }
}

impl GameWorld {
    /// Install (or with `None`, remove) the world's tick tap.
    pub fn set_tap(&mut self, tap: Option<Box<dyn TickTap>>) {
        self.resources.tap = tap;
    }

    /// The installed tap, if it is a `T`.
    pub fn tap_mut<T: TickTap>(&mut self) -> Option<&mut T> {
        let tap: &mut dyn Any = self.resources.tap.as_deref_mut()?;
        tap.downcast_mut::<T>()
    }

    /// Remove the installed tap and hand it back.
    pub fn take_tap(&mut self) -> Option<Box<dyn TickTap>> {
        self.resources.tap.take()
    }

    /// Call the tap at `point`, moved out of the world for the call so it may take
    /// `&mut self`. A tap that installed a replacement keeps it.
    pub(super) fn tap(&mut self, point: TapPoint) {
        if let Some(mut tap) = self.resources.tap.take() {
            tap.at(point, self);
            self.resources.tap.get_or_insert(tap);
        }
    }

    /// The tap at [`TapPoint::BeforeInput`], then the tick's `dt` as it has it.
    pub(super) fn tap_input(&mut self, dt: f32) -> f32 {
        self.tap(TapPoint::BeforeInput);
        self.resources.tap.as_mut().map_or(dt, |tap| tap.dt(dt))
    }

    /// The play transition, with the tap told before and after one happens.
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
