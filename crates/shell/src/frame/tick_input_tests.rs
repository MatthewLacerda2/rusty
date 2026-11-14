//! Input across the fixed ticks of a frame (#1877): a frame that owes no tick leaves
//! it pending, and of several ticks only the first sees its edges and mouse delta.

use std::cell::RefCell;
use std::rc::Rc;

use super::fixed_tick_tests::falling;
use super::*;
use rusty_app::{TapPoint, TickTap};

/// What each tick saw of a held key's edge and of the mouse delta.
#[derive(Default)]
struct Seen(Rc<RefCell<Vec<(bool, f64)>>>);

impl TickTap for Seen {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld) {
        if point == TapPoint::AfterTick {
            let input = world.input().borrow();
            let seen = (input.get_key_down("W"), input.mouse_delta().0);
            self.0.borrow_mut().push(seen);
        }
    }
}

#[test]
fn input_reaches_the_first_tick_of_a_frame_and_waits_for_a_frame_with_none() {
    let (mut game, _) = falling();
    let mut ticks = FixedTicks::default();
    let mut poses = DrawPoses::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    let log = Rc::new(RefCell::new(Vec::new()));
    game.set_tap(Some(Box::new(Seen(Rc::clone(&log)))));
    game.input().borrow_mut().press("W");
    game.input().borrow_mut().add_mouse_delta(4.0, 0.0);
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.005); // no tick owed
    assert!(log.borrow().is_empty(), "nothing ran");
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.05); // three ticks
    assert_eq!(*log.borrow(), vec![(true, 4.0), (false, 0.0), (false, 0.0)]);
}
