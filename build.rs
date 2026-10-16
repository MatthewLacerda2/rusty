//! Build script: stamp the engine commit into **release** builds only (#853, #931).
//!
//! A shipped binary has no checkout to ask, so a release build carries its commit as
//! `RUSTY_ENGINE_COMMIT`, which a project's `project.rusty` records. A debug build
//! stamps nothing and asks git at run time (`src/core/project/commit.rs`): a stamped
//! value, and the `HEAD` it reruns on, are part of the crate's fingerprint, so every
//! commit, checkout and rebase would rebuild the whole crate once per feature set.
//!
//! The scripting-API reference (`docs/api/`) is no longer assembled here: the drift
//! gates and the MCP resource read it at run time (`scripting::api_doc`), so a doc
//! edit reruns tests, never the compiler.

use std::{env, path::Path};

#[expect(
    dead_code,
    reason = "the build script uses only the git helpers of the shared file"
)]
#[path = "src/core/project/commit.rs"]
mod commit;

fn main() {
    // Without this line cargo reruns the script on any change in the package.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/core/project/commit.rs");
    if env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stamp = commit::git_commit(root).unwrap_or_else(|| commit::UNKNOWN_COMMIT.into());
    println!("cargo:rustc-env=RUSTY_ENGINE_COMMIT={stamp}");

    // Rerun when HEAD moves: `HEAD` itself (a checkout) and the ref it points to (a
    // commit). A rerun-if-changed path that doesn't exist reruns every build, so only
    // name the files git really has (a packed ref has no loose file; packed-refs
    // covers it). `-dirty` is as fresh as the last commit or checkout, not the last edit.
    let head_ref = commit::git(root, &["symbolic-ref", "-q", "HEAD"]);
    let watched = ["HEAD", head_ref.as_deref().unwrap_or("HEAD"), "packed-refs"];
    for name in watched {
        if let Some(path) = commit::git(root, &["rev-parse", "--git-path", name]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
