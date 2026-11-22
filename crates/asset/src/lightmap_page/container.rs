//! The `ktx2` crate only reads. Per spec the level index runs largest first, the
//! images smallest first.

use ktx2::Reader;

use super::{Page, PageFormat};

const MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const HEADER_LEN: usize = 80;
const LEVEL_ENTRY_LEN: usize = 24;
/// KTX 2.0 §3.9 `mipPadding`: lcm(block bytes, 4).
fn level_align(format: PageFormat) -> usize {
    match format {
        PageFormat::Bc6h => 16,
        PageFormat::Rgba8 => 4,
    }
}

pub fn write(page: &Page) -> Vec<u8> {
    let dfd = dfd(page.format);
    let dfd_at = HEADER_LEN + page.levels.len() * LEVEL_ENTRY_LEN;
    let data_at = dfd_at + dfd.len();
    let mut out = vec![0u8; data_at];
    out[0..12].copy_from_slice(&MAGIC);
    let words = [
        page.format.vk_format(),
        1, // typeSize
        page.edge,
        page.edge,
        0,
        0,
        1,
        page.levels.len() as u32,
        0,
        dfd_at as u32,
        dfd.len() as u32,
        0,
        0,
    ];
    for (i, w) in words.iter().enumerate() {
        put(&mut out, 12 + 4 * i, &w.to_le_bytes());
    }
    out[dfd_at..data_at].copy_from_slice(&dfd);
    for (i, level) in page.levels.iter().enumerate().rev() {
        out.resize(out.len().next_multiple_of(level_align(page.format)), 0);
        let at = HEADER_LEN + i * LEVEL_ENTRY_LEN;
        let (offset, len) = (out.len() as u64, level.len() as u64);
        put(&mut out, at, &offset.to_le_bytes());
        put(&mut out, at + 8, &len.to_le_bytes());
        put(&mut out, at + 16, &len.to_le_bytes());
        out.extend_from_slice(level);
    }
    out
}

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

/// Bit offset, bit length less one, channel, upper bound.
type Sample = (u32, u32, u32, u32);

/// Linear transfer: lightmaps hold linear light and raw directions.
fn dfd(format: PageFormat) -> Vec<u8> {
    let (model, block_dims, plane0, samples): (u32, u32, u32, &[Sample]) = match format {
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
        4 + block,
        0,
        2 | (block << 16),            // versionNumber 2, descriptorBlockSize
        model | (1 << 8) | (1 << 16), // model, primaries BT.709, transfer linear
        block_dims,                   // each dimension less one
        plane0,                       // bytesPlane0
        0,
    ];
    for &(offset, len, channel, upper) in samples {
        words.extend([offset | (len << 16) | (channel << 24), 0, 0, upper]);
    }
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn put(out: &mut [u8], at: usize, bytes: &[u8]) {
    out[at..at + bytes.len()].copy_from_slice(bytes);
}
