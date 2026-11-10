//! What the `Sound` tests share: a Lua state with the namespace registered, a
//! temp path a Lua literal can carry, and a percussive patch.

use mlua::Lua;

use super::register;

/// A temp path a Lua literal can carry: [`crate::test_temp::lua_path`].
pub(super) use crate::test_temp::lua_path as tmp;

/// A percussive noise patch — the shape every impact / footstep SFX takes.
pub(super) const IMPACT_PATCH: &str = r#"{
    source = { kind = "noise" },
    amp = { a = 0.001, d = 0.08, s = 0.0, r = 0.05 },
    filter = { kind = "lowpass", cutoff = 400, resonance = 0.3, env_octaves = 3.5,
               adsr = { a = 0.0, d = 0.06, s = 0.0, r = 0.05 } },
}"#;

pub(super) fn lua_with_sound() -> Lua {
    let lua = Lua::new();
    register(&lua).expect("the namespace registers");
    lua
}
