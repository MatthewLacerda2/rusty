//! src/components/ui/ — the in-game UI's first-class components (#414).
//!
//! The uGUI primitives, each pure authoring data: [`CanvasComponent`] (a UI root),
//! [`RectTransformComponent`] (2D placement inside the parent rect), the
//! [`ImageComponent`] graphic, the [`TextComponent`] label, the texture-free
//! [`ShapeComponent`] graphic and the blend / gradient look they share (#425), the
//! [`CanvasGroupComponent`] (subtree alpha and interaction flags) and the
//! [`RectMaskComponent`] clip, the [`SelectableComponent`] (an interactive
//! element, #420), and the [`LayoutGroupComponent`] / [`LayoutElementComponent`]
//! pair that arranges children automatically (#421). Computed state — rects,
//! batches — lives in `ui::UiLayout` and the renderer, never here. The model is
//! recorded in `docs/ui.md`.

pub mod canvas;
pub mod canvas_group;
pub mod image;
pub mod layout_element;
pub mod layout_group;
pub mod look;
pub mod rect_mask;
pub mod rect_transform;
pub mod selectable;
pub mod shape;
pub mod text;

pub use canvas::{CanvasComponent, CanvasRenderMode, CanvasSway};
pub use canvas_group::CanvasGroupComponent;
pub use image::{FillMethod, FillOrigin, ImageComponent, ImageType};
pub use layout_element::{LayoutAxisFit, LayoutElementComponent};
pub use layout_group::{LayoutConstraint, LayoutCorner, LayoutGroupComponent, LayoutKind};
pub use look::{GradientKind, GradientStop, UiBlend, UiGradient, MAX_GRADIENT_STOPS};
pub use rect_mask::RectMaskComponent;
pub use rect_transform::{RectTransformComponent, WorldAnchor};
pub use selectable::{NavigationMode, SelectableComponent, SelectableTransition, SelectionState};
pub use shape::{ShapeComponent, ShapeCorner, ShapeGlow, ShapeKind, ShapeShadow};
pub use text::{TextAlignment, TextComponent, TextOverflow};
