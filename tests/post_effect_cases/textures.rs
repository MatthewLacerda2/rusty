//! The completeness gate's composite-texture cases: a declared texture not bound,
//! not passed to its own function, passed to another call or out of place.

use super::engine;
use rusty::render::postfx::completeness::{check, TextureUse};

#[test]
fn a_composite_texture_not_bound_or_not_passed_is_flagged() {
    let mut l = engine();
    assert!(
        (l.textures.iter()).any(|t| t.effect == "lens_flare" && t.name == "lens_flare_image"),
        "the lens flare declares a composite texture"
    );
    l.composite = (l.composite.lines())
        .filter(|line| !line.contains("var lens_flare_image"))
        .map(|line| line.replace(", bloom_dirt", ""))
        .collect::<Vec<_>>()
        .join("\n");
    let problems = check(&l).join("\n");
    for needle in [
        "`lens_flare`'s composite texture `lens_flare_image` is not bound at group 1",
        "`bloom`'s composite texture `bloom_dirt` is not passed to its function",
    ] {
        assert!(
            problems.contains(needle),
            "not flagged: {needle}\n{problems}"
        );
    }
}

#[test]
fn a_composite_texture_passed_to_another_call_or_out_of_place_is_flagged() {
    let mut l = engine();
    let swap = |line: &str| match line.starts_with("    c = ") {
        true => (line
            .replace("bloom_dirt", "@")
            .replace("lens_flare_image", "bloom_dirt"))
        .replace('@', "lens_flare_image"),
        false => line.to_string(),
    };
    l.composite = l.composite.lines().map(swap).collect::<Vec<_>>().join("\n");
    // Two textures of one function, passed in the wrong order.
    for (position, name) in ["twin_a", "twin_b"].into_iter().enumerate() {
        l.textures.push(TextureUse {
            effect: "twin".into(),
            name: name.into(),
            call: "    c = twin::hdr(c, ".into(),
            position,
        });
    }
    l.composite += "\n    c = twin::hdr(c, f, twin_b, twin_a);";
    let problems = check(&l).join("\n");
    for name in ["bloom_dirt", "lens_flare_image", "twin_a", "twin_b"] {
        let needle = format!("composite texture `{name}` is not passed to its function");
        assert!(
            problems.contains(&needle),
            "not flagged: {name}\n{problems}"
        );
    }
}
