//! `Debug.PreviewMaterial` (#404): a named material asset renders headlessly, whole —
//! its base colour, and its authored surface shader with the runtime param values
//! stored on it. Driven through a real [`Session`], so this is the verb an agent calls.
//! The render half skips without an adapter (the verb returns `nil`); the
//! unknown-name check runs regardless.

use rusty::dev::session::Session;

fn session() -> Session {
    Session::new("").expect("session boots in edit mode")
}

/// Preview `name` at 64² on the sphere; the centre pixel's RGB, or `None` without
/// an adapter.
fn centre(sess: &Session, name: &str, file: &str) -> Option<[u8; 3]> {
    let out = crate::temp::dir().join("rusty-preview-material").join(file);
    let _ = std::fs::remove_file(&out);
    let call = format!(
        "return Debug.PreviewMaterial({name:?}, {:?}, {{ resolution = 64 }})",
        out.to_string_lossy()
    );
    let result = sess.eval(&call).expect("Debug.PreviewMaterial is callable");
    if result.trim() == "nil" || result.trim().is_empty() {
        return None;
    }
    let image = image::open(&out).expect("a readable PNG").to_rgb8();
    assert_eq!(image.dimensions(), (64, 64));
    Some(image.get_pixel(32, 32).0)
}

#[test]
fn an_unknown_material_name_raises_by_name() {
    let sess = session();
    let err = sess.eval(r#"return Debug.PreviewMaterial("no_such_mat", "out.png")"#);
    let text = match err {
        Err(e) => e.to_string(),
        Ok(s) => s,
    };
    assert!(text.contains("no_such_mat"), "the error names it: {text}");
}

#[test]
fn a_defined_asset_previews_in_its_own_colour() {
    let sess = session();
    sess.eval(r#"Material.DefineAsset("red", { base_color = {1, 0, 0}, roughness = 1 })"#)
        .expect("define red");
    let Some([r, g, b]) = centre(&sess, "red", "red.png") else {
        eprintln!("no GPU adapter — skipping the render half");
        return;
    };
    assert!(
        r > 2 * g.max(b) && r > 40,
        "red asset renders red, got {r},{g},{b}"
    );
}

#[test]
fn the_authored_shader_draws_with_its_stored_params() {
    // A hit-flash shader at amount 1 replaces the lit colour with the flash colour:
    // only a preview that uses both the shader *and* the values stored on the
    // material turns the default white material green.
    let sess = session();
    let name = format!("test_preview_flash_{}", std::process::id());
    sess.eval(&format!(
        r#"Shader.Bake({{ pass = "surface", name = {name:?}, blocks = {{ {{ id = "hit_flash" }} }} }})
           local id = Scene.CreateEntity("E")
           Material.SetShader(id, {name:?})
           MAT = "entity_" .. id .. "_material"
           Material.SetAssetShaderParam(MAT, "hit_flash.color", {{0, 1, 0}})
           Material.SetAssetShaderParam(MAT, "hit_flash.amount", 1)"#
    ))
    .expect("bake and assign the flash shader");
    let key = sess.eval("return MAT").expect("material key");
    let pixel = centre(&sess, key.trim(), "flash.png");
    for ext in ["wgsl", "params.json"] {
        let _ = std::fs::remove_file(format!("project/assets/shaders/{name}.{ext}"));
    }
    let Some([r, g, b]) = pixel else {
        eprintln!("no GPU adapter — skipping the render half");
        return;
    };
    assert!(
        g > 2 * r.max(b) && g > 40,
        "flashed material renders green, got {r},{g},{b}"
    );
}
