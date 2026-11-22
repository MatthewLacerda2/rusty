pub(crate) fn is_gpu_test(name: &str, integration: bool) -> bool {
    let test_fn = name.rsplit("::").next().unwrap_or(name);
    (integration && name.starts_with("gpu::")) || test_fn.starts_with("gpu_")
}

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
         GPU test, so nextest will not cap it with the others (the `gpu` or `gpu-macos` \
         test group in .config/nextest.toml). Start the test function's name with `gpu_`, or move an \
         integration test under tests/gpu/. See docs/testing.md."
    );
}

#[cfg(test)]
mod tests {
    use super::is_gpu_test;

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
