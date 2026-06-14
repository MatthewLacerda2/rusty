//! src/dev/screenshot.rs — Offscreen render -> PNG (the agent's eyes)
//!
//! The ONLY place the dev layer touches the renderer. Creates a wgpu device with no
//! window surface, renders one frame into an offscreen colour texture, copies it
//! back, and writes a PNG (via the `image` crate, already a dependency). Lets an
//! agent literally SEE a frame and judge lighting / SSR / shadows against the
//! CS1.6 -> FEAR -> Trepang2 visual bar.
//!
//! Runtime caveat: needs a GPU or software adapter (e.g. lavapipe) in the container.
//! The rest of the dev layer (Step/StepUntil/state) needs no GPU at all.
//!
//! Allowed deps: render (headless path), api.
//! Status: SCAFFOLD — structure only; not yet implemented.
