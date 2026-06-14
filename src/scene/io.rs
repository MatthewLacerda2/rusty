//! src/scene/io.rs — Scene file I/O
//!
//! save_to_file / load_from_file plus path handling. Single active scene: load
//! REPLACES the current World (no multi-scene).
//!
//! Editor integration:
//!   - EditorState.current_scene_path: Option<String> — Save writes back HERE
//!     (not a hardcoded demo path); Load (incl. double-click in the assets browser)
//!     sets it.
//!   - one standardised scene extension (e.g. `.scene`, JSON inside).
//!
//! Default scene (there is always at least one):
//!   - Authored/checked in at  assets/scenes/default.scene  (TRACKED).
//!   - Seeded into  project/scenes/  on boot, the same way bot.lua is seeded
//!     today, because /project/ is gitignored runtime workspace.
//!   - Contents = the bot-chase demo: floor, Player, Enemy (chases via bot.lua),
//!     obstacle walls, a directional "sun" light, and a silly non-pitch-white
//!     skybox. This replaces the 130-line procedural demo currently built in
//!     main.rs, de-hardcoding startup and letting the harness boot any scene file.
//!
//! Allowed deps: scene::serialize, ecs.
//! Status: SCAFFOLD — structure only; not yet implemented.
