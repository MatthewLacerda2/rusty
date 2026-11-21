//! How a test starts an engine binary: [`engine_bin!`] names it by its Cargo
//! target and hands back a `Command`. Gated on `dev`, like every test that spawns
//! one.

/// `engine_bin!("session")`: a `Command` for that binary of this package.
macro_rules! engine_bin {
    ($name:literal) => {
        std::process::Command::new(env!(concat!("CARGO_BIN_EXE_", $name)))
    };
}
pub(crate) use engine_bin;
