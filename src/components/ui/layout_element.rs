//! src/components/ui/layout_element.rs — LayoutElement: layout sizes + content fit (#421).
//!
//! Unity's `LayoutElement` and `ContentSizeFitter` in one component. A layout group
//! sizes each child from three numbers per axis — **min** (never smaller),
//! **preferred** (the size it asks for) and **flexible** (its share of any space
//! left over) — which normally come from the child's content: a `Text`'s measured
//! block, an `Image`'s native texture size, a nested group's children. Each
//! `Some` field here overrides that axis; `None` keeps the content's value.
//! `ignore_layout` takes the element out of its parent group entirely.
//!
//! `horizontal_fit` / `vertical_fit` are the content-size fitter: they size this
//! element's own rect to its min or preferred size along that axis, around its
//! pivot — a text box that grows with its string, a list that grows with its
//! items. The fitter lives here rather than as a third component because it reads
//! the same sizes (recorded in `docs/ui.md`). Pure authoring data.

use serde::{Deserialize, Serialize};

/// What a content fitter sizes one axis to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAxisFit {
    /// Leave the RectTransform's size alone.
    #[default]
    Unconstrained,
    /// The content's min size.
    MinSize,
    /// The content's preferred size.
    PreferredSize,
}

/// Layout sizes and content fitting for one UI element. See the module docs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutElementComponent {
    /// Skip this element in its parent's layout group (it keeps its own anchors).
    pub ignore_layout: bool,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub preferred_width: Option<f32>,
    pub preferred_height: Option<f32>,
    pub flexible_width: Option<f32>,
    pub flexible_height: Option<f32>,
    /// Content fitter along the width.
    pub horizontal_fit: LayoutAxisFit,
    /// Content fitter along the height.
    pub vertical_fit: LayoutAxisFit,
}

impl LayoutElementComponent {
    /// The `(min, preferred, flexible)` overrides along `axis` (0 width, 1 height).
    pub fn overrides(&self, axis: usize) -> [Option<f32>; 3] {
        if axis == 0 {
            [self.min_width, self.preferred_width, self.flexible_width]
        } else {
            [self.min_height, self.preferred_height, self.flexible_height]
        }
    }

    /// The content fitter along `axis`.
    pub fn fit(&self, axis: usize) -> LayoutAxisFit {
        [self.horizontal_fit, self.vertical_fit][axis]
    }
}
