pub mod effects;
mod fog;
mod options;
mod quality;
mod render_scale;
mod shadows;
mod ssao;
mod state;

use mlua::Lua;

use super::Reg;
use crate::api::ApiCell;
use rusty_core::application::Application;
use rusty_core::settings::GraphicsSettings;
use rusty_scene::Scene;
use rusty_ui::ScreenSize;

pub fn register(
    lua: &Lua,
    scene: ApiCell<Scene>,
    (graphics, screen): (ApiCell<GraphicsSettings>, ApiCell<ScreenSize>),
    app: ApiCell<Application>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    effects::register(lua, &table, scene)?;
    quality::register_quality(lua, &table, graphics, app)?;
    render_scale::register(lua, &table, (graphics, screen), app)?;
    options::register_options(lua, &table, graphics, app)?;
    shadows::register_shadows(lua, &table, scene)?;
    fog::register_fog(lua, &table, scene)?;
    ssao::register_ssao(lua, &table, scene)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}
