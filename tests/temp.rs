//! A scratch directory owned by one test (#629, #709): [`dir`] is
//! `rusty_test_support::temp`'s, the one helper every workspace member's tests and
//! this suite share (its module doc says why a test never writes under a bare
//! `std::env::temp_dir()`). The suite adds the spellings its tests pass around.

use std::path::PathBuf;

pub use rusty_test_support::temp::{dir, lua_path};

/// `name` inside [`dir`].
pub fn file(name: &str) -> PathBuf {
    dir().join(name)
}

/// [`file`] as a string: [`lua_path`] under the name these tests import.
pub use rusty_test_support::temp::lua_path as path;
