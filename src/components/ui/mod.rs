//! src/components/ui/ — the in-game UI's first-class components (#414).
//!
//! The uGUI primitives, each pure authoring data: [`CanvasComponent`] (a UI root),
//! [`RectTransformComponent`] (2D placement inside the parent rect), the
//! [`ImageComponent`] graphic, the [`TextComponent`] label, the
//! [`CanvasGroupComponent`] (subtree alpha and interaction flags) and the
//! [`RectMaskComponent`] clip, and the [`SelectableComponent`] (an interactive
//! element, #420). Computed state — rects,
//! batches — lives in `ui::UiLayout` and the renderer, never here. The model is
//! recorded in `docs/ui.md`.

pub mod canvas;
pub mod canvas_group;
pub mod image;
pub mod rect_mask;
pub mod rect_transform;
pub mod selectable;
pub mod text;

pub use canvas::{CanvasComponent, CanvasRenderMode};
pub use canvas_group::CanvasGroupComponent;
pub use image::{FillMethod, FillOrigin, ImageComponent, ImageType};
pub use rect_mask::RectMaskComponent;
pub use rect_transform::RectTransformComponent;
pub use selectable::{NavigationMode, SelectableComponent, SelectableTransition, SelectionState};
pub use text::{TextAlignment, TextComponent, TextOverflow};
