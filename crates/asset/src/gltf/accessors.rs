//! Every accessor the import reads has the shape glTF requires of its use. The `gltf`
//! crate's readers assume it (a `JOINTS_0` of `VEC2` panics them), so a file breaking it
//! is refused before anything is read.

use gltf::accessor::{Accessor, DataType, Dimensions};
use gltf::animation::Property;
use gltf::Semantic;

use DataType::{F32, I16, I8, U16, U32, U8};

/// The first accessor whose shape its use does not allow, named for that use.
pub(super) fn check(document: &gltf::Document) -> Result<(), String> {
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let at = format!("mesh {} primitive {}", mesh.index(), primitive.index());
            for (semantic, accessor) in primitive.attributes() {
                if let Some((dims, types)) = attribute_shape(&semantic) {
                    fits(&accessor, dims, types, &format!("{at} {semantic:?}"))?;
                }
            }
            if let Some(indices) = primitive.indices() {
                fits(
                    &indices,
                    Dimensions::Scalar,
                    &[U8, U16, U32],
                    &format!("{at} indices"),
                )?;
            }
        }
    }
    for skin in document.skins() {
        if let Some(matrices) = skin.inverse_bind_matrices() {
            let at = format!("skin {} inverse binds", skin.index());
            fits(&matrices, Dimensions::Mat4, &[F32], &at)?;
        }
    }
    for animation in document.animations() {
        for channel in animation.channels() {
            let sampler = channel.sampler();
            let at = format!(
                "animation {} channel {}",
                animation.index(),
                channel.index()
            );
            fits(
                &sampler.input(),
                Dimensions::Scalar,
                &[F32],
                &format!("{at} times"),
            )?;
            let (dims, types): (_, &[DataType]) = match channel.target().property() {
                Property::Translation | Property::Scale => (Dimensions::Vec3, &[F32]),
                Property::Rotation => (Dimensions::Vec4, &[F32, I8, U8, I16, U16]),
                Property::MorphTargetWeights => (Dimensions::Scalar, &[F32, I8, U8, I16, U16]),
            };
            fits(&sampler.output(), dims, types, &format!("{at} values"))?;
        }
    }
    Ok(())
}

/// `None` for an attribute the import never reads.
fn attribute_shape(semantic: &Semantic) -> Option<(Dimensions, &'static [DataType])> {
    Some(match semantic {
        Semantic::Positions | Semantic::Normals => (Dimensions::Vec3, &[F32]),
        Semantic::Tangents => (Dimensions::Vec4, &[F32]),
        Semantic::TexCoords(0 | 1) => (Dimensions::Vec2, &[F32, U8, U16]),
        Semantic::Joints(0) => (Dimensions::Vec4, &[U8, U16]),
        Semantic::Weights(0) => (Dimensions::Vec4, &[F32, U8, U16]),
        _ => return None,
    })
}

fn fits(accessor: &Accessor, dims: Dimensions, types: &[DataType], at: &str) -> Result<(), String> {
    let (got_dims, got_type) = (accessor.dimensions(), accessor.data_type());
    let index = accessor.index();
    if got_dims != dims || !types.contains(&got_type) {
        return Err(format!(
            "accessor {index} ({at}) is {got_dims:?} of {got_type:?}; its use needs {dims:?} of {types:?}"
        ));
    }
    if accessor.count() == 0 {
        return Err(format!("accessor {index} ({at}) holds no elements"));
    }
    let stride = accessor.view().and_then(|v| v.stride());
    if stride.is_some_and(|s| s < accessor.size()) {
        return Err(format!(
            "accessor {index} ({at}): a stride of {} bytes is under its element's {}",
            stride.unwrap_or(0),
            accessor.size()
        ));
    }
    Ok(())
}
