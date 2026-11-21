use glam::Vec2;

use crate::ui::mesh::to_ndc;
use rusty_components::UiShader;
use rusty_ui::UiRect;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiShade {
    pub(crate) shader: UiShader,
    pub(crate) rect: [[f32; 2]; 3],
    pub(crate) size: Vec2,
}

pub(in crate::ui) fn shade(
    shader: &Option<UiShader>,
    rect: &UiRect,
    screen: Vec2,
) -> Option<UiShade> {
    let shader = shader.as_ref().filter(|s| !s.name.is_empty())?;
    let [bl, tl, _, br] = rect.corners;
    let [o, x, y] = [bl, br, tl].map(|c| Vec2::from(to_ndc(c, rect, screen)));
    Some(UiShade {
        shader: shader.clone(),
        rect: [o.to_array(), (x - o).to_array(), (y - o).to_array()],
        size: rect.rect.1 * rect.scale_factor,
    })
}
