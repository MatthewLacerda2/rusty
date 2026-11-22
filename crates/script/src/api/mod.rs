// lint: allow(dir-size) a flat registry: one file per Lua namespace, so the list is the surface's index
// sorted: namespace modules and their helpers; no module reads another's declaration order
pub mod ai_perception;
pub mod animator;
pub mod application;
pub mod assets;
pub mod audio;
pub mod backdrop_filter;
pub mod camera;
pub mod canvas;
pub mod canvas_group;
pub mod character_controller;
pub mod component;
pub mod damage;
pub mod decals;
pub mod entity_arg;
pub mod environment_query;
pub mod graphics;
pub mod image;
pub mod input;
pub mod joint;
pub mod layers;
pub mod layout_element;
pub mod layout_group;
pub mod light;
pub mod light_probe_proxy_volume;
pub mod lighting;
pub mod line;
pub mod lod_group;
pub mod lua_json;
pub mod mask;
pub mod material;
pub mod mesh;
pub mod nav;
pub mod nav_mesh_modifier;
pub mod nav_modifier;
pub mod nav_obstacle;
pub mod offmesh_link;
pub mod particle;
pub mod physics;
pub mod probe;
pub mod ragdoll;
pub mod random;
pub mod rect_mask;
pub mod rect_transform;
pub mod reflection;
pub mod reverb_zone;
mod ribbon_style;
pub mod scene;
pub mod scene_hierarchy;
pub mod scene_load;
pub mod scene_load_async;
pub mod scene_prefab;
pub mod selectable;
pub mod shader;
pub mod shape;
pub mod snapshot;
pub mod sound;
mod state;
pub mod storage;
pub mod text;
mod text_effects;
mod text_layout;
pub mod texture;
pub mod time;
pub mod timer;
pub mod trail;
pub mod transform;
pub mod tween;
pub mod ui;
mod ui_look;
pub mod video;

pub use lua_json::recipe_from_json_str;
#[cfg(test)]
pub(crate) use state::attempt;
pub use state::{bind, ApiCell, ApiCtx};

use std::sync::RwLock;

use mlua::{Function, Lua, Table};

use rusty_scene::Scene;

pub type Reg = Result<(), String>;

fn register_navigation(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table
    ai_perception::register(lua, ctx.scene)?;
    nav::register(lua, ctx.scene, ctx.nav)?;
    nav_mesh_modifier::register(lua, ctx.scene)?;
    nav_modifier::register(lua, ctx.scene)?;
    nav_obstacle::register(lua, ctx.scene)?;
    offmesh_link::register(lua, ctx.scene, ctx.nav)
}

fn register_renderers(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    // sorted: each registrar makes its own global table
    light_probe_proxy_volume::register(lua, scene)?;
    lod_group::register(lua, scene)?;
    material::register(lua, scene)?;
    mesh::register(lua, scene)
}

fn register_scene(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table
    component::register(lua, ctx.scene)?;
    scene::register(lua, ctx.scene, ctx.scene_path)
}

pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table; the ones extending a table
    // (`register_hitscan`, `register_reimport`, `register_events`)
    // sort after the one that made it
    animator::register(lua, ctx.scene, ctx.console)?;
    application::register(lua, ctx.application)?;
    assets::register(lua)?;
    assets::register_reimport(lua, ctx.scene)?;
    audio::register(lua, ctx)?;
    camera::register(lua, ctx)?;
    canvas::register(lua, ctx.scene, ctx.screen, ctx.video)?;
    character_controller::register(lua, ctx.scene, ctx.physics)?;
    damage::register(lua, ctx.scene, ctx.physics)?;
    decals::register(lua, ctx.scene)?;
    environment_query::register(lua, ctx)?;
    graphics::register(lua, ctx.scene, (ctx.graphics, ctx.screen), ctx.application)?;
    input::register(lua, ctx.input)?;
    joint::register(lua, ctx.scene)?;
    layers::register(lua, ctx.scene)?;
    light::register(lua, ctx.scene, ctx.console)?;
    lighting::register(lua, ctx.scene)?;
    line::register(lua, ctx.scene)?;
    particle::register(lua, ctx.scene)?;
    physics::register(lua, ctx.scene)?;
    physics::register_hitscan(lua, ctx.scene, ctx.physics)?;
    probe::register(lua, ctx.scene)?;
    ragdoll::register(lua, ctx.scene, ctx.physics)?;
    random::register(lua, ctx.random)?;
    rect_transform::register(lua, ctx.scene, (ctx.screen, ctx.video))?;
    reflection::register(lua, ctx.scene)?;
    register_navigation(lua, ctx)?;
    register_renderers(lua, ctx.scene)?;
    register_scene(lua, ctx)?;
    register_ui_components(lua, ctx.scene)?;
    reverb_zone::register(lua, ctx.scene)?;
    selectable::register(lua, ctx.scene, ctx.event_system)?;
    shader::register(lua)?;
    shape::register(lua, ctx.scene)?;
    sound::register(lua)?;
    storage::register(lua, ctx.storage)?;
    text::register(lua, ctx.scene, ctx.screen, ctx.video)?;
    texture::register(lua)?;
    time::register(lua, ctx.time)?;
    timer::register(lua, ctx.scene, ctx.timers, ctx.console)?;
    trail::register(lua, ctx.scene)?;
    transform::register(lua, ctx.scene)?;
    tween::register(lua, ctx.scene, ctx.timers)?;
    ui::register(lua, ctx.scene, (ctx.screen, ctx.video), ctx.camera)?;
    ui::register_events(lua, ctx)?;
    video::register(lua, ctx.video, ctx.screen)?;

    for ext in EXTENSIONS.read().map_err(|e| e.to_string())?.iter() {
        ext(lua, ctx)?;
    }
    Ok(())
}

/// Rust halves of namespace functions written in Lua. The Lua half looks each up at call time, never keeping a copy, so a layer above can replace one.
pub const RUST_HALVES: &str = "rusty.rust_halves";

pub(crate) fn rust_halves<'f>(
    lua: &Lua,
    namespace: &str,
    halves: impl IntoIterator<Item = (&'static str, &'f Function)>,
) -> mlua::Result<Table> {
    let table = rust_halves_table(lua, namespace)?;
    for (key, f) in halves {
        table.set(key, f)?;
    }
    Ok(table)
}

fn rust_halves_table(lua: &Lua, namespace: &str) -> mlua::Result<Table> {
    let all: Table = match lua.named_registry_value(RUST_HALVES)? {
        Some(all) => all,
        None => {
            let all = lua.create_table()?;
            lua.set_named_registry_value(RUST_HALVES, &all)?;
            all
        }
    };
    if let Some(table) = all.get::<Option<Table>>(namespace)? {
        return Ok(table);
    }
    let table = lua.create_table()?;
    all.set(namespace, &table)?;
    Ok(table)
}

pub type Extension = fn(&Lua, &ApiCtx) -> Reg;

static EXTENSIONS: RwLock<Vec<Extension>> = RwLock::new(Vec::new());

/// Process-wide; install before the first VM starts: a running VM keeps the surface it has.
pub fn extend(ext: Extension) {
    if let Ok(mut exts) = EXTENSIONS.write() {
        exts.push(ext);
    }
}

fn register_ui_components(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    // sorted: each registrar makes its own global table
    backdrop_filter::register(lua, scene)?;
    canvas_group::register(lua, scene)?;
    image::register(lua, scene)?;
    layout_element::register(lua, scene)?;
    layout_group::register(lua, scene)?;
    mask::register(lua, scene)?;
    rect_mask::register(lua, scene)
}

pub fn put(table: &Table, name: &str, f: mlua::Result<Function>) -> Reg {
    table
        .set(name, f.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn global_table(lua: &Lua, name: &str) -> Result<Table, String> {
    lua.globals().get(name).map_err(|e| e.to_string())
}
