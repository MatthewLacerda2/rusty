//! src/ui/events/pointer.rs — the mouse half of the event system (#420).
//!
//! Unity's `StandaloneInputModule.ProcessMouseEvent`, per tick: hover first (so a
//! pointer that arrives and clicks in one tick reads enter → down → up → click),
//! then each button's press / release, then its drag, then the wheel.
//!
//! - **Hover.** Everything from the hit entity up to the root is *inside* the
//!   pointer. When the hit changes, entities left behind get `OnPointerExit` and new
//!   ones `OnPointerEnter`, deepest first; moving between a button and its own label
//!   leaves the button inside, so it neither exits nor re-enters.
//! - **Press.** Down, up and click go to one entity: the nearest ancestor-or-self of
//!   the hit that handles any of the three, or is a `Selectable`. A click fires on
//!   release when the pointer is still over that same entity. A left press also
//!   moves the focus to the nearest interactable, navigable Selectable under it (or
//!   clears it).
//! - **Drag.** The nearest handler of any drag callback, once the held pointer has
//!   moved [`DRAG_THRESHOLD`] pixels: `OnBeginDrag`, then `OnDrag` on every tick it
//!   moves, then `OnEndDrag` on release. A drag taken by another entity than the
//!   press cancels the press (it gets `OnPointerUp` and no click) — a scroll view
//!   dragged from one of its buttons.
//! - **Scroll.** The wheel goes to the nearest `OnScroll` handler, `delta = (0, lines)`.

use glam::Vec2;

use super::tree::{ancestors_or_self, nearest};
use super::{
    accepts, raycast_pointer, Delivery, EventSystem, Frame, PointerButton, PointerEvent, UiHook,
    DRAG_THRESHOLD,
};
use crate::components::NavigationMode;

/// What one button's step sees.
struct Pointer {
    hit: Option<u32>,
    position: Option<Vec2>,
    delta: Vec2,
}

impl Pointer {
    fn event(&self, button: PointerButton, delta: Vec2) -> PointerEvent {
        PointerEvent {
            button,
            position: self.position.unwrap_or(Vec2::splat(-1.0)),
            delta,
            target: self.hit,
        }
    }
}

impl EventSystem {
    /// The pointer phase of [`EventSystem::process`].
    pub(super) fn process_pointer(&mut self, f: &Frame, out: &mut Vec<Delivery>) {
        let position = pointer_position(f);
        let delta = match (position, self.pointer) {
            (Some(p), Some(q)) => p - q,
            _ => Vec2::ZERO,
        };
        self.pointer = position;
        let mut pointer = f.view.pointer(position);
        if let Some((origin, dir)) = pointer.ray {
            pointer.ray_limit = (f.walls)(origin, dir);
        }
        let hit = raycast_pointer(f.world, f.layout, &pointer);
        let p = Pointer {
            hit,
            position,
            delta,
        };
        self.hover_to(f, &p, out);
        for button in PointerButton::ALL {
            self.process_button(f, button, &p, out);
        }
        let wheel = f.input.scroll_delta() as f32;
        let scroller = hit.and_then(|h| nearest(f.world, h, |c| handles(f, c, &[UiHook::Scroll])));
        if let Some(target) = scroller.filter(|&s| wheel != 0.0 && accepts(f.world, s)) {
            let event = p.event(PointerButton::Left, Vec2::new(0.0, wheel));
            emit(f, out, target, UiHook::Scroll, Some(event));
        }
    }

    /// Exit what the pointer left, enter what it reached — deepest first.
    fn hover_to(&mut self, f: &Frame, p: &Pointer, out: &mut Vec<Delivery>) {
        let now = p
            .hit
            .map(|h| ancestors_or_self(f.world, h))
            .unwrap_or_default();
        let event = Some(p.event(PointerButton::Left, p.delta));
        let old = std::mem::replace(&mut self.hover, now);
        for &id in old.iter().filter(|id| !self.hover.contains(id)) {
            emit(f, out, id, UiHook::PointerExit, event);
        }
        for &id in self.hover.iter().filter(|id| !old.contains(id)) {
            emit(f, out, id, UiHook::PointerEnter, event);
        }
    }

    /// One button's edges (release before press when both a release and a re-press
    /// happened since last tick), then its drag.
    fn process_button(
        &mut self,
        f: &Frame,
        b: PointerButton,
        p: &Pointer,
        out: &mut Vec<Delivery>,
    ) {
        let key = b.key();
        let (down, up) = (f.input.get_key_down(key), f.input.get_key_up(key));
        if up && self.buttons[b as usize].held {
            self.release(f, b, p, out);
        }
        if down {
            self.press(f, b, p, out);
            if up && !f.input.is_key_down(key) {
                self.release(f, b, p, out);
            }
        }
        if self.buttons[b as usize].held {
            self.drag(f, b, p, out);
        }
    }

