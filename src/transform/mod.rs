//! src/transform/mod.rs — Transform component (the one MANDATORY component)
//!
//! Like Unity's Transform: every scene entity has exactly ONE and it cannot be
//! removed. Spawning a scene object always attaches a Transform — it is NOT an
//! optional component the way Mesh/Rigidbody/Light/etc. are. Holds
//! position/rotation/scale and produces the local-to-world matrix.
//!
//! Unity: Transform.
//! Allowed deps: glam.
//! Status: SCAFFOLD — structure only; not yet implemented.
