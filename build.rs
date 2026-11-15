//! Build script: stamp a `dev` build with the engine content it was built with.
//!
//! A built binary reads `engine/` (shaders, fonts, bundled scripts) from its
//! checkout at run time, so a checkout or a pull under a kept binary silently pairs
//! its code with another tree's shaders. A `dev` build carries the digest of the
//! `engine/` it was built with as `RUSTY_ENGINE_CONTENT`; `rusty::init` registers it
//! and warns once when the folder on disk differs
//! (`core::project::engine_content`). `rerun-if-changed=engine` re-stamps on any
//! change under the folder, so a cargo build is never stale; a hidden file the
//! digest skips (a swap file, `.DS_Store`) re-runs it too, with an unchanged stamp.
//!
//! It is the root package's script, not `rusty-core`'s: a changed stamp rebuilds
//! every target of the package that prints it, and only the root package (the
//! binaries, the re-exports, the integration tests) reads it, so a shader edit
//! never rebuilds the crates. A build without `dev` (the shipped `player`) stamps
//! nothing and never watches the folder.

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
    // A directory is scanned whole: any file added, removed or written under it.
    println!("cargo:rerun-if-changed=engine");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    match digest::digest(&root.join("engine")) {
        Ok(content) => println!("cargo:rustc-env=RUSTY_ENGINE_CONTENT={content}"),
        // Unstamped, the binary cannot check its engine content: say so at build time.
        Err(why) => println!("cargo:warning=engine/ was not stamped ({why})"),
    }
}
