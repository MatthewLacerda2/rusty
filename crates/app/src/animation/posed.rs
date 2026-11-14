//! The palette the engine skins a mesh with while one clip plays (#453, #1209): the
//! sampled pose written onto the bone GameObjects, as the `animate` system writes
//! it, then [`Scene::build_skin_palettes`]. Tests that need an animated palette
//! read it here, so they check the path a frame takes.

use glam::Mat4;

use super::bone_writes;
use rusty_asset::anim_data::AnimationClip;
use rusty_asset::mesh_data::SkinData;
use rusty_components::{AnimatorComponent, MeshComponent, Motion};
use rusty_scene::Scene;

/// `skin`'s palette with `clip` sampled at `time` seconds.
pub fn posed_palette(skin: &SkinData, clip: &AnimationClip, time: f32) -> Vec<Mat4> {
    let mut scene = Scene::new();
    let owner = scene.add_entity("Rig".to_string());
    let mesh = MeshComponent {
        bind_palette: skin.bind_palette(),
        skin: Some(skin.clone()),
        clips: vec![clip.clone()],
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    scene.world.set_mesh(owner, Some(mesh));
    scene.sync_skeleton(owner);
    let mut anim = AnimatorComponent {
        is_playing: true,
        ..Default::default()
    };
    anim.base.play(Motion::Clip(&clip.name));
    anim.base.time = time;
    let writes = bone_writes(
        &anim,
        &scene.world.mesh(owner).expect("the rig was given its mesh"),
        None,
    );
    assert!(
        !writes.is_empty(),
        "the clip posed no bone: the palette would be the bind one"
    );
    for (bone, local) in writes {
        *scene
            .world
            .transform_mut(bone)
            .expect("a bone has a transform") = local;
    }
    scene.build_skin_palettes();
    let palette = scene
        .world
        .mesh(owner)
        .expect("the rig was given its mesh")
        .pose_palette
        .clone();
    palette
}
