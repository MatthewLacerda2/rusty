use crate::Renderer;
use rusty_asset::convert::texture_ref;
use rusty_asset::sidecar::meta_path;
use rusty_core::fs::written::Stale;

impl Renderer {
    /// Once per frame, before any texture is resolved: drops uploads whose file or `.meta`
    /// was re-written, and retries misses whose file appeared.
    pub(crate) fn refresh_textures(&mut self) {
        let changes = self.texture_freshness.poll_with(texture_ref::exists);
        if changes.retried {
            self.retry_particle_meshes();
        }
        if !changes.stale.is_empty() {
            self.invalidate_textures(&changes.stale);
        }
    }

    pub(crate) fn invalidate_textures(&mut self, paths: &[String]) {
        let stale = Stale::new(paths.to_vec());
        let is_stale = |key: &str| texture_written(&stale, key);
        self.reflection_probes.forget(is_stale);
        self.forget_particle_meshes(&stale);
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
        self.retained.touch();
        if is_stale(&self.skybox_path) {
            self.skybox_path.clear();
        }
    }
}

pub(crate) fn texture_written(stale: &Stale, key: &str) -> bool {
    texture_ref::is_written(stale, key) || settings_written(stale, key)
}

fn settings_written(stale: &Stale, key: &str) -> bool {
    let file = texture_ref::file(key);
    file.is_some_and(|f| stale.matches(&meta_path(&f).to_string_lossy()))
}
