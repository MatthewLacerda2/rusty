//! The time-sliced `EnvironmentQuery` (#2116): `Request` starts a query and hands
//! back its ticket, `Poll` reads it (Unity's polled `pathPending`, as
//! `NavMeshAgent.PathPending` is), `Cancel` drops it, and
//! `Set`/`GetIterationsPerFrame` is the budget every waiting query shares
//! (`ai::query::queue`). `Run` stays the synchronous form.

use mlua::{IntoLuaMulti, Lua, MultiValue, Table};

use super::{answer, read_mode, with_query};
use crate::ai::query::queue::Status;
use crate::ai::query::Progress;
use crate::api::entity_arg::EntityArg;
use crate::api::{put, ApiCtx};

/// Add the time-sliced verbs to the `EnvironmentQuery` table.
pub(super) fn register(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Result<(), String> {
    let c = *ctx;
    put(
        table,
        "Request",
        lua.create_function(
            move |lua, (id, spec, contexts, mode): (EntityArg, Table, Option<Table>, Option<String>)| {
                let mode = read_mode(mode.as_deref(), "Request")?;
                let (owner, progress) = with_query(lua, &c, id, "Request", &spec, contexts, |world, querier, spec| {
                    (querier.entity.unwrap_or(0), Progress::start(world, querier, spec))
                })?;
                let queries = c.queries.get(lua)?;
                c.random.with(lua, |rng| {
                    Ok(queries.borrow_mut().request(owner, progress, mode, &mut rng.borrow_mut()))
                })
            },
        ),
    )?;
    put(
        table,
        "Poll",
        lua.create_function(move |lua, ticket: i64| {
            let ticket = u32::try_from(ticket).unwrap_or(0);
            let status = c.queries.with(lua, |q| Ok(q.borrow_mut().poll(ticket)))?;
            match status {
                Status::Pending => "pending".into_lua_multi(lua),
                Status::Gone => "gone".into_lua_multi(lua),
                Status::Done(item) => {
                    let (found, x, y, z, score) = answer(item);
                    let mut out: MultiValue = (found, x, y, z, score).into_lua_multi(lua)?;
                    out.push_front(mlua::Value::String(lua.create_string("done")?));
                    Ok(out)
                }
            }
        }),
    )?;
    put(
        table,
        "Cancel",
        lua.create_function(move |lua, ticket: i64| {
            let ticket = u32::try_from(ticket).unwrap_or(0);
            c.queries.with(lua, |q| Ok(q.borrow_mut().cancel(ticket)))
        }),
    )?;
    put(
        table,
        "SetIterationsPerFrame",
        lua.create_function(move |lua, n: i64| {
            let n = u32::try_from(n.max(1)).unwrap_or(u32::MAX);
            c.queries.with(lua, |q| {
                q.borrow_mut().set_iterations_per_frame(n);
                Ok(())
            })
        }),
    )?;
    put(
        table,
        "GetIterationsPerFrame",
        lua.create_function(move |lua, ()| {
            c.queries
                .with(lua, |q| Ok(q.borrow().iterations_per_frame()))
        }),
    )?;
    Ok(())
}
