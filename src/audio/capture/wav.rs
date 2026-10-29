//! src/audio/capture/wav.rs — the file a capture streams into (#1579).
//!
//! A capture can run for minutes, so the samples go to disk as they are drained and
//! the RIFF header, whose sizes and rate are known only at the end, is written last
//! over a placeholder. The format is **32-bit float** stereo: the mix is measured,
//! not shipped, so nothing is quantised and a peak over full scale stays readable.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

/// Bytes of header before the samples begin.
const HEADER_LEN: usize = 44;
/// Bytes per stereo frame of `f32` samples.
const BYTES_PER_FRAME: u32 = 8;
/// Format tag 3 = IEEE float.
const FORMAT_FLOAT: u16 = 3;

/// A float WAV being written a run of samples at a time.
pub struct WavStream {
    file: BufWriter<File>,
    /// Interleaved samples written so far.
    samples: u64,
}

impl WavStream {
    /// Create (or truncate) `path`, creating its parent directory.
    pub fn create(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        // lint: allow(asset-write) streamed; `Recorder::finish` notes the write once whole
        let mut file = BufWriter::new(File::create(path)?);
        file.write_all(&[0; HEADER_LEN])?;
        Ok(Self { file, samples: 0 })
    }

    /// Append interleaved stereo samples.
    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        for sample in samples {
            self.file.write_all(&sample.to_le_bytes())?;
        }
        self.samples += samples.len() as u64;
        Ok(())
    }

    /// Write the header for `rate` over the placeholder and close the file.
    pub fn finish(mut self, rate: u32) -> io::Result<()> {
        let data_len = u32::try_from(self.samples * 4)
            .map_err(|_| io::Error::other("capture longer than a WAV can hold (4 GiB)"))?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header(rate, data_len))?;
        self.file.flush()
    }
}

/// The 44-byte RIFF header of a stereo float WAV at `rate` with `data_len` bytes.
pub fn header(rate: u32, data_len: u32) -> [u8; HEADER_LEN] {
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&FORMAT_FLOAT.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // channels
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * BYTES_PER_FRAME).to_le_bytes()); // byte rate
    out.extend_from_slice(&(BYTES_PER_FRAME as u16).to_le_bytes()); // block align
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    let mut header = [0; HEADER_LEN];
    header.copy_from_slice(&out);
    header
}
