//! Each glTF mesh is one `SubMesh`; its primitives merge into one stream, each primitive's
//! indices a `MaterialRange`. Every triangle mode becomes a triangle list.

use super::{anim, conform, material, nodes, skin, topology};
use crate::material_range::push_range;
use crate::mesh_data::{ImportedAsset, SkinData, SubMesh, Vertex};
use crate::tangents::Missing;
use crate::ImportError;
use std::path::Path;

/// Images are never decoded here, so an undecodable one (a TGA) never fails the model.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_from(path, None)
}

/// A texture the conversion extracted comes back as a reference to it in `source`,
/// never as its path in the cache.
pub fn import_from(path: &Path, source: Option<&str>) -> Result<ImportedAsset, ImportError> {
    import_as(path, source, Missing::Generate)
}

pub fn import_as(
    path: &Path,
    source: Option<&str>,
    missing: Missing,
) -> Result<ImportedAsset, ImportError> {
    let bytes = std::fs::read(path).map_err(|e| ImportError::Parse(e.to_string()))?;
    let gltf = conform::parse(&bytes)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let name = path.display().to_string();
    import_gltf(gltf, &name, Some(base_dir), source, missing)
}

/// A buffer naming a file is an error.
pub fn import_slice(bytes: &[u8]) -> Result<ImportedAsset, ImportError> {
    let gltf = conform::parse(bytes)?;
    import_gltf(gltf, "<memory>", None, None, Missing::Generate)
}

fn parse(e: gltf::Error) -> ImportError {
    ImportError::Parse(e.to_string())
}

fn import_gltf(
    gltf: gltf::Gltf,
    name: &str,
    base_dir: Option<&Path>,
    source: Option<&str>,
    missing: Missing,
) -> Result<ImportedAsset, ImportError> {
    let gltf::Gltf { document, blob } = gltf;
    let buffers = gltf::import_buffers(&document, base_dir, blob).map_err(parse)?;
    let images_dir = base_dir.unwrap_or_else(|| Path::new(""));
    let materials = document
        .materials()
        .map(|m| material::material_data(m, images_dir, source))
        .collect();
    let file = File {
        name,
        document: &document,
        buffers: &buffers,
        missing,
    };
    let skins = skin::skins_by_mesh(&file);

    let meshes: Vec<gltf::Mesh> = document.meshes().collect();
    let triangles = |m: &gltf::Mesh| m.primitives().map(|p| corners(&p) / 3).sum();
    // Threaded: a mesh's read and tangents depend on that mesh alone.
    let sub_meshes: Vec<SubMesh> = crate::cook::par::map(meshes, triangles, |mesh| {
        let mut sub = sub_mesh_from_gltf(&mesh, &file, skins.get(&mesh.index()).cloned());
        sub.node_name = node_name(&document, mesh.index());
        sub
    });

    let ids: Vec<&str> = sub_meshes.iter().map(|s| s.id.as_str()).collect();
    let nodes = nodes::model_nodes(&document, &ids);
    let asset = ImportedAsset {
        sub_meshes,
        materials,
        nodes,
    };
    debug_assert_eq!(
        asset.validate(),
        Ok(()),
        "{name}: the import broke an invariant"
    );
    Ok(asset)
}

/// `name` names the file in warnings.
pub(super) struct File<'a> {
    pub(super) name: &'a str,
    pub(super) document: &'a gltf::Document,
    pub(super) buffers: &'a [gltf::buffer::Data],
    missing: Missing,
}

impl File<'_> {
    pub(super) fn data(&self, buffer: gltf::Buffer) -> Option<&[u8]> {
        self.buffers.get(buffer.index()).map(|d| d.0.as_slice())
    }
}

fn node_name(document: &gltf::Document, mesh: usize) -> Option<String> {
    document
        .nodes()
        .find(|n| n.mesh().is_some_and(|m| m.index() == mesh))
        .and_then(|n| n.name().map(str::to_string))
}

fn sub_mesh_from_gltf(mesh: &gltf::Mesh, file: &File, skin: Option<SkinData>) -> SubMesh {
    let id = mesh
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("mesh_{}", mesh.index()));

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut ranges = Vec::new();

    for primitive in mesh.primitives() {
        let start = indices.len() as u32;
        append_primitive(&primitive, file, (&mut vertices, &mut indices));
        let run = start..indices.len() as u32;
        push_range(&mut ranges, run, primitive.material().index());
    }

    let clips = match &skin {
        Some(s) => {
            skin::drop_stray_influences(&mut vertices, s.joint_nodes.len(), file.name, &id);
            anim::clips_for_skin(file, &s.joint_nodes)
        }
        None => Vec::new(),
    };

    SubMesh {
        id,
        vertices: vertices.into(),
        indices: indices.into(),
        ranges,
        skin,
        clips,
        node_name: None,
        clusters: None,
    }
}

fn append_primitive(
    primitive: &gltf::Primitive,
    file: &File,
    (vertices, indices): (&mut Vec<Vertex>, &mut Vec<u32>),
) {
    let missing = file.missing;
    let Some((own, local, has_normals)) = read_primitive(primitive, file) else {
        return;
    };
    let (own, local) = match has_normals {
        true => (own, local),
        false => crate::flat_normals::flat_shaded(&own, &local, missing),
    };
    let base = vertices.len() as u32;
    vertices.extend(own);
    indices.extend(local.into_iter().map(|i| i + base));
}

fn read_primitive(
    primitive: &gltf::Primitive,
    file: &File,
) -> Option<(Vec<Vertex>, Vec<u32>, bool)> {
    let missing = file.missing;
    let reader = primitive.reader(|b| file.data(b));

    let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();

    let normals: Option<Vec<[f32; 3]>> = reader
        .read_normals()
        .map(|n| n.collect::<Vec<_>>())
        .filter(|n| n.len() == positions.len());
    // Without one normal per position the primitive is shaded flat, as glTF's spec asks.

    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|t| t.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let lightmap_uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(1)
        .map(|t| t.into_f32().collect())
        .unwrap_or_default();

    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .map(|j| j.into_u16().collect())
        .unwrap_or_default();
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .map(|w| w.into_f32().collect())
        .unwrap_or_default();

    let read: Option<Vec<u32>> = reader.read_indices().map(|r| r.into_u32().collect());
    let local = topology::triangle_list(primitive.mode(), read, positions.len() as u32)?;
    let local = topology::whole_triangles(local, positions.len(), file.name, primitive)?;
    let tangents: Vec<[f32; 4]> = match &normals {
        Some(normals) => reader
            .read_tangents()
            .map(|t| t.collect())
            .unwrap_or_else(|| crate::tangents::fill(missing, &positions, normals, &uvs, &local)),
        None => Vec::new(),
    };

    let mut own: Vec<Vertex> = Vec::with_capacity(positions.len());
    for (i, position) in positions.iter().enumerate() {
        let normal = normals.as_ref().map_or([0.0; 3], |n| n[i]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let mut vertex = Vertex::new((*position).into(), normal.into(), uv);
        vertex.tangent = tangents.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]);
        vertex.lightmap_uv = lightmap_uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        if let (Some(j), Some(w)) = (joints.get(i), weights.get(i)) {
            let ji = [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32];
            vertex = vertex.with_skin(ji, *w);
        }
        own.push(vertex);
    }
    Some((own, local, normals.is_some()))
}

fn corners(primitive: &gltf::Primitive) -> usize {
    let indices = primitive.indices();
    let positions = || primitive.get(&gltf::Semantic::Positions);
    indices.or_else(positions).map_or(0, |a| a.count())
}
