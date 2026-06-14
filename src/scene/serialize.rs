//! src/scene/serialize.rs — World <-> SceneData
//!
//! Converts between the runtime hecs World and the serde `SceneData` document:
//!   to_scene(&World)      -> SceneData   (read component values out, by entity)
//!   from_scene(SceneData) -> World       (spawn entities, attach components)
//!
//! Holds the registry of serializable component types (the explicit list of what a
//! scene persists). On load it rehydrates non-serialized data: primitive meshes
//! from `primitive_type`, model meshes from their asset path, collider world AABBs,
//! etc. GPU buffers are never stored, only rebuilt.
//!
//! Allowed deps: ecs, components.
//! Status: SCAFFOLD — structure only; not yet implemented.
