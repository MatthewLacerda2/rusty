//! Glossy SSR (#1736, #2028, #2158): a rough surface's reflection, blurred by its
//! GGX lobe, soft at its outline as inside it, and a mirror's sharp.

mod contact;
mod edge;
mod fading;
mod glossy;
mod jitter;
mod outline;
mod patch;
mod resolve;
mod shelf;
