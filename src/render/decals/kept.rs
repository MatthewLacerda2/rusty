//! What the decal upload keeps between frames (#1706): each decal's maps, re-read
//! only when the decal names another material or texture or the library changed,
//! and the records last uploaded, so a frame whose decals hold still writes none.
//!
//! A decal changes per frame by its age (a lifetime's fade-out, a budget's eviction
//! fade: `DecalPose::opacity`) and by its owner's pose: both are read every frame,
//! and its record is rebuilt from them. Only its maps and the upload are kept.

use std::collections::HashMap;

use super::record::{self, DecalMaps, GpuDecal};
use crate::asset::MapRef;
use crate::scene::decal::Decal;
use crate::scene::revision::Revision;
use crate::scene::{Scene, SceneId};

/// One decal's maps, and what they were read from.
struct Maps {
    material: Option<String>,
    texture: Option<MapRef>,
    maps: DecalMaps,
    /// The frame it was last used in: an entry past it is dropped.
    used: u64,
}

#[derive(Default)]
pub(crate) struct KeptDecals {
    maps: HashMap<u32, Maps>,
    /// The scene and material-library revision the maps were read under.
    library: Option<(SceneId, Revision)>,
    frame: u64,
    /// The records the buffer holds now.
    pub(crate) uploaded: Vec<GpuDecal>,
}

impl KeptDecals {
    /// Start a frame of `scene`: maps read under another scene or library go.
    pub(crate) fn begin(&mut self, scene: &Scene) {
        let library = Some((scene.id(), scene.materials.revision()));
        if self.library != library {
            self.maps.clear();
            self.library = library;
        }
        self.frame += 1;
    }

    /// Keep `decal`'s maps in `scene`, read only when not kept as they are.
    pub(crate) fn read(&mut self, scene: &Scene, decal: &Decal) {
        let frame = self.frame;
        let entry = self.maps.entry(decal.id);
        let kept = entry.or_insert_with(|| Maps {
            material: None,
            texture: None,
            maps: Default::default(),
            used: 0,
        });
        if kept.used == 0 || kept.material != decal.material || kept.texture != decal.texture {
            let material = super::gpu::resolve(&scene.materials, decal);
            kept.maps = record::maps(decal, material);
            kept.material.clone_from(&decal.material);
            kept.texture.clone_from(&decal.texture);
        }
        kept.used = frame;
    }

    /// The kept maps of decal `id`, [read](Self::read) this frame.
    pub(crate) fn of(&self, id: u32) -> &DecalMaps {
        &self.maps[&id].maps
    }

    /// Drop the maps of decals this frame did not draw.
    pub(crate) fn end(&mut self) {
        let frame = self.frame;
        self.maps.retain(|_, m| m.used == frame);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::components::MaterialAsset;
    use crate::scene::decal::DecalSpec;

    fn albedo(kept: &mut KeptDecals, scene: &Scene) -> Option<MapRef> {
        kept.begin(scene);
        let decal = &scene.decals.iter().next().expect("a decal");
        kept.read(scene, decal);
        let map = kept.of(decal.id)[0].clone();
        kept.end();
        map
    }

    /// A decal's maps are kept until its material, its texture or the library
    /// changes, and dropped with the decal.
    #[test]
    fn a_decals_maps_follow_its_material_and_the_library() {
        let mut scene = Scene::new();
        let paint = |map: &str| MaterialAsset {
            base_color_map: Some(map.into()),
            ..MaterialAsset::default()
        };
        scene.materials.insert("paint".into(), paint("a.png"));
        let spec = DecalSpec {
            material: Some("paint".into()),
            ..DecalSpec::default()
        };
        let id = scene.spawn_decal(Vec3::ZERO, Vec3::Y, spec).unwrap();
        let mut kept = KeptDecals::default();
        assert_eq!(albedo(&mut kept, &scene), Some("a.png".into()));
        scene.materials.insert("paint".into(), paint("b.png"));
        assert_eq!(
            albedo(&mut kept, &scene),
            Some("b.png".into()),
            "the library"
        );
        scene.decals.set_material(id, None);
        assert_eq!(albedo(&mut kept, &scene), None, "its own material");
        scene.clear_decals(false);
        kept.begin(&scene);
        kept.end();
        assert!(kept.maps.is_empty(), "a decal gone drops its maps");
    }
}
