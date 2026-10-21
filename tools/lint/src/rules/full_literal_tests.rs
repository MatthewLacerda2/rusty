use super::{Components, RULE};
use crate::rules::{allowed, scan::Scan, test_code::test_lines, Finding};

const ENTITY: &str = "pub struct Entity {\n    pub id: u32,\n    pub transform: TransformComponent,\n    \
                      pub collider: Option<ColliderComponent>,\n    pub material: Option<MaterialComponent>,\n}\n";
const COLLIDER: &str = "pub struct ColliderComponent {\n    /// The shape.\n    pub shape: Shape,\n    \
                        #[serde(default)]\n    pub is_trigger: bool,\n    pub(crate) active: bool,\n}\n\
                        pub struct MaterialComponent {\n    pub name: String,\n}\n";

fn components() -> Components {
    Components::load([
        ("src/components/entity/mod.rs", ENTITY),
        ("src/components/collider/mod.rs", COLLIDER),
    ])
}

fn check(path: &str, src: &str) -> Vec<Finding> {
    components().check(path, src, &test_lines(path, src, false))
}

fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src).iter().map(|f| f.line).collect()
}

#[test]
fn a_literal_naming_every_field_is_flagged() {
    // The `tests/physics_gravity.rs` shape: every field, multi-line, shorthand too.
    let src = "let c = ColliderComponent {\n    shape: Shape::Cube { half: Vec3::ONE },\n    \
               is_trigger: false,\n    active,\n};\n\
               let d = Some(rusty::components::ColliderComponent { shape, is_trigger: true, active: true });\n";
    assert_eq!(lines("tests/physics/gravity.rs", src), [1, 6]);
    let f = &check("tests/a.rs", src)[0];
    assert_eq!(f.rule, RULE);
    assert!(f.message.contains("all 3 fields"), "{}", f.message);
}

#[test]
fn a_rest_a_constructor_or_a_partial_literal_passes() {
    let src = "let a = ColliderComponent { is_trigger: true, ..Default::default() };\n\
               let b = ColliderComponent::from_mesh_bounds(true, min, max);\n\
               let c = ColliderComponent { shape, active: true, ..default_collider() };\n\
               let ColliderComponent { shape, .. } = c;\n";
    assert!(lines("tests/a.rs", src).is_empty());
}

#[test]
fn only_test_code_counts_and_headers_are_not_literals() {
    let full = "ColliderComponent { shape, is_trigger, active }";
    let prod = format!(
        "pub struct ColliderComponent {{\nfn f() -> ColliderComponent {{\n    {full}\n}}\n"
    );
    assert!(
        lines("src/physics/build.rs", &prod).is_empty(),
        "production code"
    );
    let inline = format!("{prod}#[cfg(test)]\nmod tests {{\n    fn t() {{ {full}; }}\n}}\n");
    assert_eq!(lines("src/physics/build.rs", &inline), [7]);
    assert_eq!(lines("src/physics/build_tests.rs", &prod), [3]);
    let header = "impl Default for ColliderComponent {\nfn d() -> ColliderComponent {\n";
    assert!(lines("tests/a.rs", header).is_empty());
}

#[test]
fn small_components_and_reasoned_exceptions_pass() {
    assert!(lines("tests/a.rs", "MaterialComponent { name }\n").is_empty());
    let src = "// lint: allow(full-literal) every field is what the round-trip compares\n\
               let c = ColliderComponent { shape, is_trigger, active };\n";
    assert_eq!(lines("tests/a.rs", src), [2]);
    assert!(allowed(src, 2, RULE), "`rules::run` applies the allow");
}

#[test]
fn cfg_all_test_is_test_code_and_any_test_is_not() {
    let full = "ColliderComponent { shape, is_trigger, active }";
    let gated = |attr: &str| format!("{attr}\nmod tests {{\n    fn t() {{ {full}; }}\n}}\n");
    let all = gated("#[cfg(all(test, feature = \"dev\"))]");
    assert_eq!(lines("src/physics/build.rs", &all), [3]);
    let any = gated("#[cfg(any(test, feature = \"dev\"))]");
    assert!(lines("src/physics/build.rs", &any).is_empty());
}

#[test]
fn cfg_all_with_test_last_or_wrapped_is_test_code() {
    let full = "ColliderComponent { shape, is_trigger, active }";
    for attr in [
        "#[cfg(all(feature = \"dev\", test))]",
        "#[cfg(all(\n    feature = \"dev\",\n    test,\n))]",
    ] {
        let src = format!("{attr}\nmod tests {{\n    fn t() {{ {full}; }}\n}}\n");
        let line = src.lines().count() - 1;
        assert_eq!(lines("src/physics/build.rs", &src), [line], "{attr}");
    }
    let not = format!("#[cfg(all(feature = \"dev\", not(test)))]\nfn f() {{ {full}; }}\n");
    assert!(lines("src/physics/build.rs", &not).is_empty());
}

#[test]
fn a_gated_item_is_test_code_and_the_lines_below_it_are_not() {
    let full = "ColliderComponent { shape, is_trigger, active }";
    let src = format!(
        "#[cfg(test)]\nuse crate::x::Helper;\nfn f() -> ColliderComponent {{\n    {full}\n}}\n\
         #[cfg(test)]\nfn helper(\n    a: u32,\n) {{\n    {full};\n}}\nfn g() {{ {full}; }}\n\
         #[cfg(test)]\nmod tests {{\n    fn t() {{ {full}; }}\n}}\n"
    );
    assert_eq!(lines("src/physics/build.rs", &src), [10, 15]);
}

#[test]
fn a_gated_fn_with_a_wrapped_signature_is_test_code_to_its_closing_brace() {
    let full = "ColliderComponent { shape, is_trigger, active }";
    let wheres =
        format!("#[cfg(test)]\nfn f<T>(t: T)\nwhere\n    T: Clone,\n{{\n    {full};\n}}\n");
    let returns =
        format!("#[cfg(test)]\nfn f() -> Result<\n    A,\n    B,\n> {{\n    {full}\n}}\n");
    let generics = format!("#[cfg(test)]\nfn f<\n    T: X,\n>(t: T) {{\n    {full};\n}}\n");
    assert_eq!(lines("src/physics/build.rs", &wheres), [6]);
    assert_eq!(lines("src/physics/build.rs", &returns), [6]);
    assert_eq!(lines("src/physics/build.rs", &generics), [5]);
}

#[test]
fn a_file_only_a_gated_mod_declares_is_test_code() {
    let root = std::env::temp_dir().join(format!("lint_full_literal_{}", std::process::id()));
    let ui = root.join("src/ui");
    std::fs::create_dir_all(&ui).unwrap();
    let file = |path: &str, text: &str| std::fs::write(root.join(path), text).unwrap();
    file(
        "src/ui/fixture.rs",
        "fn f() {\n    ColliderComponent { shape, is_trigger, active };\n}\n",
    );
    std::fs::create_dir_all(root.join("src/components/entity")).unwrap();
    file("src/components/entity/mod.rs", ENTITY);
    file("src/components/collider.rs", COLLIDER);
    let found = |decl: &str| {
        file("src/ui/mod.rs", decl);
        let found = super::run(&Scan::at(&root));
        found
            .iter()
            .map(|f| (f.path.clone(), f.line))
            .collect::<Vec<_>>()
    };
    let gated = found("#[cfg(test)]\nmod fixture;\n");
    let plain = found("mod fixture;\n");
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(gated, [("src/ui/fixture.rs".to_string(), 2)]);
    assert!(plain.is_empty(), "{plain:?}");
}
