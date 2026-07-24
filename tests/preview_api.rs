//! Integration coverage for `Debug.Preview` (#353) — the headless asset preview,
//! driven through a real [`Session`] so the test exercises the *shipped* surface (the
//! Lua verb an agent actually calls), not the Rust function behind it.
//!
//! The render half needs a GPU or software adapter, which CI boxes may not have. Every
//! test here therefore treats "no adapter" as a skip, exactly as the verb does: it
//! returns `nil` instead of raising, and the assertions that don't involve pixels
//! (argument validation) run regardless.
#![cfg(feature = "dev")]

use rusty::dev::session::Session;

/// A committed shader, so the arm that matters most for #353 (see what a baked `.wgsl`
/// actually renders) is the one covered end to end.
const SHADER: &str = "assets/shaders/shader.wgsl";

fn session() -> Session {
    Session::new("").expect("session boots in edit mode")
}

/// Render `asset` to `name` under a scratch dir; `None` when the box has no adapter.
fn preview(sess: &Session, asset: &str, name: &str, opts: &str) -> Option<std::path::PathBuf> {
    let out = std::env::temp_dir().join("rusty-preview-api").join(name);
    let _ = std::fs::remove_file(&out);
    let call = format!(
        "return Debug.Preview({:?}, {:?}{})",
        asset,
        out.to_string_lossy(),
        opts
    );
    let result = sess.eval(&call).expect("Debug.Preview is callable");
    // The verb returns the written path on success and nil when no adapter exists.
    (result.trim() != "nil" && !result.trim().is_empty()).then_some(out)
}

#[test]
fn previewing_a_shader_writes_the_png_it_returns() {
    let sess = session();
    let Some(out) = preview(&sess, SHADER, "shader.png", ", { resolution = 128 }") else {
        eprintln!("no GPU adapter — skipping the render half");
        return;
    };
    let image = image::open(&out).expect("a readable PNG landed at the returned path");
    assert_eq!((image.width(), image.height()), (128, 128));
}

#[test]
fn framing_is_deterministic_so_two_shots_are_comparable() {
    // The whole point of fixed framing (#353): an agent diffs before/after captures of
    // the same asset, so the same input must produce byte-identical output.
    let sess = session();
    let (Some(a), Some(b)) = (
        preview(&sess, SHADER, "det-a.png", ", { resolution = 64 }"),
        preview(&sess, SHADER, "det-b.png", ", { resolution = 64 }"),
    ) else {
        eprintln!("no GPU adapter — skipping the render half");
        return;
    };
    let (a, b) = (
        std::fs::read(&a).expect("first shot"),
        std::fs::read(&b).expect("second shot"),
    );
    assert_eq!(a, b, "two captures of one asset must be identical");
}

#[test]
fn a_bad_path_or_mesh_raises_instead_of_returning_a_blank_picture() {
    let sess = session();
    let missing = sess.eval(r#"return Debug.Preview("nope.glb", "out.png")"#);
    assert!(
        missing.is_err() || missing.as_deref().unwrap_or("").contains("no such asset"),
        "a missing asset must surface as an error, got {missing:?}"
    );

    let bad_mesh = sess.eval(&format!(
        r#"return Debug.Preview({SHADER:?}, "out.png", {{ mesh = "spere" }})"#
    ));
    assert!(
        bad_mesh.is_err()
            || bad_mesh
                .as_deref()
                .unwrap_or("")
                .contains("unknown preview mesh"),
        "a misspelled mesh must surface as an error, got {bad_mesh:?}"
    );
}
