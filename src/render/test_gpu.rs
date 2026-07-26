//! src/render/test_gpu.rs — the one place a test asks for a GPU (#366).
//!
//! Every in-crate GPU test used to open with the same incantation:
//!
//! ```ignore
//! let Some(mut renderer) = pollster::block_on(Renderer::new_headless(64, 64)) else {
//!     return; // no adapter
//! };
//! ```
//!
//! Sixteen copies of a decision that has to be identical everywhere — *what counts as
//! "no GPU here", and what a test does about it* — is a rule with sixteen chances to
//! drift. It lives here instead.
//!
//! Integration tests (`tests/*.rs`) do not use this: they reach the renderer through
//! the dev layer (`screenshot::capture`, `Debug.Preview`, the probe bakes), which owns
//! its own skip handling. The budget in [`super::setup::budget`] covers both, because
//! it sits inside the constructor rather than in this helper.

use crate::render::Renderer;

/// A headless renderer, or `None` when this machine has no GPU or software adapter.
///
/// `None` means **skip, don't fail**: the Linux CI job has no adapter at all, so every
/// GPU test returns early there, while macOS (real Metal) and Windows (WARP) actually
/// run them. A test that cannot tolerate being skipped does not belong on the GPU —
/// pin the rule it cares about with an adapter-free unit test as well, so it is
/// enforced on every platform rather than two of three.
///
/// The returned renderer holds a slot in the headless budget until it is dropped, so
/// callers should let it fall out of scope promptly rather than parking it in a
/// long-lived static.
pub(crate) fn headless_or_skip(width: u32, height: u32) -> Option<Renderer> {
    pollster::block_on(Renderer::new_headless(width, height))
}
