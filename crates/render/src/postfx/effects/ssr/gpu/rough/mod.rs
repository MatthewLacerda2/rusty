//! Glossy SSR (#1736, #2028, #2158): a rough surface's reflection, blurred by its
//! GGX lobe, soft at its outline as inside it, and a mirror's sharp.

mod edge;
mod glossy;
mod outline;
mod resolve;
