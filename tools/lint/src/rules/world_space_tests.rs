use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_system_that_decomposes_a_parent_matrix() {
    let src = "fn aim(scene: &Scene, id: u32) -> Quat {\n\
               let parent = scene.world.local_to_world(p);\n\
               let (_, rot, _) = parent.to_scale_rotation_translation();\n\
               rot\n}\n";
    assert_eq!(found("src/physics/aim.rs", src), [3]);
}

#[test]
fn flags_the_retired_walks_and_a_transform_folded_over_ancestors() {
    assert_eq!(
        found("src/a.rs", "let m = scene.compute_world_matrix(id);\n"),
        [1]
    );
    assert_eq!(found("src/a.rs", "let m = scene.world_matrix(id);\n"), [1]);
    assert_eq!(
        found("src/a.rs", "let m = world.parent_world_matrix(id);\n"),
        [1]
    );
    let fold = "let m = world\n    .ancestors(id)\n    .fold(Mat4::IDENTITY, |m, a| {\n        world.transform(a).unwrap().to_matrix() * m\n    });\n";
    assert_eq!(found("src/a.rs", fold), [2]);
}

#[test]
fn passes_the_world_api_membership_walks_and_owners() {
    let api = "fn aim(scene: &Scene, id: u32) -> Quat {\n    scene.world.world_rotation(id)\n}\n";
    assert!(found("src/physics/aim.rs", api).is_empty());
    let member = "if self.world.ancestors(p).any(|a| a == id) {\n    return Err(e);\n}\n";
    assert!(found("src/scene/scene.rs", member).is_empty());
    assert!(found("src/a.rs", "let world_matrix = m;\nfoo(world_matrix);\n").is_empty());
    let split = "let (s, r, t) = m.to_scale_rotation_translation();\n";
    assert!(found("src/ecs/world_space/mod.rs", split).is_empty());
    assert!(found("src/components/transform.rs", split).is_empty());
    assert!(found("tests/ik/aim.rs", split).is_empty());
    assert!(found("src/a.rs", "// m.to_scale_rotation_translation()\n").is_empty());
    let test = "#[cfg(test)]\nmod tests {\n    fn t() { m.to_scale_rotation_translation(); }\n}\n";
    assert!(found("src/a.rs", test).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(world-space) converts the file's bind pose, not the scene\nlet (s, r, t) = m.to_scale_rotation_translation();\n";
    let f = check("src/asset/a.rs", src, &[]);
    assert!(allowed(src, f[0].line, RULE));
}
