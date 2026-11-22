// Panic-free sim core: bare `.unwrap()` is denied here, as in the other
// sim modules. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod events;
pub mod layout;
mod screen;
mod size_reader;
pub mod space;
pub mod sway;
pub mod text;

pub use events::EventSystem;
pub use layout::{TextureSizes, UiLayout, UiRect};
pub use screen::ScreenSize;
pub use size_reader::TextureSizeReader;
pub use space::{CanvasSpace, UiPointer, UiView};
pub use sway::SWAY_RECOVERY;
