//! The in-crate tests' scratch directory: [`dir`] is `rusty_test_support::temp`'s,
//! shared by every workspace member; [`lua_path`] adds the root package's stored-path
//! spelling.

pub(crate) use rusty_test_support::temp::dir;

/// `name` inside [`dir`] in the stored form a script passes
/// (`core::paths::to_slash`), so the same string serves a Lua literal and the
/// test's own reads and assertions (#1937).
/// The integration suite's twin is `tests/temp.rs`'s `lua_path`.
pub(crate) fn lua_path(name: &str) -> String {
    crate::core::paths::to_slash(&dir().join(name))
}
