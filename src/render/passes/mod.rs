//! Self-contained render passes drawn into the HDR target: billboard particles, trail/line ribbons, the shadow pass, the SSAO passes, the editor's selection outline, and the alpha-blended
//! transparent pass.

pub(crate) mod blended;
pub(crate) mod outline;
pub(crate) mod particles;
pub(crate) mod ribbons;
pub(crate) mod shadows;
pub(crate) mod ssao;
pub(crate) mod transparent;
