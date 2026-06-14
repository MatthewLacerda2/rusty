//! src/scene/ — Scene documents: save / load / snapshot (single active scene)
//!
//! No multi-scene / open-world loading — loading a scene REPLACES the World.
//!
//! On-disk format is a plain serde `SceneData` document (entities + their
//! component VALUES + scene settings), kept separate from runtime storage: the
//! hecs World is runtime, `SceneData` is the file. This bridges hecs (which does
//! not serialize a World for free), keeps scene files human-readable/diffable, and
//! is also what the headless harness loads.
//!
//! Principle: scenes store REFERENCES (primitive name, asset path) + values, never
//! baked GPU buffers — those are rehydrated on load (see serialize.rs).
//!
//! Submodules:
//!   serialize — World <-> SceneData
//!   io        — save/load, path + extension, current-scene-path
//!   snapshot  — edit-mode snapshot/restore around Play
//!
//! Status: SCAFFOLD — structure only; not yet implemented.
