//! An ASCII FBX quad whose one material embeds the images it is given (#2365):
//! what an import of a model with embedded textures is tested on, at any size,
//! without shipping a binary fixture.

use std::fmt::Write;

/// One embedded map: the texture's name, the material property it fills
/// (`"DiffuseColor"`, `"NormalMap"`, `"EmissiveColor"`) and the PNG's bytes.
pub struct Embedded<'a> {
    pub name: &'a str,
    pub property: &'a str,
    pub png: &'a [u8],
}

/// A quad of two triangles with one material, `Mat`, each of `maps` embedded in
/// it and connected to its property.
pub fn embedded_fbx(maps: &[Embedded]) -> String {
    let mut objects = String::new();
    let mut connections = String::new();
    for (i, map) in maps.iter().enumerate() {
        let (video, texture) = (4000 + i * 10, 4001 + i * 10);
        let name = map.name;
        let content = base64(map.png);
        let _ = write!(
            objects,
            "\tVideo: {video}, \"Video::{name}\", \"Clip\" {{\n\t\tType: \"Clip\"\n\
             \t\tFilename: \"/missing/{name}.png\"\n\t\tRelativeFilename: \"{name}.png\"\n\
             \t\tContent: , \"{content}\"\n\t}}\n\
             \tTexture: {texture}, \"Texture::{name}\", \"\" {{\n\t\tType: \"TextureVideoClip\"\n\
             \t\tVersion: 202\n\t\tTextureName: \"Texture::{name}\"\n\
             \t\tFileName: \"/missing/{name}.png\"\n\t\tRelativeFilename: \"{name}.png\"\n\t}}\n"
        );
        let _ = write!(
            connections,
            "\tC: \"OO\",{video},{texture}\n\tC: \"OP\",{texture},3000, \"{}\"\n",
            map.property
        );
    }
    format!("{QUAD}{objects}}}\nConnections:  {{\n\tC: \"OO\",2000,0\n\tC: \"OO\",1000,2000\n\tC: \"OO\",3000,2000\n{connections}}}\n")
}

/// A `size`² RGBA PNG of a gradient `seed` shifts: a generated map, distinct per seed.
pub fn png(size: u32, seed: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(size, size, |x, y| {
        let [x, y] = [x, y].map(|v| u8::try_from(v * 255 / size.max(1)).unwrap_or(255));
        image::Rgba([x.wrapping_add(seed), y, seed, 255])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("a PNG encodes");
    bytes.into_inner()
}

/// `bytes` in standard base64, padded: how an FBX embeds a file's content.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = chunk
            .iter()
            .enumerate()
            .fold(0u32, |w, (i, b)| w | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            match i <= chunk.len() {
                true => out.push(char::from(ALPHABET[(word >> (18 - 6 * i) & 63) as usize])),
                false => out.push('='),
            }
        }
    }
    out
}

/// Everything before the textures: the quad's geometry, its model and material.
const QUAD: &str = r#"; FBX 7.4.0 project file
FBXHeaderExtension:  {
	FBXHeaderVersion: 1003
	FBXVersion: 7400
	Creator: "rusty test fixture"
}
Objects:  {
	Geometry: 1000, "Geometry::Quad", "Mesh" {
		Vertices: *12 {
			a: 0,0,0,1,0,0,1,1,0,0,1,0
		}
		PolygonVertexIndex: *6 {
			a: 0,1,-3,0,2,-4
		}
		GeometryVersion: 124
		LayerElementUV: 0 {
			Version: 101
			Name: "UVMap"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "IndexToDirect"
			UV: *8 {
				a: 0,0,1,0,1,1,0,1
			}
			UVIndex: *6 {
				a: 0,1,2,0,2,3
			}
		}
		Layer: 0 {
			Version: 100
			LayerElement:  {
				Type: "LayerElementUV"
				TypedIndex: 0
			}
		}
	}
	Model: 2000, "Model::Quad", "Mesh" {
		Version: 232
	}
	Material: 3000, "Material::Mat", "" {
		Version: 102
		ShadingModel: "lambert"
	}
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn base64_pads_like_the_standard() {
        assert_eq!(super::base64(b"M"), "TQ==");
        assert_eq!(super::base64(b"Ma"), "TWE=");
        assert_eq!(super::base64(b"Man"), "TWFu");
    }
}
