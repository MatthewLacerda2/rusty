//! src/api/environment_query/ — the `EnvironmentQuery` namespace (#1892):
//! Unreal's Environment Query System, so a script can ask "where should I
//! stand?". `Run` answers one spot, `RunAll` the ranked list. The query itself is
//! [`crate::ai::query`]; this directory reads the Lua spec into it (`spec`,
//! `measures`, `fields`) and resolves the names it uses (`contexts`). `sliced` is
//! the time-sliced form (#2116): `Request` / `Poll` / `Cancel` and its budget.

mod contexts;
mod entry;
mod fields;
mod measures;
mod sliced;
mod spec;

#[cfg(test)]
mod purpose_tests;
#[cfg(test)]
mod tests;

use mlua::{Lua, Table, Value};

use super::{put, ApiCtx, Reg};
use crate::ai::query::{self, Anchor, Item, QuerySpec, QueryWorld, RunMode};
use crate::api::entity_arg::EntityArg;
use crate::scene::game_camera_from_scene;
use contexts::Contexts;

/// Register the `EnvironmentQuery` namespace onto `lua`.
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let c = *ctx;
    put(
        &table,
        "Run",
        lua.create_function(
            move |lua, (id, spec, contexts, mode): (EntityArg, Table, Option<Table>, Option<String>)| {
                let mode = read_mode(mode.as_deref(), "Run")?;
                let ranked = run(lua, &c, id, "Run", &spec, contexts)?;
                let picked = c.random.with(lua, |rng| Ok(query::pick(&ranked, mode, &mut rng.borrow_mut())))?;
                Ok(answer(picked))
            },
        ),
    )?;
    put(
        &table,
        "RunAll",
        lua.create_function(
            move |lua, (id, spec, contexts): (EntityArg, Table, Option<Table>)| {
                let ranked = run(lua, &c, id, "RunAll", &spec, contexts)?;
                let list = lua.create_table()?;
                for (i, item) in ranked.iter().enumerate() {
                    let t = lua.create_table()?;
                    t.set("x", item.position.x)?;
                    t.set("y", item.position.y)?;
                    t.set("z", item.position.z)?;
                    t.set("score", item.score)?;
                    list.set(i + 1, t)?;
                }
                Ok(Value::Table(list))
            },
        ),
    )?;
    sliced::register(lua, &table, ctx)?;
    lua.globals()
        .set("EnvironmentQuery", table)
        .map_err(|e| e.to_string())
}

/// `Run`'s and `Poll`'s answer: `found, x, y, z, score`.
fn answer(picked: Option<Item>) -> (bool, f32, f32, f32, f32) {
    match picked {
        Some(Item { position: p, score }) => (true, p.x, p.y, p.z, score),
        None => (false, 0.0, 0.0, 0.0, 0.0),
    }
}

/// The pick mode a `verb` call names: `best` when left out.
fn read_mode(name: Option<&str>, verb: &str) -> mlua::Result<RunMode> {
    match name {
        None => Ok(RunMode::Best),
        Some(name) => RunMode::parse(name).ok_or_else(|| {
            let names = RunMode::names();
            mlua::Error::RuntimeError(format!(
                "EnvironmentQuery.{verb}: unknown mode '{name}' (expected {names})"
            ))
        }),
    }
}

/// Resolve the querier and contexts, read the spec, and run it: the ranked items.
fn run(
    lua: &Lua,
    c: &ApiCtx,
    id: EntityArg,
    verb: &str,
    spec: &Table,
    contexts: Option<Table>,
) -> mlua::Result<Vec<Item>> {
    with_query(lua, c, id, verb, spec, contexts, |world, querier, spec| {
        query::run(world, querier, &spec)
    })
}

/// Resolve the querier and contexts and read the spec, then hand `f` the world
/// the query reads, the querier and the spec.
fn with_query<R>(
    lua: &Lua,
    c: &ApiCtx,
    id: EntityArg,
    verb: &str,
    spec: &Table,
    contexts: Option<Table>,
    f: impl FnOnce(&QueryWorld, Anchor, QuerySpec) -> R,
) -> mlua::Result<R> {
    let id = id.live("EnvironmentQuery", verb)?;
    let scene = c.scene.get(lua)?;
    let scene = scene.borrow();
    let querier = contexts::entity(&scene, id, &format!("EnvironmentQuery.{verb}: querier"))?;
    let contexts = Contexts::read(&scene, querier, contexts)?;
    let spec = spec::read(spec, &contexts)?;
    let (nav, physics, camera) = (c.nav.get(lua)?, c.physics.get(lua)?, c.camera.get(lua)?);
    let (screen, video) = (c.screen.get(lua)?, c.video.get(lua)?);
    let (nav, physics) = (nav.borrow(), physics.borrow());
    let view = game_camera_from_scene(&camera.borrow(), &scene);
    let world = QueryWorld {
        scene: &scene,
        nav: &nav,
        physics: physics.as_ref(),
        view: &view,
        screen: screen.borrow().pixels(&video.borrow()),
    };
    Ok(f(&world, querier, spec))
}
