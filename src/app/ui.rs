//! src/app/ui.rs — the UI systems: layout (#417) and events (#420).
//!
//! The layout pass is the first `LateUpdate` system: it recomputes every UI rect
//! into [`Resources::ui_layout`] from the live scene and the screen size. `LateUpdate`
//! runs after the whole `FixedUpdate` stage — scripts' `Update` and `LateUpdate`,
//! physics, animation and the deferred destroys — so a script's RectTransform or
//! Canvas change shows in the layout the same tick, and the next tick's pointer
//! dispatch and this frame's UI draw (#418) read a settled layout. The pass is a
//! pure function of (scene, screen size); see `ui::layout`. Right after it, every
//! Selectable's state is shown on its graphic (`ui::events::transition`).
//!
//! [`dispatch_ui_events`] is a `FixedUpdate` system that `play::register` slots
//! between the scripts' `Start` and `Update`: the event system turns this tick's
//! input into UI callbacks, fired straight into the scripts.

use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use crate::ui::events::{Frame, UiHook};

/// Register the layout pass: the first `LateUpdate` system, after every
/// `FixedUpdate` system of the tick.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::LateUpdate, layout_ui)
        .add_system(Stage::LateUpdate, show_selectable_states);
}

/// Turn this tick's input into UI callbacks and fire them (#420). Reads last tick's
/// settled layout — or computes one on the first tick of Play, before this
/// session's first `LateUpdate` ran.
pub(super) fn dispatch_ui_events(world: &mut World, res: &mut Resources) {
    let screen = res.screen_pixels();
    if res.play_frame == 0 {
        res.ui_layout = crate::ui::UiLayout::compute(&world.scene.borrow().world, screen);
    }
    let deliveries = {
        let scene = world.scene.borrow();
        let input = res.input.borrow();
        let scripts = &res.script_manager;
        let handles = |id: u32, hook: UiHook| scripts.has_ui_handler(id, hook);
        let frame = Frame {
            world: &scene.world,
            layout: &res.ui_layout,
            input: &input,
            screen,
            handles: &handles,
        };
        res.event_system.borrow_mut().process(&frame)
    };
    res.script_manager.dispatch_ui_events(&deliveries);
}

/// Show every Selectable's state on its graphic, fading on unscaled time.
fn show_selectable_states(world: &mut World, res: &mut Resources) {
    let dt = res.time.borrow().unscaled_delta_time;
    let mut scene = world.scene.borrow_mut();
    res.event_system
        .borrow_mut()
        .apply_transitions(&mut scene.world, dt);
}

/// Recompute every canvas's layout for this tick.
fn layout_ui(world: &mut World, res: &mut Resources) {
    let screen = res.screen_pixels();
    let scene = world.scene.borrow();
    res.ui_layout = crate::ui::UiLayout::compute(&scene.world, screen);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use glam::Vec2;

    use crate::app::GameWorld;
    use crate::components::{CanvasComponent, ImageComponent, RectTransformComponent};
    use crate::core::input::InputState;
    use crate::navigation::NavigationGraph;
    use crate::scene::{Scene, ScriptComponent};
    use crate::scripting::ConsoleLogs;

    #[test]
    fn a_play_tick_lays_out_the_ui_on_the_video_resolution() {
        let mut scene = Scene::new();
        let root = scene.add_entity("Canvas".to_string());
        scene
            .world
            .set_canvas(root, Some(CanvasComponent::default()));
        let hud = scene.add_entity("Hud".to_string());
        let rt = RectTransformComponent::default();
        scene.world.set_rect_transform(hud, Some(rt));
        scene.set_parent(hud, Some(root)).expect("parent exists");
        let mut game = GameWorld::new(
            Rc::new(RefCell::new(scene)),
            Rc::new(RefCell::new(InputState::new())),
            Rc::new(RefCell::new(NavigationGraph::new(
                -1.0, 1.0, -1.0, 1.0, 1.0,
            ))),
            Rc::new(RefCell::new(ConsoleLogs::new())),
        );
        game.set_playing(true);
        game.tick(crate::time::FIXED_DELTA_TIME);
        let layout = &game.resources.ui_layout;
        assert_eq!(layout.len(), 2);
        // Headless: the default 1280×720 video resolution, matching width → the
        // canvas spans the 1920×1080 reference; the element sits centred.
        let r = layout.get(hud).expect("laid out");
        assert!((r.scale_factor - 1280.0 / 1920.0).abs() < 1e-5);
        assert!((r.rect.0 - Vec2::new(910.0, 490.0)).abs().max_element() < 1e-3);
    }

    #[test]
    fn the_first_play_tick_hit_tests_before_any_late_update_ran() {
        // A script frees the cursor in `Start`; the pointer already rests on a
        // panel. The first tick's dispatch runs before its `LateUpdate` layout, so
        // it must lay out on its own to see the panel.
        let script = std::env::temp_dir().join("rusty_420_first_tick.lua");
        std::fs::write(
            &script,
            "return { Start = function() Input.SetCursorLocked(false) end }",
        )
        .expect("write script");
        let mut scene = Scene::new();
        let root = scene.add_entity("Canvas".to_string());
        scene
            .world
            .set_canvas(root, Some(CanvasComponent::default()));
        let panel = scene.add_entity("Panel".to_string());
        let rt = RectTransformComponent::default();
        scene.world.set_rect_transform(panel, Some(rt));
        scene
            .world
            .set_image(panel, Some(ImageComponent::default()));
        *scene.world.scripts_mut(panel).expect("scripts") = vec![ScriptComponent {
            path: script.to_string_lossy().replace('\\', "/"),
            ..Default::default()
        }];
        scene.set_parent(panel, Some(root)).expect("parent exists");
        let input = Rc::new(RefCell::new(InputState::new()));
        input.borrow_mut().move_mouse(960.0, 540.0);
        let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
        let mut game = GameWorld::new(
            Rc::new(RefCell::new(scene)),
            input,
            Rc::new(RefCell::new(nav)),
            Rc::new(RefCell::new(ConsoleLogs::new())),
        );
        game.resources.screen.borrow_mut().set_game_view(1920, 1080);
        game.set_playing(true);
        game.tick(crate::time::FIXED_DELTA_TIME);
        assert!(game.resources.event_system.borrow().is_pointer_over_ui());
    }
}
