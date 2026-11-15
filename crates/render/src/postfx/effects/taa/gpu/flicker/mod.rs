//! #2228 on lavapipe: a still view under TAA stays still where some jitter offsets
//! give a patch a very different value. The case: a satin bronze ceiling over a
//! small, very bright bulb in a dark room, seen from below with SSR on; on some
//! offsets the reflection's blur reaches the bulb's hits over a patch and on others
//! it doesn't (`postfx/ssr.wgsl`'s `blur_radius`), and the patch is its own 3×3,
//! so URP's clamp hands each swing to the output; the camera's anti-flicker holds
//! it. What the scene is: `room`; how a run is measured: `measure`; what the
//! anti-flicker leaves alone (a lone flash on a calm ceiling, a moving cube's
//! trail): `change`; a hue change at equal luma: `hue`; what it costs (a repeated
//! flash, a lone flash on a blinking ceiling, TAAU): `cost`.

mod change;
mod cost;
mod hue;
mod measure;
mod room;
mod still;
