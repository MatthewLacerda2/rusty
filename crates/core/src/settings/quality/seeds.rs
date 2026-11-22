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

const fn ssil(samples: u32, resolution_divisor: u32) -> SsilQuality {
    SsilQuality {
        samples,
        resolution_divisor,
    }
}

// The `Medium` preset's SSR rows are HDRP's Max Ray Steps (16, 32, 64), reaching at
// most 8, 12, 16 m.
const fn march(steps: u32, step_length: f32) -> SsrQuality {
    SsrQuality { steps, step_length }
}

const fn froxels(resolution: u32, slices: u32) -> VolumetricFogQuality {
    VolumetricFogQuality { resolution, slices }
}

/// The `Low` preset: an integrated GPU's floor.
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
        gpu_occlusion_culling: false,
        gpu_cluster_culling: false,
    }
}

/// The `Medium` preset, and the values that run when a game defines none.
pub(super) fn medium() -> QualityValues {
    QualityValues {
        ssao: levels(ssao(6, 2), ssao(8, 2), ssao(16, 1)),
        ssil: levels(ssil(6, 2), ssil(8, 2), ssil(12, 2)),
        ssr: levels(march(16, 0.5), march(32, 0.375), march(64, 0.25)),
        bloom_resolution_divisor: levels(4, 2, 2),
        depth_of_field_resolution_divisor: levels(2, 2, 1),
        motion_blur_samples: levels(16, 32, 32),
        volumetric_fog: levels(froxels(80, 48), froxels(120, 64), froxels(160, 64)),
        ui_blur_divisor: 2,
        shadowmask_mode: ShadowmaskMode::Shadowmask,
        gpu_occlusion_culling: false,
        gpu_cluster_culling: false,
    }
}

/// The `High` preset: a discrete GPU.
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
        gpu_occlusion_culling: false,
        gpu_cluster_culling: false,
    }
}
