//! src/ui/ — rusty's in-game UI (Unity uGUI, adapted; #414, #417).
//!
//! UI is GameObjects: a `Canvas` entity is a UI root and its descendants carry a
//! `RectTransform` beside their mandatory Transform. This module holds the
//! GPU-free, deterministic half of the UI that runs in the sim:
//!
//! - [`ScreenSize`] — the sim input the layout is a function of (the game view's
//!   pixel size, or the video resolution when headless);
//! - [`layout`] — the pass that turns canvases and rect transforms into rectangles
//!   in canvas reference units and screen pixels ([`UiLayout`], [`UiRect`]);
//! - [`text`] — fonts, rich text, measuring, wrapping, overflow and auto-size for
//!   the `Text` component (#419);
//! - [`events`] — the event system: hit-testing, pointer and focus callbacks, and
//!   the `Selectable` states (#420).
//!
//! The render pass (#418) draws these; layout groups (#421) build on them. The model is recorded in `docs/ui.md`.
//!
//! Allowed deps: components, ecs, core, scene (tests), glam, ab_glyph / ttf-parser (fonts). Never `render` / `editor` / `wgpu` /
//! `egui` — the layout runs headless (the direction guard enforces it).

// Panic-free sim core (#195): bare `.unwrap()` is denied here, as in the other
// sim modules. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod events;
pub mod layout;
mod screen;
pub mod text;

pub use events::EventSystem;
pub use layout::{UiLayout, UiRect};
pub use screen::ScreenSize;
