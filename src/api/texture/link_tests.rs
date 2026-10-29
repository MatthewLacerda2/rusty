//! Linked params from Lua (#1494): a bad link is refused before anything is
//! written, and `Stats` reads a node that only drives a param like any other.

use mlua::{Lua, Table};

use super::register;

/// A Lua VM with `Texture` registered and `OUT` a path to bake to.
fn lua() -> (Lua, std::path::PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    let out = crate::test_temp::dir().join("linked.png");
    lua.globals().set("OUT", out.to_str().unwrap()).unwrap();
    (lua, out)
}

/// `Texture.Bake` of a noise warped by itself, `warp`'s fields spliced in.
fn bake_err(warp: &str, extra: &str) -> String {
    let (lua, out) = lua();
    let src = format!(
        r#"return Texture.Bake({{ resolution = 16, nodes = {{
             {{ id = "n", op = "noise", kind = "fbm", scale = 4 {extra} }},
             {{ id = "w", op = "warp", inputs = {{"n", "n"}}, {warp} }} }} }}, OUT, "data")"#
    );
    let err = lua.load(src).eval::<String>().unwrap_err().to_string();
    assert!(!out.exists(), "a refused recipe wrote {out:?}: {err}");
    err
}

#[test]
fn a_link_on_a_whole_count_is_refused_naming_the_params_that_link() {
    let err = bake_err("strength = 0.1", r#", octaves = { node = "w" }"#);
    assert!(
        err.contains(r#"param "octaves" takes no link; noise links gain"#),
        "{err}"
    );
}

#[test]
fn a_link_to_an_unknown_node_is_refused() {
    let err = bake_err(r#"strength = { node = "nope" }"#, "");
    assert!(
        err.contains("node 'w' links strength to unknown node 'nope'"),
        "{err}"
    );
}

#[test]
fn a_cycle_through_a_link_is_refused() {
    let err = bake_err("strength = 0.1", r#", gain = { node = "w" }"#);
    assert!(err.contains("cycle"), "{err}");
}

#[test]
fn stats_reads_a_node_that_only_drives_a_param() {
    let (lua, out) = lua();
    let stats: Table = lua
        .load(
            r#"return Texture.Stats({ resolution = 32, nodes = {
                 { id = "k", op = "constant", color = {0.5, 0.5, 0.5, 1} },
                 { id = "n", op = "noise", kind = "fbm", scale = 4 },
                 { id = "g", op = "gamma", gamma = { node = "k", max = 2 }, inputs = {"n"} } } })"#,
        )
        .eval()
        .unwrap();
    let mean = |id: &str| -> f32 {
        let node: Table = stats.get(id).unwrap();
        node.get::<Table>("mean").unwrap().get(1).unwrap()
    };
    assert_eq!(mean("k"), 0.5);
    // Gamma 1 (0 + 2 × 0.5) leaves the noise as it is.
    assert_eq!(mean("g"), mean("n"));
    assert!(!out.exists());
}
