//! `Debug.Preview` (the headless asset preview) driven through a real [`Session`], so
//! the shipped Lua verb is exercised. With no adapter the verb returns `nil`
//! instead of raising; argument-validation tests run regardless.

use rusty::dev::session::Session;

/// A committed shader, so what a baked `.wgsl` renders is the arm covered end to end.
const SHADER: &str = "engine/shaders/shader.wgsl";

fn session() -> Session {
    Session::new("").expect("session boots in edit mode")
}

fn preview(sess: &Session, asset: &str, name: &str, opts: &str) -> Option<std::path::PathBuf> {
    let out = crate::temp::dir().join("rusty-preview-api").join(name);
    let _ = std::fs::remove_file(&out);
    let call = format!(
        "return Debug.Preview({:?}, {:?}{})",
        asset,
        out.to_string_lossy(),
        opts
    );
    let result = sess.eval(&call).expect("Debug.Preview is callable");
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
    // Fixed framing: agents diff captures, so output must be byte-identical.
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
    let message = format!("{bad_mesh:?}");
    assert!(
        message.contains("unknown preview mesh") && message.contains("plane"),
        "a misspelled mesh must raise, listing every mesh, got {bad_mesh:?}"
    );

    let no_repeats = sess.eval(&format!(
        r#"return Debug.Preview({SHADER:?}, "out.png", {{ repeats = 0 }})"#
    ));
    assert!(
        format!("{no_repeats:?}").contains("repeats must be 1 or more"),
        "zero repeats must raise, got {no_repeats:?}"
    );

    let too_many = sess.eval(&format!(
        r#"return Debug.Preview({SHADER:?}, "out.png", {{ repeats = 100000 }})"#
    ));
    assert!(
        format!("{too_many:?}").contains("at most 1000 repeats"),
        "a count past the renderer's tiling range must raise, got {too_many:?}"
    );
}
