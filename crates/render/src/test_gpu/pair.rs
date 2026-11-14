//! crates/render/src/test_gpu/pair.rs — two headless renderers alive in one test (#1844).
//!
//! The budget (`render::setup::budget`) gives a test one slot at a time, so a test
//! comparing two renderers takes both together here instead of holding one while
//! waiting for the next.

use crate::Renderer;

/// A headless renderer and the slot for a second one, taken together, or `None`
/// (skip) as for [`headless_or_skip`](super::headless_or_skip).
///
/// **The one way a test keeps two renderers alive at once** (#1844), such as one
/// kept across edits beside a fresh one built after each. Two `headless_or_skip`
/// calls would hold one budget slot while waiting for the next, which deadlocks
/// under plain `cargo test` the moment two such tests run together; the budget
/// panics on that second call and points here.
pub fn headless_pair_or_skip(width: u32, height: u32) -> Option<(Renderer, HeadlessPair)> {
    let pair = headless_pair();
    let first = pair.build(width, height)?;
    Some((first, pair))
}

/// Both slots of a pair with no renderer built yet, for a test whose two
/// renderers the engine builds (a capture host beside a bake): while it lives,
/// every headless renderer built on the test's thread takes one of its slots.
/// Hold it from the test's first line.
pub fn headless_pair() -> HeadlessPair {
    HeadlessPair(crate::setup::budget::acquire_headless_pair())
}

/// Two headless slots held by one test ([`headless_pair_or_skip`], [`headless_pair`]):
/// at most two renderers alive on them at once, however they are built.
pub struct HeadlessPair(crate::setup::budget::Pair);

impl HeadlessPair {
    /// A renderer on the pair's slot. The first one built proved an adapter here,
    /// so this one is expected to build too.
    ///
    /// # Panics
    ///
    /// When the adapter is gone, or two renderers already live on the pair.
    pub fn renderer(&self, width: u32, height: u32) -> Renderer {
        self.build(width, height)
            .expect("the pair's first renderer found an adapter")
    }

    fn build(&self, width: u32, height: u32) -> Option<Renderer> {
        pollster::block_on(Renderer::new_headless_with(self.0.permit(), width, height))
    }
}
