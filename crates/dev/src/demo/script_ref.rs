//! A scenario's `Scene.GetScript` handle acts, noted for a demo and replayed
//! from it (#2120).
//!
//! The handle (`bridge::script_ref`) writes a live script instance's fields and
//! calls its functions in the gameplay VM: `match.over = true`, `bot:Refill(30)`.
//! Neither passes through an API function, so the wrap in `calls` never sees
//! them. Each is noted here as a call of its own, `ScriptRef.Set` or
//! `ScriptRef.Call`, with the entity, the script's name and plain-data values,
//! and [`invoke`] makes it again on that entity's live script of that name. A
//! value that is not plain taints the demo instead, as an API call's does.
//!
//! `ScriptRef` is no Lua namespace: the prefix only names these acts in the
//! file, the way `Harness.` names the harness verbs a demo replays.

use std::cell::RefCell;

use mlua::{Function, MultiValue, Table, Value};

use super::calls::{plain, to_lua};
use rusty_script::scripting::{HostCall, HostJournal, Plain, ScriptManager};

/// The prefix of a handle act's name in a demo.
pub const PREFIX: &str = "ScriptRef.";

/// Note `script.key = value`, written through the handle on entity `id`'s
/// script `name`.
pub fn note_set(
    journal: &RefCell<HostJournal>,
    (id, name): (u32, &str),
    key: &Value,
    value: &Value,
) {
    let act = match (plain(key, 0), plain(value, 0)) {
        (Ok(key), Ok(value)) => call("Set", id, name, [key, value].into_iter()),
        (Err(kind), _) | (_, Err(kind)) => taint("Set", name, kind),
    };
    journal.borrow_mut().push(act);
}

/// Note the call of the script's function `key` with `args`; `with_self` when
/// the first argument was the handle itself (`ref:F(...)`), which `args` leaves out.
pub fn note_call(
    journal: &RefCell<HostJournal>,
    (id, name): (u32, &str),
    (key, with_self): (&str, bool),
    args: &[Value],
) {
    let plain: Result<Vec<Plain>, &str> = args.iter().map(|v| plain(v, 0)).collect();
    let act = match plain {
        Ok(args) => {
            let head = [Plain::Str(key.into()), Plain::Bool(with_self)];
            call("Call", id, name, head.into_iter().chain(args))
        }
        Err(kind) => taint("Call", name, kind),
    };
    journal.borrow_mut().push(act);
}

fn call(verb: &str, id: u32, name: &str, rest: impl Iterator<Item = Plain>) -> HostCall {
    let head = [Plain::Int(id.into()), Plain::Str(name.into())];
    HostCall::Call {
        function: format!("{PREFIX}{verb}"),
        args: head.into_iter().chain(rest).collect(),
    }
}

fn taint(verb: &str, name: &str, kind: &str) -> HostCall {
    HostCall::Taint(format!(
        "a handle on script '{name}' ({verb}) was given a {kind}"
    ))
}

/// Make the handle act `verb` (`Set` or `Call`) with `args` on `game`'s live
/// script, as the scenario's handle did.
pub fn invoke(game: &ScriptManager, verb: &str, args: &[Plain]) -> Result<(), String> {
    let fail = |what: &str| format!("{PREFIX}{verb}: {what}");
    let (id, name, rest) = match args {
        [Plain::Int(id), Plain::Str(name), rest @ ..] => (*id, name.as_str(), rest),
        _ => return Err(fail("an entity and a script name")),
    };
    let id = u32::try_from(id).map_err(|_| fail("an entity id out of range"))?;
    let made = game.with_script(id, name, |glua, t| match (verb, rest) {
        ("Set", [key, value]) => t.raw_set(to_lua(glua, key)?, to_lua(glua, value)?),
        ("Call", [Plain::Str(key), Plain::Bool(with_self), args @ ..]) => {
            let f: Function = t.raw_get(key.as_str())?;
            let mut values = MultiValue::new();
            if *with_self {
                values.push_back(Value::Table(Table::clone(t)));
            }
            for arg in args {
                values.push_back(to_lua(glua, arg)?);
            }
            f.call::<MultiValue>(values).map(drop)
        }
        _ => Err(mlua::Error::RuntimeError(
            "not a handle act a demo replays".into(),
        )),
    });
    match made.map_err(|e| fail(&e.to_string()))? {
        Some(()) => Ok(()),
        None => Err(fail(&format!("entity {id} has no live script '{name}'"))),
    }
}
