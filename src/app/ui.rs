//! src/app/ui.rs — the UI layout system (#417).
//!
//! One `LateUpdate` system that recomputes every UI rect into
//! [`Resources::ui_layout`] from the live scene and the screen size. `LateUpdate`
//! runs after the whole `FixedUpdate` stage — scripts' `Update` and `LateUpdate`,
//! physics, animation and the deferred destroys — so a script's RectTransform or
//! Canvas change shows in the layout the same tick, and the next tick's pointer
//! dispatch (#420) and this frame's UI draw (#418) read a settled layout. The pass
//! is a pure function of (scene, screen size); see `ui::layout`.

use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

/// Register the layout pass: the first `LateUpdate` system, after every
/// `FixedUpdate` system of the tick.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::LateUpdate, layout_ui);
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
    use crate::components::{CanvasComponent, RectTransformComponent};
    use crate::core::input::InputState;
    use crate::navigation::NavigationGraph;
    use crate::scene::Scene;
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
}
