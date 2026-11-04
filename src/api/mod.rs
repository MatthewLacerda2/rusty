//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (one page each in `docs/api/`, which
//! `tests/api_doc_drift.rs` holds to the live surface) is registered from this tree
//! onto a Lua VM once, when it starts; each function reaches engine state through
//! the VM (`state`, #1709). Layers above the sim add their own through [`extend`]:
//! `dev` installs `Debug`, `Editor`, the provider verbs and the GPU bake verbs that
//! way (#737, `dev::lua_surface`), so the sim surface never imports the platform layer.
//! `scripting` owns the runtime and lifecycle; `api` owns the surface. One surface, three
//! callers — they never drift apart.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

// lint: allow(dir-size) a flat registry: one file per Lua namespace, so the list is the surface's index
// sorted: namespace modules and their helpers; no module reads another's declaration order
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
pub mod decals;
pub mod entity_arg;
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
pub(crate) mod lua_json;
pub mod mask;
pub mod material;
pub mod nav;
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

use crate::scene::Scene;

/// Result alias every namespace registrar returns.
pub type Reg = Result<(), String>;

/// The navigation namespaces: `Navigation` + `NavMeshAgent`, `NavMeshObstacle`,
/// `NavMeshModifierVolume` (#460) and `OffMeshLink`.
fn register_navigation(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table
    nav::register(lua, ctx.scene, ctx.nav)?;
    nav_modifier::register(lua, ctx.scene)?;
    nav_obstacle::register(lua, ctx.scene)?;
    offmesh_link::register(lua, ctx.scene, ctx.nav)
}

/// The scene namespaces: `Scene` (structure) and `Component` (fields by name, #1369).
fn register_scene(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table
    component::register(lua, ctx.scene)?;
    scene::register(lua, ctx.scene, ctx.scene_path)
}

/// Register every namespace onto `lua`, once per VM: plain functions that reach the
/// engine through the cells `ctx` names, attached to `lua` beforehand
/// (`scripting::ApiCells::install`). This is the one place the whole script
/// surface is wired up, shared by gameplay scripts, the console REPL and
/// bot-players.
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar makes its own global table; the ones extending a table
    // (`register_hitscan`, `register_reimport`, `register_events`)
    // sort after the one that made it
    animator::register(lua, ctx.scene, ctx.console)?;
    application::register(lua, ctx.application)?;
    // `Assets` mostly reads no engine cell (it walks the project asset root on each
    // call); only the import-setting setters that re-import the scene's meshes
    // (#831) reach the scene.
    assets::register(lua)?;
    assets::register_reimport(lua, ctx.scene)?;
    audio::register(
        lua,
        ctx.scene,
        ctx.audio,
        ctx.time,
        ctx.camera,
        ctx.application,
    )?;
    camera::register(lua, ctx)?;
    canvas::register(lua, ctx.scene, ctx.screen, ctx.video)?;
    character_controller::register(lua, ctx.scene, ctx.physics)?;
    decals::register(lua, ctx.scene)?;
    graphics::register(lua, ctx.scene, ctx.graphics, ctx.application)?;
    input::register(lua, ctx.input)?;
    joint::register(lua, ctx.scene)?;
    layers::register(lua, ctx.scene)?;
    light::register(lua, ctx.scene, ctx.console)?;
    light_probe_proxy_volume::register(lua, ctx.scene)?;
    lighting::register(lua, ctx.scene)?;
    line::register(lua, ctx.scene)?;
    lod_group::register(lua, ctx.scene)?;
    material::register(lua, ctx.scene)?;
    particle::register(lua, ctx.scene)?;
    physics::register(lua, ctx.scene)?;
    physics::register_hitscan(lua, ctx.scene, ctx.physics)?;
    probe::register(lua, ctx.scene)?;
    ragdoll::register(lua, ctx.scene, ctx.physics)?;
    random::register(lua, ctx.random)?;
    rect_transform::register(lua, ctx.scene, (ctx.screen, ctx.video))?;
    reflection::register(lua, ctx.scene)?;
    register_navigation(lua, ctx)?;
    register_scene(lua, ctx)?;
    register_ui_components(lua, ctx.scene)?;
    reverb_zone::register(lua, ctx.scene)?;
    selectable::register(lua, ctx.scene, ctx.event_system)?;
    // `Shader` (#272) borrows no engine state — it reads a recipe, validates it
    // through naga_oil, and writes a `.wgsl` file — so it is a plain static namespace.
    shader::register(lua)?;
    shape::register(lua, ctx.scene)?;
    // `Sound` (#357) is `Shader`'s story for audio: it reads a patch and writes a
    // `.wav`, so it too registers as a plain static namespace.
    sound::register(lua)?;
    storage::register(lua, ctx.storage)?;
    text::register(lua, ctx.scene, ctx.screen, ctx.video)?;
    // `Texture` (#270) reads a recipe and writes a PNG: a plain static namespace too.
    texture::register(lua)?;
    time::register(lua, ctx.time)?;
    timer::register(lua, ctx.scene, ctx.timers, ctx.console)?;
    trail::register(lua, ctx.scene)?;
    transform::register(lua, ctx.scene)?;
    tween::register(lua, ctx.scene, ctx.timers)?;
    ui::register(lua, ctx.scene, (ctx.screen, ctx.video), ctx.camera)?;
    ui::register_events(lua, ctx)?;
    video::register(lua, ctx.video)?;

    // Last, so an extension can add verbs to a namespace registered above.
    for ext in EXTENSIONS.read().map_err(|e| e.to_string())?.iter() {
        ext(lua, ctx)?;
    }
    Ok(())
}

/// A registrar a layer above the sim adds to the surface: the same shape as
/// [`register`], run after every built-in namespace on every VM.
pub type Extension = fn(&Lua, &ApiCtx) -> Reg;

static EXTENSIONS: RwLock<Vec<Extension>> = RwLock::new(Vec::new());

/// Add `ext` to the surface every later [`register`] call installs (#737). This is how
/// the platform layer contributes bindings without the sim importing it: the arrow
/// runs `dev → api`. Process-wide; callers install once (see `dev::install_api`),
/// before the first VM starts: a VM already running keeps the surface it has.
pub fn extend(ext: Extension) {
    if let Ok(mut exts) = EXTENSIONS.write() {
        exts.push(ext);
    }
}

/// The UI component namespaces that borrow only the scene: the graphic, its
/// group, clips and backdrop (#418, #426, #428), and the layout pair (#421).
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

/// Set a named entry on `table` from a built function value.
pub(crate) fn put(table: &Table, name: &str, f: mlua::Result<Function>) -> Reg {
    table
        .set(name, f.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Fetch an existing global table so we can extend it in place.
pub(crate) fn global_table(lua: &Lua, name: &str) -> Result<Table, String> {
    lua.globals().get(name).map_err(|e| e.to_string())
}
