//! Which `session-mcp` the engine's `.mcp.json` entries run (#2024): an entry already
//! on a build of this checkout keeps it, a fresh one takes the running build's profile.

use serde_json::json;

use super::agent::{is_checkout_binary, merge, session_binary, SERVERS};
use super::*;

fn native(slashed: &str) -> PathBuf {
    PathBuf::from(crate::core::paths::native_string(slashed))
}

/// [`session_binary`] for `/engine`, run from `exe`, with only `built` on disk.
fn pick(exe: Option<&str>, built: &[&str]) -> PathBuf {
    let built: Vec<PathBuf> = built.iter().map(|b| native(b)).collect();
    let exe = exe.map(native);
    session_binary(&native("/engine"), exe.as_deref(), |p| {
        built.iter().any(|b| b == p)
    })
}

#[test]
fn a_fresh_entry_runs_the_running_builds_profile() {
    let release = "/engine/target/release/session-mcp";
    let exe = Some("/engine/target/release/rusty");
    assert_eq!(
        pick(exe, &[release, "/engine/target/debug/session-mcp"]),
        native(release)
    );
    let custom = "/engine/target/fast/session-mcp";
    assert_eq!(
        pick(Some("/engine/target/fast/rusty"), &[custom]),
        native(custom)
    );
}

#[test]
fn a_profile_without_the_binary_falls_back_to_one_that_has_it() {
    let debug = "/engine/target/debug/session-mcp";
    let release = "/engine/target/release/session-mcp";
    assert_eq!(
        pick(Some("/engine/target/release/rusty"), &[debug]),
        native(debug)
    );
    assert_eq!(
        pick(Some("/engine/target/debug/rusty"), &[release]),
        native(release)
    );
    assert_eq!(pick(Some("/elsewhere/rusty"), &[release]), native(release));
    // A test binary sits in `target/debug/deps/`: no profile of its own.
    let deps = Some("/engine/target/debug/deps/rusty-1234");
    assert_eq!(
        pick(deps, &[]),
        native(debug),
        "nothing built: today's path"
    );
    assert_eq!(pick(None, &[]), native(debug));
}

#[test]
fn only_a_session_mcp_under_this_checkouts_target_is_kept() {
    let engine = native("/engine");
    let kept = |c: &str| is_checkout_binary(&native(c).to_string_lossy(), &engine);
    assert!(kept("/engine/target/release/session-mcp"));
    assert!(kept(
        "/engine/target/x86_64-unknown-linux-gnu/release/session-mcp"
    ));
    assert!(
        !kept("/old/engine/target/debug/session-mcp"),
        "another checkout"
    );
    assert!(!kept("/engine/target/release/rusty"), "not the bridge");
    assert!(!kept("/engine/session-mcp"), "not a build");
    assert!(!is_checkout_binary("stale", &engine));
}

#[test]
fn a_kept_entry_gets_only_its_args_refreshed() {
    let release = native("/engine/target/release/session-mcp");
    let release = release.to_string_lossy();
    let debug = native("/engine/target/debug/session-mcp");
    let mut config = json!({ "mcpServers": {
        SERVERS[0]: { "command": release, "args": ["--attach", "old"] },
        SERVERS[1]: { "command": "/moved/target/debug/session-mcp", "args": [] },
    }});
    merge(&mut config, "/games/horde", &native("/engine"), &debug).unwrap();
    let servers = &config["mcpServers"];
    assert_eq!(
        servers[SERVERS[0]],
        json!({ "command": release, "args": ["--attach"] })
    );
    let headless =
        json!({ "command": debug.to_string_lossy(), "args": ["--project", "/games/horde"] });
    assert_eq!(
        servers[SERVERS[1]], headless,
        "another checkout's entry is rewritten"
    );
}
