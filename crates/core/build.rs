//! Release builds only: a debug build asks git at run time, because a stamp (and the
//! `HEAD` rerun) would rebuild the crate on every commit. `RUSTY_ENGINE_ORDER` is
//! left out in a shallow clone, which can't count.
//! `cargo:rustc-env` reaches only the package whose script prints it.

use std::{env, path::Path};

#[expect(
    dead_code,
    reason = "the build script uses only the git helpers of the shared file"
)]
#[path = "src/project/engine/commit.rs"]
mod commit;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/project/engine/commit.rs");
    if env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stamp = commit::git_commit(root).unwrap_or_else(|| commit::UNKNOWN_COMMIT.into());
    println!("cargo:rustc-env=RUSTY_ENGINE_COMMIT={stamp}");
    if let Some(order) = commit::git_order(root) {
        println!("cargo:rustc-env=RUSTY_ENGINE_ORDER={order}");
    }

    // A rerun-if-changed path that doesn't exist reruns every build, so name only
    // files git has (a packed ref has no loose file; packed-refs covers it).
    let head_ref = commit::git(root, &["symbolic-ref", "-q", "HEAD"]);
    let head = head_ref.as_deref().unwrap_or("HEAD");
    let watched = ["HEAD", head, "packed-refs", "shallow"];
    for name in watched {
        if let Some(path) = commit::git(root, &["rev-parse", "--git-path", name]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
