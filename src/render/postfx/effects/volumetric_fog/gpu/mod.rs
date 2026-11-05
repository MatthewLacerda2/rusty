//! Volumetric fog on a GPU (#1703): a lamp in a dark room gets a halo, a `Baked`
//! one too, a shadowed spot casts a shaft with a dark side; off is off, whatever the
//! volume held; a moved light leaves no trail past the ghost frames, a cut starts
//! over, a resize holds, wild settings stay bounded, and the `Baked` lamps' list is
//! kept, not rebuilt every frame. The sky and glass read the volume, Additive and
//! Multiply surfaces take no air's light, SSR and SSIL add none the fog took, a
//! bright lamp in thick fog stays finite, and the air's ambient is the probes'.

mod blends;
mod bounds;
mod downstream;
mod fixture;
mod halo;
mod history;
mod kept;
mod probes;
mod readers;
mod shaft;
