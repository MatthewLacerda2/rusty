//! src/api/sound/from_lua.rs — marshal the `Sound` namespace's Lua arguments into
//! the engine's types (#357).
//!
//! Three conversions, all of them tiny, kept out of the registrar so the namespace
//! file reads as a list of verbs:
//!
//! - a **patch table** → [`Patch`], via the shared [`crate::api::lua_json`]
//!   converter, so the field decoding lives ONCE in the patch's serde derive and a
//!   Lua-authored patch is the same document as one loaded from `.json`;
//! - a **note** → a MIDI number, accepting either the name a score would use
//!   (`"C#4"`) or a raw number;
//! - an **opts table** → [`NoteOpts`], every field optional.
//!
//! The Lua patch shape mirrors the JSON one-to-one:
//!
//! ```lua
//! {
//!   source = { kind = "osc_stack", oscs = { { wave = "saw", detune_cents = -7 } } },
//!   amp    = { a = 0.005, d = 0.1, s = 0.7, r = 0.3 },
//!   filter = { kind = "lowpass", cutoff = 1200, resonance = 0.4 },
//!   fx     = { { fx = "reverb", size = 0.6, damp = 0.5, mix = 0.2 } },
//! }
//! ```

use mlua::{Table, Value};

use crate::api::lua_json::table_to_json;
use crate::soundgen::{parse_note, NoteOpts, Patch};

/// Parse a Lua patch `table` into a [`Patch`].
pub fn patch_from_table(table: &Table) -> Result<Patch, String> {
    let json = table_to_json(table)?;
    Patch::from_json(&serde_json::to_string(&json).map_err(|e| e.to_string())?)
}

/// Resolve a `note` argument to a MIDI number: a name (`"C#4"`, `"Bb3"`) or the
/// number itself.
pub fn note_from_value(note: &Value) -> Result<f32, String> {
    match note {
        Value::String(s) => parse_note(s.to_str().map_err(|e| e.to_string())?),
        Value::Integer(i) => Ok(*i as f32),
        Value::Number(n) => Ok(*n as f32),
        other => Err(format!(
            "note must be a name like \"C#4\" or a MIDI number, got {other:?}"
        )),
    }
}

/// Read the optional `{ duration, velocity, seed }` table, defaulting each field it
/// omits (see [`NoteOpts::default`]).
pub fn opts_from_table(opts: Option<&Table>) -> Result<NoteOpts, String> {
    let mut out = NoteOpts::default();
    let Some(table) = opts else { return Ok(out) };
    if let Some(duration) = get_number(table, "duration")? {
        out.duration = duration as f32;
    }
    if let Some(velocity) = get_number(table, "velocity")? {
        out.velocity = velocity as f32;
    }
    if let Some(seed) = get_number(table, "seed")? {
        out.seed = seed as i64 as u64;
    }
    Ok(out)
}

/// Read an optional numeric field, reporting the key when the value is not a number.
fn get_number(table: &Table, key: &str) -> Result<Option<f64>, String> {
    match table.get::<_, Value>(key).map_err(|e| e.to_string())? {
        Value::Nil => Ok(None),
        Value::Integer(i) => Ok(Some(i as f64)),
        Value::Number(n) => Ok(Some(n)),
        other => Err(format!("opts.{key} must be a number, got {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;
    use crate::soundgen::patch::{FilterKind, Source};

    fn eval_table<'a>(lua: &'a Lua, src: &str) -> Table<'a> {
        lua.load(src).eval().expect("the table evaluates")
    }

    #[test]
    fn parses_a_patch_table_with_every_stage() {
        let lua = Lua::new();
        let table = eval_table(
            &lua,
            r#"return {
                source = { kind = "osc_stack", oscs = {
                    { wave = "saw", detune_cents = -7, gain = 0.5 },
                    { wave = "saw", detune_cents = 7, gain = 0.5, octave = -1 } } },
                amp = { a = 0.005, d = 0.1, s = 0.7, r = 0.3 },
                filter = { kind = "lowpass", cutoff = 1200, resonance = 0.4,
                           env_amount = 2000, adsr = { a = 0.01, d = 0.2, s = 0.3, r = 0.2 } },
                lfo = { rate = 5, depth = 0.5, target = "pitch" },
                fx = { { fx = "delay", time = 0.25, feedback = 0.35, mix = 0.3 } },
            }"#,
        );
        let patch = patch_from_table(&table).expect("patch parses");
        match &patch.source {
            Source::OscStack { oscs } => assert_eq!(oscs.len(), 2),
            other => panic!("wrong source: {other:?}"),
        }
        assert_eq!(patch.filter.expect("filter").kind, FilterKind::Lowpass);
        assert!(patch.lfo.is_some());
        assert_eq!(patch.fx.len(), 1);
    }

    #[test]
    fn reports_an_unknown_or_missing_tag() {
        let lua = Lua::new();
        let bogus = eval_table(&lua, r#"return { source = { kind = "theremin" } }"#);
        assert!(patch_from_table(&bogus).is_err());
        let no_amp = eval_table(&lua, r#"return { source = { kind = "noise" } }"#);
        assert!(patch_from_table(&no_amp).is_err(), "amp is mandatory");
    }

    #[test]
    fn a_note_may_be_a_name_or_a_number() {
        let lua = Lua::new();
        let name: Value = lua.load(r#"return "C#4""#).eval().unwrap();
        let integer: Value = lua.load("return 61").eval().unwrap();
        let number: Value = lua.load("return 61.5").eval().unwrap();
        assert_eq!(note_from_value(&name).unwrap(), 61.0);
        assert_eq!(note_from_value(&integer).unwrap(), 61.0);
        assert_eq!(note_from_value(&number).unwrap(), 61.5);
        let table: Value = lua.load("return {}").eval().unwrap();
        assert!(note_from_value(&table).is_err());
    }

    #[test]
    fn opts_default_field_by_field() {
        let lua = Lua::new();
        assert_eq!(opts_from_table(None).unwrap(), NoteOpts::default());
        let partial = eval_table(&lua, "return { duration = 2.5, seed = 99 }");
        let opts = opts_from_table(Some(&partial)).unwrap();
        assert_eq!(opts.duration, 2.5);
        assert_eq!(opts.seed, 99);
        assert_eq!(opts.velocity, NoteOpts::default().velocity, "untouched");
    }

    #[test]
    fn a_non_numeric_opt_names_the_field_it_came_from() {
        let lua = Lua::new();
        let bad = eval_table(&lua, r#"return { duration = "long" }"#);
        let err = opts_from_table(Some(&bad)).expect_err("not a number");
        assert!(err.contains("duration"), "got {err}");
    }
}
