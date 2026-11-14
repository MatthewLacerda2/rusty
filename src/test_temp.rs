//! The in-crate tests' scratch directory: [`dir`] and [`lua_path`] are
//! `rusty_test_support::temp`'s, shared by every workspace member.

pub(crate) use rusty_test_support::temp::dir;
/// Only the `dev` layer's tests take it here.
#[cfg(feature = "dev")]
pub(crate) use rusty_test_support::temp::lua_path;
