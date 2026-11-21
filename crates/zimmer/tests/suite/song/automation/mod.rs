//! A value that moves across the song.
//!
//! What is asserted here is the **mechanism**, not that an automated song came
//! out: the value at a given beat is the one the curve says, each easing bends
//! a segment its own way, a beat outside the written span clamps rather than
//! extrapolating, and a curve parked at the number a track was already written
//! at renders the identical samples. A suite that only asked whether a build
//! got louder somewhere would pass on a curve read backwards, at the wrong
//! tempo, or half as fast.

mod curve;
mod document;
mod faders;
mod fitting;
mod layers;
mod refusals;
mod sending;
mod setup;
mod sweeps;
