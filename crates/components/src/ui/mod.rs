//! The in-game UI's first-class components, pure authoring data: computed rects and batches
//! live in `ui::UiLayout` and the renderer. The model is `docs/ui.md`.

pub mod backdrop;
pub mod image;
pub mod layout;
pub mod look;
pub mod mask;
pub mod rect_mask;
pub mod selectable;
pub mod shader;
pub mod shape;
pub mod text;

pub use backdrop::BackdropFilterComponent;
pub use image::{FillMethod, FillOrigin, ImageComponent, ImageType};
pub use layout::canvas::{CanvasComponent, CanvasRenderMode, CanvasSway};
pub use layout::canvas_group::CanvasGroupComponent;
pub use layout::element::{LayoutAxisFit, LayoutElementComponent};
pub use layout::group::{LayoutConstraint, LayoutCorner, LayoutGroupComponent, LayoutKind};
pub use layout::rect_transform::{RectTransformComponent, WorldAnchor};
pub use look::{GradientKind, GradientStop, UiBlend, UiGradient, MAX_GRADIENT_STOPS};
pub use mask::MaskComponent;
pub use rect_mask::RectMaskComponent;
pub use selectable::{NavigationMode, SelectableComponent, SelectableTransition, SelectionState};
pub use shader::UiShader;
pub use shape::{ShapeComponent, ShapeCorner, ShapeGlow, ShapeKind, ShapeShadow};
pub use text::{TextAlignment, TextComponent, TextOverflow};
