//! crates/script/src/test_temp.rs — the crate's tests' scratch paths: `rusty_test_support::temp`'s
//! per-test dir, in the stored form a script passes.

/// `name` inside the test's own temp dir in the stored form a script passes
/// (`core::paths::to_slash`), so the same string serves a Lua literal and the
/// test's own reads and assertions (#1937). The root package keeps its twin
/// (`src/test_temp.rs`) for its own tests.
pub(crate) fn lua_path(name: &str) -> String {
    rusty_core::paths::to_slash(&rusty_test_support::temp::dir().join(name))
}
