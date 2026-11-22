//! Stamps a `dev` build with the digest of the `engine/` it was built with, so
//! `rusty::init` warns when a checkout pairs the binary with another tree's
//! shaders (`core::project::engine_content`).
//!
//! It is the root package's script, not `rusty-core`'s: a changed stamp rebuilds
//! every target of the package printing it, and only the root package reads it.

use std::{env, path::Path};

#[path = "crates/core/src/project/engine/digest.rs"]
mod digest;

fn main() {
    // Without this line cargo reruns the script on any change in the package.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=crates/core/src/project/engine/digest.rs");
    if env::var_os("CARGO_FEATURE_DEV").is_none() {
        return;
    }
    println!("cargo:rerun-if-changed=engine");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    match digest::digest(&root.join("engine")) {
        Ok(content) => println!("cargo:rustc-env=RUSTY_ENGINE_CONTENT={content}"),
        Err(why) => println!("cargo:warning=engine/ was not stamped ({why})"),
    }
}
