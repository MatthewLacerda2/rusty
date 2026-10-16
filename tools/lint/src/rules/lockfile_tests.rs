//! `tools/lint` is held to the policy it enforces (#935): std-only, so its
//! `Cargo.lock` names exactly one package, itself. A dependency is a decision, not
//! a drive-by; drop this test only with the issue that makes it.

fn packages(lock: &str) -> usize {
    lock.lines().filter(|l| l.trim() == "[[package]]").count()
}

#[test]
fn tools_lint_has_no_dependencies() {
    let lock = include_str!("../../Cargo.lock");
    assert_eq!(packages(lock), 1, "tools/lint must stay std-only:\n{lock}");
}

#[test]
fn a_second_package_is_counted() {
    let lock = "[[package]]\nname = \"lint\"\n\n[[package]]\nname = \"regex\"\n";
    assert_eq!(packages(lock), 2);
}
