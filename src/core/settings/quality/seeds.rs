//! The presets a new project is seeded with (#1522): `Low`, `Medium` and `High`,
//! each a bundle of every effect's three levels. `Medium`'s `Medium` level is the
//! old Medium tier's numbers and `High`'s the old High tier's, so a volume at its
//! default level costs what it did (motion blur's 32 taps are the old default
//! camera's). Every row moves between its levels (#1716): a test fails on one whose
//! three levels are equal, unless it names the row with a reason.

use super::{
    levels, QualityValues, ShadowmaskMode, SsaoQuality, SsilQuality, SsrQuality,
    VolumetricFogQuality,
};

const fn ssao(samples: u32, resolution_divisor: u32) -> SsaoQuality {
    SsaoQuality {
        samples,
        resolution_divisor,
    }
}

/// SSIL's gather: Godot 4's levels run 4 to 16 samples at half resolution, which
/// is where every level but the `High` preset's `High` stays (#1750's budget).
const fn ssil(samples: u32, resolution_divisor: u32) -> SsilQuality {
    SsilQuality {
        samples,
        resolution_divisor,
    }
}

/// The march HDRP's quality levels name: its *Max Ray Steps* (Low 16, Medium 32,
/// High 64), each step shorter as the count grows, so each also reaches further
/// (8, 12, 16 m); 128 steps reach 25.6 m. HDRP traces a depth pyramid in screen
/// space, so its reach is the screen; this march steps in world space.
const fn march(steps: u32, step_length: f32) -> SsrQuality {
    SsrQuality { steps, step_length }
}

/// Volumetric fog's froxel grid (#1703): UE's default is 160×90×64, the `Medium`
/// preset's `High`; each level below roughly halves the froxels.
const fn froxels(resolution: u32, slices: u32) -> VolumetricFogQuality {
    VolumetricFogQuality { resolution, slices }
}

/// The seeded `Low` preset: an iGPU floor.
pub(super) fn low() -> QualityValues {
    QualityValues {
        ssao: levels(ssao(4, 2), ssao(6, 2), ssao(8, 2)),
        ssil: levels(ssil(4, 2), ssil(6, 2), ssil(8, 2)),
        ssr: levels(march(16, 0.5), march(16, 0.5), march(32, 0.375)),
        bloom_resolution_divisor: levels(4, 4, 2),
        depth_of_field_resolution_divisor: levels(4, 2, 2),
        motion_blur_samples: levels(4, 8, 16),
        volumetric_fog: levels(froxels(64, 32), froxels(80, 48), froxels(120, 64)),
        ui_blur_divisor: 4,
        shadowmask_mode: ShadowmaskMode::Shadowmask,
        gpu_occlusion_culling: true,
    }
}

/// The seeded `Medium` preset, and the engine's values when a game defines none.
pub(super) fn medium() -> QualityValues {
    QualityValues {
        ssao: levels(ssao(6, 2), ssao(8, 2), ssao(16, 1)),
        ssil: levels(ssil(6, 2), ssil(8, 2), ssil(12, 2)),
        ssr: levels(march(16, 0.5), march(32, 0.375), march(64, 0.25)),
        bloom_resolution_divisor: levels(4, 2, 2),
        depth_of_field_resolution_divisor: levels(2, 2, 1),
        // 32 at `Medium`: what the default camera ran before presets (#1522).
        motion_blur_samples: levels(16, 32, 32),
        volumetric_fog: levels(froxels(80, 48), froxels(120, 64), froxels(160, 64)),
        ui_blur_divisor: 2,
        shadowmask_mode: ShadowmaskMode::Shadowmask,
        gpu_occlusion_culling: true,
    }
}

/// The seeded `High` preset: a discrete GPU.
pub(super) fn high() -> QualityValues {
    QualityValues {
        ssao: levels(ssao(8, 2), ssao(16, 1), ssao(32, 1)),
        ssil: levels(ssil(8, 2), ssil(12, 2), ssil(16, 1)),
        ssr: levels(march(32, 0.375), march(64, 0.25), march(128, 0.2)),
        bloom_resolution_divisor: levels(4, 3, 2),
        depth_of_field_resolution_divisor: levels(2, 1, 1),
        motion_blur_samples: levels(16, 32, 32),
        volumetric_fog: levels(froxels(120, 64), froxels(160, 64), froxels(160, 128)),
        ui_blur_divisor: 2,
        shadowmask_mode: ShadowmaskMode::DistanceShadowmask,
        gpu_occlusion_culling: true,
    }
}
