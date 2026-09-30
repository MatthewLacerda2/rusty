//! src/ui/layout/sizes.rs — the min / preferred / flexible sizes layout reads (#421).
//!
//! Unity's `ILayoutElement` inputs, per axis (0 width, 1 height): **min**, the size
//! an element never shrinks below; **preferred**, the size it asks for; and
//! **flexible**, its relative share of space left over once every sibling has its
//! preferred size. An element's *content* supplies them — its own `LayoutGroup`
//! (from its children), its `Text` (the measured block) and its `Image` (the
//! native texture size); several sources combine by taking the largest. A
//! `LayoutElement` then overrides any axis it sets. Preferred is never below min.
//!
//! Heights depend on widths — wrapped text is taller in a narrower box, and a
//! nested group's rows depend on how wide its children end up — so the height
//! queries take the width the element will have, as Unity runs its horizontal
//! pass before its vertical one.

use glam::Vec2;

use super::group;
use crate::components::{ImageType, LayoutAxisFit};
use crate::ecs::World;
use crate::ui::text::preferred_size;

/// Deeper nesting than this reads as empty — a malformed parent cycle can never
/// recurse the size queries forever.
pub(super) const MAX_DEPTH: u32 = 64;

/// One axis's layout sizes. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sizes {
    pub min: f32,
    pub preferred: f32,
    pub flexible: f32,
}

impl Sizes {
    /// A fixed size: min = preferred = `size`, not flexible.
    pub(super) fn fixed(size: f32) -> Self {
        Self {
            min: size,
            preferred: size,
            flexible: 0.0,
        }
    }

    fn max(self, o: Sizes) -> Sizes {
        Sizes {
            min: self.min.max(o.min),
            preferred: self.preferred.max(o.preferred),
            flexible: self.flexible.max(o.flexible),
        }
    }
}

/// `id`'s layout sizes along `axis`; `width` is the width it will have (read for
/// heights only). Content, then the `LayoutElement` overrides.
pub fn element_sizes(world: &World, id: u32, axis: usize, width: f32, depth: u32) -> Sizes {
    if depth > MAX_DEPTH {
        return Sizes::default();
    }
    let mut s = content_sizes(world, id, axis, width, depth);
    if let Some(le) = world.layout_element(id) {
        let [min, preferred, flexible] = le.overrides(axis);
        s.min = min.unwrap_or(s.min);
        s.preferred = preferred.unwrap_or(s.preferred);
        s.flexible = flexible.unwrap_or(s.flexible);
    }
    s.preferred = s.preferred.max(s.min);
    s
}

/// The sizes `id`'s content asks for: the largest of its group, text and image.
fn content_sizes(world: &World, id: u32, axis: usize, width: f32, depth: u32) -> Sizes {
    let mut s = Sizes::default();
    if let Some(g) = world.layout_group(id) {
        s = s.max(group::group_sizes(world, id, &g, axis, width, depth + 1));
    }
    if let Some(t) = world.text(id) {
        s.preferred = s.preferred.max(preferred_size(&t, width)[axis]);
    }
    if let Some(img) = world.image(id) {
        s.preferred = s.preferred.max(image_preferred(&img)[axis]);
    }
    s
}

/// The size `id` keeps along `axis` when nothing controls it: its content fitter's
/// pick, else its RectTransform's `size_delta` (Unity reads `sizeDelta`, which a
/// fitter writes).
pub(super) fn own_size(world: &World, id: u32, axis: usize, width: f32, depth: u32) -> f32 {
    let fit = world
        .layout_element(id)
        .map_or(LayoutAxisFit::Unconstrained, |le| le.fit(axis));
    match fit {
        LayoutAxisFit::Unconstrained => world
            .rect_transform(id)
            .map_or(0.0, |rt| rt.size_delta[axis]),
        LayoutAxisFit::MinSize => element_sizes(world, id, axis, width, depth).min,
        LayoutAxisFit::PreferredSize => element_sizes(world, id, axis, width, depth).preferred,
    }
}

/// The children a group arranges, in hierarchy order: active, rect-carrying and
/// not `ignore_layout`. The rest keep their own anchors.
pub(super) fn layout_children(world: &World, id: u32) -> Vec<u32> {
    world
        .children(id)
        .into_iter()
        .filter(|&c| world.is_active(c) && world.has_rect_transform(c))
        .filter(|&c| !world.layout_element(c).is_some_and(|le| le.ignore_layout))
        .collect()
}

/// An Image's preferred size (Unity's `Image.preferredWidth/Height`): the texture's
/// native size, or for `Sliced` / `Tiled` the sum of its borders. Zero without a
/// texture, or when the file cannot be read.
fn image_preferred(img: &crate::components::ImageComponent) -> Vec2 {
    let Some(path) = img.texture.as_deref() else {
        return Vec2::ZERO;
    };
    match img.image_type {
        ImageType::Sliced | ImageType::Tiled => {
            Vec2::new(img.border.x + img.border.z, img.border.y + img.border.w)
        }
        ImageType::Simple | ImageType::Filled => super::native::texture_size(path),
    }
}
