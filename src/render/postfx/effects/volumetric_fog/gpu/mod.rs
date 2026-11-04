//! Volumetric fog on a GPU (#1703): a lamp in a dark room gets a halo, a `Baked`
//! one too, a shadowed spot casts a shaft with a dark side; off is off, whatever the
//! volume held; a moved light leaves no trail past the ghost frames, a cut starts
//! over, a resize holds, wild settings stay bounded, and the `Baked` lamps' list is
//! kept, not rebuilt every frame.

mod downstream;
mod fixture;
mod halo;
mod history;
mod kept;
mod shaft;
