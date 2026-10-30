//! src/render/gpu/textures/freshness.rs — keep the path-keyed texture cache in step
//! with the files on disk (#689), without a per-frame `stat` of every map.
//!
//! Two ways an upload goes stale, two cheap answers, both checked once per frame
//! before anything draws:
//!
//! - **The engine re-wrote the file** (`Texture.Bake`, `BakeSet`, `Material.Rebake`
//!   all go through one PNG writer, and a reflection bake writes its cubemap, each
//!   noting the path in [`crate::core::fs::written`], #1207). The renderer drains the
//!   notes and drops the matching upload — colour and data views together, they are
//!   one `GpuTexture` — plus every material bind group, which still holds the old
//!   upload, the matching reflection cubemap, and a mesh emitter's model whose
//!   source was re-imported (`passes::particles::mesh_cache`). Unity reimports on
//!   write; this is that.
//! - **The file was missing when first drawn.** A failed load is remembered as a
//!   *miss*, not cached as the default under its path, and misses are re-checked
//!   (`Path::exists`, one per missing path) every `MISS_RETRY_FRAMES` frames, so a
//!   map that appears later — baked, imported or copied in — is picked up. A mesh
//!   emitter's model that was missing is retried on the same beat, once its
//!   file has appeared.
//!
//! Both are kept by the shared [`Freshness`](crate::core::fs::freshness::Freshness)
//! (#1241), which the UI's texture-size table uses too.
//!
//! A material map naming a texture an import extracted (`model.fbx#tex/Skin`,
//! #1130) is dropped when its source is noted, which a re-conversion does, and a
//! missing one is retried once the file it resolves to exists.
//!
//! A texture whose import settings changed (#1323) is dropped the same way: the
//! settings live in the image's `.meta` sidecar, whose write is noted, so the
//! upload goes when its sidecar's path is in the log and the next lookup builds it
//! with the new mips and sampler.
//!
//! A file changed on disk by an outside tool at a path that already loaded reaches
//! the same log: in the editor, its file watch notes every change under the
//! project's `assets/` (#1327), so an image editor's save reloads like a bake, with
//! no per-frame stat. A shipped game watches nothing.

use crate::asset::convert::texture_ref;
use crate::asset::sidecar::meta_path;
use crate::core::fs::written::Stale;
use crate::render::Renderer;

impl Renderer {
    /// Drop every cached upload whose file was re-written or has appeared since the
    /// last frame. Called once per frame, before any texture is resolved.
    pub(crate) fn refresh_textures(&mut self) {
        let changes = self.texture_freshness.poll_with(texture_ref::exists);
        if changes.retried {
            // A mesh emitter's model whose file has appeared is tried again too.
            self.retry_particle_meshes();
        }
        if !changes.stale.is_empty() {
            self.invalidate_textures(&changes.stale);
        }
    }

    /// Forget the uploads (and misses) of `paths`, so their next lookup reads the
    /// file again. A cached key naming the same file by another spelling (relative
    /// vs absolute) goes too.
    pub(crate) fn invalidate_textures(&mut self, paths: &[String]) {
        let stale = Stale::new(paths.to_vec());
        let is_stale =
            |key: &str| texture_ref::is_written(&stale, key) || settings_written(&stale, key);
        // A cubemap loaded from a re-written file (#1207). Its probe-array slot
        // goes too, so the next frame copies the new cube in (#1206).
        self.cubemaps.retain(|key, _| !is_stale(key));
        self.reflection_probes.forget(is_stale);
        // A mesh emitter's re-imported model (#1219).
        self.forget_particle_meshes(&stale);
        // The decal atlas keeps its own copy of each map (#638).
        self.decals.atlas.forget(is_stale);
        let before = self.gpu_textures.len();
        self.gpu_textures.retain(|key, _| !is_stale(key.as_str()));
        self.texture_fallbacks
            .retain(|key, _| !is_stale(key.as_str()));
        let missed = self.texture_freshness.forget_misses_where(is_stale);
        let dropped = self.gpu_textures.len() != before || missed;
        if !dropped {
            return;
        }
        self.materials.forget_groups();
        if is_stale(&self.skybox_path) {
            // An empty path makes the next frame reload the scene's skybox.
            self.skybox_path.clear();
        }
    }
}

/// Whether the import-settings sidecar of the file the map `key` names is in
/// `stale` (#1323).
fn settings_written(stale: &Stale, key: &str) -> bool {
    let file = texture_ref::file(key);
    file.is_some_and(|f| stale.matches(&meta_path(&f).to_string_lossy()))
}
