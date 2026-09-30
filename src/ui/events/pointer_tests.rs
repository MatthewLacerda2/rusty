//! Tests for pointer dispatch (#420): hover enter/exit, press → click with
//! bubbling, drag stealing a press, scroll bubbling, interactable gating, the
//! consumed flag, a locked cursor, and the Selectable states the pointer drives.

use glam::Vec2;

use super::fixture::{button, panel, scene, Handlers, Rig};
use super::UiHook::*;
use crate::components::SelectionState;

#[test]
fn hover_enters_and_exits_the_chain_but_not_between_a_button_and_its_label() {
    let (mut s, canvas) = scene();
    let btn = button(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let label = panel(&mut s, btn, Vec2::splat(10.0), Vec2::splat(20.0));
    let h = Handlers::default()
        .on(btn, &[PointerEnter, PointerExit])
        .on(label, &[PointerEnter, PointerExit]);
    let mut rig = Rig::new(s, h);
    rig.point(50.0, 50.0);
    assert_eq!(rig.hooks(), vec![(btn, PointerEnter)]);
    rig.point(15.0, 15.0);
    assert_eq!(
        rig.hooks(),
        vec![(label, PointerEnter)],
        "still inside the button"
    );
    rig.point(500.0, 500.0);
    assert_eq!(rig.hooks(), vec![(label, PointerExit), (btn, PointerExit)]);
    assert!(rig.hooks().is_empty(), "nothing changes, nothing fires");
}

#[test]
fn a_click_bubbles_to_the_nearest_handler_in_order() {
    let (mut s, canvas) = scene();
    let btn = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let label = panel(&mut s, btn, Vec2::splat(10.0), Vec2::splat(20.0));
    let h = Handlers::default().on(btn, &[PointerEnter, PointerDown, PointerUp, PointerClick]);
    let mut rig = Rig::new(s, h);
    rig.point(15.0, 15.0);
    rig.input.press("Mouse0");
    rig.input.release("Mouse0");
    let d = rig.tick();
    let hooks: Vec<_> = d.iter().map(|d| (d.entity, d.hook)).collect();
    let expected = [PointerEnter, PointerDown, PointerUp, PointerClick].map(|h| (btn, h));
    assert_eq!(hooks, expected);
    assert_eq!(
        d[3].event.and_then(|e| e.target),
        Some(label),
        "the hit entity"
    );
    assert_eq!(d[3].event.map(|e| e.position), Some(Vec2::splat(15.0)));
}

#[test]
fn releasing_elsewhere_is_no_click_and_right_clicks_carry_their_button() {
    let (mut s, canvas) = scene();
    let btn = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let h = Handlers::default().on(btn, &[PointerDown, PointerUp, PointerClick]);
    let mut rig = Rig::new(s, h);
    rig.point(50.0, 50.0);
    rig.input.press("Mouse0");
    assert_eq!(rig.hooks(), vec![(btn, PointerDown)]);
    rig.point(500.0, 500.0);
    rig.input.release("Mouse0");
    assert_eq!(
        rig.hooks(),
        vec![(btn, PointerUp)],
        "released off the button"
    );
    rig.point(50.0, 50.0);
    rig.input.press("Mouse1");
    rig.input.release("Mouse1");
    let d = rig.tick();
    assert_eq!(d.len(), 3);
    assert!(d
        .iter()
        .all(|d| d.event.is_some_and(|e| e.button.name() == "Right")));
}

#[test]
fn a_drag_past_the_threshold_steals_the_press_from_another_entity() {
    let (mut s, canvas) = scene();
    let view = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(400.0));
    let item = panel(&mut s, view, Vec2::ZERO, Vec2::splat(100.0));
    let h = Handlers::default()
        .on(view, &[BeginDrag, Drag, EndDrag])
        .on(item, &[PointerDown, PointerUp, PointerClick]);
    let mut rig = Rig::new(s, h);
    rig.point(50.0, 50.0);
    rig.input.press("Mouse0");
    assert_eq!(rig.hooks(), vec![(item, PointerDown)]);
    rig.point(55.0, 50.0);
    assert!(rig.hooks().is_empty(), "under the threshold");
    rig.point(70.0, 50.0);
    let expected = vec![(item, PointerUp), (view, BeginDrag), (view, Drag)];
    assert_eq!(rig.hooks(), expected);
    rig.input.release("Mouse0");
    assert_eq!(
        rig.hooks(),
        vec![(view, EndDrag)],
        "no click after a stolen press"
    );
}

#[test]
fn scroll_bubbles_past_a_disabled_button_that_does_not_handle_it() {
    let (mut s, canvas) = scene();
    let view = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(400.0));
    let btn = button(&mut s, view, Vec2::ZERO, Vec2::splat(100.0));
    s.world
        .selectable_mut(btn)
        .expect("selectable")
        .interactable = false;
    let h = Handlers::default()
        .on(view, &[Scroll])
        .on(btn, &[PointerDown, PointerClick]);
    let mut rig = Rig::new(s, h);
    rig.point(50.0, 50.0);
    rig.input.scroll(-2.0);
    rig.input.press("Mouse0");
    let d = rig.tick();
    assert_eq!(d.len(), 1, "the disabled button starts nothing");
    assert_eq!((d[0].entity, d[0].hook), (view, Scroll));
    assert_eq!(d[0].event.map(|e| e.delta), Some(Vec2::new(0.0, -2.0)));
}

#[test]
fn over_ui_consumed_and_a_locked_cursor() {
    let (mut s, canvas) = scene();
    let btn = button(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let mut rig = Rig::new(s, Handlers::default());
    rig.point(50.0, 50.0);
    rig.input.press("Mouse0");
    rig.tick();
    assert!(rig.events.is_pointer_over_ui() && rig.events.is_pointer_consumed());
    let world = &rig.scene.world;
    assert_eq!(rig.events.state_of(world, btn), SelectionState::Pressed);
    rig.point(500.0, 500.0);
    rig.tick();
    assert!(!rig.events.is_pointer_over_ui());
    assert!(
        rig.events.is_pointer_consumed(),
        "the press began on the UI"
    );
    let world = &rig.scene.world;
    assert_eq!(rig.events.state_of(world, btn), SelectionState::Selected);
    rig.input.release("Mouse0");
    rig.point(50.0, 50.0);
    rig.input.set_cursor_locked(true);
    rig.tick();
    assert!(
        !rig.events.is_pointer_consumed(),
        "a locked cursor is off the UI"
    );
}
