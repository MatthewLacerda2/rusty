use std::cell::RefCell;

use crate::NavigationGraph;

#[derive(Default)]
pub(crate) struct FloodCache(RefCell<Option<BakeState>>);

pub(super) struct BakeState {
    key: (u64, usize),
    exits: Vec<u64>,
}

impl BakeState {
    fn new(nav: &NavigationGraph, key: (u64, usize)) -> Self {
        let mut exits = vec![0u64; nav.spans.len().div_ceil(64)];
        let mut mark = |s: Option<crate::SpanRef>| {
            if let Some(s) = s {
                exits[s.index as usize / 64] |= 1 << (s.index % 64);
            }
        };
        for link in nav.offmesh_links() {
            mark(nav.link_span(link.from));
            if link.bidirectional {
                mark(nav.link_span(link.to));
            }
        }
        Self { key, exits }
    }

    pub(super) fn has_exit(&self, index: u32) -> bool {
        self.exits
            .get(index as usize / 64)
            .is_some_and(|w| w & (1 << (index % 64)) != 0)
    }
}

impl FloodCache {
    /// Run `query` on the state for `nav` as it is now baked, made afresh first
    /// when a bake came since. A query nested inside another runs on fresh state.
    pub(super) fn with<R>(
        &self,
        nav: &NavigationGraph,
        query: impl FnOnce(&mut BakeState) -> R,
    ) -> R {
        let key = (nav.bake_generation, nav.spans.len());
        match self.0.try_borrow_mut() {
            Ok(mut state) => {
                let state = match &mut *state {
                    Some(s) if s.key == key => s,
                    slot => slot.insert(BakeState::new(nav, key)),
                };
                query(state)
            }
            Err(_) => query(&mut BakeState::new(nav, key)),
        }
    }
}
