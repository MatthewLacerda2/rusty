//! Tests for keyboard focus (#420): automatic and explicit navigation, Tab
//! cycling, submit / cancel, select / deselect announcements, and dropping a focus
//! that went inactive.

use glam::Vec2;

use super::fixture::{button, scene, Handlers, Rig};
use super::UiHook::*;
use super::{nav_actions, NavAction};
use crate::components::NavigationMode;

/// A 2×2 grid of buttons: `[bottom-left, bottom-right, top-left, top-right]`.
fn grid() -> (crate::scene::Scene, [u32; 4]) {
    let (mut s, canvas) = scene();
    let at = |x: f32, y: f32| Vec2::new(x, y);
    let size = Vec2::splat(100.0);
    let bl = button(&mut s, canvas, at(0.0, 0.0), size);
    let br = button(&mut s, canvas, at(300.0, 0.0), size);
    let tl = button(&mut s, canvas, at(0.0, 300.0), size);
    let tr = button(&mut s, canvas, at(300.0, 300.0), size);
    (s, [bl, br, tl, tr])
}

fn key(rig: &mut Rig, name: &str) -> Option<u32> {
    rig.input.press(name);
    rig.input.release(name);
    rig.tick();
    rig.events.selected()
}

#[test]
fn keys_map_to_logical_actions() {
    let mut input = crate::core::input::InputState::new();
    for k in ["Up", "Right", "Tab", "LeftShift", "Enter", "Escape"] {
        input.press(k);
    }
    input.begin_tick();
    let expected = [
        NavAction::Move(0),
        NavAction::Move(3),
        NavAction::Previous,
        NavAction::Submit,
        NavAction::Cancel,
    ];
    assert_eq!(nav_actions(&input), expected);
}

#[test]
fn arrows_move_by_geometry_and_tab_cycles_in_draw_order() {
    let (s, [bl, br, tl, tr]) = grid();
    let mut rig = Rig::new(s, Handlers::default());
    assert_eq!(key(&mut rig, "Up"), None, "nothing focused, nothing moves");
    rig.events.set_selected(Some(bl));
    assert_eq!(key(&mut rig, "Right"), Some(br));
    assert_eq!(key(&mut rig, "Up"), Some(tr));
    assert_eq!(key(&mut rig, "Left"), Some(tl));
    assert_eq!(key(&mut rig, "Left"), Some(tl), "nothing further left");
    assert_eq!(key(&mut rig, "Tab"), Some(tr));
    assert_eq!(key(&mut rig, "Tab"), Some(bl), "wraps around");
    rig.input.press("LeftShift");
    assert_eq!(key(&mut rig, "Tab"), Some(tr));
}

#[test]
fn explicit_none_and_disabled_selectables_shape_navigation() {
    let (mut s, [bl, br, tl, tr]) = grid();
    let sel = |s: &mut crate::scene::Scene, id| s.world.selectable_mut(id).expect("sel").clone();
    let mut explicit = sel(&mut s, bl);
    explicit.navigation = NavigationMode::Explicit;
    explicit.select_on[0] = Some(tr);
    s.world.set_selectable(bl, Some(explicit));
    s.world.selectable_mut(br).expect("sel").interactable = false;
    s.world.selectable_mut(tl).expect("sel").navigation = NavigationMode::None;
    let mut rig = Rig::new(s, Handlers::default());
    rig.events.set_selected(Some(bl));
    assert_eq!(
        key(&mut rig, "Right"),
        Some(bl),
        "explicit without a right target"
    );
    assert_eq!(key(&mut rig, "Up"), Some(tr), "explicit up");
    assert_eq!(
        key(&mut rig, "Down"),
        Some(bl),
        "br is disabled: the next best"
    );
    assert_eq!(key(&mut rig, "Tab"), Some(tr), "Tab skips tl and br");
    assert_eq!(key(&mut rig, "Left"), Some(bl), "tl is not navigable");
}

#[test]
fn focus_changes_announce_and_enter_escape_reach_the_focus() {
    let (s, [bl, br, _, _]) = grid();
    let h = Handlers::default()
        .on(bl, &[Select, Deselect, Submit, Cancel])
        .on(br, &[Select]);
    let mut rig = Rig::new(s, h);
    rig.events.set_selected(Some(bl));
    assert_eq!(rig.hooks(), vec![(bl, Select)]);
    rig.input.press("Enter");
    rig.input.press("Escape");
    assert_eq!(rig.hooks(), vec![(bl, Submit), (bl, Cancel)]);
    rig.input.press("Right");
    assert_eq!(rig.hooks(), vec![(bl, Deselect), (br, Select)]);
    // A left click on empty space clears the focus.
    rig.point(1000.0, 1000.0);
    rig.input.press("Mouse0");
    rig.tick();
    assert_eq!(rig.events.selected(), None);
}

#[test]
fn an_inactive_focus_is_dropped() {
    let (s, [bl, ..]) = grid();
    let h = Handlers::default().on(bl, &[Deselect]);
    let mut rig = Rig::new(s, h);
    rig.events.set_selected(Some(bl));
    rig.tick();
    rig.scene.world.set_active(bl, false);
    assert_eq!(rig.hooks(), vec![(bl, Deselect)]);
    assert_eq!(rig.events.selected(), None);
}
