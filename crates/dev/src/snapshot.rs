use serde_json::Value;

use rusty_app::GameWorld;

pub fn snapshot(world: &GameWorld) -> Value {
    let scene = world.scene().borrow();
    rusty_script::api::snapshot::world_value(
        &scene,
        &world.main_view(),
        world.play_frame(),
        scene.is_playing(),
        &world.resources.ui_view(&scene),
    )
}
