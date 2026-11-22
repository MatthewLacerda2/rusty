//! What the `gltf` crate assumes of a file but never checks, checked first so a file
//! breaking it is refused, not a panic: each accessor the import reads exists and has
//! the shape its use requires, an animation channel names a node and a property, the
//! nodes form a forest, and a `.glb`'s declared length covers its 12-byte header.

use crate::ImportError;
use gltf::accessor::{Accessor, DataType, Dimensions};
use gltf::animation::Property;
use gltf::Semantic;

use DataType::{F32, I16, I8, U16, U32, U8};

pub(super) fn parse(bytes: &[u8]) -> Result<gltf::Gltf, ImportError> {
    let error = |e: gltf::Error| ImportError::Parse(e.to_string());
    let declared = bytes
        .get(8..12)
        .map(|l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]));
    if bytes.starts_with(b"glTF") && declared.is_none_or(|l| l < 12) {
        return Err(ImportError::Parse(
            "the .glb header declares no length past itself".into(),
        ));
    }
    let gltf::Gltf { document, blob } =
        gltf::Gltf::from_slice_without_validation(bytes).map_err(error)?;
    let json = document.into_json();
    unchecked(&json).map_err(ImportError::Parse)?;
    let document = gltf::Document::from_json(json).map_err(error)?;
    check(&document).map_err(ImportError::Parse)?;
    Ok(gltf::Gltf { document, blob })
}

fn unchecked(json: &gltf::json::Root) -> Result<(), String> {
    let count = json.accessors.len();
    for (m, mesh) in json.meshes.iter().enumerate() {
        let named = mesh.primitives.iter().flat_map(|p| p.attributes.values());
        if let Some(missing) = named.map(|a| a.value()).find(|&a| a >= count) {
            return Err(format!("mesh {m} names accessor {missing} of {count}"));
        }
    }
    forest(&json.nodes)?;
    for (a, animation) in json.animations.iter().enumerate() {
        for (c, channel) in animation.channels.iter().enumerate() {
            let target = &channel.target;
            if target.node.value() >= json.nodes.len() {
                return Err(format!("animation {a} channel {c} targets no node"));
            }
            if !matches!(target.path, gltf::json::validation::Checked::Valid(_)) {
                return Err(format!(
                    "animation {a} channel {c} targets no known property"
                ));
            }
        }
    }
    Ok(())
}

/// Every walk of the hierarchy recurses into children, so a cycle never ends.
fn forest(nodes: &[gltf::json::Node]) -> Result<(), String> {
    let mut parent: Vec<Option<usize>> = vec![None; nodes.len()];
    for (n, node) in nodes.iter().enumerate() {
        for child in node.children.iter().flatten().map(|c| c.value()) {
            if child >= nodes.len() || child == n || parent[child].replace(n).is_some() {
                return Err(format!("node {n}'s child {child} is not a node of its own"));
            }
        }
    }
    for start in 0..nodes.len() {
        let mut at = start;
        for _ in 0..=nodes.len() {
            match parent[at] {
                Some(p) => at = p,
                None => break,
            }
        }
        if parent[at].is_some() {
            return Err(format!("node {start} is its own ancestor"));
        }
    }
    Ok(())
}

/// The first accessor whose shape its use does not allow, named for that use.
fn check(document: &gltf::Document) -> Result<(), String> {
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
