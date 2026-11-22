use rusty_components::CanvasGroupComponent;
use rusty_scene::rules::sanitize::unit;

pub fn set_alpha(g: &mut CanvasGroupComponent, alpha: f32) {
    g.alpha = unit(alpha);
}

pub fn set_interactable(g: &mut CanvasGroupComponent, interactable: bool) {
    g.interactable = interactable;
}

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
