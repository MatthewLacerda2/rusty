//! A lightmap page's container (#1694): KTX2, one 2D texture (BC6H or, for a
//! direction page, RGBA8: #1996) with its mip chain, no supercompression. As the spec lays it out, the level index
//! runs largest first and the level images smallest first, each 16-byte aligned,
//! so other KTX2 tools open a page. Written by hand (the
//! `ktx2` crate only reads) over the layout its `Reader` validates, and read back
//! through that reader. A file that does not parse, names another format or size,
//! or whose levels are not the lengths its format and size give, is refused with
//! why, never uploaded at the wrong length.

use ktx2::Reader;

use super::{Page, PageFormat};

const MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const HEADER_LEN: usize = 80;
const LEVEL_ENTRY_LEN: usize = 24;
/// Where a level image may start: lcm(a 16-byte block, 4).
const LEVEL_ALIGN: usize = 16;

/// The KTX2 bytes of `page`.
pub fn write(page: &Page) -> Vec<u8> {
    let dfd = dfd(page.format);
    let dfd_at = HEADER_LEN + page.levels.len() * LEVEL_ENTRY_LEN;
    let data_at = dfd_at + dfd.len();
    let mut out = vec![0u8; data_at];
    out[0..12].copy_from_slice(&MAGIC);
    let words = [
        page.format.vk_format(),
        1, // typeSize: 1 for a block-compressed format and for 8-bit channels
        page.edge,
        page.edge,
        0, // pixelDepth: 2D
        0, // layerCount: not an array
        1, // faceCount
        page.levels.len() as u32,
        0, // supercompressionScheme: none
        dfd_at as u32,
        dfd.len() as u32,
        0, // kvdByteOffset: no key/values
        0, // kvdByteLength
    ];
    for (i, w) in words.iter().enumerate() {
        put(&mut out, 12 + 4 * i, &w.to_le_bytes());
    }
    // sgdByteOffset and sgdByteLength stay 0.
    out[dfd_at..data_at].copy_from_slice(&dfd);
    // The level images run smallest first, each at a multiple of the block size
    // (KTX 2.0 §3.9, `mipPadding`); the index stays largest first.
    for (i, level) in page.levels.iter().enumerate().rev() {
        out.resize(out.len().next_multiple_of(LEVEL_ALIGN), 0);
        let at = HEADER_LEN + i * LEVEL_ENTRY_LEN;
        let (offset, len) = (out.len() as u64, level.len() as u64);
        put(&mut out, at, &offset.to_le_bytes());
        put(&mut out, at + 8, &len.to_le_bytes());
        put(&mut out, at + 16, &len.to_le_bytes());
        out.extend_from_slice(level);
    }
    out
}

/// The page in `bytes`, or why it is refused.
pub fn read(bytes: &[u8]) -> Result<Page, String> {
    let reader = Reader::new(bytes).map_err(|e| format!("not a KTX2 file ({e:?})"))?;
    let head = reader.header();
    let format = PageFormat::ALL
        .into_iter()
        .find(|f| head.format.is_some_and(|k| k.0.get() == f.vk_format()))
        .ok_or_else(|| format!("format {:?} is not a lightmap page's", head.format))?;
    let edge = head.pixel_width;
    let flat = head.pixel_depth == 0 && head.layer_count == 0 && head.face_count == 1;
    if head.pixel_height != edge || !flat || head.supercompression_scheme.is_some() {
        return Err("not a square, flat, uncompressed 2D page".into());
    }
    let levels: Vec<Vec<u8>> = reader.levels().map(<[u8]>::to_vec).collect();
    if levels.is_empty() || levels.len() > super::max_levels(edge) as usize {
        return Err(format!("{} levels for a {edge}² page", levels.len()));
    }
    for (l, level) in levels.iter().enumerate() {
        let want = format.level_len(edge >> l);
        if level.len() != want {
            return Err(format!("level {l} holds {} bytes, not {want}", level.len()));
        }
    }
    Ok(Page {
        format,
        edge,
        levels,
    })
}

/// A Basic Data Format Descriptor for `format`, linear transfer (lightmaps hold
/// linear light and raw directions): BC6H's one 128-bit float sample over a 4×4
/// block, or RGBA8's four 8-bit samples over one texel.
fn dfd(format: PageFormat) -> Vec<u8> {
    // Per sample: (bit offset, bit length less one, channel and qualifiers, upper).
    let (model, block_dims, plane0, samples): (u32, u32, u32, &[(u32, u32, u32, u32)]) =
        match format {
            // KHR_DF_MODEL_BC6H; channel 0 with KHR_DF_SAMPLE_DATATYPE_FLOAT.
            PageFormat::Bc6h => (133, 3 | (3 << 8), 16, &[(0, 127, 0x80, u32::MAX)]),
            // KHR_DF_MODEL_RGBSDA; channels R, G, B and A (15), each a byte.
            PageFormat::Rgba8 => (
                1,
                0,
                4,
                &[
                    (0, 7, 0, 255),
                    (8, 7, 1, 255),
                    (16, 7, 2, 255),
                    (24, 7, 15, 255),
                ],
            ),
        };
    let block = 24 + 16 * samples.len() as u32;
    let mut words = vec![
        4 + block,                    // dfdTotalSize
        0,                            // vendorId 0 (Khronos), descriptorType 0 (basic)
        2 | (block << 16),            // versionNumber 2, descriptorBlockSize
        model | (1 << 8) | (1 << 16), // model, primaries BT.709, transfer linear
        block_dims,                   // texel block size, each dimension less one
        plane0,                       // bytesPlane0: one block's (or texel's) bytes
        0,                            // bytesPlane4..7
    ];
    for &(offset, len, channel, upper) in samples {
        // The sample's offset, length, channel; its positions; lower; upper.
        words.extend([offset | (len << 16) | (channel << 24), 0, 0, upper]);
    }
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn put(out: &mut [u8], at: usize, bytes: &[u8]) {
    out[at..at + bytes.len()].copy_from_slice(bytes);
}
