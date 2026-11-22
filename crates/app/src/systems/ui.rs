use glam::Vec3;

use crate::game::PlayTransition;
use crate::registry::App;
use crate::resources::Resources;
use crate::stage::Stage;
use crate::world::World;
use rusty_ui::events::{Frame, UiHook, WALL_RANGE, WALL_TOLERANCE};
use rusty_ui::UiLayout;

impl crate::GameWorld {
    pub fn refresh_ui_textures(&self) {
        let scene = self.world.scene.borrow();
        let mut screen = self.resources.screen.borrow_mut();
        let textures = screen.shared_textures_mut();
        self.resources
            .ui_textures
            .borrow_mut()
            .refresh(&scene.world, textures);
    }

    pub(crate) fn refreshed_after(&self, transition: PlayTransition) -> PlayTransition {
        if matches!(transition, PlayTransition::Entered) {
            self.refresh_ui_textures();
        }
        transition
    }
}

pub(crate) fn register(app: &mut App) {
    app.add_system(Stage::LateUpdate, sway_ui)
        .add_system(Stage::LateUpdate, layout_ui)
        .add_system(Stage::LateUpdate, show_selectable_states);
}

pub(crate) fn dispatch_ui_events(world: &mut World, res: &mut Resources) {
    let view = res.ui_view(&world.scene.borrow());
    if res.play_frame == 0 {
        res.ui_layout = UiLayout::compute_in(&world.scene.borrow().world, &view);
    }
    let deliveries = {
        let scene = world.scene.borrow();
        let input = res.input.borrow();
        let physics = res.physics.borrow();
        let scripts = &res.script_manager;
        let handles = |id: u32, hook: UiHook| scripts.has_ui_handler(id, hook);
        let walls = |origin: Vec3, dir: Vec3| {
            physics
                .as_ref()
                .and_then(|p| p.first_surface_ahead(origin, dir, WALL_RANGE))
                .map_or(f32::INFINITY, |h| h.distance + WALL_TOLERANCE)
        };
        let frame = Frame {
            world: &scene.world,
            layout: &res.ui_layout,
            input: &input,
            view: &view,
            walls: &walls,
            handles: &handles,
            dt: res.time.borrow().unscaled_delta_time,
        };
        res.event_system.borrow_mut().process(&frame)
    };
    res.script_manager.dispatch_ui_events(&deliveries);
}

fn show_selectable_states(world: &mut World, res: &mut Resources) {
    let dt = res.time.borrow().unscaled_delta_time;
    let mut scene = world.scene.borrow_mut();
    res.event_system
        .borrow_mut()
        .apply_transitions(&mut scene.world, dt);
}

fn sway_ui(world: &mut World, res: &mut Resources) {
    let dt = res.time.borrow().unscaled_delta_time;
    let mut scene = world.scene.borrow_mut();
    let camera = rusty_scene::game_camera_from_scene(&res.camera.borrow(), &scene);
    rusty_ui::sway::update_sway(&mut scene.world, &camera, dt);
}

fn layout_ui(world: &mut World, res: &mut Resources) {
    let scene = world.scene.borrow();
    let view = res.ui_view(&scene);
    res.ui_layout = UiLayout::compute_in(&scene.world, &view);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use glam::Vec2;

    use crate::GameWorld;
    use rusty_components::{CanvasComponent, ImageComponent, RectTransformComponent};
    use rusty_core::input::InputState;
    use rusty_navigation::NavigationGraph;
    use rusty_scene::{Scene, ScriptComponent};
    use rusty_script::scripting::ConsoleLogs;

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
        game.tick(rusty_time::FIXED_DELTA_TIME);
        let layout = &game.resources.ui_layout;
        assert_eq!(layout.iter().count(), 2);
        let r = layout.get(hud).expect("laid out");
        assert!((r.scale_factor - 1280.0 / 1920.0).abs() < 1e-5);
        assert!((r.rect.0 - Vec2::new(910.0, 490.0)).abs().max_element() < 1e-3);
    }

    #[test]
    fn the_first_play_tick_hit_tests_before_any_late_update_ran() {
        let script = rusty_test_support::temp::dir().join("rusty_420_first_tick.lua");
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
            path: rusty_core::paths::to_slash(&script),
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
        game.tick(rusty_time::FIXED_DELTA_TIME);
        assert!(game.resources.event_system.borrow().is_pointer_over_ui());
    }
}
