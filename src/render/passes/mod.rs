//! Self-contained render passes drawn into the HDR target: box-projector decals,
//! billboard particles, the shadow pass, the SSAO passes, and the alpha-blended
//! transparent pass.

pub(crate) mod decals;
pub(crate) mod decals_draw;
pub(crate) mod particles;
pub(crate) mod shadows;
pub(crate) mod ssao;
pub(crate) mod transparent;
