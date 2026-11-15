//! #2228 on lavapipe: a still view under TAA stays still where some jitter offsets
//! give a patch a very different value. The case: a satin bronze ceiling over a
//! small, very bright bulb in a dark room, seen from below with SSR on; on some
//! offsets the reflection's blur reaches the bulb's hits over a patch and on others
//! it doesn't (`postfx/ssr.wgsl`'s `blur_radius`), and the patch is its own 3×3,
//! so URP's clamp hands each swing to the output. What the scene is and how a run
//! is measured: `room`; what must not get worse (a lone flash, a moving cube's
//! trail): `change`.

mod change;
mod room;
mod still;
