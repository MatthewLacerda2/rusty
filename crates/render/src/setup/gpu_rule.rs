//! crates/render/src/setup/gpu_rule.rs — which tests are GPU tests, decided by name (#484).
//!
//! Under cargo-nextest every test runs in its own process, so the in-process
//! [`budget`](super::budget) no longer bounds how many renderers exist at once. The
//! nextest equivalent is the `gpu` test group in `.config/nextest.toml`, and a test
//! group selects tests by **name** — it cannot see which tests build a renderer. So
//! the name carries it:
//!
//! - an integration test that renders lives under `tests/gpu/` (path `gpu::…`);
//! - an in-crate test that renders has a function name starting with `gpu_`.
//!
//! A rule a filter depends on drifts the moment someone forgets it, so it is checked
//! where renderers are made: in debug builds, a test thread that builds a headless
//! renderer without a GPU name panics and says why. libtest (and nextest, which
//! runs it) names each test's thread after the test, which is what makes this work.

/// Whether libtest's full test name (`module::path::test_fn`) marks a GPU test.
///
/// `integration` says the test runs in the integration binary (`tests/main.rs`),
/// nextest's `kind(test)`: only there does a `gpu::` path count, since this crate's
/// own unit tests sit under its `gpu` module and are named `gpu::…` too.
pub(crate) fn is_gpu_test(name: &str, integration: bool) -> bool {
    let test_fn = name.rsplit("::").next().unwrap_or(name);
    (integration && name.starts_with("gpu::")) || test_fn.starts_with("gpu_")
}

/// Panic when a test thread builds a headless renderer without a GPU test name.
///
/// Test threads are recognised by a `::` in the thread name — every test in this
/// crate sits in a module — and no engine thread is named that way. Debug builds
/// only: tests always are, and a ship build has no tests to police.
pub(crate) fn check_current_thread() {
    if !cfg!(debug_assertions) {
        return;
    }
    let thread = std::thread::current();
    let Some(name) = thread.name() else { return };
    // `cfg!(test)` holds only in this crate's own unit-test binary. Every other
    // test binary is taken for the integration one: no other crate's tests sit in a
    // top-level `gpu` module, so the overstatement selects nothing.
    // lint: allow(cfg-test-switch) it tells this crate's own test binary from the rest, which is the point
    let integration = !cfg!(test);
    assert!(
        !name.contains("::") || is_gpu_test(name, integration),
        "test `{name}` builds a headless renderer but its name does not mark it as a \
         GPU test, so nextest will not cap it with the others (the `gpu` test group in \
         .config/nextest.toml). Start the test function's name with `gpu_`, or move an \
         integration test under tests/gpu/. See docs/testing.md."
    );
}

#[cfg(test)]
mod tests {
    use super::is_gpu_test;

    /// The nextest filterset for [`is_gpu_test`], exactly as `.config/nextest.toml` has it.
    const NEXTEST_FILTER: &str = "(kind(test) & test(/^gpu::/)) | test(/(^|::)gpu_[^:]*$/)";

    #[test]
    fn integration_gpu_module_and_gpu_prefixed_fns_are_gpu_tests() {
        assert!(is_gpu_test(
            "gpu::fxaa_screenshot::fxaa_softens_edges",
            true
        ));
        assert!(is_gpu_test(
            "view::view_tests::gpu_resize_tracks_the_new_size",
            false
        ));
        assert!(is_gpu_test("test_gpu::tests::gpu_adapter_present", false));
        assert!(is_gpu_test(
            "gpu::shaders::tests::gpu_compiles_every_shader",
            false
        ));
    }

    #[test]
    fn a_gpu_word_elsewhere_in_the_path_is_not_enough() {
        // This crate's own `gpu` module: a unit test there is a CPU test by its path.
        assert!(!is_gpu_test(
            "gpu::shaders::tests::all_shaders_compose",
            false
        ));
        assert!(!is_gpu_test(
            "gpu::textures::mips::tests::halves_each_level",
            false
        ));
        assert!(!is_gpu_test("test_gpu::tests::adapter_present", false));
        assert!(!is_gpu_test("gpu_utils::tests::packs_rows", true));
        assert!(!is_gpu_test("physics::tests::gpu", true));
    }

    /// The Rust predicate and the nextest filter are one rule written twice; this pins
    /// the second copy so editing either alone fails here.
    #[test]
    fn nextest_config_uses_the_same_filter() {
        let config = include_str!("../../../../.config/nextest.toml");
        let line = format!("filter = '{NEXTEST_FILTER}'");
        assert!(
            config.lines().any(|l| l.trim() == line),
            "`.config/nextest.toml` must select the gpu test group with `{line}`, the \
             filter render::setup::gpu_rule::is_gpu_test mirrors"
        );
    }
}