    fn press(&mut self, f: &Frame, b: PointerButton, p: &Pointer, out: &mut Vec<Delivery>) {
        let world = f.world;
        if b == PointerButton::Left {
            let focus = p
                .hit
                .and_then(|h| nearest(world, h, |c| world.has_selectable(c)))
                .filter(|&s| accepts(world, s))
                .filter(|&s| {
                    world
                        .selectable(s)
                        .is_some_and(|s| s.navigation != NavigationMode::None)
                });
            self.select(f, focus, out);
        }
        let press = p
            .hit
            .and_then(|h| press_owner(f, h))
            .filter(|&o| accepts(world, o));
        let drag_hooks = [UiHook::BeginDrag, UiHook::Drag, UiHook::EndDrag];
        let drag = p
            .hit
            .and_then(|h| nearest(world, h, |c| handles(f, c, &drag_hooks)))
            .filter(|&d| accepts(world, d));
        self.buttons[b as usize] = super::ButtonState {
            held: true,
            over_ui: p.hit.is_some(),
            press_position: p.position.unwrap_or_default(),
            press,
            drag,
            dragging: false,
        };
        if let Some(owner) = press {
            emit(
                f,
                out,
                owner,
                UiHook::PointerDown,
                Some(p.event(b, p.delta)),
            );
        }
    }

    fn release(&mut self, f: &Frame, b: PointerButton, p: &Pointer, out: &mut Vec<Delivery>) {
        let st = std::mem::take(&mut self.buttons[b as usize]);
        let event = Some(p.event(b, p.delta));
        if let Some(owner) = st.press {
            emit(f, out, owner, UiHook::PointerUp, event);
            let over_owner = p.hit.and_then(|h| press_owner(f, h)) == Some(owner);
            if over_owner {
                emit(f, out, owner, UiHook::PointerClick, event);
            }
        }
        if let (true, Some(d)) = (st.dragging, st.drag) {
            emit(f, out, d, UiHook::EndDrag, event);
        }
    }

    fn drag(&mut self, f: &Frame, b: PointerButton, p: &Pointer, out: &mut Vec<Delivery>) {
        let st = &mut self.buttons[b as usize];
        let (Some(target), Some(pos)) = (st.drag, p.position) else {
            return;
        };
        let event = Some(p.event(b, p.delta));
        if !st.dragging && pos.distance(st.press_position) >= DRAG_THRESHOLD {
            st.dragging = true;
            if let Some(owner) = st.press.filter(|&o| o != target) {
                st.press = None;
                emit(f, out, owner, UiHook::PointerUp, event);
            }
            emit(f, out, target, UiHook::BeginDrag, event);
        }
        if st.dragging && p.delta != Vec2::ZERO {
            emit(f, out, target, UiHook::Drag, event);
        }
    }
}

/// The pointer in UI screen pixels (bottom-left, y-up), or `None` while the cursor
/// is locked (its ray then runs through the screen centre). `Input` reports it
/// top-left, y-down.
fn pointer_position(f: &Frame) -> Option<Vec2> {
    if f.input.cursor().locked {
        return None;
    }
    let (x, y) = f.input.mouse_position();
    Some(Vec2::new(x as f32, f.view.screen.y - y as f32))
}

/// The entity a press on `hit` belongs to: the nearest ancestor-or-self handling
/// down, up or click, or carrying a Selectable.
fn press_owner(f: &Frame, hit: u32) -> Option<u32> {
    let hooks = [UiHook::PointerDown, UiHook::PointerUp, UiHook::PointerClick];
    nearest(f.world, hit, |c| {
        f.world.has_selectable(c) || handles(f, c, &hooks)
    })
}

/// Whether `id` handles any of `hooks`.
fn handles(f: &Frame, id: u32, hooks: &[UiHook]) -> bool {
    hooks.iter().any(|&h| (f.handles)(id, h))
}

/// Queue `hook` on `entity` when one of its scripts defines it.
pub(super) fn emit(
    f: &Frame,
    out: &mut Vec<Delivery>,
    entity: u32,
    hook: UiHook,
    event: Option<PointerEvent>,
) {
    if (f.handles)(entity, hook) {
        out.push(Delivery {
            entity,
            hook,
            event,
        });
    }
}
