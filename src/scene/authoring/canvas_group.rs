//! src/scene/authoring/canvas_group.rs — Shared CanvasGroup-authoring ops (#418).
//!
//! The ONE place the engine mutates an entity's first-class `CanvasGroupComponent`.
//! The editor's Canvas Group card and the Lua `CanvasGroup.*` namespace both route
//! every write through these; the alpha is kept in `[0, 1]`.
//!
//! Allowed deps: components (the `CanvasGroupComponent` data). Pure.

use crate::components::CanvasGroupComponent;

/// Set the subtree opacity, clamped to `[0, 1]`.
pub fn set_alpha(g: &mut CanvasGroupComponent, alpha: f32) {
    g.alpha = alpha.clamp(0.0, 1.0);
}

/// Set whether the subtree's selectables accept input.
pub fn set_interactable(g: &mut CanvasGroupComponent, interactable: bool) {
    g.interactable = interactable;
}

/// Set whether the subtree's graphics can be hit by the pointer.
pub fn set_blocks_raycasts(g: &mut CanvasGroupComponent, blocks: bool) {
    g.blocks_raycasts = blocks;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut g = CanvasGroupComponent::default();
        set_alpha(&mut g, 1.5);
        assert_eq!(g.alpha, 1.0);
        set_alpha(&mut g, -0.5);
        assert_eq!(g.alpha, 0.0);
        set_interactable(&mut g, false);
        set_blocks_raycasts(&mut g, false);
        assert!(!g.interactable && !g.blocks_raycasts);
    }
}
