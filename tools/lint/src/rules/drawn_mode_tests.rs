use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_pass_that_reads_the_scenes_light_mode() {
    let src = "fn sun(scene: &Scene, id: u32, l: &LightComponent) -> bool {\n    \
               scene.light_mode(id, l).renders_live(false)\n}\n";
    assert_eq!(found("crates/render/src/passes/shadows/frame.rs", src), [2]);
    let f = check("crates/render/src/passes/a.rs", src, &[]);
    assert!(f[0].message.contains("drawn_mode"), "says what to read");
}

#[test]
fn passes_the_entry_point_the_bake_crates_and_other_names() {
    let src = "    scene.light_mode(id, light)\n";
    assert!(found("crates/render/src/clusters/unbaked.rs", src).is_empty());
    assert!(found("crates/dev/src/bake/lightmap/mod.rs", src).is_empty());
    assert!(found("crates/lighting/src/lightmap/lights/mod.rs", src).is_empty());
    let drawn = "    let m = crate::clusters::drawn_mode(scene, id, &l);\n";
    assert!(found("crates/render/src/draw/lighting.rs", drawn).is_empty());
    let longer = "    scene.baked_light_mode(id)\n";
    assert!(found("crates/render/src/draw/lighting.rs", longer).is_empty());
    assert!(found("crates/render/src/a.rs", "// scene.light_mode(id, l)\n").is_empty());
    let test = "#[cfg(test)]\nmod tests {\n    fn t() { scene.light_mode(id, l); }\n}\n";
    assert!(found("crates/render/src/a.rs", test).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(drawn-mode) the bake capture reads the light's own mode\n\
               let m = scene.light_mode(id, l);\n";
    let f = check("crates/render/src/a.rs", src, &[]);
    assert!(allowed(src, f[0].line, RULE));
}
